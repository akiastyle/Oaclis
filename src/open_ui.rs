// open_ui.rs v1.10.175
use std::{env, io::{Read, Write}, net::{SocketAddr, TcpStream}, process::Command, thread, time::Duration};

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
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    if env::var_os("WSL_DISTRO_NAME").is_some() || kernel.to_lowercase().contains("microsoft") {
        println!("Webapp Windows: {URL}. Apertura gestita da run.ps1.");
        return;
    }
    if cfg!(target_os = "macos") || env::var_os("DISPLAY").is_some() || env::var_os("WAYLAND_DISPLAY").is_some() {
        let launcher = if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
        if Command::new(launcher).arg(URL).status().is_ok_and(|s| s.success()) { return; }
    }
    println!("Webapp: {URL}");
}

#[cfg(test)]
mod tests {
    use super::*;
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
