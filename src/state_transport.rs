// state_transport.rs v1.22.389
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
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let id = NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
    if rows.len() == 500 { rows.pop_front(); }
    rows.push_back(json!({"id":id,"time":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis(),"text":message}));
}

pub(crate) fn logs() -> Value {
    json!(*LOG.get_or_init(Default::default).lock().unwrap())
}

pub(crate) fn expire() {
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    if let Some(log)=LOG.get() {
        log.lock().unwrap().retain(|row|row["time"].as_u64().is_some_and(|time|now.saturating_sub(time as u128)<3_600_000));
    }
}

pub(crate) fn chat_name(chat: &str) -> String {
    crate::state_store::cached_label(chat).or_else(||crate::catalog::cached_name(chat)).unwrap_or_else(||"Chat senza nome".into())
}
pub(crate) fn console_message(chat: &str, action: &str, image: bool, result: &io::Result<Value>) -> String {
    let outcome=match result {
        Err(_)=>"operazione non riuscita",
        Ok(value) if value["unchanged"]==true=>"nessuna variazione, testo non ritrasmesso",
        Ok(_) => match action {"read"=>"testo console inviato","open"=>"console aperta","input" if image=>"immagine inviata alla console","input"=>"comando inviato alla console","create"=>"nuova chat creata","models"=>"elenco modelli ricevuto","engines"=>"configurazione AI ricevuta",_=>"operazione completata"},
    };
    format!("{chat}: {outcome}")
}
fn request_description(value: &Value) -> String {
    let action=match value["kind"].as_str().unwrap_or("") {
        "shared-exchange"=>"scambio impostazioni e stato (nessun trasferimento chat)",
        "catalog"=>"lettura elenco chat",
        "chat-proof"=>"confronto chat",
        "snapshot"=>"lettura stato salvato",
        "chat-rename"=>"rinomina chat",
        "prepare-path"=>"preparazione cartella",
        "terminal"=>match value["value"]["action"].as_str().unwrap_or("") {"read"=>"lettura console","input"=>"invio alla console","create"=>"creazione chat","engines"=>"lettura configurazione AI","models"=>"lettura modelli AI",_=>"apertura console"},
        "device-control"=>match value["action"].as_str().unwrap_or("") {"update"=>"aggiornamento Oaclis","restart"=>"riavvio Oaclis","status"=>"controllo versione Oaclis","toggle-awake"|"power-always"|"power-screen-off"|"power-normal"=>"modifica risparmio energia",_=>"operazione sul dispositivo"},
        "environment"=>match value["operation"].as_str().unwrap_or("") {"install"=>"installazione applicazioni","analyze"=>"analisi progetto","paths-status"=>"aggiornamento stato percorsi",_=>"gestione ambiente progetto"},
        "database"=>match value["operation"].as_str().unwrap_or("") {"load"|"links-get"|"preferences-get"|"analysis-list"|"analysis-get"|"inventory-list"|"inventory-get"|"preparation-get"|"status-read"|"transfer-list"=>"lettura stato",_=>"aggiornamento stato"},
        _=>"operazione sul dispositivo",
    };
    match value["chat_id"].as_str().or_else(||value["value"]["chat_id"].as_str()) {
        Some(chat)=>format!("{action} · {}",chat_name(chat)), None=>action.into(),
    }
}
fn log_request(direction: &str, peer: &str, value: &Value) {
    log(format!("{peer} · {} · {}",request_description(value),if direction=="SEND"{"richiesta inviata"}else{"richiesta ricevuta"}));
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
    log_request("SEND", &device.name, value);
    let mut errors=crate::staging::StagedFile::new(&std::env::temp_dir())?;
    use std::os::unix::process::CommandExt;
    let mut child = Command::new("ssh").process_group(0)
        .args(crate::ssh::OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias)
        .arg(if value["kind"]=="environment" {"$HOME/.local/bin/oaclis environment-request"} else {"$HOME/.local/bin/oaclis state-request"})
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(errors.file.try_clone()?).spawn()?;
    let seconds=match (value["kind"].as_str(),value["operation"].as_str()) {
        (Some("environment"),Some("install"))=>7200,
        (Some("environment"),Some("analyze"))=>240,
        _=>60,
    };
    let (done,timer)=crate::ssh::deadline(child.id(),Duration::from_secs(seconds));
    if let Err(error)=child.stdin.take().unwrap().write_all(&serde_json::to_vec(value)?) {let _=child.kill();let _=child.wait();return Err(error);}
    let result=read_response(BufReader::new(child.stdout.take().unwrap()),value["kind"]=="environment" && value["operation"]=="install",notify);
    if result.is_err() {let _=child.kill();}
    let status=child.wait();
    let _=done.send(());
    if timer.join().unwrap_or(true) { return Err(io::Error::new(io::ErrorKind::TimedOut,"Tempo massimo della richiesta remota superato")); }
    let status=status?;
    log(format!("{} · {} · {}",device.name,request_description(value),if status.success() && result.is_ok(){"completato"}else{"non riuscito"}));
    if !status.success() {
        use std::io::{Seek,SeekFrom};
        let size=errors.file.metadata()?.len();errors.file.seek(SeekFrom::Start(size.saturating_sub(4096)))?;
        let mut text=String::new();errors.file.read_to_string(&mut text)?;
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
                let mut description=String::from("richiesta");
                let result = (|| -> io::Result<Value> {
                    let request = receive(&mut stream)?;
                    description=request_description(&request);
                    log_request("RECV", "Questo dispositivo", &request);
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
                Some("shared-exchange")=>{let mut value=crate::shared_settings::exchange(&config,Some(&request["value"]))?;value["runtime"]=runtime.lock().unwrap().status();value["jobs"]=crate::jobs::snapshot();Ok(value)},
                Some("snapshot")=>serde_json::to_value(crate::state_store::snapshot(&config)?.as_ref()).map_err(io::Error::other),
                Some("database")=>crate::state_store::database(&config,request["key"].as_str().unwrap_or(""),request["operation"].as_str().ok_or_else(||invalid("Operazione mancante"))?,request.get("value")),
                _=>Err(invalid("Azione stato non valida")),
            }
                })();
                log(format!("Questo dispositivo · {description}: {}",if result.is_ok(){"completato"}else{"non riuscito"}));
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
    fn human_console_log_distinguishes_delivery_and_no_change_without_payloads() {
        use super::*;
        let unchanged=console_message("Oaclis","read",false,&Ok(json!({"unchanged":true,"text":"secret"})));
        assert!(unchanged.contains("testo non ritrasmesso"));assert!(!unchanged.contains("secret"));
        assert!(console_message("Santi","input",true,&Ok(json!({}))).contains("immagine inviata"));
        assert!(console_message("Santi","input",true,&Err(io::Error::other("private-command"))).contains("non riuscita"));
        assert!(!request_description(&json!({"kind":"shared-exchange"})).contains("shared-exchange"));
        let hidden=request_description(&json!({"kind":"terminal","value":{"action":"read","chat_id":"unresolved-private-id","text":"private-input"}}));
        assert!(!hidden.contains("unresolved-private-id"));assert!(!hidden.contains("private-input"));
    }
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
        let row = rows.as_array().unwrap().iter().find(|r|r["text"].as_str().unwrap().contains("preparazione cartella")).unwrap();
        assert!(!row.to_string().contains("never-log"));
        for _ in 0..510 {super::log("fixture".into());}
        assert!(super::logs().as_array().unwrap().len() <= 500);
        let previous=super::logs().as_array().unwrap().last().unwrap()["id"].as_u64().unwrap();
        {let mut rows=super::LOG.get().unwrap().lock().unwrap();rows.front_mut().unwrap()["time"]=serde_json::json!(0);}
        super::expire();
        assert!(super::logs().as_array().unwrap().iter().all(|row|row["time"]!=0));
        super::log("after-expiry".into());
        assert!(super::logs().as_array().unwrap().last().unwrap()["id"].as_u64().unwrap()>previous);
    }
}
