// device_runtime.rs v1.22.407
//! Explicit app controls; no background SSH polling or system services.
use crate::{devices, invalid};
use serde_json::{Value, json};
use std::{fs, io::{self, BufRead, BufReader}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, thread, time::Duration};

fn executable_path(path: PathBuf) -> io::Result<PathBuf> {
    if path.is_file() { return Ok(path); }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        if let Some(bytes) = path.as_os_str().as_bytes().strip_suffix(b" (deleted)") {
            let installed = PathBuf::from(std::ffi::OsStr::from_bytes(bytes));
            if installed.is_file() { return Ok(installed); }
        }
    }
    Err(io::Error::new(io::ErrorKind::NotFound, "Eseguibile Oaclis installato non trovato per il riavvio"))
}

pub(crate) fn build() -> Value {
    json!({"version":env!("OACLIS_APP_VERSION"),"source_id":env!("OACLIS_SOURCE_ID"),"revision":env!("OACLIS_REVISION")})
}

pub(crate) fn snapshot(config: &Path, local: &std::sync::Mutex<Runtime>) -> io::Result<Value> {
    let mut states = crate::shared_settings::runtime(config)?;
    states["master"] = local.lock().unwrap().status();
    Ok(states)
}

pub(crate) struct Runtime {
    child: Option<Child>,
    config: PathBuf,
    error: Option<String>,
    restarting: bool,
    awake_pending: Option<bool>,
    desktop: std::sync::Arc<std::sync::Mutex<Value>>,
    update: std::sync::Arc<std::sync::Mutex<Value>>,
}
impl Runtime {
    pub(crate) fn new(config: &Path) -> Self {
        let mut state = Self { child: None, config: config.into(), error: None, restarting: false, awake_pending: None, desktop: Default::default(), update: Default::default() };
        let desktop = state.desktop.clone();
        thread::spawn(move || { *desktop.lock().unwrap() = crate::remote_desktop::discover(); });
        if fs::read_to_string(config.join("keep-awake.json")).ok().is_some_and(|s| s.trim() == "true") {
            match Self::start_inhibitor() { Ok(child)=>state.child=Some(child), Err(error)=>state.error=Some(error.to_string()) }
        }
        state
    }
    pub(crate) fn status(&mut self) -> Value {
        if self.child.as_mut().is_some_and(|child| !matches!(child.try_wait(), Ok(None))) {
            self.child.take();
            self.error = Some("Il blocco standby non è più attivo".into());
        }
        json!({"available":true,"observed_at":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"active":self.awake_pending.unwrap_or(self.child.is_some()),"error":self.error,"restarting":self.restarting,"build":build(),"desktop":*self.desktop.lock().unwrap(),"update":*self.update.lock().unwrap()})
    }
    fn start_inhibitor() -> io::Result<Child> {
        let mut child = Command::new(executable_path(std::env::current_exe()?)?).arg("keep-awake-hold")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let mut output = String::new();
        let stdout = child.stdout.take().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let result = BufReader::new(stdout).read_line(&mut output).map(|_|output);
            let _ = send.send(result);
        });
        let output = match receive.recv_timeout(Duration::from_secs(20)) {
            Ok(Ok(output)) => output,
            _ => { let _ = child.kill(); let _ = child.wait(); return Err(invalid("Il sistema non ha risposto alla richiesta di blocco standby")); }
        };
        let result: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
        if result["active"] != true {
            let _ = child.kill(); let _ = child.wait();
            return Err(invalid(result["error"].as_str().unwrap_or("Blocco standby non disponibile")));
        }
        Ok(child)
    }
    fn stop(&mut self) {
        if let Some(child) = self.child.take() {
            stop_inhibitor(child);
        }
    }
    pub(crate) fn start_update(&mut self, revision: &str, job: std::sync::Arc<std::sync::Mutex<Value>>) -> io::Result<Value> {
        if revision.len() != 40 || !revision.bytes().all(|byte|byte.is_ascii_hexdigit()) { return Err(invalid("Revisione master non disponibile")); }
        if self.awake_pending.is_some() || self.restarting || self.update.lock().unwrap()["status"] == "running" { return Err(invalid("Aggiornamento o riavvio già in corso")); }
        let executable = executable_path(std::env::current_exe()?)?;
        let revision = revision.to_owned();
        let state = self.update.clone();
        let mut current_job = job.lock().unwrap();
        if current_job["status"] == "running" { return Err(invalid("Un'operazione e' gia' in corso")); }
        *state.lock().unwrap() = json!({"status":"running"});
        *current_job = json!({"status":"running","kind":"app_update"});
        drop(current_job);
        thread::spawn(move || {
            let result = crate::app_update::update(Path::new(env!("CARGO_MANIFEST_DIR")), &revision);
            let error = match result {
                Ok(()) => {
                    #[cfg(unix)] {
                        use std::os::unix::process::CommandExt;
                        Command::new(executable).args(std::env::args_os().skip(1)).exec().to_string()
                    }
                    #[cfg(not(unix))] { "Riavvio non supportato".to_owned() }
                },
                Err(error) => error.to_string(),
            };
            *state.lock().unwrap() = json!({"status":"failed","error":error});
            *job.lock().unwrap() = json!({"status":"idle"});
        });
        Ok(self.status())
    }
    pub(crate) fn action(&mut self, action: &str) -> io::Result<Value> {
        match action {
            "status" => {},
            "restart" => {
                #[cfg(not(unix))]
                return Err(invalid("Riavvio dell'app non supportato in questo ambiente"));
                if self.restarting { return Err(invalid("Riavvio già in corso")); }
                let executable = executable_path(std::env::current_exe()?)?;
                self.stop(); self.restarting = true;
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(500));
                    #[cfg(unix)] {
                        use std::os::unix::process::CommandExt;
                        let error = Command::new(executable).args(std::env::args_os().skip(1)).exec();
                        eprintln!("Riavvio Oaclis non riuscito: {error}");
                    }
                });
            },
            _ => return Err(invalid("Comando dispositivo non valido")),
        }
        Ok(self.status())
    }
}
fn stop_inhibitor(mut child: Child) {
    child.stdin.take();
    let deadline = std::time::Instant::now()+Duration::from_secs(2);
    while matches!(child.try_wait(),Ok(None)) && std::time::Instant::now()<deadline {
        thread::sleep(Duration::from_millis(10));
    }
    if !matches!(child.try_wait(),Ok(Some(_))) { let _=child.kill(); }
    let _=child.wait();
}
impl Drop for Runtime { fn drop(&mut self) { self.stop(); } }

pub(crate) fn remote(config: &Path, id: &str, action: &str) -> io::Result<Value> {
    if !matches!(action, "status" | "restart" | "toggle-awake" | "update") { return Err(invalid("Comando dispositivo non valido")); }
    let device = devices::load(config.join("devices.json"))?.into_iter().find(|d|d.id==id).ok_or_else(||invalid("Dispositivo non trovato"))?;
    crate::state_transport::remote(&device, &json!({"kind":"device-control","action":action,"revision":env!("OACLIS_REVISION")}))
        .map_err(|error|io::Error::other(format!("{}: {error}", device.name)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn restart_resolves_replaced_binary_without_changing_existing_paths() {
        let folder = std::env::temp_dir().join(format!("oaclis-executable-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&folder).unwrap();
        let installed = folder.join("oaclis");
        let deleted = folder.join("oaclis (deleted)");
        assert!(executable_path(deleted.clone()).is_err());
        fs::write(&installed, b"installed").unwrap();
        assert_eq!(executable_path(deleted.clone()).unwrap(), installed);
        fs::write(&deleted, b"real filename").unwrap();
        assert_eq!(executable_path(deleted.clone()).unwrap(), deleted);
        fs::remove_dir_all(folder).unwrap();
    }
    #[test]
    fn status_and_invalid_commands_have_no_side_effects() {
        let mut state = Runtime { child: None, config: PathBuf::from("/unused"), error: None, restarting: false, awake_pending: None, desktop: Default::default(), update: Default::default() };
        *state.desktop.lock().unwrap() = json!({"id":"96335341"});
        let status = state.action("status").unwrap();
        assert_eq!(status["active"], false);
        assert_eq!(status["desktop"]["id"], "96335341");
        assert!(state.action("reboot-host").is_err());
        assert!(!state.restarting);
    }
    #[test]
    fn dead_inhibitor_is_never_reported_as_active() {
        let mut child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        child.wait().unwrap();
        let mut state = Runtime { child: Some(child), config: PathBuf::from("/unused"), error: None, restarting: false, awake_pending: None, desktop: Default::default(), update: Default::default() };
        let status = state.status();
        assert_eq!(status["active"], false);
        assert!(status["error"].is_string());
    }
}

#[cfg(test)]
mod update_tests {
    use super::*;
    #[test]
    fn source_identity_is_available_and_bad_revision_never_starts_a_job() {
        assert_eq!(build()["source_id"].as_str().unwrap().len(),64);
        assert!(build()["version"].as_str().unwrap().starts_with('v'));
        let mut state = Runtime { child: None, config: PathBuf::from("/unused"), error: None, restarting: false, awake_pending: None, desktop: Default::default(), update: Default::default() };
        let job = std::sync::Arc::new(std::sync::Mutex::new(json!({"status":"idle"})));
        assert!(state.start_update("HEAD; exit 1",job.clone()).is_err());
        assert_eq!(job.lock().unwrap()["status"],"idle");
    }
    #[test]
    fn app_update_cannot_replace_an_active_transfer() {
        let mut runtime = Runtime::new(Path::new("/unused"));
        let job = std::sync::Arc::new(std::sync::Mutex::new(json!({"status":"running","chat_id":"active"})));
        assert!(runtime.start_update(&"a".repeat(40),job.clone()).is_err());
        assert_eq!(job.lock().unwrap()["chat_id"],"active");
        assert_ne!(runtime.status()["update"]["status"],"running");
    }

}

pub(crate) fn control(
    runtime: &std::sync::Arc<std::sync::Mutex<Runtime>>,
    job: &std::sync::Arc<std::sync::Mutex<Value>>,
    analyses: &std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>,
    role: &str, action: &str, revision: &str,
) -> io::Result<Value> {
    if matches!(action, "restart" | "update") && (job.lock().unwrap()["status"] == "running" || !analyses.lock().unwrap().is_empty()) {
        return Err(invalid("Attendi il completamento delle operazioni in corso prima di riavviare"));
    }
    if action == "update" {
        if role != "client" { return Err(invalid("Aggiornare il master dalla sua installazione locale")); }
        return runtime.lock().unwrap().start_update(revision, job.clone());
    }
    if action == "toggle-awake" {
        let (child, config) = {
            let mut state = runtime.lock().unwrap();
            if state.awake_pending.is_some() || state.restarting { return Err(invalid("Operazione dispositivo già in corso")); }
            state.status();
            state.awake_pending = Some(state.child.is_some());
            (state.child.take(), state.config.clone())
        };
        let result = if let Some(child) = child { stop_inhibitor(child); Ok(None) } else { Runtime::start_inhibitor().map(Some) };
        let active = result.as_ref().is_ok_and(|child|child.is_some());
        let saved = if result.is_ok() { crate::staging::save_json(&config.join("keep-awake.json"),&active) } else { Ok(()) };
        let mut state = runtime.lock().unwrap();
        state.awake_pending = None;
        match result {
            Ok(child) => { state.child=child; state.error=None; }
            Err(error) => { state.error=Some(error.to_string()); return Err(error); }
        }
        if let Err(error) = saved { state.error=Some(error.to_string()); return Err(error); }
        return Ok(state.status());
    }
    let mut state = runtime.lock().unwrap();
    if state.awake_pending.is_some() && action != "status" { return Err(invalid("Operazione dispositivo già in corso")); }
    state.action(action)
}

#[cfg(test)]
mod optimization_tests {
    use super::*;
    #[test]
    fn pending_awake_operation_keeps_status_available_and_rejects_another_toggle() {
        let root=std::env::temp_dir().join("oaclis-pending-awake-test");
        let runtime=std::sync::Arc::new(std::sync::Mutex::new(Runtime {child:None,config:root,error:None,restarting:false,awake_pending:Some(true),desktop:Default::default(),update:Default::default()}));
        let job=Default::default();let analyses=Default::default();
        assert_eq!(control(&runtime,&job,&analyses,"master","status","").unwrap()["active"],true);
        assert!(control(&runtime,&job,&analyses,"master","toggle-awake","").is_err());
        assert!(control(&runtime,&job,&analyses,"master","restart","").is_err());
    }
    #[test]
    fn unresponsive_inhibitor_does_not_block_shutdown_forever() {
        let child=Command::new("sleep").arg("10").stdin(Stdio::piped()).spawn().unwrap();
        let started=std::time::Instant::now();stop_inhibitor(child);
        assert!(started.elapsed()<Duration::from_secs(4));
    }
}
