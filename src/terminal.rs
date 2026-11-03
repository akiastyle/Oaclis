// terminal.rs v1.30.538
//! Persistent tmux sessions; browser disconnects never stop Codex.
use crate::{devices::Device, invalid};
use serde_json::{Value, json};
use std::{env, fs, io::{self, BufRead, BufReader, Read}, os::unix::fs::PermissionsExt, path::{Path, PathBuf}, process::Command, sync::atomic::{AtomicU64, Ordering}, time::Duration};

pub(crate) fn access_flags(access: &str) -> io::Result<&'static str> {
    match access {
        "default" => Ok(""),
        "read-only" => Ok("--sandbox read-only --ask-for-approval on-request"),
        "workspace-write" => Ok("--sandbox workspace-write --ask-for-approval on-request"),
        "full" => Ok("--dangerously-bypass-approvals-and-sandbox"),
        _ => Err(invalid("Autorizzazione console non valida")),
    }
}
pub(crate) fn validate_model(model:&str)->io::Result<()> {
    if model.len()>160||!model.bytes().all(|c|c.is_ascii_alphanumeric()||b"._-:/".contains(&c)){return Err(invalid("Modello non valido"));}Ok(())
}
fn validate(value: &Value) -> io::Result<&str> {
    if let Some(engine)=value.get("engine") {
        if engine!="codex" {return Err(invalid("Chat per questo motore non ancora disponibili"));}
    }
    if let Some(model)=value.get("model"){validate_model(model.as_str().ok_or_else(||invalid("Modello non valido"))?)?;}
    if let Some(access)=value.get("access") { access_flags(access.as_str().ok_or_else(||invalid("Autorizzazione console non valida"))?)?; }
    if let Some(speech)=value.get("speech") { if !speech.is_boolean() {return Err(invalid("Lettura non valida"));} }
    if value.get("speech_cursor").is_some_and(|cursor|!cursor.is_null() && cursor.as_u64().is_none()) {return Err(invalid("Cursore lettura non valido"));}
    if value.get("screen_revision").is_some_and(|v|!v.is_null() && !v.as_str().is_some_and(|s|s.is_empty() || s.len()==64 && s.bytes().all(|b|b.is_ascii_hexdigit()))) {return Err(invalid("Revisione console non valida"));}
    let chat = value["chat_id"].as_str().filter(|s| !s.is_empty() && s.len()<=160 && s.bytes().all(|b|b.is_ascii_alphanumeric() || b"_-".contains(&b)))
        .ok_or_else(||invalid("Chat non valida"))?;
    match value["action"].as_str() {
        Some("open"|"read"|"reset"|"stop"|"engines"|"models") => {},
        Some("create") => {crate::appearance::Appearance{label:value["text"].as_str().unwrap_or("").into(),icon:value["icon"].as_str().unwrap_or("").into(),color:value["color"].as_str().unwrap_or("").into(),..Default::default()}.validate()?;},
        Some("input") => {
            let text = value.get("text").map_or(Some(""),Value::as_str).ok_or_else(||invalid("Messaggio non valido"))?;
            if text.chars().count()>8000 || text.contains('\0') { return Err(invalid("Messaggio non valido")); }
            let key = value.get("key").map_or(Some(""),Value::as_str).ok_or_else(||invalid("Tasto non valido"))?;
            if !matches!(key,""|"Enter"|"Escape"|"Tab"|"Up"|"Down"|"Left"|"Right"|"BSpace"|"C-c"|"1"|"2"|"3") { return Err(invalid("Tasto non valido")); }
        },
        _ => return Err(invalid("Azione non valida")),
    }
    if let Some(image)=value.get("image").filter(|image|!image.is_null()) {
        if value["action"]!="input" || value["key"]!="Enter" {return Err(invalid("Invio immagine non valido"));}
        image_bytes(image.as_str().ok_or_else(||invalid("Immagine non valida"))?)?;
    }
    Ok(chat)
}
pub(crate) fn image_bytes(value: &str) -> io::Result<Vec<u8>> {
    use base64::Engine;
    let data=value.strip_prefix("data:image/jpeg;base64,").filter(|value|value.len()<=699052).ok_or_else(||invalid("Immagine non valida"))?;
    let bytes=base64::engine::general_purpose::STANDARD.decode(data).map_err(|_|invalid("Immagine non valida"))?;
    if bytes.len()>512*1024 || !bytes.starts_with(&[0xff,0xd8,0xff]) || !bytes.ends_with(&[0xff,0xd9]) {return Err(invalid("Immagine non valida"));}
    Ok(bytes)
}
fn save_image(value: &str) -> io::Result<PathBuf> {
    use std::io::Write;
    use std::os::unix::fs::DirBuilderExt;
    use sha2::{Digest,Sha256};
    let bytes=image_bytes(value)?;
    let home=env::var_os("HOME").map(PathBuf::from).ok_or_else(||invalid("HOME non definita"))?;
    let directory=home.join(".local/share/oaclis/attachments");fs::DirBuilder::new().recursive(true).mode(0o700).create(&directory)?;
    let path=directory.join(format!("{}.jpg",Sha256::digest(&bytes).iter().map(|byte|format!("{byte:02x}")).collect::<String>()));
    let mut staged=crate::staging::StagedFile::new(&directory)?;staged.file.write_all(&bytes)?;staged.file.sync_all()?;
    fs::rename(&staged.path,&path)?;
    Ok(path)
}
fn paste(target: &str, text: &str) -> io::Result<()> {
    static INPUT_ID: AtomicU64 = AtomicU64::new(0);
    let buffer=format!("oaclis-{}-{}",std::process::id(),INPUT_ID.fetch_add(1,Ordering::Relaxed));
    crate::ssh_settings::command(Command::new("tmux").args(["load-buffer","-b",&buffer,"-"]),Some(text.as_bytes()),Duration::from_secs(5))?;
    let result=tmux(&["paste-buffer","-p","-r","-d","-b",&buffer,"-t",target]);
    if result.is_err() {let _=tmux(&["delete-buffer","-b",&buffer]);}
    result?;Ok(())
}
fn tmux(args: &[&str]) -> io::Result<Vec<u8>> {
    crate::ssh_settings::command(Command::new("tmux").args(args),None,Duration::from_secs(5))
}
fn executable(path: &Path) -> bool {
    path.metadata().is_ok_and(|m|m.is_file() && m.permissions().mode() & 0o111 != 0)
}
pub(crate) fn codex(home: &Path) -> io::Result<PathBuf> {
    if let Some(path) = env::split_paths(&env::var_os("PATH").unwrap_or_default()).map(|p|p.join("codex")).find(|p|executable(p)) { return Ok(path); }
    let directory = home.join(".nvm/versions/node");
    let mut versions = Vec::new();
    match fs::read_dir(directory) {
        Ok(entries) => for entry in entries { let path=entry?.path().join("bin/codex");if executable(&path) { versions.push(path); } },
        Err(error) if error.kind()==io::ErrorKind::NotFound => {},
        Err(error) => return Err(error),
    }
    versions.into_iter().max_by_key(|p|p.metadata().and_then(|m|m.modified()).ok()).ok_or_else(||invalid("Codex non trovato sul dispositivo"))
}
fn archive(root: &Path, chat: &str) -> io::Result<PathBuf> {
    let mut pending = vec![root.join("sessions")];
    let suffix = format!("{chat}.jsonl");
    while let Some(directory) = pending.pop() {
        let entries = match fs::read_dir(directory) { Ok(entries)=>entries,Err(error) if error.kind()==io::ErrorKind::NotFound=>continue,Err(error)=>return Err(error) };
        for entry in entries {
            let entry=entry?;let kind=entry.file_type()?;
            if kind.is_dir() { pending.push(entry.path()); }
            else if kind.is_file() && entry.file_name().to_string_lossy().ends_with(&suffix) { return Ok(entry.path()); }
        }
    }
    Err(invalid("Chat non presente su questo dispositivo: trasferiscila prima"))
}
fn working_directory(home:&Path,root:&Path,chat:&str)->io::Result<PathBuf> {
    if chat=="new-chat" {return Ok(home.into());}
    let path=archive(root,chat)?;
    let (file,_)=crate::open_regular(&path)?;
    let mut line=String::new();
    BufReader::new(file.take(1024*1024+1)).read_line(&mut line)?;
    if line.len()>1024*1024 { return Err(invalid("Metadati chat troppo grandi")); }
    let meta:Value=serde_json::from_str(&line)?;
    Ok(meta["payload"]["cwd"].as_str().map(Path::new).filter(|p|p.is_dir()).unwrap_or(home).to_path_buf())

}
fn open(home: &Path, root: &Path, chat: &str, session: &str, access: &str, model:&str) -> io::Result<()> {
    if tmux(&["has-session","-t",session]).is_ok() { return Ok(()); }
    let cwd=working_directory(home,root,chat)?;
    let executable=codex(home)?;
    validate_model(model)?;
    let model=if model.is_empty(){String::new()}else{format!("--model {}",crate::ssh::quote(model))};
    let command=format!("{} resume {} --no-alt-screen {} {}",crate::ssh::quote(executable.to_str().ok_or_else(||invalid("Percorso Codex non UTF-8"))?),crate::ssh::quote(chat),access_flags(access)?,model);
    tmux(&["new-session","-d","-s",session,"-c",cwd.to_str().ok_or_else(||invalid("Cartella chat non UTF-8"))?,"-x","120","-y","36",&command])?;
    tmux(&["set-option","-t",session,"@oaclis-access",access])?;
    Ok(())
}
#[derive(Default)]
struct ConsoleCapture {
    screen: Option<(std::time::Instant, String, String)>,
}
impl ConsoleCapture {
    fn read(&mut self, load: impl FnOnce() -> io::Result<String>) -> io::Result<(String, String)> {
        if let Some((at, plain, ansi)) = &self.screen {
            if at.elapsed() < Duration::from_millis(100) { return Ok((plain.clone(), ansi.clone())); }
        }
        let ansi = load()?;
        let plain = sgr().replace_all(&ansi, "").into_owned();
        let (plain, ansi) = bounded_screen(&plain, Some(&ansi));
        let ansi = ansi.unwrap_or_else(||plain.clone());
        self.screen = Some((std::time::Instant::now(), plain.clone(), ansi.clone()));
        Ok((plain, ansi))
    }
}
static SESSIONS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<std::sync::Mutex<ConsoleCapture>>>>> = std::sync::OnceLock::new();
pub(crate) fn expire() {
    if let Some(sessions)=SESSIONS.get() {
        sessions.lock().unwrap().retain(|_,session|std::sync::Arc::strong_count(session)>1 || session.try_lock().map(|capture|capture.screen.as_ref().is_some_and(|(at,_,_)|at.elapsed()<Duration::from_secs(60))).unwrap_or(true));
    }
}
fn session_lock(chat: &str) -> io::Result<std::sync::Arc<std::sync::Mutex<ConsoleCapture>>> {
    use std::sync::Arc;
    let mut sessions = SESSIONS.get_or_init(Default::default).lock().map_err(|_|io::Error::other("Console non disponibile"))?;
    sessions.retain(|_, session| Arc::strong_count(session) > 1 || session.try_lock().map(|s|s.screen.as_ref().is_some_and(|(at,_,_)|at.elapsed()<Duration::from_secs(60))).unwrap_or(true));
    Ok(sessions.entry(chat.into()).or_default().clone())
}
pub(crate) fn local(value: &Value) -> io::Result<Value> {
    let chat=validate(value)?;
    if value["action"]=="reset" || value["action"]=="stop" {
        let lock=session_lock(chat)?;
        let mut capture=lock.lock().map_err(|_|io::Error::other("Console non disponibile"))?;
        if value["action"]=="stop" {
            let session=format!("=oaclis-{chat}");
            if tmux(&["has-session","-t",&session]).is_ok() {tmux(&["kill-session","-t",&session])?;}
        }
        capture.screen=None;
        return Ok(json!({"ok":true}));
    }
    if matches!(value["action"].as_str(),Some("engines"|"models")) {
        let home=env::var_os("HOME").map(PathBuf::from).ok_or_else(||invalid("HOME non definita"))?;
        return if value["action"]=="engines" {Ok(json!({"engines":crate::engines::settings(&home)?}))} else {crate::llama::models(&home)};
    }
    if let Some(home)=env::var_os("HOME") {
        if crate::chat_deletion::blocked(&PathBuf::from(home).join(".config/oaclis"),chat)? { return Err(invalid("Chat eliminata definitivamente")); }
    }
    if value["action"]=="create" {
        let home=env::var_os("HOME").map(PathBuf::from).ok_or_else(||invalid("HOME non definita"))?;
        let root=env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(||home.join(".codex"));
        let cwd=value["cwd"].as_str().filter(|v|!v.is_empty()).map(PathBuf::from).map(Ok).unwrap_or_else(||working_directory(&home,&root,chat))?;
        let name=value["text"].as_str().unwrap_or("").trim();
        let default_model=crate::engines::creation(&home,value)?;
        let entry=crate::catalog::create(&home,&root,&cwd,name,value["model"].as_str().filter(|v|!v.is_empty()).unwrap_or(&default_model))?;let id=entry["id"].as_str().unwrap();
        let config=home.join(".config/oaclis");
        let appearance=crate::appearance::Appearance{label:name.into(),icon:value["icon"].as_str().unwrap_or("").into(),color:value["color"].as_str().unwrap_or("").into(),..Default::default()};
        let preferences=crate::state_store::database(&config,"","preferences-get",None)?;
        let access=value["access"].as_str().or_else(||preferences["preferences"][chat]["console_access"].as_str()).unwrap_or("default");
        crate::state_store::database(&config,id,"chat-settings",Some(&json!({"value":appearance,"console_access":access,"console_model":entry["model"]})))?;
        return Ok(json!({"new_chat":{"id":id,"name":name,"icon":value["icon"].as_str().unwrap_or(""),"color":value["color"].as_str().unwrap_or(""),"console_access":access,"model":entry["model"]}}));
    }
    let session_lock = session_lock(chat)?;
    let mut capture = session_lock.lock().map_err(|_|io::Error::other("Console non disponibile"))?;
    if value["action"] != "read" { capture.screen = None; }
    let session=format!("oaclis-{chat}");let target=format!("{session}:0.0");
    match value["action"].as_str().unwrap() {
        "open" => {
            if !env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|p|executable(&p.join("tmux"))) { return Err(invalid("Serve tmux sul dispositivo per aprire Codex")); }
            let home=env::var_os("HOME").map(PathBuf::from).ok_or_else(||invalid("HOME non definita"))?;
            let root=env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(||home.join(".codex"));
            let preferences=crate::state_store::database(&home.join(".config/oaclis"),"","preferences-get",None)?;
            let access=value["access"].as_str().or_else(||preferences["preferences"][chat]["console_access"].as_str()).unwrap_or("default");
            access_flags(access)?;
            open(&home,&root,chat,&session,access,preferences["preferences"][chat]["console_model"].as_str().unwrap_or(""))?;
        },
        "input" => {
            let text=value.get("text").and_then(Value::as_str).unwrap_or("");let key=value.get("key").and_then(Value::as_str).unwrap_or("");
            if let Some(image)=value["image"].as_str() {
                if key!="Enter" {return Err(invalid("Invio immagine non valido"));}
                let path=save_image(image)?;
                paste(&target,path.to_str().ok_or_else(||invalid("Percorso immagine non valido"))?)?;
                std::thread::sleep(Duration::from_millis(150));
            }
            if !text.is_empty() {
                paste(&target,text)?;
                if !key.is_empty() { std::thread::sleep(Duration::from_millis(100)); }
            }
            if !key.is_empty() { tmux(&["send-keys","-t",&target,key])?; }
        },
        _ => {},
    }
    let (plain, ansi)=capture.read(|| {
        let screen=tmux(&["capture-pane","-p","-e","-t",&target,"-S","-200"]).map_err(|_|invalid("Sessione terminata: riapri la chat"))?;
        String::from_utf8(screen).map_err(io::Error::other)
    })?;
    drop(capture);
    let access=if value["action"]=="open" {
        tmux(&["show-options","-v","-t",&session,"@oaclis-access"]).ok().and_then(|v|String::from_utf8(v).ok()).map(|v|v.trim().to_owned()).filter(|v|access_flags(v).is_ok()).unwrap_or_else(||"existing".into())
    } else { String::new() };
    let mut result=screen_response(&plain,&ansi,&value["screen_revision"],value["action"]=="read");
    result["active_access"]=json!(access);result["jobs"]=crate::jobs::for_chat(chat);
    if value["speech"]==true {result["speech"]=crate::assistant_stream::snapshot(chat,value["speech_cursor"].as_u64());}
    Ok(result)
}
fn screen_response(plain: &str, ansi: &str, known: &Value, read: bool) -> Value {
    use sha2::{Digest,Sha256};
    let revision=Sha256::digest(ansi.as_bytes()).iter().map(|byte|format!("{byte:02x}")).collect::<String>();
    if read && known.as_str()==Some(revision.as_str()) {json!({"screen_revision":revision,"unchanged":true})}
    else {json!({"screen":plain,"screen_ansi":ansi,"screen_revision":revision})}
}
fn sgr() -> &'static regex::Regex {
    static SGR: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    SGR.get_or_init(||regex::Regex::new(r"\x1b\[[0-9;:]*m").unwrap())
}
fn bounded_screen(plain: &str, ansi: Option<&str>) -> (String, Option<String>) {
    let start=plain.char_indices().rev().nth(13999).map_or(0,|(index,_)|index);
    let tail=plain[start..].to_owned();
    let mut styling=None;
    if let Some(ansi)=ansi {
        let skip=plain.len()-tail.len();
        let (mut consumed,mut cursor,mut prefix)=(0,0,String::new());
        for code in sgr().find_iter(ansi) {
            let text=code.start()-cursor;
            if consumed+text>=skip {break;}
            consumed+=text;
            if matches!(code.as_str(),"\x1b[m"|"\x1b[0m") {prefix.clear();}
            prefix.push_str(code.as_str());
            cursor=code.end();
        }
        if let Some(rest)=ansi.get(cursor+skip.saturating_sub(consumed)..) {
            prefix.push_str(rest);
            // Keep the existing relay request bounded; oversized styling falls back to plain text.
            if prefix.len()<=24000 {styling=Some(prefix);}
        }
    }
    (tail, styling)
}
pub(crate) fn mobile_screen(value: &Value) -> Value {
    if value["unchanged"]==true {return json!({"unchanged":true,"screen_revision":value["screen_revision"]});}
    let (plain, ansi)=bounded_screen(value["screen"].as_str().unwrap_or(""),value["screen_ansi"].as_str());
    let mut result=json!({"screen":plain});
    if value["screen_revision"].is_string() {result["screen_revision"]=value["screen_revision"].clone();}
    if let Some(ansi)=ansi {result["screen_ansi"]=json!(ansi);}
    result
}
pub fn request(device: Option<&Device>, value: &Value) -> io::Result<Value> {
    validate(value)?;
    if let Some(device)=device {
        crate::state_transport::remote(device,&json!({"kind":"terminal","value":value}))
    } else { local(value) }
}

#[cfg(test)]
mod tests {
    #[test]
    fn stop_ends_only_the_selected_console_session_and_clears_its_capture() {
        let chat=format!("stop-fixture-{}",std::process::id());let session=format!("oaclis-{chat}");
        tmux(&["new-session","-d","-s",&session,"sleep 30"]).unwrap();
        let lock=session_lock(&chat).unwrap();lock.lock().unwrap().read(||Ok("Old".into())).unwrap();
        let result=local(&json!({"chat_id":chat,"action":"stop"}));
        let exists=tmux(&["has-session","-t",&format!("={session}")]).is_ok();
        if exists {let _=tmux(&["kill-session","-t",&format!("={session}")]);}
        assert_eq!(result.unwrap(),json!({"ok":true}));assert!(!exists);assert!(lock.lock().unwrap().screen.is_none());
    }
    #[test]
    fn reset_discards_only_the_capture_without_starting_or_stopping_codex() {
        let lock=session_lock("reset-cache-test").unwrap();
        lock.lock().unwrap().read(||Ok("Old screen".into())).unwrap();
        assert_eq!(local(&json!({"chat_id":"reset-cache-test","action":"reset"})).unwrap(),json!({"ok":true}));
        assert!(lock.lock().unwrap().screen.is_none());
        assert_eq!(lock.lock().unwrap().read(||Ok("Fresh screen".into())).unwrap().0,"Fresh screen");
    }
    #[test]
    fn unchanged_screen_omits_text_but_input_and_style_changes_send_it() {
        let first=screen_response("Hello","\x1b[31mHello",&Value::Null,true);
        let unchanged=screen_response("Hello","\x1b[31mHello",&first["screen_revision"],true);
        assert_eq!(unchanged["unchanged"],true);assert!(unchanged.get("screen").is_none());
        assert!(mobile_screen(&unchanged).get("screen").is_none());
        assert!(screen_response("Hello","\x1b[32mHello",&first["screen_revision"],true).get("screen").is_some());
        assert!(screen_response("Hello","\x1b[31mHello",&first["screen_revision"],false).get("screen").is_some());
    }
    #[test]
    fn capture_reuses_recent_screen_and_invalidation_reads_again() {
        let mut capture = super::ConsoleCapture::default();
        assert_eq!(capture.read(||Ok("\x1b[31mfirst".into())).unwrap().0, "first");
        assert_eq!(capture.read(||panic!("duplicate capture")).unwrap().0, "first");
        capture.screen = None;
        assert_eq!(capture.read(||Ok("second".into())).unwrap().0, "second");
        capture.screen.as_mut().unwrap().0 -= std::time::Duration::from_secs(1);
        assert_eq!(capture.read(||Ok("third".into())).unwrap().0, "third");
    }

    #[test]
    fn shared_capture_bounds_unicode_and_excessive_styling_before_caching() {
        let mut capture=super::ConsoleCapture::default();
        let input=format!("{}{}", "\x1b[31m".repeat(6000), "尾🙂".repeat(9000));
        let (plain,ansi)=capture.read(||Ok(input)).unwrap();
        assert_eq!(plain.chars().count(),14000);
        assert!(ansi.len()<=24000 || ansi==plain);
        assert_eq!(super::sgr().replace_all(&ansi,""),plain);
        assert_eq!(capture.read(||panic!("capture should be cached")).unwrap(),(plain,ansi));
    }

    use super::*;
    #[test]
    fn inputs_share_a_lock_only_for_the_same_chat() {
        let first = session_lock("same-chat").unwrap();
        let second = session_lock("same-chat").unwrap();
        let other = session_lock("other-chat").unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        let held = first.lock().unwrap();
        assert!(second.try_lock().is_err());
        assert!(other.try_lock().is_ok());
        drop(held);
        assert!(second.try_lock().is_ok());
    }
    #[test]
    fn console_access_only_accepts_explicit_presets() {
        assert_eq!(access_flags("default").unwrap(),"");
        assert_eq!(access_flags("full").unwrap(),"--dangerously-bypass-approvals-and-sandbox");
        for access in ["read-only","workspace-write"] {
            assert!(access_flags(access).unwrap().contains("--ask-for-approval on-request"));
            assert!(validate(&json!({"chat_id":"chat-1","action":"open","access":access})).is_ok());
        }
        for access in [json!(true),json!("full; touch /tmp/bad"),json!("danger-full-access"),Value::Null] {
            assert!(validate(&json!({"chat_id":"chat-1","action":"open","access":access})).is_err());
        }
    }
    #[test]
    fn step4_terminal_rejects_invalid_actions_keys_and_ids_before_commands() {
        for value in [json!({"action":"open","chat_id":"x;touch bad"}),json!({"action":"read","chat_id":"../chat"}),json!({"action":"input","chat_id":"chat-1","key":"run-shell"}),json!({"action":"input","chat_id":"chat-1","text":true}),json!({"action":"input","chat_id":"chat-1","text":"nul\0"}),json!({"action":"input","chat_id":"chat-1","text":"x".repeat(8001)}),json!({"action":"delete","chat_id":"chat-1"})] { assert!(request(None,&value).is_err()); }
        assert!(validate(&json!({"action":"input","chat_id":"chat-1","text":"$(touch bad)","key":"Enter"})).is_ok());
        for key in ["1","2","3"] {assert!(validate(&json!({"action":"input","chat_id":"chat-1","key":key})).is_ok());}
        assert!(validate(&json!({"action":"input","chat_id":"chat-1","key":"4"})).is_err());
    }
    #[test]
    fn mobile_color_tail_keeps_style_and_unicode_without_splitting_escape_codes() {
        let ansi=format!("\x1b[1;35m{}\x1b[38;2;12;34;56m尾🙂\x1b[0m", "x".repeat(15000));
        let plain=sgr().replace_all(&ansi,"").into_owned();
        let screen=mobile_screen(&json!({"screen":plain,"screen_ansi":ansi}));
        assert_eq!(screen["screen"].as_str().unwrap().chars().count(),14000);
        let styled=screen["screen_ansi"].as_str().unwrap();
        assert!(styled.starts_with("\x1b[1;35m"));
        assert!(styled.ends_with("尾🙂\x1b[0m"));
        assert_eq!(sgr().replace_all(styled,""),screen["screen"].as_str().unwrap());
        let excessive=format!("{}Text", "\x1b[31m".repeat(6000));
        let fallback=mobile_screen(&json!({"screen":"Text","screen_ansi":excessive}));
        assert_eq!(fallback["screen"],"Text");
        assert!(fallback.get("screen_ansi").is_none());
        assert_eq!(mobile_screen(&json!({"screen":"legacy"}))["screen"],"legacy");
    }

}
