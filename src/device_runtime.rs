// device_runtime.rs v1.13.211
//! Explicit app controls; no background SSH polling or system services.
use crate::{devices, invalid};
use serde_json::{Value, json};
use std::{fs, io::{self, BufRead, BufReader}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, thread, time::Duration};

pub(crate) fn build() -> Value {
    json!({"version":env!("OACLIS_APP_VERSION"),"source_id":env!("OACLIS_SOURCE_ID"),"revision":env!("OACLIS_REVISION")})
}

pub(crate) fn snapshot(config: &Path, local: &std::sync::Mutex<Runtime>) -> io::Result<Value> {
    let fresh = fs::metadata(config.join("shared-status.json")).and_then(|m|m.modified()).ok()
        .and_then(|time|time.elapsed().ok()).is_some_and(|age|age < Duration::from_secs(30));
    let mut states = if fresh { crate::shared_settings::read_positions(config,"shared-status.json")?["runtime"].clone() } else { Value::Null };
    if !states.is_object() { states = json!({}); }
    states["master"] = local.lock().unwrap().status();
    Ok(states)
}

pub(crate) struct Runtime {
    child: Option<Child>,
    config: PathBuf,
    error: Option<String>,
    restarting: bool,
    update: std::sync::Arc<std::sync::Mutex<Value>>,
}
impl Runtime {
    pub(crate) fn new(config: &Path) -> Self {
        let mut state = Self { child: None, config: config.into(), error: None, restarting: false, update: Default::default() };
        if fs::read_to_string(config.join("keep-awake.json")).ok().is_some_and(|s| s.trim() == "true") {
            if let Err(error) = state.enable() { state.error = Some(error.to_string()); }
        }
        state
    }
    pub(crate) fn status(&mut self) -> Value {
        if self.child.as_mut().is_some_and(|child| !matches!(child.try_wait(), Ok(None))) {
            self.child.take();
            self.error = Some("Il blocco standby non è più attivo".into());
        }
        json!({"available":true,"observed_at":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,"active":self.child.is_some(),"error":self.error,"restarting":self.restarting,"build":build(),"update":*self.update.lock().unwrap()})
    }
    fn enable(&mut self) -> io::Result<()> {
        let mut child = Command::new("python3").args(["-u", "-c", include_str!("../tools/keep_awake.py")])
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let mut output = String::new();
        let stdout = child.stdout.take().unwrap();
        let (send, receive) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let result = BufReader::new(stdout).read_line(&mut output).map(|_|output);
            let _ = send.send(result);
        });
        let output = match receive.recv_timeout(Duration::from_secs(10)) {
            Ok(Ok(output)) => output,
            _ => { let _ = child.kill(); let _ = child.wait(); return Err(invalid("Il sistema non ha risposto alla richiesta di blocco standby")); }
        };
        let result: Value = serde_json::from_str(&output).unwrap_or(Value::Null);
        if result["active"] != true {
            let _ = child.kill(); let _ = child.wait();
            return Err(invalid(result["error"].as_str().unwrap_or("Blocco standby non disponibile")));
        }
        self.child = Some(child); self.error = None;
        Ok(())
    }
    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            child.stdin.take();
            let _ = child.wait();
        }
    }
    pub(crate) fn start_update(&mut self, revision: &str, job: std::sync::Arc<std::sync::Mutex<Value>>) -> io::Result<Value> {
        if revision.len() != 40 || !revision.bytes().all(|byte|byte.is_ascii_hexdigit()) { return Err(invalid("Revisione master non disponibile")); }
        if self.restarting || self.update.lock().unwrap()["status"] == "running" { return Err(invalid("Aggiornamento o riavvio già in corso")); }
        let executable = std::env::current_exe()?;
        let revision = revision.to_owned();
        let state = self.update.clone();
        *state.lock().unwrap() = json!({"status":"running"});
        *job.lock().unwrap() = json!({"status":"running","kind":"app_update"});
        thread::spawn(move || {
            let result = Command::new("python3").args(["-c", include_str!("../tools/update_app.py"), env!("CARGO_MANIFEST_DIR"), &revision]).output();
            let error = match result {
                Ok(output) if output.status.success() => {
                    #[cfg(unix)] {
                        use std::os::unix::process::CommandExt;
                        Command::new(executable).args(std::env::args_os().skip(1)).exec().to_string()
                    }
                    #[cfg(not(unix))] { "Riavvio non supportato".to_owned() }
                },
                Ok(output) => String::from_utf8_lossy(&output.stderr).chars().rev().take(2000).collect::<String>().chars().rev().collect(),
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
            "toggle-awake" => {
                self.status();
                if self.child.is_some() { self.stop(); self.error = None; }
                else if let Err(error) = self.enable() { self.error = Some(error.to_string()); return Err(error); }
                crate::staging::save_json(&self.config.join("keep-awake.json"), &self.child.is_some())?;
            },
            "restart" => {
                #[cfg(not(unix))]
                return Err(invalid("Riavvio dell'app non supportato in questo ambiente"));
                if self.restarting { return Err(invalid("Riavvio già in corso")); }
                let executable = std::env::current_exe()?;
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
impl Drop for Runtime { fn drop(&mut self) { self.stop(); } }

pub(crate) fn remote(config: &Path, id: &str, action: &str) -> io::Result<Value> {
    if !matches!(action, "status" | "restart" | "toggle-awake" | "update") { return Err(invalid("Comando dispositivo non valido")); }
    let device = devices::load(config.join("devices.json"))?.into_iter().find(|d|d.id==id).ok_or_else(||invalid("Dispositivo non trovato"))?;
    let script = r#"import json,os,pwd,re,sys,urllib.request,urllib.error
page=urllib.request.urlopen('http://localhost:23232/',timeout=5).read().decode()
token=re.search(r'name="oaclis-token" content="([^"]+)"',page).group(1)
request=urllib.request.Request('http://localhost:23232/api/device/control',data=json.dumps({'device_id':'master','action':sys.argv[1],'revision':sys.argv[2]}).encode(),headers={'Content-Type':'application/json','X-Oaclis-Token':token})
try:
    print(urllib.request.urlopen(request,timeout=15).read().decode())
except urllib.error.HTTPError as error:
    detail = error.read().decode()
    try: detail = json.loads(detail).get('error', detail)
    except ValueError: pass
    if 'Failed to inhibit: Access denied' in detail:
        user = pwd.getpwuid(os.geteuid()).pw_name
        detail = f"L'utente {user} non è autorizzato da Linux a bloccare lo standby. Il gruppo sudo non concede automaticamente questo permesso."
    print(sys.argv[3] + ": " + detail, file=sys.stderr)
    sys.exit(1)
"#;
    serde_json::from_slice(&crate::ssh::run(&device, &format!("python3 -c {} {} {} {}", crate::ssh::quote(script), crate::ssh::quote(action), crate::ssh::quote(env!("OACLIS_REVISION")), crate::ssh::quote(&device.name)))?).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_and_invalid_commands_have_no_side_effects() {
        let mut state = Runtime { child: None, config: PathBuf::from("/unused"), error: None, restarting: false, update: Default::default() };
        assert_eq!(state.action("status").unwrap()["active"], false);
        assert!(state.action("reboot-host").is_err());
        assert!(!state.restarting);
    }
    #[test]
    fn dead_inhibitor_is_never_reported_as_active() {
        let mut child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        child.wait().unwrap();
        let mut state = Runtime { child: Some(child), config: PathBuf::from("/unused"), error: None, restarting: false, update: Default::default() };
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
        let mut state = Runtime { child: None, config: PathBuf::from("/unused"), error: None, restarting: false, update: Default::default() };
        let job = std::sync::Arc::new(std::sync::Mutex::new(json!({"status":"idle"})));
        assert!(state.start_update("HEAD; exit 1",job.clone()).is_err());
        assert_eq!(job.lock().unwrap()["status"],"idle");
    }
}
