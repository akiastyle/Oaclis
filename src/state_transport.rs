// state_transport.rs v1.14.226
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

static LOG: std::sync::OnceLock<Mutex<std::collections::VecDeque<Value>>> = std::sync::OnceLock::new();

pub(crate) fn log(message: String) {
    let mut rows = LOG.get_or_init(Default::default).lock().unwrap();
    let id = rows.back().and_then(|row|row["id"].as_u64()).unwrap_or(0) + 1;
    if rows.len() == 500 { rows.pop_front(); }
    rows.push_back(json!({"id":id,"time":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),"text":message}));
}

pub(crate) fn logs() -> Value {
    json!(*LOG.get_or_init(Default::default).lock().unwrap())
}

fn log_request(direction: &str, peer: &str, value: &Value) {
    let fields: serde_json::Map<String,Value> = ["kind","action","operation","key","path","root","chat_id","revision"]
        .into_iter().filter_map(|key|value.get(key).filter(|v|v.is_string()).map(|v|(key.into(),v.clone()))).collect();
    log(format!("{direction} {peer} {}", Value::Object(fields)));
}


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
    log_request("SEND", &device.ssh_alias, value);
    let mut child = Command::new("ssh")
        .args(crate::ssh::OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias)
        .arg(if value["kind"]=="environment" {"$HOME/.local/bin/oaclis environment-request"} else {"$HOME/.local/bin/oaclis state-request"})
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
    log(format!("RECV {} {}", device.ssh_alias, output.status));
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
    job: Arc<Mutex<Value>>,
    analyses: Arc<Mutex<std::collections::HashSet<String>>>,
    role: String,
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
        let job = job.clone();
        let analyses = analyses.clone();
        let role = role.clone();
        thread::spawn(move || {
            loop {
                let stream = receiver.lock().unwrap().recv();
                let Ok(mut stream) = stream else { return };
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
                let result = (|| -> io::Result<Value> {
                    let request = receive(&mut stream)?;
                    log_request("RECV", "local", &request);
                    match request["kind"].as_str(){
                Some("terminal")=>crate::terminal::local(&request["value"]),
                Some("device-control")=>crate::device_runtime::control(&runtime,&job,&analyses,&role,request["action"].as_str().ok_or_else(||invalid("Comando richiesto"))?,request["revision"].as_str().unwrap_or("")),
                Some("prepare-path")=>crate::path_access::prepare(request.clone()),
                Some("catalog")=>crate::catalog::local_at(request["root"].as_str().map(Path::new).unwrap_or(&codex),&config).map(|chats|json!({"root":request["root"].as_str().map(Path::new).unwrap_or(&codex),"chats":chats})),
                Some("chat-proof")=>crate::chat::proof(Path::new(request["path"].as_str().ok_or_else(||invalid("Percorso chat mancante"))?),request["chat_id"].as_str().ok_or_else(||invalid("Chat mancante"))?,request["prefix_bytes"].as_u64().ok_or_else(||invalid("Prefisso non valido"))?),
                Some("shared-exchange")=>{let mut value=crate::shared_settings::exchange(&config,Some(&request["value"]))?;value["runtime"]=runtime.lock().unwrap().status();Ok(value)},
                Some("snapshot")=>serde_json::to_value(crate::state_store::snapshot(&config)?.as_ref()).map_err(io::Error::other),
                Some("database")=>crate::state_store::database(&config,request["key"].as_str().unwrap_or(""),request["operation"].as_str().ok_or_else(||invalid("Operazione mancante"))?,request.get("value")),
                _=>Err(invalid("Azione stato non valida")),
            }
                })();
                log(format!("SEND local {}", match &result { Ok(_) => "OK".to_owned(), Err(error) => error.to_string() }));
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

#[cfg(test)]
mod log_tests {
    #[test]
    fn receiver_log_omits_passwords_and_payloads_and_is_bounded() {
        super::log_request("RECV", "fixture", &serde_json::json!({"kind":"prepare-path","path":"/var/www/log-fixture","password":"never-log-password","value":{"token":"never-log-token"}}));
        let rows = super::logs();
        let row = rows.as_array().unwrap().iter().find(|r|r["text"].as_str().unwrap().contains("/var/www/log-fixture")).unwrap();
        assert!(!row.to_string().contains("never-log"));
        for _ in 0..510 {super::log("fixture".into());}
        assert!(super::logs().as_array().unwrap().len() <= 500);
    }
}
