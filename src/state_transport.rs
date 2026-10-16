// state_transport.rs v1.10.178
//! A local Unix socket forwards approved SSH requests into the running Rust app.
use crate::{devices::Device, invalid};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    os::unix::{
        fs::{FileTypeExt, PermissionsExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
const LIMIT: u64 = 16 * 1024 * 1024;

fn receive(stream: &mut impl Read) -> io::Result<Value> {
    let mut bytes = Vec::new();
    BufReader::new(stream.take(LIMIT + 1)).read_until(b'\n', &mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("Richiesta troppo grande"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}
pub(crate) fn local(config: &Path, value: &Value) -> io::Result<Value> {
    let mut stream = UnixStream::connect(config.join("state.sock"))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    serde_json::to_writer(&mut stream, value)?;
    stream.write_all(b"\n")?;
    let response = receive(&mut stream)?;
    if response["ok"] != true {
        return Err(io::Error::other(
            response["error"]
                .as_str()
                .unwrap_or("Stato remoto non disponibile"),
        ));
    }
    Ok(response["value"].clone())
}
pub fn forward(home: &Path) -> io::Result<()> {
    let mut bytes = Vec::new();
    io::stdin().take(LIMIT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > LIMIT {
        return Err(invalid("Richiesta troppo grande"));
    }
    let request: Value = serde_json::from_slice(&bytes)?;
    let value = local(&home.join(".config/oaclis"), &request)?;
    serde_json::to_writer(io::stdout(), &value)?;
    Ok(())
}
pub(crate) fn remote(device: &Device, value: &Value) -> io::Result<Value> {
    device.validate()?;
    let mut child = Command::new("ssh")
        .args(crate::ssh::OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias)
        .arg("$HOME/.local/bin/oaclis state-request")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(value)?)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "SSH: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    if output.stdout.len() as u64 > LIMIT {
        return Err(invalid("Impostazioni troppo grandi"));
    }
    serde_json::from_slice(&output.stdout).map_err(io::Error::other)
}
pub(crate) fn start(
    config: PathBuf,
    codex: PathBuf,
    runtime: Arc<Mutex<crate::device_runtime::Runtime>>,
) -> io::Result<()> {
    let path = config.join("state.sock");
    if let Ok(metadata) = path.symlink_metadata() {
        if !metadata.file_type().is_socket() {
            return Err(invalid("Socket stato non valido"));
        }
        std::fs::remove_file(&path)?;
    }
    let listener = UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    let (sender, receiver) = mpsc::sync_channel::<UnixStream>(16);
    let receiver = Arc::new(Mutex::new(receiver));
    for _ in 0..4 {
        let receiver = receiver.clone();
        let config = config.clone();
        let codex = codex.clone();
        let runtime = runtime.clone();
        thread::spawn(move || {
            loop {
                let stream = receiver.lock().unwrap().recv();
                let Ok(mut stream) = stream else { return };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
                let result = (|| -> io::Result<Value> {
                    let request = receive(&mut stream)?;
                    match request["kind"].as_str(){
                Some("catalog")=>crate::catalog::local_at(request["root"].as_str().map(Path::new).unwrap_or(&codex),&config).map(|chats|json!({"root":request["root"].as_str().map(Path::new).unwrap_or(&codex),"chats":chats})),
                Some("chat-proof")=>crate::chat::proof(Path::new(request["path"].as_str().ok_or_else(||invalid("Percorso chat mancante"))?),request["chat_id"].as_str().ok_or_else(||invalid("Chat mancante"))?,request["prefix_bytes"].as_u64().ok_or_else(||invalid("Prefisso non valido"))?),
                Some("shared-exchange")=>{let mut value=crate::shared_settings::exchange(&config,Some(&request["value"]))?;value["runtime"]=runtime.lock().unwrap().status();Ok(value)},
                Some("snapshot")=>serde_json::to_value(crate::state_store::snapshot(&config)?.as_ref()).map_err(io::Error::other),
                Some("database")=>crate::state_store::database(&config,request["key"].as_str().unwrap_or(""),request["operation"].as_str().ok_or_else(||invalid("Operazione mancante"))?,request.get("value")),
                _=>Err(invalid("Azione stato non valida")),
            }
                })();
                let response = match result {
                    Ok(value) => json!({"ok":true,"value":value}),
                    Err(error) => json!({"ok":false,"error":error.to_string()}),
                };
                let _ = serde_json::to_writer(&mut stream, &response);
                let _ = stream.write_all(b"\n");
            }
        });
    }
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if sender.send(stream).is_err() {
                break;
            }
        }
    });
    Ok(())
}
