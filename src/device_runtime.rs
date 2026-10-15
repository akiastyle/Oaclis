// device_runtime.rs v1.9.123
//! Explicit app controls; no background SSH polling or system services.
use crate::{devices, invalid};
use serde_json::{Value, json};
use std::{fs, io::{self, BufRead, BufReader}, path::{Path, PathBuf}, process::{Child, Command, Stdio}, thread, time::Duration};

pub(crate) struct Runtime {
    child: Option<Child>,
    config: PathBuf,
    error: Option<String>,
    restarting: bool,
}
impl Runtime {
    pub(crate) fn new(config: &Path) -> Self {
        let mut state = Self { child: None, config: config.into(), error: None, restarting: false };
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
        json!({"active":self.child.is_some(),"error":self.error,"restarting":self.restarting})
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
    pub(crate) fn action(&mut self, action: &str) -> io::Result<Value> {
        match action {
            "status" => {},
            "toggle-awake" => {
                self.status();
                if self.child.is_some() { self.stop(); self.error = None; }
                else { self.enable()?; }
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
    if !matches!(action, "status" | "restart" | "toggle-awake") { return Err(invalid("Comando dispositivo non valido")); }
    let device = devices::load(config.join("devices.json"))?.into_iter().find(|d|d.id==id).ok_or_else(||invalid("Dispositivo non trovato"))?;
    let script = r#"import json,re,sys,urllib.request,urllib.error
page=urllib.request.urlopen('http://localhost:23232/',timeout=5).read().decode()
token=re.search(r'name="oaclis-token" content="([^"]+)"',page).group(1)
request=urllib.request.Request('http://localhost:23232/api/device/control',data=json.dumps({'device_id':'master','action':sys.argv[1]}).encode(),headers={'Content-Type':'application/json','X-Oaclis-Token':token})
try:
    print(urllib.request.urlopen(request,timeout=15).read().decode())
except urllib.error.HTTPError as error:
    print(error.read().decode())
    sys.exit(1)
"#;
    serde_json::from_slice(&crate::ssh::run(&device, &format!("python3 -c {} {}", crate::ssh::quote(script), crate::ssh::quote(action)))?).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_and_invalid_commands_have_no_side_effects() {
        let mut state = Runtime { child: None, config: PathBuf::from("/unused"), error: None, restarting: false };
        assert_eq!(state.action("status").unwrap()["active"], false);
        assert!(state.action("reboot-host").is_err());
        assert!(!state.restarting);
    }
    #[test]
    fn dead_inhibitor_is_never_reported_as_active() {
        let mut child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        child.wait().unwrap();
        let mut state = Runtime { child: Some(child), config: PathBuf::from("/unused"), error: None, restarting: false };
        let status = state.status();
        assert_eq!(status["active"], false);
        assert!(status["error"].is_string());
    }
}
