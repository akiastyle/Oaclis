// keep_awake.rs v1.14.376
//! Hold native OS inhibitors only for the lifetime of the Oaclis owner.
use crate::invalid;
use serde_json::json;
use std::{env, fs, io::{self, BufRead, BufReader, IsTerminal, Read, Write}, path::Path, process::{Child, Command, Stdio}, sync::mpsc, thread, time::Duration};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};

fn host() -> io::Result<&'static str> {
    if Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists() { return Err(invalid("Il blocco standby deve essere gestito sul sistema host, non nel container")); }
    if cfg!(target_os = "macos") { return Ok("macos"); }
    if cfg!(target_os = "linux") {
        let kernel = fs::read_to_string("/proc/sys/kernel/osrelease")?;
        return Ok(if kernel.to_lowercase().contains("microsoft") { "windows" } else { "linux" });
    }
    Err(invalid("Blocco standby non supportato in questo ambiente"))
}

fn windows(script: &str) -> Command {
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
        let script = "$ErrorActionPreference='Stop'; $path=Join-Path $env:LOCALAPPDATA 'Oaclis\\keep-awake.ps1'; if (-not (Test-Path $path)) { throw 'Preparare il supporto Windows con run.ps1' }; & $path -Check";
        if crate::app_update::run(&mut windows(script), Duration::from_secs(15))? != "ready" { return Err(invalid("Supporto standby Windows non disponibile")); }
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

fn hold() -> io::Result<()> {
    let mut children = Vec::new();
    match host()? {
        "windows" => children.push(start(&mut windows("$ErrorActionPreference='Stop'; & (Join-Path $env:LOCALAPPDATA 'Oaclis\\keep-awake.ps1')"), true)?),
        "macos" => children.push(start(Command::new("caffeinate").args(["-i", "-w", &std::process::id().to_string()]), false)?),
        _ => {
            for kind in ["sleep", "idle"] {
                // Separate requests avoid lid-switch authorization on systemd 255.
                children.push(start(Command::new("systemd-inhibit").args([&format!("--what={kind}"), "--mode=block", "--who=Oaclis", "--why=Oaclis sempre attivo", "cat"]), false)?);
            }
        }
    }
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

pub fn run() -> io::Result<()> {
    let result = hold();
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
