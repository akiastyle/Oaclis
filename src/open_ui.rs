// open_ui.rs v1.7.143
use std::{env, fs, path::{Path, PathBuf}, io::{Read, Write}, net::{SocketAddr, TcpStream}, process::Command, thread, time::Duration};

pub(crate) fn prepare_access(config: &Path, token: &str) -> std::io::Result<()> {
    let mut staged = crate::staging::StagedFile::new(config)?;
    write!(staged.file, "<!doctype html><meta name=\"referrer\" content=\"no-referrer\"><meta http-equiv=\"refresh\" content=\"0;url=http://localhost:23232/#access={token}\">")?;
    staged.file.sync_all()?;
    fs::rename(&staged.path, config.join("open-ui.html"))
}

pub fn launch_path() -> std::io::Result<PathBuf> {
    let path = PathBuf::from(env::var_os("HOME").ok_or_else(|| crate::invalid("HOME non definita"))?).join(".config/oaclis/open-ui.html");
    crate::open_regular(&path)?;
    Ok(path)
}

fn available(address: SocketAddr) -> bool {
    let Ok(mut socket) = TcpStream::connect_timeout(&address, Duration::from_secs(1)) else { return false; };
    if socket.set_read_timeout(Some(Duration::from_secs(1))).is_err() || socket.set_write_timeout(Some(Duration::from_secs(1))).is_err() { return false; }
    if socket.write_all(b"GET / HTTP/1.1\r\nHost: localhost:23232\r\nConnection: close\r\n\r\n").is_err() { return false; }
    let mut line = Vec::new();
    for _ in 0..256 {
        let mut byte = [0];
        if socket.read_exact(&mut byte).is_err() { return false; }
        if byte[0] == b'\n' { break; }
        line.push(byte[0]);
    }
    String::from_utf8_lossy(&line).split_whitespace().nth(1) == Some("200")
}

pub fn open() {
    const URL: &str = "http://localhost:23232";
    let ready = (0..30).any(|_| { if available(SocketAddr::from(([127, 0, 0, 1], 23232))) { true } else { thread::sleep(Duration::from_secs(1)); false } });
    if !ready { println!("Interfaccia non ancora disponibile: {URL}. Verificare il servizio Oaclis."); return; }
    let path = match launch_path() { Ok(path) => path, Err(_) => { println!("Avviare Oaclis prima di aprire la webapp."); return; } };
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    if env::var_os("WSL_DISTRO_NAME").is_some() || kernel.to_lowercase().contains("microsoft") {
        println!("Webapp Windows: {URL}. Apertura gestita da run.ps1.");
        return;
    }
    if cfg!(target_os = "macos") || env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some() {
        let launcher = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        if Command::new(launcher).arg(&path).status().is_ok_and(|s| s.success()) { return; }
    }
    println!("Aprire la webapp dal file personale: {}", path.display());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn personal_launcher_is_private_and_keeps_credentials_out_of_http_requests() {
        use std::os::unix::fs::PermissionsExt;
        let directory = env::temp_dir().join(format!("oaclis-launch-{}", std::process::id()));
        fs::create_dir(&directory).unwrap();
        let token = "a".repeat(64);
        prepare_access(&directory, &token).unwrap();
        let file = directory.join("open-ui.html");
        assert_eq!(fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert!(fs::read_to_string(&file).unwrap().contains(&format!("/#access={token}")));
        fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn bootstrap_ui_checks_http_status_without_opening_browser() {
        for (status, expected) in [(200, true), (403, false), (503, false)] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut request = [0; 1024];
                socket.read(&mut request).unwrap();
                socket.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Length: 0\r\n\r\n").as_bytes()).unwrap();
            });
            assert_eq!(available(address), expected);
            server.join().unwrap();
        }
    }
}
