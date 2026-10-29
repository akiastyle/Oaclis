// terminal.rs v1.14.245
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
fn validate(value: &Value) -> io::Result<&str> {
    if let Some(access)=value.get("access") { access_flags(access.as_str().ok_or_else(||invalid("Autorizzazione console non valida"))?)?; }
    let chat = value["chat_id"].as_str().filter(|s| !s.is_empty() && s.len()<=160 && s.bytes().all(|b|b.is_ascii_alphanumeric() || b"_-".contains(&b)))
        .ok_or_else(||invalid("Chat non valida"))?;
    match value["action"].as_str() {
        Some("open"|"read") => {},
        Some("input") => {
            let text = value.get("text").map_or(Some(""),Value::as_str).ok_or_else(||invalid("Messaggio non valido"))?;
            if text.chars().count()>8000 || text.contains('\0') { return Err(invalid("Messaggio non valido")); }
            let key = value.get("key").map_or(Some(""),Value::as_str).ok_or_else(||invalid("Tasto non valido"))?;
            if !matches!(key,""|"Enter"|"Escape"|"Tab"|"Up"|"Down"|"Left"|"Right"|"BSpace"|"C-c") { return Err(invalid("Tasto non valido")); }
        },
        _ => return Err(invalid("Azione non valida")),
    }
    Ok(chat)
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
fn open(home: &Path, root: &Path, chat: &str, session: &str, access: &str) -> io::Result<()> {
    if tmux(&["has-session","-t",session]).is_ok() { return Ok(()); }
    let path=archive(root,chat)?;
    let executable=codex(home)?;
    let (file,_)=crate::open_regular(&path)?;
    let mut line=String::new();
    BufReader::new(file.take(1024*1024+1)).read_line(&mut line)?;
    if line.len()>1024*1024 { return Err(invalid("Metadati chat troppo grandi")); }
    let meta:Value=serde_json::from_str(&line)?;
    let cwd=meta["payload"]["cwd"].as_str().map(Path::new).filter(|p|p.is_dir()).unwrap_or(home);
    let command=format!("{} resume {} --no-alt-screen {}",crate::ssh::quote(executable.to_str().ok_or_else(||invalid("Percorso Codex non UTF-8"))?),crate::ssh::quote(chat),access_flags(access)?);
    tmux(&["new-session","-d","-s",session,"-c",cwd.to_str().ok_or_else(||invalid("Cartella chat non UTF-8"))?,"-x","120","-y","36",&command])?;
    tmux(&["set-option","-t",session,"@oaclis-access",access])?;
    Ok(())
}
pub(crate) fn local(value: &Value) -> io::Result<Value> {
    let chat=validate(value)?;
    if let Some(home)=env::var_os("HOME") {
        if crate::chat_deletion::blocked(&PathBuf::from(home).join(".config/oaclis"),chat)? { return Err(invalid("Chat eliminata definitivamente")); }
    }
    let session=format!("oaclis-{chat}");let target=format!("{session}:0.0");
    match value["action"].as_str().unwrap() {
        "open" => {
            if !env::split_paths(&env::var_os("PATH").unwrap_or_default()).any(|p|executable(&p.join("tmux"))) { return Err(invalid("Serve tmux sul dispositivo per aprire Codex")); }
            let home=env::var_os("HOME").map(PathBuf::from).ok_or_else(||invalid("HOME non definita"))?;
            let root=env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or_else(||home.join(".codex"));
            let preferences=crate::state_store::database(&home.join(".config/oaclis"),"","preferences-get",None)?;
            let access=value["access"].as_str().or_else(||preferences["preferences"][chat]["console_access"].as_str()).unwrap_or("default");
            access_flags(access)?;
            open(&home,&root,chat,&session,access)?;
        },
        "input" => {
            let text=value.get("text").and_then(Value::as_str).unwrap_or("");let key=value.get("key").and_then(Value::as_str).unwrap_or("");
            if !text.is_empty() {
                static INPUT_ID: AtomicU64 = AtomicU64::new(0);
                let buffer=format!("oaclis-{}-{}",std::process::id(),INPUT_ID.fetch_add(1,Ordering::Relaxed));
                crate::ssh_settings::command(Command::new("tmux").args(["load-buffer","-b",&buffer,"-"]),Some(text.as_bytes()),Duration::from_secs(5))?;
                let pasted=tmux(&["paste-buffer","-p","-r","-d","-b",&buffer,"-t",&target]);
                if pasted.is_err() { let _=tmux(&["delete-buffer","-b",&buffer]); }
                pasted?;
                if !key.is_empty() { std::thread::sleep(Duration::from_millis(100)); }
            }
            if !key.is_empty() { tmux(&["send-keys","-t",&target,key])?; }
        },
        _ => {},
    }
    let screen=tmux(&["capture-pane","-p","-e","-t",&target,"-S","-200"]).map_err(|_|invalid("Sessione terminata: riapri la chat"))?;
    let ansi=String::from_utf8(screen).map_err(io::Error::other)?;
    let access=if value["action"]=="open" {
        tmux(&["show-options","-v","-t",&session,"@oaclis-access"]).ok().and_then(|v|String::from_utf8(v).ok()).map(|v|v.trim().to_owned()).filter(|v|access_flags(v).is_ok()).unwrap_or_else(||"existing".into())
    } else { String::new() };
    Ok(json!({"screen":sgr().replace_all(&ansi,""),"screen_ansi":ansi,"active_access":access}))
}
fn sgr() -> &'static regex::Regex {
    static SGR: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    SGR.get_or_init(||regex::Regex::new(r"\x1b\[[0-9;:]*m").unwrap())
}
pub(crate) fn mobile_screen(value: &Value) -> Value {
    let plain=value["screen"].as_str().unwrap_or("");
    let tail=plain.chars().rev().take(14000).collect::<String>().chars().rev().collect::<String>();
    let mut result=json!({"screen":tail});
    if let Some(ansi)=value["screen_ansi"].as_str() {
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
            if prefix.len()<=24000 {result["screen_ansi"]=json!(prefix);}
        }
    }
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
    use super::*;
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
