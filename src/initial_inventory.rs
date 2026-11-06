// initial_inventory.rs v2.24.1276
//! One initial inventory per local chat; saved results remain cached.
use crate::{catalog::{local as catalog,remote as remote_catalog},devices,environment,invalid};
use serde_json::{Value,json};
use std::{fs,io,path::{Path,PathBuf},sync::{Arc,Mutex},thread};

fn reserve(active: &Mutex<std::collections::HashSet<String>>, key: &str) -> io::Result<bool> {
    let mut running=active.lock().unwrap();
    if running.contains(key) { return Ok(false); }
    if running.len()>=2 { return Err(invalid("Analisi già in corso; riprova tra poco")); }
    running.insert(key.to_owned());
    Ok(true)
}

// Each device analyzes its own archives serially; unchanged failures are not retried.
pub(crate) fn start(codex: PathBuf, config: PathBuf, active: Arc<Mutex<std::collections::HashSet<String>>>) {
    thread::spawn(move || loop {
        if let Ok(chats) = catalog(&codex) {
            if let Err(error)=crate::catalog::number_chats(&config,&chats){eprintln!("Oaclis: numerazione chat: {error}");}
            for chat in chats {
                let Some(id) = chat["id"].as_str() else { continue; };
                let Ok(key) = environment::key("master", id) else { continue; };
                let revision = environment::revision(&chat, None);
                let cached = environment::database(&config, &key, "analysis-get", None).unwrap_or(Value::Null);
                let auth_revision = fs::metadata(codex.join("auth.json"))
                    .and_then(|metadata| metadata.modified())
                    .map(|time| format!("{time:?}"))
                    .unwrap_or_default();
                if matches!(cached["status"].as_str(), Some("ready" | "failed")) {
                    if cached["status"] == "ready" {
                        if let Ok(manifest)=serde_json::from_value::<environment::Manifest>(cached["manifest"].clone()) {
                            if let Err(error)=crate::preparation::refresh(&config,id,Path::new(chat["path"].as_str().unwrap_or("")),&manifest) {eprintln!("Oaclis: check locale: {error}");}
                        }
                        let _ = crate::app_inventory::request(&config,&json!({"chat_id":id}));
                    }
                    continue;
                }
                if !matches!(reserve(&active,&key),Ok(true)) { continue; }
                let _ = environment::database(&config, &key, "analysis-save", Some(&json!({"status":"pending", "revision":revision})));
                let result = match environment::analyze(None, chat["path"].as_str().unwrap_or(""), id) {
                    Ok(manifest) => {
                        if let Err(error) = crate::preparation::refresh(&config,id,Path::new(chat["path"].as_str().unwrap_or("")),&manifest) { eprintln!("Oaclis: dimensioni sorgenti: {error}"); }
                        json!({"status":"ready", "revision":revision, "manifest":manifest, "chat_bytes":chat["bytes"], "chat_modified_ns":chat["modified_ns"]})
                    },
                    Err(error) => json!({"status":"failed", "revision":revision, "error":error.to_string(), "auth_revision":auth_revision}),
                };
                let _ = environment::database(&config, &key, "analysis-save", Some(&result));
                if result["status"] == "ready" { let _ = crate::app_inventory::request(&config,&json!({"chat_id":id})); }
                active.lock().unwrap().remove(&key);
            }
        }
        thread::sleep(std::time::Duration::from_secs(60));
    });
}

/// Manual refresh shares the detection pipeline but never starts a transfer.
pub(crate) fn request(config: &Path, codex: &Path, analyses: &Arc<Mutex<std::collections::HashSet<String>>>, checks_paused: bool, v: Value) -> io::Result<Value> {
    let source = v["source_device_id"].as_str().unwrap_or("master");
    let chat_id = v["chat_id"]
        .as_str()
        .ok_or_else(|| invalid("Chat richiesta"))?;
    let key = environment::key(source, chat_id)?;
    if checks_paused && v["force"] != true { return environment::database(&config,&key,"analysis-get",None); }
    let device = if source == "master" {
        None
    } else {
        Some(
            devices::load(config.join("devices.json"))?
                .into_iter()
                .find(|d| d.id == source)
                .ok_or_else(|| invalid("Dispositivo non trovato"))?,
        )
    };
    let list = if let Some(device) = &device {
        remote_catalog(device)?["chats"].clone()
    } else {
        json!(catalog(&codex)?)
    };
    let chat = list
        .as_array()
        .and_then(|cs| cs.iter().find(|c| c["id"] == chat_id))
        .ok_or_else(|| invalid("Chat non trovata"))?;
    let revision = environment::revision(chat, device.as_ref());
    let cached = environment::database(&config, &key, "analysis-get", None)?;
    if matches!(cached["status"].as_str(), Some("ready" | "failed")) && v["force"] != true {
        return Ok(cached);
    }
    let path = chat["path"].as_str().ok_or_else(||invalid("Archivio chat mancante"))?.to_owned();
    if !reserve(analyses,&key)? { return environment::database(config,&key,"analysis-get",None); }
    let pending = match environment::database(config,&key,"analysis-save",Some(&json!({"status":"pending","revision":revision}))) {
        Ok(value) => value,
        Err(error) => { analyses.lock().unwrap().remove(&key); return Err(error); }
    };
    let active = analyses.clone();
    let config = config.to_path_buf();
    let chat_id = chat_id.to_owned();
    thread::spawn(move || {
        let result = match environment::analyze(device.as_ref(), &path, &chat_id) {
            Ok(manifest) => {
                if device.is_none() {
                    if let Err(error) = crate::preparation::refresh(&config,&chat_id,Path::new(&path),&manifest) { eprintln!("Oaclis: dimensioni sorgenti: {error}"); }
                }
                json!({"status":"ready","revision":revision,"manifest":manifest})
            }
            Err(error) => {
                json!({"status":"failed","revision":revision,"error":error.to_string()})
            }
        };
        if let Err(error) =
            environment::database(&config, &key, "analysis-save", Some(&result))
        {
            eprintln!("Oaclis: salvataggio analisi non riuscito: {error}");
        }
        active.lock().unwrap().remove(&key);
    });
    Ok(pending)
}

#[cfg(test)]
mod optimization_tests {
    use super::*;
    #[test]
    fn simultaneous_requests_reserve_once_and_respect_capacity() {
        let active=Arc::new(Mutex::new(std::collections::HashSet::new()));
        let gate=Arc::new(std::sync::Barrier::new(12));
        let workers:Vec<_>=(0..12).map(|_|{let active=active.clone();let gate=gate.clone();thread::spawn(move||{gate.wait();reserve(&active,"same").unwrap()})}).collect();
        assert_eq!(workers.into_iter().filter_map(|worker|worker.join().unwrap().then_some(())).count(),1);
        assert!(reserve(&active,"second").unwrap());
        assert!(reserve(&active,"third").is_err());
        assert_eq!(active.lock().unwrap().len(),2);
    }
}
