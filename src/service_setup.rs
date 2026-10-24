// service_setup.rs v1.8.192
use crate::invalid;
use std::{env, fs, io::{self, Write}, net::{TcpStream, SocketAddr}, path::Path, process::Command, time::Duration};

fn quoted(value: &str) -> io::Result<String> {
    if value.contains(['\n', '\r', '\0']) { return Err(invalid("Percorso o ambiente non valido per systemd")); }
    Ok(format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"").replace('%', "%%")))
}

fn unit(home: &Path, uid: &str, gid: &str, name: &str, environment: &[(String, String)]) -> io::Result<String> {
    if uid == "0" { return Err(invalid("Il servizio Oaclis richiede un utente normale")); }
    let mut paths = vec![home.join(".local/bin"), home.join(".cargo/bin")];
    for path in env::split_paths(&environment.iter().find(|(k, _)| k == "PATH").map(|(_, v)| v.as_str()).unwrap_or("/usr/local/bin:/usr/bin:/bin")) {
        if !paths.contains(&path) { paths.push(path); }
    }
    let mut variables = vec![("HOME", home.to_string_lossy().into_owned()), ("PATH", env::join_paths(paths).map_err(io::Error::other)?.to_string_lossy().into_owned())];
    for key in ["OACLIS_PAUSE_SETTINGS", "OACLIS_PAUSE_CHECKS", "OACLIS_PAUSE_TRANSFERS"] { variables.push((key, "0".into())); }
    for key in ["CODEX_HOME", "NVM_DIR", "WSL_DISTRO_NAME"] {
        if let Some((_, value)) = environment.iter().find(|(k, v)| k == key && !v.is_empty()) { variables.push((key, value.clone())); }
    }
    let settings = variables.iter().map(|(k, v)| quoted(&format!("{k}={v}")).map(|v| format!("Environment={v}"))).collect::<io::Result<Vec<_>>>()?.join("\n");
    quoted(name)?;
    Ok(format!("[Unit]\nDescription=Oaclis ({name})\nWants=network-online.target\nAfter=network-online.target\nStartLimitIntervalSec=0\n\n[Service]\nType=simple\nUser={uid}\nGroup={gid}\nWorkingDirectory=~\n{settings}\nExecStart={} serve\nRestart=always\nRestartSec=5\nTimeoutStopSec=30\nKillMode=control-group\nUMask=0077\n\n[Install]\nWantedBy=multi-user.target\n", quoted(&home.join(".local/bin/oaclis").to_string_lossy())?))
}

fn checked(command: &mut Command) -> io::Result<()> {
    let status = command.status()?;
    if !status.success() { return Err(io::Error::other(format!("Comando non riuscito: {command:?} ({status})"))); }
    Ok(())
}

pub fn prepare() -> io::Result<()> {
    if !cfg!(target_os = "linux") || fs::read_to_string("/proc/1/comm").unwrap_or_default().trim() != "systemd" {
        return Err(invalid("Avvio automatico richiede Linux/WSL con systemd attivo. In WSL impostare [boot] systemd=true in /etc/wsl.conf e riavviare la distribuzione. Nei container usare la politica di riavvio del container."));
    }
    let identity = Command::new("id").args(["-u"]).output()?;
    if !identity.status.success() { return Err(invalid("Impossibile identificare l'utente")); }
    let uid = String::from_utf8_lossy(&identity.stdout).trim().to_owned();
    if uid == "0" { return Err(invalid("Installare il servizio Oaclis come utente normale, senza sudo davanti allo script. Installazione root esistente viene conservata.")); }
    let record = Command::new("getent").args(["passwd", &uid]).output()?;
    if !record.status.success() { return Err(invalid("Utente corrente non disponibile")); }
    let record = String::from_utf8_lossy(&record.stdout);
    let fields: Vec<_> = record.trim_end().split(':').collect();
    if fields.len() != 7 || fields[2] != uid || !fields[3].bytes().all(|b| b.is_ascii_digit()) { return Err(invalid("Identita utente non valida")); }
    let home = Path::new(fields[5]);
    if fs::canonicalize(env::var_os("HOME").ok_or_else(|| invalid("HOME non definita"))?)? != fs::canonicalize(home)? {
        return Err(invalid("HOME non coincide con l'utente corrente: eseguire come utente dell'installazione Oaclis, senza sudo davanti allo script."));
    }
    if !home.join(".local/bin/oaclis").is_file() { return Err(invalid("Installare Oaclis prima di abilitare il servizio.")); }
    let name = format!("oaclis-{uid}.service");
    let privileged = uid != "0";
    if privileged { checked(Command::new("sudo").arg("-v"))?; }
    let content = unit(home, &uid, fields[3], fields[0], &env::vars().collect::<Vec<_>>())?;
    let mut temporary = crate::staging::StagedFile::new(&env::temp_dir())?;
    // systemd-analyze requires a recognized unit suffix.
    let source = temporary.path.with_extension("service");
    temporary.file.write_all(content.as_bytes())?;
    temporary.file.sync_all()?;
    fs::rename(&temporary.path, &source)?;
    temporary.path = source;
    checked(Command::new("systemd-analyze").arg("verify").arg(&temporary.path))?;
    let target = Path::new("/etc/systemd/system").join(&name);
    match fs::read_to_string(&target) {
        Ok(old) if old != content && !old.starts_with("[Unit]\nDescription=Oaclis (") => return Err(invalid(format!("{} contiene un servizio diverso: file conservato.", target.display()))),
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut install = if privileged { let mut c = Command::new("sudo"); c.arg("install"); c } else { Command::new("install") };
    checked(install.args(["-m", "644"]).arg(&temporary.path).arg(&target))?;
    for args in [vec!["daemon-reload"], vec!["enable", &name]] {
        let mut command = if privileged { let mut c = Command::new("sudo"); c.arg("systemctl"); c } else { Command::new("systemctl") };
        checked(command.args(args))?;
    }
    let occupied = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], 23232)), Duration::from_secs(1)).is_ok();
    if occupied && !Command::new("systemctl").args(["is-active", "--quiet", &name]).status()?.success() {
        println!("Servizio {name} abilitato. Oaclis e gia aperto fuori dal servizio: non lo interrompo; il servizio partira al prossimo avvio del sistema.");
        return Ok(());
    }
    let mut command = if privileged { let mut c = Command::new("sudo"); c.arg("systemctl"); c } else { Command::new("systemctl") };
    checked(command.args(["start", &name]))?;
    checked(Command::new("systemctl").args(["is-active", "--quiet", &name]))?;
    println!("Oaclis attivo; avvio automatico abilitato: {name}. Log: journalctl -u {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_service_preserves_identity_and_escapes() {
        let text = unit(Path::new("/home/test user"), "1001", "1001", "test", &[
            ("PATH".into(), "/home/test user/.nvm/v22/bin:/usr/bin".into()),
            ("CODEX_HOME".into(), "/home/test user/.codex".into()),
        ]).unwrap();
        for expected in ["User=1001", "Group=1001", "Restart=always", "WantedBy=multi-user.target", "ExecStart=\"/home/test user/.local/bin/oaclis\" serve", "/.nvm/v22/bin", "CODEX_HOME=", "OACLIS_PAUSE_TRANSFERS=0"] {
            assert!(text.contains(expected), "{expected}");
        }
        assert_eq!(quoted("/home/a%u").unwrap(), "\"/home/a%%u\"");
        assert!(quoted("path\nExecStart=unwanted").is_err());
        assert!(unit(Path::new("/root"), "0", "0", "root", &[]).is_err());
        assert!(!text.contains("sudo"));
        assert!(!text.contains(" setup"));
    }
}
