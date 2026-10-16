// transfer_job.rs v2.28.1605
//! Executes an authorized transfer; independent of routes and scheduling.
use crate::{catalog::{local as catalog,remote as remote_catalog},devices,environment,invalid,rules};
use serde_json::{Value,json};
use std::{fs,io,path::{Path,PathBuf},sync::{Arc,Mutex},thread};

fn module_event(job: &Arc<Mutex<Value>>, module: &str, progress: Value) {
    crate::state_transport::log(format!("TRANSFER {module} {progress}"));
    let mut state = job.lock().unwrap();
    let event = &progress["event"];
    let fraction = if event["phase"] == "completed" { 1.0 }
        else { event["percent"].as_f64().unwrap_or(0.0) / 100.0 };
    let fraction = if let (Some(index), Some(total)) = (progress["index"].as_f64(), progress["total"].as_f64()) {
        if total > 0.0 { ((index - 1.0).max(0.0) + fraction) / total } else { fraction }
    } else { fraction };
    let previous = state["module_progress"][module]["progress"].as_f64().unwrap_or(0.0);
    state["module_progress"][module]["progress"] = json!(previous.max(fraction.clamp(0.0,1.0)));
    state["progress"] = progress;
}

// Reserve the slot in memory; roll back if persistence fails before work starts.
fn begin(config: &Path, key: Option<&str>, job: &Mutex<Value>, pending: Value) -> io::Result<()> {
    let mut current = job.lock().unwrap();
    if current["status"] == "running" { return Err(invalid("Un trasferimento è già in corso")); }
    let previous = std::mem::replace(&mut *current, pending.clone());
    drop(current);
    if let Some(key) = key {
        if let Err(error) = environment::database(config, key, "transfer-save", Some(&pending)) {
            *job.lock().unwrap() = previous;
            return Err(error);
        }
    }
    Ok(())
}

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
    let app_inventory=if scopes.apps {
        if let Some(chat)=&chat {environment::database(config,&environment::key(v["source_device_id"].as_str().unwrap_or("master"),chat["id"].as_str().unwrap())?,"inventory-get",None)?} else {Value::Null}
    } else {Value::Null};
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
    let mut pending = json!({"status":"running","link_key":v["link_key"],"generation":v["generation"],"message":"Preparazione","progress":null,"chat_id":v["chat_id"],"source_device_id":v["source_device_id"].as_str().unwrap_or("master"),"scopes":scopes,"device_id":v["device_id"],
        "title":chat.as_ref().map(|c|c["name"].clone()).unwrap_or_else(||json!(rule.as_ref().unwrap().name)),
        "source":if incoming {device.name.as_str()}else{"Questo dispositivo"},
        "destination":if incoming {"Questo dispositivo"}else{device.name.as_str()}});
    pending["modules"] = v["modules"].clone();
    pending["module_progress"] = json!({
        "chat":{"changed":scopes.chat,"progress":0.0},
        "paths":{"changed":scopes.paths && manifest.as_ref().is_some_and(|m|!m.paths.is_empty()),"progress":0.0},
        "apps":{"changed":scopes.apps,"progress":0.0}
    });
    if v["modules"].is_object() {
        for name in ["chat","paths","apps"] {
            pending["module_progress"][name]["changed"]=json!(v["modules"][name]["enabled"]==true && v["modules"][name]["changed"]==true);
        }
    }
    let transfer_key = chat.as_ref().map(|c| environment::key(v["source_device_id"].as_str().unwrap_or("master"), c["id"].as_str().unwrap())).transpose()?;
    begin(config, transfer_key.as_deref(), job, pending)?;
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
                let mut installation_inventory=app_inventory.clone();
                installation_inventory["paths_pending"]=json!(scopes.paths);
                thread::scope(|scope| -> io::Result<Value> {
                let mut workers = Vec::new();
                for (module, enabled) in [("paths",scopes.paths),("apps",scopes.apps)] {
                    if !enabled {continue;}
                    let device = &device;
                    let manifest = &manifest;
                    let app_inventory = &installation_inventory;
                    let v = &v;
                    let state = &state;
                    let chat = &chat;
                    let job = &job;
                    workers.push((module, scope.spawn(move || -> io::Result<Value> {
                        let selected = environment::Scopes {chat:false,paths:module=="paths",apps:module=="apps"};
                        let result = environment::execute(
                            device,manifest.as_ref().unwrap(),app_inventory,selected,incoming,
                            v["operation"].as_str().unwrap_or("copy"),
                            &state.join(format!("environment-{}-{}{}",device.id,chat["id"].as_str().unwrap(),if module == "apps" { "-apps" } else { "" })),
                            |target,path| crate::path_access::ensure(target,path,job),
                            |progress| module_event(job,module,progress),
                        );
                        match &result {
                            Ok(_) => module_event(job,module,json!({"event":{"phase":"completed"}})),
                            Err(error) => job.lock().unwrap()["module_progress"][module]["error"] = json!(error.to_string()),
                        }
                        result
                    })));
                }
                let chat_result = (|| -> io::Result<Value> {
                if !scopes.chat { return Ok(Value::Null); }
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
                            module_event(&job,"chat",json!({"event":event}))
                        },
                    )?;
                    if outcome != crate::sync::Outcome::Conflict { module_event(&job,"chat",json!({"event":{"phase":"completed"}})); }
                    return Ok(
                        json!(outcome),
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
                        module_event(&job,"chat",json!({"event":event}))
                    },
                )?;
                module_event(&job,"chat",json!({"event":{"phase":"completed"}}));
                Ok(json!(receipt))
                })();
                let mut results = json!({});
                let mut errors = Vec::new();
                match chat_result {
                    Ok(value) => results["chat"] = value,
                    Err(error) => {
                        job.lock().unwrap()["module_progress"]["chat"]["error"] = json!(error.to_string());
                        errors.push(format!("Chat: {error}"));
                    }
                }
                for (module, worker) in workers {
                    match worker.join().unwrap_or_else(|_| Err(io::Error::other("Worker interrotto"))) {
                        Ok(value) => results[module] = value,
                        Err(error) => errors.push(format!("{module}: {error}")),
                    }
                }
                if errors.is_empty() { Ok(results) } else { Err(io::Error::other(errors.join("; "))) }
                })
            }
        })();
        let mut finished = job.lock().unwrap().clone();
        match result {
            Ok(results) => {
                finished["status"] = json!("completed");
                finished["results"] = results;
            }
            Err(e) => {
                finished["status"] = json!("failed");
                finished["message"] = json!(e.to_string());
            }
        }
        if crate::automation::conflict(&finished["results"]) {finished["status"]=json!("failed");finished["message"]=json!("Copie divergenti: nessuna sovrascrittura");}
        if let Some(key) = &transfer_key { let _ = environment::database(&transfer_config, key, "transfer-save", Some(&finished)); }
        if let Some(key) = &flow_key { let _ = crate::link_flow::finish(&transfer_config,key,&flow_generation,finished["status"]=="completed",finished["message"].as_str().unwrap_or("Trasferimento non completato")); }
        *job.lock().unwrap() = finished;
    });
    Ok(json!({"ok":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_is_monotonic_and_modules_are_independent() {
        let job=Arc::new(Mutex::new(json!({"module_progress":{
            "chat":{"changed":true,"progress":0.0},
            "paths":{"changed":true,"progress":0.0},
            "apps":{"changed":false,"progress":0.0}}})));
        module_event(&job,"paths",json!({"index":1,"total":2,"event":{"percent":50}}));
        assert_eq!(job.lock().unwrap()["module_progress"]["paths"]["progress"],0.25);
        module_event(&job,"paths",json!({"event":{"phase":"preparing"}}));
        assert_eq!(job.lock().unwrap()["module_progress"]["paths"]["progress"],0.25);
        assert_eq!(job.lock().unwrap()["module_progress"]["apps"]["progress"],0.0);
        module_event(&job,"chat",json!({"event":{"phase":"completed"}}));
        assert_eq!(job.lock().unwrap()["module_progress"]["chat"]["progress"],1.0);
    }
    #[test]
    fn failed_start_save_preserves_job_and_allows_retry() {
        let path = std::env::temp_dir().join(format!("oaclis-job-start-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&path).unwrap();
        environment::database(&path,"","migrate",None).unwrap();
        fs::remove_file(path.join("transfers.json")).unwrap();
        fs::create_dir(path.join("transfers.json")).unwrap();
        let previous = json!({"status":"completed","chat_id":"previous"});
        let job = Mutex::new(previous.clone());
        let pending = json!({"status":"running","chat_id":"next"});
        assert!(begin(&path,Some("master:next"),&job,pending.clone()).is_err());
        assert_eq!(*job.lock().unwrap(),previous);
        assert_eq!(environment::database(&path,"","transfer-list",None).unwrap(),json!({}));
        fs::remove_dir(path.join("transfers.json")).unwrap();
        begin(&path,Some("master:next"),&job,pending.clone()).unwrap();
        assert_eq!(*job.lock().unwrap(),pending);
        assert!(begin(&path,None,&job,json!({"status":"running","chat_id":"duplicate"})).is_err());
        assert_eq!(*job.lock().unwrap(),pending);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn saving_start_does_not_hold_the_job_lock_and_still_rejects_duplicates() {
        use std::{sync::mpsc, time::{Duration, Instant}};
        let path = std::env::temp_dir().join(format!("oaclis-job-lock-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&path).unwrap();
        environment::database(&path,"","migrate",None).unwrap();
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let config = path.clone();
        let writer = thread::spawn(move || crate::state_store::change(&config, |_| {
            started.send(()).unwrap(); wait.recv().unwrap(); Ok(())
        }));
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        let job = Arc::new(Mutex::new(json!({"status":"idle"})));
        let state = job.clone();
        let config = path.clone();
        let starter = thread::spawn(move || begin(&config, Some("master:next"), &state, json!({"status":"running"})));
        let deadline = Instant::now()+Duration::from_secs(1);
        let mut readable = false;
        while Instant::now()<deadline {
            if job.try_lock().is_ok_and(|value|value["status"]=="running") {readable=true;break;}
            thread::yield_now();
        }
        let duplicate = if readable { Some(begin(&path,None,&job,json!({"status":"running"}))) } else {None};
        release.send(()).unwrap();
        writer.join().unwrap().unwrap();
        starter.join().unwrap().unwrap();
        assert!(readable,"Job reads blocked by JSON persistence");
        assert!(duplicate.unwrap().is_err());
        fs::remove_dir_all(path).unwrap();
    }

}
