// mobile.rs v1.18.256
//! Explicit mobile invitations. Relay credentials never enter browser responses or logs.
use crate::invalid;
use qrcode::{QrCode, render::svg};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::{self, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{Arc, Mutex}, thread, time::Duration};

const ENDPOINT: &str = "https://www.krprojects.it/oaclis/auth";
const ACCESS_URL: &str = "https://www.krprojects.it/oaclis/";
const INVITE_PREFIX: &str = "https://www.krprojects.it/oaclis/auth#pair=";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    admin_key: String,
}

fn invitation_qr(url: &str) -> io::Result<String> {
    let code = url.strip_prefix(INVITE_PREFIX).ok_or_else(|| invalid("Invito mobile non valido"))?;
    if code.len() != 64 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Invito mobile non valido"));
    }
    Ok(QrCode::new(url).map_err(io::Error::other)?
        .render::<svg::Color>().min_dimensions(288, 288).build())
}

fn request(config: &Path, payload: Value) -> io::Result<Value> {
    let settings: Settings = serde_json::from_reader(crate::open_regular(&config.join("mobile-relay.json"))?.0)
        .map_err(|_| invalid("Collegamento mobile da configurare"))?;
    if settings.admin_key.len() != 64 || !settings.admin_key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Collegamento mobile da configurare"));
    }
    if serde_json::to_vec(&payload)?.len() > 131072 { return Err(invalid("Selezione troppo ampia")); }
    let action = payload["action"].as_str().unwrap_or("");
    crate::state_transport::log(format!("SEND mobile-relay {action}"));
    // Feed the credential over stdin: it must never appear in process arguments.
    let data = serde_json::to_string(&serde_json::to_string(&payload)?)?;
    let input = format!("url = \"{ENDPOINT}\"\nheader = \"Origin: https://www.krprojects.it\"\nheader = \"Content-Type: application/json\"\nheader = \"X-Admin-Key: {}\"\ndata = {data}\n", settings.admin_key);
    let mut child = Command::new("curl")
        .args(["--silent", "--fail", "--proto", "=https", "--connect-timeout", "3", "--max-time", "10", "--max-filesize", "131072", "--config", "-"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let sent = child.stdin.take().unwrap().write_all(input.as_bytes());
    if sent.is_err() { let _ = child.kill(); let _ = child.wait(); return Err(io::Error::other("Invito non disponibile. Riprova.")); }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        crate::state_transport::log(format!("RECV mobile-relay {action} failed {}", output.status));
        return Err(io::Error::other("Invito non disponibile. Riprova."));
    }
    let reply: Value = serde_json::from_slice(&output.stdout).map_err(|_| io::Error::other("Invito non disponibile. Riprova."))?;
    crate::state_transport::log(format!("RECV mobile-relay {action} OK"));
    Ok(reply)
}

pub(crate) fn access_qr() -> io::Result<Value> {
    let svg = QrCode::new(ACCESS_URL).map_err(io::Error::other)?.render::<svg::Color>().min_dimensions(288,288).build();
    Ok(json!({"qr_svg":svg}))
}

pub(crate) fn invite(config: &Path) -> io::Result<Value> {
    let reply = request(config, json!({"action":"invite"}))?;
    let qr = invitation_qr(reply["url"].as_str().unwrap_or(""))?;
    Ok(json!({"qr_svg":qr,"expires_in":600}))
}


fn listed_devices(reply: &Value, status: &str) -> io::Result<Value> {
    let devices = reply["devices"].as_array().ok_or_else(|| invalid("Richieste mobile non disponibili"))?;
    let mut pending = Vec::new();
    for device in devices.iter().filter(|d| d["status"] == status) {
        let id = device["id"].as_str().unwrap_or("");
        let name = device["name"].as_str().unwrap_or("");
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) || name.is_empty() || name.chars().count() > 60 || name.chars().any(char::is_control) {
            return Err(invalid("Richiesta mobile non valida"));
        }
        pending.push(if status == "approved" { json!({"id":format!("mobile-{id}"),"name":name,"kind":"mobile","active":device["active"] == true}) } else { json!({"id":id,"name":name}) });
    }
    Ok(json!({"devices":pending}))
}

pub(crate) fn pending(config: &Path) -> io::Result<Value> {
    listed_devices(&request(config, json!({"action":"admin-list"}))?, "pending")
}

pub(crate) fn approve(config: &Path, value: &Value, registry: &Registry) -> io::Result<Value> {
    let _exchange = registry.exchange.lock().unwrap();
    let id = value["id"].as_str().unwrap_or("");
    let accept = value["accept"].as_bool().ok_or_else(|| invalid("Conferma mobile richiesta"))?;
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(invalid("Telefono non valido")); }
    let reply = request(config, json!({"action":if accept {"approve"} else {"revoke"},"id":id}))?;
    if reply["ok"] != true { return Err(io::Error::other("Conferma non riuscita. Riprova.")); }
    if accept {
        if let Ok(reply) = request(config, json!({"action":"admin-list"})).and_then(|r| listed_devices(&r, "approved")) {
            *registry.devices.lock().unwrap() = reply["devices"].as_array().unwrap().clone();
        }
    } else {
        registry.devices.lock().unwrap().retain(|d| d["id"] != format!("mobile-{id}"));
    }
    Ok(json!({"ok":true,"devices":registry.snapshot()}))
}

pub(crate) struct Registry {
    devices: Mutex<Vec<Value>>,
    exchange: Mutex<()>,
}
impl Registry {
    pub(crate) fn snapshot(&self) -> Vec<Value> { self.devices.lock().unwrap().clone() }
}

pub(crate) fn start(config: PathBuf, enabled: bool) -> Arc<Registry> {
    let registry = Arc::new(Registry { devices: Mutex::new(Vec::new()), exchange: Mutex::new(()) });
    if enabled && config.join("mobile-relay.json").is_file() {
        let worker = registry.clone();
        thread::spawn(move || loop {
            {
                let _exchange = worker.exchange.lock().unwrap();
                match request(&config, json!({"action":"admin-list"})).and_then(|r| listed_devices(&r, "approved")) {
                    Ok(reply) => *worker.devices.lock().unwrap() = reply["devices"].as_array().unwrap().clone(),
                    Err(_) => { for device in worker.devices.lock().unwrap().iter_mut() { device["active"] = json!(false); } }
                }
            }
            thread::sleep(Duration::from_secs(10));
        });
    }
    registry
}

pub(crate) fn remove(config: &Path, value: &Value, registry: &Registry) -> io::Result<Value> {
    let id = value["id"].as_str().and_then(|s| s.strip_prefix("mobile-")).ok_or_else(|| invalid("Telefono non valido"))?;
    approve(config, &json!({"id":id,"accept":false}), registry)
}

fn chat_choices(config: &Path, codex: &Path, existing: &Value) -> io::Result<std::collections::BTreeMap<String, Value>> {
    let state = crate::state_store::snapshot(config)?;
    let mut choices = std::collections::BTreeMap::new();
    let mut entries = existing.as_array().cloned().unwrap_or_default();
    entries.extend(state.links.values().filter_map(|link| link.get("entry").cloned()));
    entries.extend(crate::catalog::local_at(codex, config)?);
    for entry in entries {
        let id = entry["id"].as_str().unwrap_or("");
        if !crate::devices::identifier(id) || id.len() > 160 { continue; }
        let label = state.appearance.get(&format!("chat:{id}"))
            .and_then(|r| r.value["label"].as_str()).filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| entry["name"].as_str().unwrap_or("Senza titolo"));
        choices.insert(id.into(), json!({"id":id,"name":label.chars().filter(|c| !c.is_control()).take(320).collect::<String>()}));
    }
    Ok(choices)
}

pub(crate) fn chat_access(config: &Path, codex: &Path, value: &Value, registry: &Registry) -> io::Result<Value> {
    let id = value["id"].as_str().and_then(|s| s.strip_prefix("mobile-")).ok_or_else(|| invalid("Telefono non valido"))?;
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(invalid("Telefono non valido")); }
    let _exchange = registry.exchange.lock().unwrap();
    let saved = request(config, json!({"action":"chat-access","id":id}))?;
    let choices = chat_choices(config, codex, &saved["chats"])?;
    if let Some(selected) = value.get("chats") {
        let selected = selected.as_array().filter(|a| a.len() <= 500).ok_or_else(|| invalid("Selezione chat non valida"))?;
        let mut ids = std::collections::BTreeSet::new();
        let mut chats = Vec::new();
        for item in selected {
            let chat_id = item.as_str().ok_or_else(|| invalid("Chat non valida"))?;
            let chat = choices.get(chat_id).ok_or_else(|| invalid("Chat non disponibile"))?;
            if !ids.insert(chat_id) { return Err(invalid("Chat duplicata")); }
            chats.push(chat.clone());
        }
        let reply = request(config, json!({"action":"set-chat-access","id":id,"chats":chats}))?;
        if reply["ok"] != true { return Err(io::Error::other("Salvataggio non riuscito. Riprova.")); }
        return Ok(json!({"ok":true}));
    }
    Ok(json!({"chats":choices.into_values().collect::<Vec<_>>(),"selected":saved["chats"].as_array().into_iter().flatten().filter_map(|c|c["id"].as_str()).collect::<Vec<_>>()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_filters_public_fields_and_rejects_invalid_identity() {
        let input = json!({"devices":[{"id":"a".repeat(64),"name":"Telefono","status":"pending","key":"private"},{"id":"b".repeat(64),"name":"Altro","status":"approved"}]});
        assert_eq!(listed_devices(&input, "pending").unwrap(), json!({"devices":[{"id":"a".repeat(64),"name":"Telefono"}]}));
        assert!(listed_devices(&json!({"devices":[{"id":"bad","name":"Telefono","status":"pending"}]}), "pending").is_err());
        assert!(listed_devices(&json!({}), "pending").is_err());
    }
    #[test]
    fn approved_roster_contains_only_public_mobile_fields() {
        let input = json!({"devices":[{"id":"c".repeat(64),"name":"Telefono","status":"approved","active":true,"key":"secret"},{"id":"d".repeat(64),"name":"Revocato","status":"revoked"}]});
        assert_eq!(listed_devices(&input,"approved").unwrap(),json!({"devices":[{"id":format!("mobile-{}","c".repeat(64)),"name":"Telefono","kind":"mobile","active":true}]}));
    }
    #[test]
    fn choices_reuse_saved_chats_links_and_local_titles_without_paths() {
        let root = std::env::temp_dir().join(format!("oaclis-mobile-choices-{}", std::process::id()));
        std::fs::create_dir_all(root.join("sessions")).unwrap();
        std::fs::write(root.join("pairing_ed25519.pub"), "ssh-ed25519 mobile-fixture").unwrap();
        let id = "019dc528-3e98-7943-bc6b-fb1a2738d7db";
        std::fs::write(root.join("sessions").join(format!("rollout-{id}.jsonl")), "").unwrap();
        std::fs::write(root.join("session_index.jsonl"), json!({"id":id,"thread_name":"Originale"}).to_string()).unwrap();
        crate::state_store::change(&root, |state| {
            state.appearance.entry(format!("chat:{id}")).or_default().value = json!({"label":"Rinominata"});
            state.links.insert("remote".into(),json!({"entry":{"id":"remote-chat","name":"Remota"}}));
            Ok(())
        }).unwrap();
        let choices = chat_choices(&root, &root, &json!([{"id":"saved-chat","name":"Salvata"}])).unwrap();
        assert_eq!(choices[id]["name"],"Rinominata");
        assert_eq!(choices["remote-chat"]["name"],"Remota");
        assert_eq!(choices["saved-chat"]["name"],"Salvata");
        assert!(choices.values().all(|v|v.as_object().unwrap().len()==2));
        std::fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn qr_accepts_only_the_fixed_relay_and_random_code() {
        let url = format!("{INVITE_PREFIX}{}", "a".repeat(64));
        let qr = invitation_qr(&url).unwrap();
        assert!(qr.contains("<svg"));
        assert!(!qr.contains(&url));
        let access = access_qr().unwrap();
        assert!(access["qr_svg"].as_str().unwrap().contains("<svg"));
        assert!(!access["qr_svg"].as_str().unwrap().contains("#pair="));
        for invalid in ["https://example.com/#pair=abc".to_string(), format!("{INVITE_PREFIX}short"), format!("{INVITE_PREFIX}{}", "g".repeat(64)), format!("{url}&admin=anything")] {
            assert!(invitation_qr(&invalid).is_err());
        }
    }
}
