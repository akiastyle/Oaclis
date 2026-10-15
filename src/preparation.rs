// preparation.rs v1.5.118
//! Persistent source sizes, collected once without SSH, hashing or transfers.
use crate::{environment::{self, Manifest}, invalid, rules};
use serde_json::{Value, json};
use std::{io, path::Path, time::{SystemTime, UNIX_EPOCH}};

pub(crate) fn capture(config: &Path, chat_id: &str, chat_path: &Path, manifest: &Manifest) -> io::Result<Value> {
    let key = environment::key("master", chat_id)?;
    let saved = environment::database(config, &key, "preparation-get", None)?;
    if saved["status"] != "missing" { return Ok(saved); }
    manifest.validate()?;
    let (_, chat) = crate::open_regular(chat_path)?;
    let mut paths = Vec::new();
    let mut total = Some(0u64);
    let mut files = 0u64;
    for root in manifest.paths.iter().collect::<std::collections::BTreeSet<_>>() {
        if manifest.paths.iter().any(|other| other != root && root.starts_with(other)) { continue; }
        match rules::selected_size(root) {
            Ok((bytes, count)) => {
                total = total.and_then(|value|value.checked_add(bytes));
                files += count;
                paths.push(json!({"path":root,"bytes":bytes,"files":count}));
            }
            Err(error) => { total = None; paths.push(json!({"path":root,"bytes":null,"error":error.to_string()})); }
        }
    }
    let value = json!({"status":if total.is_some(){"ready"}else{"partial"},
        "measured_at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        "chat_bytes":chat.len(),"chat_modified_ns":chat.modified()?.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos().to_string(),
        "paths":paths,"paths_bytes":total,"path_files":files});
    environment::database(config, &key, "preparation-save", Some(&value))?;
    Ok(value)
}

/// Local measurements are independent of AI detection and of transfer execution.
pub(crate) fn refresh(config: &Path, chat_id: &str, chat_path: &Path, manifest: &Manifest) -> io::Result<Value> {
    manifest.validate()?;
    let key=environment::key("master",chat_id)?;
    let saved=environment::database(config,&key,"preparation-get",None)?;
    let (_,chat)=crate::open_regular(chat_path)?;
    let mut paths=Vec::new();let mut total=Some(0u64);let mut files=0u64;
    for root in manifest.paths.iter().collect::<std::collections::BTreeSet<_>>() {
        if manifest.paths.iter().any(|other|other!=root && root.starts_with(other)) {continue;}
        let previous=saved["paths"].as_array().and_then(|items|items.iter().find(|v|v["path"]==json!(root))).unwrap_or(&Value::Null);
        match rules::selected_snapshot(root,previous) {
            Ok(value)=>{total=total.and_then(|n|n.checked_add(value["bytes"].as_u64().unwrap()));files+=value["files"].as_u64().unwrap();paths.push(value);},
            Err(error)=>{total=None;paths.push(json!({"path":root,"error":error.to_string()}));}
        }
    }
    let value=json!({"status":if total.is_some(){"ready"}else{"partial"},"measured_at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
        "chat_bytes":chat.len(),"chat_modified_ns":chat.modified()?.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos().to_string(),
        "paths":paths,"paths_bytes":total,"path_files":files});
    environment::database(config,&key,"preparation-save",Some(&value))?;
    Ok(value)
}

pub fn from_saved(home: &Path, chat_id: &str) -> io::Result<Value> {
    let config = home.join(".config/oaclis");
    let key = environment::key("master", chat_id)?;
    let saved = environment::database(&config, &key, "preparation-get", None)?;
    if saved["status"] != "missing" { return Ok(saved); }
    let analysis = environment::database(&config, &key, "analysis-get", None)?;
    if analysis["status"] != "ready" { return Err(invalid("Percorsi non ancora definiti nel DB")); }
    let manifest: Manifest = serde_json::from_value(analysis["manifest"].clone()).map_err(io::Error::other)?;
    let root = std::env::var_os("CODEX_HOME").map(std::path::PathBuf::from).unwrap_or_else(||home.join(".codex"));
    let catalog = environment::python(None, include_str!("../tools/chat_catalog.py"), &[root.to_str().ok_or_else(||invalid("Cartella Codex non UTF-8"))?])?;
    let chat_path = catalog["chats"].as_array().and_then(|chats|chats.iter().find(|chat|chat["id"]==chat_id)).and_then(|chat|chat["path"].as_str()).ok_or_else(||invalid("Chat locale non trovata"))?;
    capture(&config, chat_id, Path::new(chat_path), &manifest)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes_match_transfer_selection_and_saved_values_are_not_recomputed() {
        let root = std::env::temp_dir().join(format!("oaclis-sizes-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let project = root.join("project");
        std::fs::create_dir_all(project.join("node_modules")).unwrap();
        std::fs::create_dir_all(project.join("child")).unwrap();
        std::fs::write(project.join("file.txt"),b"abc").unwrap();
        std::fs::write(project.join("child/file.txt"),b"de").unwrap();
        std::fs::write(project.join("node_modules/package.txt"),b"ignored").unwrap();
        std::fs::write(project.join(".env"),b"private").unwrap();
        let chat = root.join("chat.jsonl");
        std::fs::write(&chat,b"1234567").unwrap();
        let manifest = Manifest { home:root.join("home"), codex_root:root.join("home/.codex"),
            paths:vec![project.clone(),project.join("child"),project.clone()], packages:vec![],configs:vec![],plugins:vec![],uncertainties:vec![],path_notes:vec![],apps:vec![],app_notes:vec![],read_notes:vec![],fingerprints:vec![] };
        let config = root.join("config");
        let id="019f99ae-6802-7b30-902b-f73dbd2459b4";
        let first = capture(&config,id,&chat,&manifest).unwrap();
        let snapshot=refresh(&config,id,&chat,&manifest).unwrap();
        assert!(snapshot["paths"][0]["sha256"].is_string());
        std::fs::write(project.join("file.txt"),b"changed content").unwrap();
        let updated=refresh(&config,id,&chat,&manifest).unwrap();
        assert_ne!(snapshot["paths"][0]["sha256"],updated["paths"][0]["sha256"]);
        assert_eq!(updated["paths_bytes"],17);

        assert_eq!(first["chat_bytes"],7);
        assert_eq!(first["paths_bytes"],5);
        assert_eq!(first["path_files"],2);
        assert_eq!(first["paths"].as_array().unwrap().len(),1);
        std::fs::remove_dir_all(&project).unwrap();
        std::fs::remove_file(&chat).unwrap();
        assert_eq!(capture(&config,id,&chat,&manifest).unwrap(),updated);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn an_unreadable_root_is_not_reported_as_zero_bytes() {
        let root=std::env::temp_dir().join(format!("oaclis-size-error-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();
        let chat=root.join("chat.jsonl");std::fs::write(&chat,b"chat").unwrap();
        let manifest=Manifest {home:root.join("home"),codex_root:root.join("home/.codex"),paths:vec![root.join("missing")],packages:vec![],configs:vec![],plugins:vec![],uncertainties:vec![],path_notes:vec![],apps:vec![],app_notes:vec![],read_notes:vec![],fingerprints:vec![]};
        let result=capture(&root.join("config"),"019f99ae-6802-7b30-902b-f73dbd2459b4",&chat,&manifest).unwrap();
        assert_eq!(result["status"],"partial");assert!(result["paths_bytes"].is_null());assert!(result["paths"][0]["error"].is_string());
        std::fs::remove_dir_all(root).unwrap();
    }
}
