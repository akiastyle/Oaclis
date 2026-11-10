// keep_awake.rs v1.20.509
//! Native power settings; no forced monitor-off or forced sleep commands.
use crate::invalid;
use serde_json::json;
use std::{env, fs, io::{self, BufRead, BufReader, IsTerminal, Read, Write}, path::Path, process::{Child, Command, Stdio}, sync::mpsc, thread, time::Duration};
use std::os::unix::fs::{FileTypeExt, PermissionsExt, MetadataExt};

fn host() -> io::Result<&'static str> {
    if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() { return Err(invalid("Il blocco standby deve essere gestito sul sistema host, non nel container")); }
    if cfg!(target_os = "macos") { return Ok("macos"); }
    if cfg!(target_os = "linux") {
        let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")?;
        return Ok(if kernel.to_lowercase().contains("microsoft") { "windows" } else { "linux" });
    }
    Err(invalid("Blocco standby non supportato in questo ambiente"))
}

pub(crate) fn windows(script: &str) -> Command {
    let mut command = Command::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]);
    if !env::var_os("WSL_INTEROP").is_some_and(|p| fs::metadata(p).is_ok_and(|m| m.file_type().is_socket())) {
        let mut sockets = vec![Path::new("/run/WSL/1_interop").to_path_buf()];
        if let Ok(entries) = fs::read_dir("/run/WSL") {
            let mut paths: Vec<_> = entries.flatten().map(|e|e.path()).filter(|p|p.to_string_lossy().ends_with("_interop")).collect();
            paths.sort(); sockets.extend(paths);
        }
        if let Some(socket) = sockets.into_iter().find(|p|p.metadata().is_ok_and(|m|m.file_type().is_socket())) { command.env("WSL_INTEROP", socket); }
    }
    command
}

fn rule(user: &str) -> io::Result<String> {
    let name = user.strip_suffix('$').unwrap_or(user);
    if name.is_empty() || !name.bytes().next().is_some_and(|b|b.is_ascii_alphabetic() || b == b'_')
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') { return Err(invalid("Nome utente non supportato per la regola standby")); }
    Ok(format!("polkit.addRule(function(action, subject) {{ if (subject.user === {} && (action.id === \"org.freedesktop.login1.inhibit-block-sleep\" || action.id === \"org.freedesktop.login1.inhibit-block-idle\")) {{ return polkit.Result.YES; }} }});\n", serde_json::to_string(user)?))
}

fn available(name: &str) -> bool {
    env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|p|p.join(name).metadata().is_ok_and(|m|m.is_file() && m.permissions().mode() & 0o111 != 0))
}

pub fn prepare(user: Option<&str>) -> io::Result<()> {
    let host = host()?;
    if host == "windows" {
        let script = format!("& {{ {} }} -Check",include_str!("../scripts/keep-awake.ps1"));
        if crate::app_update::run(&mut windows(&script), Duration::from_secs(15))? != "ready" { return Err(invalid("Supporto standby Windows non disponibile")); }
    } else if host == "macos" {
        if !available("caffeinate") { return Err(invalid("Manca caffeinate")); }
    } else {
        if !available("systemd-inhibit") { return Err(invalid("Blocco standby non disponibile: manca systemd-inhibit")); }
        let user = user.map(str::to_owned).or_else(||env::var("SUDO_USER").ok().filter(|s|!s.is_empty()))
            .unwrap_or(crate::app_update::run(Command::new("id").arg("-un"), Duration::from_secs(5))?);
        let rule = rule(&user)?;
        let uid = crate::app_update::run(Command::new("id").args(["-u", &user]), Duration::from_secs(5))?;
        if uid.is_empty() || !uid.bytes().all(|b|b.is_ascii_digit()) { return Err(invalid("UID non valido")); }
        let path = format!("/etc/polkit-1/rules.d/49-oaclis-awake-{uid}.rules");
        if fs::symlink_metadata(&path).is_ok_and(|m|!m.is_file()) { return Err(invalid("File del permesso standby non regolare")); }
        let current_uid = crate::app_update::run(Command::new("id").arg("-u"), Duration::from_secs(5))?;
        if current_uid != "0" {
            let mut command = Command::new("sudo");
            if !io::stdin().is_terminal() { command.arg("-n"); }
            // Authenticate against the terminal before piping the rule, never a password.
            if !command.arg("-v").status()?.success() { return Err(invalid("Serve una prima autorizzazione amministrativa: eseguire bash run.sh --prepare-awake dal proprio utente sul dispositivo")); }
        }
        let mut temporary = crate::staging::StagedFile::new(&env::temp_dir())?;
        temporary.file.write_all(rule.as_bytes())?; temporary.file.sync_all()?;
        let mut command = if current_uid == "0" { Command::new("install") } else { let mut c = Command::new("sudo"); c.args(["-n", "install"]); c };
        if !command.args(["-m", "0644", "-o", "root", "-g", "root"]).arg(&temporary.path).arg(&path).status()?.success() { return Err(invalid("Preparazione del permesso standby non riuscita")); }
        println!("Supporto Sempre attivo preparato per {user}. Il blocco si attiva dal pulsante Oaclis.");
        return Ok(());
    }
    println!("Supporto Sempre attivo preparato. Il blocco si attiva dal pulsante Oaclis.");
    Ok(())
}

struct Inhibitor(Child);
impl Drop for Inhibitor {
    fn drop(&mut self) {
        self.0.stdin.take();
        for _ in 0..30 {
            if self.0.try_wait().ok().flatten().is_some() { return; }
            thread::sleep(Duration::from_millis(100));
        }
        let _ = self.0.kill(); let _ = self.0.wait();
    }
}

fn start(command: &mut Command, ready: bool) -> io::Result<Inhibitor> {
    let mut child = Inhibitor(command.stdin(Stdio::piped()).stdout(if ready { Stdio::piped() } else { Stdio::null() }).stderr(Stdio::piped()).spawn()?);
    if ready {
        let stdout = child.0.stdout.take().unwrap();
        let (send, receive) = mpsc::channel();
        thread::spawn(move || { let mut line = String::new(); let _ = send.send(BufReader::new(stdout).read_line(&mut line).map(|_|line)); });
        match receive.recv_timeout(Duration::from_secs(15)) {
            Ok(Ok(line)) if line.trim() == "ready" => {},
            _ => {
                let mut detail = String::new();
                if child.0.try_wait()?.is_some() {
                    if let Some(stderr) = child.0.stderr.take() { stderr.take(2000).read_to_string(&mut detail)?; }
                }
                return Err(invalid(format!("Windows non ha attivato il blocco standby: {}", if detail.trim().is_empty() { "verificare il supporto con run.ps1 -PrepareAwake" } else { detail.trim() })));
            },
        }
    }
    Ok(child)
}

struct DisplayPower {
    environment: Vec<(String,String)>,
    restore: Vec<String>,
}
impl DisplayPower {
    fn command(&self, args: &[&str]) -> Command {
        let mut command=Command::new("xset");command.envs(self.environment.iter().cloned()).args(args);command
    }
    fn apply(mode: &str) -> io::Result<Option<Self>> {
        let mut environment=Vec::new();
        if env::var_os("DISPLAY").is_none() {
            let sessions=crate::app_update::run(Command::new("loginctl").args(["list-sessions","--no-legend"]),Duration::from_secs(3)).unwrap_or_default();
            for line in sessions.lines().take(32) {
                let fields:Vec<_>=line.split_whitespace().collect();if fields.len()<3 {continue;}
                let properties=crate::app_update::run(Command::new("loginctl").args(["show-session",fields[0],"-p","Active","-p","Type","-p","Display"]),Duration::from_secs(2)).unwrap_or_default();
                if !properties.lines().any(|line|line=="Active=yes") {continue;}
                if properties.lines().any(|line|line=="Type=wayland") {return Err(invalid("Controllo monitor non disponibile in questa sessione Wayland"));}
                if !properties.lines().any(|line|line=="Type=x11") {continue;}
                let runtime=format!("XDG_RUNTIME_DIR=/run/user/{}",fields[1]);let bus=format!("DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/{}/bus",fields[1]);
                let mut command=if fs::metadata("/proc/self").is_ok_and(|metadata|fields[1].parse::<u32>().ok()==Some(metadata.uid())) {Command::new("env")} else {let mut c=Command::new("runuser");c.args(["-u",fields[2],"--","env"]);c};
                let values=crate::app_update::run(command.args([&runtime,&bus,"systemctl","--user","show-environment"]),Duration::from_secs(3)).unwrap_or_default();
                for line in values.lines() {if let Some((key,value))=line.split_once('=') {if matches!(key,"DISPLAY"|"XAUTHORITY") {environment.push((key.into(),value.into()));}}}
                if !environment.iter().any(|(key,_)|key=="DISPLAY") {if let Some(display)=properties.lines().find_map(|line|line.strip_prefix("Display=")).filter(|v|!v.is_empty()) {environment.push(("DISPLAY".into(),display.into()));}}
                break;
            }
            if !environment.iter().any(|(key,_)|key=="DISPLAY") {
                if mode=="screen-off" {return Err(invalid("Nessun monitor disponibile"));}
                return Ok(None);
            }
        } else if env::var_os("WAYLAND_DISPLAY").is_some() {return Err(invalid("Controllo monitor non disponibile in questa sessione Wayland"));}
        let mut display=Self {environment,restore:Vec::new()};
        let query=crate::app_update::run(&mut display.command(&["q"]),Duration::from_secs(3))?;
        let timeout=query.lines().find(|line|line.trim_start().starts_with("timeout:"))
            .map(|line|line.split_whitespace().collect::<Vec<_>>()).filter(|values|values.len()>=4)
            .ok_or_else(||invalid("Impostazioni monitor non disponibili"))?;
        let cycle=timeout[3].parse::<u32>().map_err(|_|invalid("Impostazioni monitor non valide"))?;let timeout=timeout[1].parse::<u32>().map_err(|_|invalid("Impostazioni monitor non valide"))?;
        let dpms=query.contains("DPMS is Enabled") || query.contains("DPMS is Disabled");
        if mode=="screen-off" && !dpms {return Err(invalid("Questo monitor non supporta lo spegnimento"));}
        display.restore=vec!["s".into(),timeout.to_string(),cycle.to_string()];
        if mode=="always" {
            let args=if dpms {vec!["s","off","-dpms"]} else {vec!["s","off"]};
            crate::app_update::run(&mut display.command(&args),Duration::from_secs(3))?;
        } else {
            let timers=query.lines().find(|line|line.contains("Standby:") && line.contains("Suspend:")).map(|line|line.split_whitespace().collect::<Vec<_>>()).filter(|v|v.len()>=6).ok_or_else(||invalid("Timeout monitor non disponibili"))?;
            display.restore.push("dpms".into());
            for index in [1,3,5] {display.restore.push(timers[index].parse::<u32>().map_err(|_|invalid("Timeout monitor non validi"))?.to_string());}
            crate::app_update::run(&mut display.command(&["s","300","300","+dpms","dpms","300","300","300"]),Duration::from_secs(3))?;
        }
        if dpms {display.restore.push(if query.contains("DPMS is Enabled") {"+dpms"} else {"-dpms"}.into());}
        Ok(Some(display))
    }
}
impl Drop for DisplayPower {
    fn drop(&mut self) {
        if !self.restore.is_empty() {let args:Vec<_>=self.restore.iter().map(String::as_str).collect();let _=crate::app_update::run(&mut self.command(&args),Duration::from_secs(1));}
    }
}

struct MacDisplay(Vec<(String,String)>);
impl MacDisplay {
    fn apply(mode: &str) -> io::Result<Self> {
        let query=crate::app_update::run(Command::new("pmset").args(["-g","custom"]),Duration::from_secs(3))?;
        let mut saved=Self(Vec::new()); let mut profile=None;
        for line in query.lines() {
            let line=line.trim();
            if line.ends_with(':') {profile=match line {"AC Power:"=>Some("-c"),"Battery Power:"=>Some("-b"),"UPS Power:"=>Some("-u"),_=>None};}
            let fields:Vec<_>=line.split_whitespace().collect();
            if fields.first()==Some(&"displaysleep") {
                if let (Some(profile),Some(value))=(profile,fields.get(1)) {
                    value.parse::<u32>().map_err(|_|invalid("Timeout monitor macOS non valido"))?;
                    saved.0.push((profile.into(),value.to_string()));
                }
            }
        }
        if saved.0.is_empty(){return Err(invalid("Timeout monitor macOS non disponibile"));}
        crate::app_update::run(Command::new("pmset").args(["-a","displaysleep",if mode=="screen-off" {"5"} else {"0"}]),Duration::from_secs(5))?;
        Ok(saved)
    }
}
impl Drop for MacDisplay {
    fn drop(&mut self) {
        for (profile,value) in &self.0 {let _=crate::app_update::run(Command::new("pmset").args([profile,"displaysleep",value]),Duration::from_secs(3));}
    }
}

fn hold(mode: &str) -> io::Result<()> {
    if !matches!(mode,"always"|"screen-off") {return Err(invalid("Modalità energia non valida"));}
    let mut display=None;
    let mut mac_display=None;
    let mut children = Vec::new();
    match host()? {
        "windows" => {
            if crate::app_update::run(&mut windows(&format!("& {{ {} }} -Mode '{mode}'",include_str!("../scripts/keep-awake.ps1"))),Duration::from_secs(15))?!="ready" {return Err(invalid("Impostazioni energia Windows non confermate"));}
        },
        "macos" => {
            children.push(start(Command::new("caffeinate").args([if mode=="always" {"-di"} else {"-i"}, "-w", &std::process::id().to_string()]), false)?);
            mac_display=Some(MacDisplay::apply(mode)?);
        },
        _ => {
            display=DisplayPower::apply(mode)?;
            for kind in if mode=="always" {vec!["sleep","idle"]} else {vec!["sleep"]} {
                // Separate requests avoid lid-switch authorization on systemd 255.
                children.push(start(Command::new("systemd-inhibit").args([&format!("--what={kind}"), "--mode=block", "--who=Oaclis", "--why=Oaclis sempre attivo", "cat"]), false)?);
            }
        }
    }
    let _display=display;
    let _mac_display=mac_display;
    thread::sleep(Duration::from_millis(250));
    for child in &mut children {
        if child.0.try_wait()?.is_some() {
            let mut error = String::new();
            if let Some(mut stderr) = child.0.stderr.take() { stderr.read_to_string(&mut error)?; }
            if error.contains("Access denied") { error = "Permesso Linux mancante: eseguire bash run.sh --prepare-awake sul dispositivo".into(); }
            return Err(invalid(if error.trim().is_empty() { "Blocco standby terminato" } else { error.trim() }));
        }
    }
    println!("{}", json!({"active":true})); io::stdout().flush()?;
    let (send, receive) = mpsc::channel();
    thread::spawn(move || { let _ = io::copy(&mut io::stdin().lock(), &mut io::sink()); let _ = send.send(()); });
    loop {
        match receive.recv_timeout(Duration::from_secs(1)) {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
            Err(mpsc::RecvTimeoutError::Timeout) => {},
        }
        if children.iter_mut().any(|c| !matches!(c.0.try_wait(), Ok(None))) { return Err(invalid("Il sistema ha interrotto il blocco standby")); }
    }
}

pub(crate) fn restore() -> io::Result<()> {
    if matches!(host(),Ok("windows")) && crate::app_update::run(&mut windows(&format!("& {{ {} }} -Mode normal",include_str!("../scripts/keep-awake.ps1"))),Duration::from_secs(15))?!="ready" {return Err(invalid("Ripristino energia Windows non confermato"));}
    Ok(())
}

pub fn run(mode: &str) -> io::Result<()> {
    let result = hold(mode);
    if let Err(error) = &result { println!("{}", json!({"active":false,"error":error.to_string()})); io::stdout().flush()?; }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn step2_awake_rule_is_user_scoped_and_rejects_injection() {
        let text = rule("kimrusso").unwrap();
        assert!(text.contains("subject.user === \"kimrusso\""));
        assert_eq!(text.matches("action.id ===").count(), 2);
        assert!(!text.contains("isInGroup"));
        assert!(rule("root\nmalicious").is_err());
        assert!(rule("-root").is_err());
        assert!(rule("user.name").is_err());
        assert!(rule("test_user$").is_ok());
    }
    #[test]
    fn step2_awake_release_closes_input_and_reports_failed_readiness() {
        let root = env::temp_dir().join(format!("oaclis-awake-test-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let marker = root.join("released");
        let mut command = Command::new("sh");
        command.args(["-c", "echo ready; cat >/dev/null; echo released > \"$1\"", "fixture"]).arg(&marker);
        let child = start(&mut command, true).unwrap();
        assert!(!marker.exists());
        drop(child);
        assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "released");
        assert!(start(Command::new("sh").args(["-c", "echo failed >&2; exit 1"]), true).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
