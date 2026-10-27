// state_transport.rs v1.17.296
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
    remote_progress(device,value,&mut |_| {})
}
pub(crate) fn remote_progress(device: &Device, value: &Value, notify: &mut impl FnMut(Value)) -> io::Result<Value> {
    device.validate()?;
    log_request("SEND", &device.ssh_alias, value);
    let mut errors=crate::staging::StagedFile::new(&std::env::temp_dir())?;
    let mut child = Command::new("ssh")
        .args(crate::ssh::OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias)
        .arg(if value["kind"]=="environment" {"$HOME/.local/bin/oaclis environment-request"} else {"$HOME/.local/bin/oaclis state-request"})
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(errors.file.try_clone()?).spawn()?;
    if let Err(error)=child.stdin.take().unwrap().write_all(&serde_json::to_vec(value)?) {let _=child.kill();let _=child.wait();return Err(error);}
    let result=read_response(BufReader::new(child.stdout.take().unwrap()),value["kind"]=="environment" && value["operation"]=="install",notify);
    if result.is_err() {let _=child.kill();}
    let status=child.wait()?;
    log(format!("RECV {} {}", device.ssh_alias, status));
    if !status.success() {
        use std::io::{Seek,SeekFrom};
        let size=errors.file.metadata()?.len();errors.file.seek(SeekFrom::Start(size.saturating_sub(4096)))?;
        let mut text=String::new();errors.file.read_to_string(&mut text)?;
        log(crate::environment_analysis::redact(&text));
        if text.trim().is_empty() {return result.and_then(|_|Err(io::Error::other(format!("SSH: {status}"))));}
        return Err(io::Error::other(format!("SSH: {}",text.trim())));
    }
    result
}
fn read_response(reader: impl Read, streaming: bool, notify: &mut impl FnMut(Value)) -> io::Result<Value> {
    let mut result=None;
    let stream=reader.take(LIMIT);
    for item in serde_json::Deserializer::from_reader(stream).into_iter::<Value>() {
        let item=item.map_err(io::Error::other)?;
        if streaming && item.get("_oaclis_progress").is_some() {notify(item["_oaclis_progress"].clone());}
        else if streaming && item.get("_oaclis_error").is_some() {return Err(io::Error::other(item["_oaclis_error"].as_str().unwrap_or("Installazione non riuscita")));}
        else if result.is_some() {return Err(invalid("Risposta multipla inattesa"));}
        else if streaming && item.get("_oaclis_result").is_none() {return Err(invalid("Risposta installazione non valida"));}
        else {result=Some(Ok(if streaming {item["_oaclis_result"].clone()} else {item}));}
    }
    result.unwrap_or_else(||Err(invalid("Risposta dispositivo assente")))
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
                Some("chat-rename")=>{
                    let home=std::env::var_os("HOME").ok_or_else(||invalid("HOME non definita"))?;
                    crate::catalog::rename(Path::new(&home),&codex,request["chat_id"].as_str().ok_or_else(||invalid("Chat mancante"))?,request["name"].as_str().ok_or_else(||invalid("Nome chat mancante"))?)?;
                    Ok(json!({"ok":true}))
                },
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
    fn receiver_streams_installation_progress_and_preserves_regular_responses() {
        let input=b"{\"_oaclis_progress\":{\"event\":{\"phase\":\"transferring\",\"percent\":50}}}\n{\"_oaclis_result\":{\"status\":\"completed\"}}\n";
        let mut events=Vec::new();
        let result=super::read_response(&input[..],true,&mut |event|events.push(event)).unwrap();
        assert_eq!(events[0]["event"]["percent"],50);assert_eq!(result["status"],"completed");
        let regular=super::read_response(&b"{\"runtime\":{},\"devices\":[]}"[..],false,&mut |_|panic!("ordinary response is not progress")).unwrap();
        assert_eq!(regular["devices"],serde_json::json!([]));
        assert!(super::read_response(&b"{} {}"[..],false,&mut |_|{}).is_err());
        assert!(super::read_response(&b"{\"_oaclis_error\":\"failed\"}"[..],true,&mut |_|{}).is_err());
        assert!(super::read_response(&b"{}"[..],true,&mut |_|{}).is_err());
    }
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
