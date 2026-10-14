// transfer_job.rs v2.21.1357
//! Executes an authorized transfer; independent of routes and scheduling.
use crate::{catalog::{local as catalog,remote as remote_catalog},devices,environment,invalid,rules};
use serde_json::{Value,json};
use std::{fs,io,path::{Path,PathBuf},sync::{Arc,Mutex},thread};

pub(crate) fn start(v: Value, codex: &Path, config: &Path, job: &Arc<Mutex<Value>>) -> io::Result<Value> {
    let ds = devices::load(config.join("devices.json"))?;
    let rule = if let Some(id) = v["rule_id"].as_str() {
        Some(
            rules::load(config.join("rules.json"))?
                .into_iter()
                .find(|r| r.id == id)
                .ok_or_else(|| invalid("Regola non trovata"))?,
        )
    } else {
        None
    };
    let incoming = v["source_device_id"]
        .as_str()
        .is_some_and(|id| id != "master");
    if let Some(operation) = v["operation"].as_str()
        && operation != "copy"
        && operation != "sync"
        && operation != "send"
    {
        return Err(invalid("Operazione non valida"));
    }
    if incoming && v["device_id"] != "master" {
        return Err(invalid("Il destinatario deve essere questo dispositivo"));
    }
    let synchronize = v["operation"] == "sync" || v["operation"] == "send";
    let direction = if v["operation"] == "send" {
        if incoming {
            crate::sync::Direction::Receive
        } else {
            crate::sync::Direction::Send
        }
    } else {
        crate::sync::Direction::Both
    };
    let device_id = rule
        .as_ref()
        .map(|r| r.device_id.as_str())
        .or_else(|| {
            if incoming {
                v["source_device_id"].as_str()
            } else {
                v["device_id"].as_str()
            }
        })
        .ok_or_else(|| invalid("Dispositivo richiesto"))?;
    let device = ds
        .into_iter()
        .find(|d| d.id == device_id)
        .ok_or_else(|| invalid("Dispositivo non trovato"))?;
    let chat = if rule.is_none() {
        Some(
            (if incoming {
                serde_json::from_value::<Vec<Value>>(
                    remote_catalog(&device)?["chats"].clone(),
                )
                .map_err(io::Error::other)?
            } else {
                catalog(codex)?
            })
            .into_iter()
            .find(|c| c["id"] == v["chat_id"])
            .ok_or_else(|| invalid("Chat non trovata"))?,
        )
    } else {
        None
    };
    let scopes: environment::Scopes = v
        .get("scopes")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(io::Error::other)?
        .unwrap_or_default();
    if rule.is_none() && !scopes.any() {
        return Err(invalid("Seleziona almeno una categoria"));
    }
    let manifest = if let Some(chat) = &chat && (scopes.paths || scopes.apps) {
        let source = v["source_device_id"].as_str().unwrap_or("master");
        let key = environment::key(source, chat["id"].as_str().unwrap())?;
        let mut cached = environment::database(&config, &key, "analysis-get", None)?;
        if cached["status"] != "ready" && incoming && chat["analysis"]["status"] == "ready" {
            cached=chat["analysis"].clone();
            environment::database(&config,&key,"analysis-save",Some(&cached))?;
        }
        if cached["status"] != "ready" {
            return Err(invalid("Attendi il completamento dell’analisi"));
        }
        let manifest: environment::Manifest =
            serde_json::from_value(cached["manifest"].clone())
                .map_err(io::Error::other)?;
        manifest.validate()?;
        if scopes.apps && environment::database(&config,&key,"inventory-get",None)?["status"] != "ready" {
            return Err(invalid("Attendi il rilevamento delle app sulla sorgente"));
        }
        Some(manifest)
    } else {
        None
    };
    let remote = if let Some(chat) = &chat
        && scopes.chat
    {
        let target = if incoming {
            json!({"root":codex,"chats":catalog(codex)?})
        } else {
            remote_catalog(&device)?
        };
        let existing = target["chats"]
            .as_array()
            .and_then(|chats| chats.iter().find(|c| c["id"] == chat["id"]))
            .and_then(|c| c["path"].as_str());
        let p =
            if let Some(existing) = existing {
                PathBuf::from(existing)
            } else {
                let relative = Path::new(
                    chat["relative_path"]
                        .as_str()
                        .ok_or_else(|| invalid("Percorso chat mancante"))?,
                );
                if !relative.starts_with("sessions")
                    || !relative
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_)))
                {
                    return Err(invalid("Percorso chat non valido"));
                }
                PathBuf::from(target["root"].as_str().ok_or_else(|| {
                    invalid("Cartella Codex destinataria mancante")
                })?)
                .join(relative)
            };
        crate::paths::validate_absolute(&p)?;
        Some(p)
    } else {
        None
    };
    let mut lock = job.lock().unwrap();
    if lock["status"] == "running" {
        return Err(invalid("Un trasferimento è già in corso"));
    }
    *lock = json!({"status":"running","link_key":v["link_key"],"generation":v["generation"],"message":"Preparazione","progress":null,"chat_id":v["chat_id"],"source_device_id":v["source_device_id"].as_str().unwrap_or("master"),"scopes":scopes,"device_id":v["device_id"],
        "title":chat.as_ref().map(|c|c["name"].clone()).unwrap_or_else(||json!(rule.as_ref().unwrap().name)),
        "source":if incoming {device.name.as_str()}else{"Questo dispositivo"},
        "destination":if incoming {"Questo dispositivo"}else{device.name.as_str()}});
    let transfer_key = chat.as_ref().map(|c| environment::key(v["source_device_id"].as_str().unwrap_or("master"), c["id"].as_str().unwrap())).transpose()?;
    if let Some(key) = &transfer_key { environment::database(&config, key, "transfer-save", Some(&lock))?; }
    drop(lock);
    let transfer_config = config.to_path_buf();
    let job = job.clone();
    let flow_key = v["link_key"].as_str().map(str::to_owned);
    let flow_generation = v["generation"].clone();
    let state = config.join("state");
    thread::spawn(move || {
        let result = (|| -> io::Result<Value> {
            if let Some(rule) = rule {
                let results = rules::execute(
                    &device,
                    &rule,
                    &state.join(&rule.id),
                    |event| job.lock().unwrap()["progress"] = json!(event),
                )?;
                Ok(json!(results))
            } else {
                let chat = chat.unwrap();
                fs::create_dir_all(&state)?;
                let transfer_environment = || -> io::Result<Value> { if scopes.paths || scopes.apps { environment::execute(
                    &device,
                    &manifest.unwrap(),
                    scopes,
                    incoming,
                    v["operation"].as_str().unwrap_or("copy"),
                    &state.join(format!(
                        "environment-{}-{}",
                        device.id,
                        chat["id"].as_str().unwrap()
                    )),
                    |progress| job.lock().unwrap()["progress"] = progress,
                ) } else { Ok(json!([])) } };
                if !scopes.chat {
                    return Ok(json!({"environment":transfer_environment()?}));
                }
                let source = Path::new(chat["path"].as_str().unwrap());
                if incoming || synchronize {
                    let destination = remote.unwrap();
                    let (local, remote) = if incoming {
                        (destination, source.to_path_buf())
                    } else {
                        (source.to_path_buf(), destination)
                    };
                    if incoming && !synchronize && local.try_exists()? {
                        return Err(invalid(
                            "La copia locale esiste già; nessuna sovrascrittura",
                        ));
                    }
                    if incoming {
                        crate::rules::ensure_local(local.parent().ok_or_else(
                            || invalid("Cartella destinataria mancante"),
                        )?)?;
                    }
                    let checkpoint = state.join(format!(
                        "chat-{}-{}.json",
                        device.id,
                        chat["id"].as_str().unwrap()
                    ));
                    let outcome = crate::sync::synchronize_with_direction(
                        &device,
                        &local,
                        &remote,
                        &checkpoint,
                        crate::sync::Kind::Chat,
                        direction,
                        |event| {
                            job.lock().unwrap()["progress"] = json!({"event":event})
                        },
                    )?;
                    return Ok(
                        json!({"chat":outcome,"environment":if outcome == crate::sync::Outcome::Conflict {json!([])} else {transfer_environment()?}}),
                    );
                }
                crate::chat::Chat::read(source)?;
                rules::ensure_remote(
                    &device,
                    remote
                        .as_ref()
                        .unwrap()
                        .parent()
                        .ok_or_else(|| invalid("Cartella destinataria mancante"))?,
                )?;
                let receipt = crate::transfer::send_file_with_progress(
                    &device,
                    source,
                    remote.unwrap(),
                    |event| {
                        job.lock().unwrap()["progress"] = json!({"event":event})
                    },
                )?;
                Ok(json!({"chat":receipt,"environment":transfer_environment()?}))
            }
        })();
        let mut lock = job.lock().unwrap();
        match result {
            Ok(results) => {
                lock["status"] = json!("completed");
                lock["results"] = results;
            }
            Err(e) => {
                lock["status"] = json!("failed");
                lock["message"] = json!(e.to_string());
            }
        }
        if crate::automation::conflict(&lock["results"]) {lock["status"]=json!("failed");lock["message"]=json!("Copie divergenti: nessuna sovrascrittura");}
        if let Some(key) = &transfer_key { let _ = environment::database(&transfer_config, key, "transfer-save", Some(&lock)); }
        if let Some(key) = &flow_key { let _ = crate::link_flow::finish(&transfer_config,key,&flow_generation,lock["status"]=="completed",lock["message"].as_str().unwrap_or("Trasferimento non completato")); }
    });
    Ok(json!({"ok":true}))
}
