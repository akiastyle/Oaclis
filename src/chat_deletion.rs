// chat_deletion.rs v1.12.172
//! Irreversible chat deletion; only UUIDs and peer acknowledgements remain for offline delivery.
use crate::{invalid, state_store::{self,State}};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet},fs,io,path::{Path,PathBuf},process::Command,sync::{Arc,Mutex},thread,time::{Duration,Instant}};

fn valid_chat(id:&str)->bool {
    id.len()==36 && id.bytes().enumerate().all(|(i,b)|if [8,13,18,23].contains(&i){b==b'-'}else{b.is_ascii_hexdigit()})
}
pub(crate) fn ids(state:&State)->Vec<String> {
    state.shared["deletions"].as_object().into_iter().flatten().map(|(id,_)|id.clone()).collect()
}
pub(crate) fn blocked(config:&Path,id:&str)->io::Result<bool> {
    Ok(state_store::snapshot(config)?.shared["deletions"].get(id).is_some())
}
pub(crate) fn validate(value:&Value)->io::Result<()> {
    if value.is_null(){return Ok(());}
    let entries=value.as_object().filter(|v|v.len()<=10000).ok_or_else(||invalid("Cancellazioni non valide"))?;
    for (id,entry) in entries {
        if !valid_chat(id) || !entry.is_object() {return Err(invalid("Chat da cancellare non valida"));}
        if entry.get("global").is_some_and(|v|!v.is_boolean()){return Err(invalid("Ambito cancellazione non valido"));}
        for name in ["targets","done"] {
            if !entry[name].as_array().is_some_and(|v|v.len()<=1000 && v.iter().all(|x|x.as_str().is_some_and(crate::devices::identifier))) {
                return Err(invalid("Destinatari cancellazione non validi"));
            }
        }
    }
    Ok(())
}
pub(crate) fn merge(state:&mut State,incoming:&Value) {
    if incoming.is_null(){return;}
    let shared=Arc::make_mut(&mut state.shared);
    if !shared["deletions"].is_object(){shared["deletions"]=json!({});}
    for (id,entry) in incoming.as_object().into_iter().flatten() {
        let was_global=shared["deletions"].get(id).is_some_and(|v|v["global"]!=false);
        let global=was_global || entry["global"]!=false;
        if !shared["deletions"][id].is_object(){shared["deletions"][id]=json!({"targets":[],"done":[]});}
        if !was_global && global {shared["deletions"][id]["done"]=json!([]);}
        shared["deletions"][id]["global"]=json!(global);
        for name in ["targets","done"] {
            let all:BTreeSet<_>=shared["deletions"][id][name].as_array().into_iter().flatten()
                .chain(entry[name].as_array().into_iter().flatten()).filter_map(Value::as_str).map(str::to_owned).collect();
            shared["deletions"][id][name]=json!(all);
        }
    }
}
fn belongs(key:&str,deleted:&[String])->bool {
    let mut parts=key.split(':');let kind=parts.next().unwrap_or("");let id=parts.next().unwrap_or("");
    matches!(kind,"chat"|"preference"|"chatdata"|"link"|"linkposition") && deleted.iter().any(|v|v==id)
}
pub(crate) fn purge_records(records:&mut Value,deleted:&[String]) {
    if let Some(records)=records.as_object_mut(){records.retain(|key,_|!belongs(key,deleted));}
}
pub(crate) fn purge(state:&mut State,files:&mut Value) {
    let deleted=ids(state);if deleted.is_empty(){return;}
    if deleted.iter().any(|id|state.appearance.contains_key(&format!("chat:{id}"))) {
        Arc::make_mut(&mut state.appearance).retain(|key,_|key.strip_prefix("chat:").is_none_or(|id|!deleted.iter().any(|v|v==id)));
    }
    if state.analysis.keys().any(|key|deleted.iter().any(|id|key.rsplit(':').next()==Some(id.as_str()))) {
        Arc::make_mut(&mut state.analysis).retain(|key,_|!deleted.iter().any(|id|key.rsplit(':').next()==Some(id.as_str())));
    }
    if state.links.values().any(|v|deleted.iter().any(|id|v["entry"]["id"]==*id)) {
        Arc::make_mut(&mut state.links).retain(|_,v|!deleted.iter().any(|id|v["entry"]["id"]==*id));
    }
    if state.transfers.iter().any(|(key,v)|deleted.iter().any(|id|v["chat_id"]==*id || key.rsplit(':').next()==Some(id.as_str()))) {
        Arc::make_mut(&mut state.transfers).retain(|key,v|!deleted.iter().any(|id|v["chat_id"]==*id || key.rsplit(':').next()==Some(id.as_str())));
    }
    if ["records","baseline"].iter().any(|name|state.shared[*name].as_object().into_iter().flatten().any(|(key,_)|belongs(key,&deleted))) {
        let shared=Arc::make_mut(&mut state.shared);
        for name in ["records","baseline"] {purge_records(&mut shared[name],&deleted);}
    }
    if let Some(points)=files.get_mut("link-positions.json").and_then(Value::as_object_mut) {
        points.retain(|key,_|!deleted.iter().any(|id|key.split(':').next()==Some(id.as_str())));
    }
}
pub(crate) fn request(config:&Path,id:&str,source:&str)->io::Result<Value> {
    if !valid_chat(id){return Err(invalid("Identificativo chat non valido"));}
    let local=state_store::identity(config)?;
    state_store::change_with_files(config,|state,files|{
        let global=source!="master" || state.links.values().any(|v|v["entry"]["id"]==id);
        let mut targets=BTreeSet::from([local]);
        if global {
            let pairing=state_store::read_json(&config.join("pairing.json"),json!({}))?;
            targets.extend(pairing["approved"].as_object().into_iter().flatten().map(|(id,_)|id.clone()));
        }
        merge(state,&json!({id:{"targets":targets,"done":[],"global":global}}));
        validate(&state.shared["deletions"])?;
        files["link-positions.json"]=state_store::read_json(&config.join("link-positions.json"),json!({}))?;
        purge(state,files);
        Ok(json!({"ok":true,"deleted_chats":ids(state)}))
    })
}
pub(crate) fn local_ids(root:&Path)->io::Result<BTreeSet<String>> {
    let mut ids=BTreeSet::new();
    let mut directories=vec![root.join("sessions"),root.join("archived_sessions")];
    while let Some(path)=directories.pop(){
        let entries=match fs::read_dir(path){Ok(v)=>v,Err(e) if e.kind()==io::ErrorKind::NotFound=>continue,Err(e)=>return Err(e)};
        for entry in entries {let entry=entry?;let kind=entry.file_type()?;
            if kind.is_dir(){directories.push(entry.path());}
            else if kind.is_file(){let name=entry.file_name();let name=name.to_string_lossy();
                if let Some(id)=name.strip_suffix(".jsonl").and_then(|s|s.get(s.len().saturating_sub(36)..)).filter(|id|valid_chat(id)){ids.insert(id.into());}
            }
        }
    }
    for database in [root.join("state_5.sqlite"),root.join("sqlite/state_5.sqlite")] {
        if !database.is_file(){continue;}
        let connection=rusqlite::Connection::open_with_flags(database,rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(io::Error::other)?;
        connection.busy_timeout(Duration::from_millis(500)).map_err(io::Error::other)?;
        let mut query=connection.prepare("SELECT id FROM threads").map_err(io::Error::other)?;
        for row in query.query_map([],|r|r.get::<_,String>(0)).map_err(io::Error::other)?{ids.insert(row.map_err(io::Error::other)?);}
    }
    Ok(ids)
}
pub(crate) fn remove_local(home:&Path,codex:&Path,id:&str,present:bool)->io::Result<()> {
    if !valid_chat(id){return Err(invalid("Identificativo chat non valido"));}
    let _=crate::ssh_settings::command(Command::new("tmux").args(["kill-session","-t",&format!("oaclis-{id}")]),None,Duration::from_secs(5));
    if present {
        crate::ssh_settings::command(Command::new(crate::terminal::codex(home)?)
            .args(["delete",id,"--force"]).env("CODEX_HOME",codex).current_dir(home),None,Duration::from_secs(30))?;
    }
    Ok(())
}
pub(crate) fn start(home:PathBuf,config:PathBuf,codex:PathBuf,job:Arc<Mutex<Value>>) {
    let codex=if codex.is_absolute(){codex}else{match std::env::current_dir(){Ok(root)=>root.join(codex),Err(_)=>return}};
    thread::spawn(move||{
        let mut attempts=BTreeMap::<String,Instant>::new();
        let mut audit=Instant::now();
        loop {
            let result=(||->io::Result<()>{
                let state=state_store::snapshot(&config)?;validate(&state.shared["deletions"])?;let local=state_store::identity(&config)?;
                let audit_due=audit.elapsed()>=Duration::from_secs(60);
                let pending:Vec<String>=ids(&state).into_iter().filter(|id|{
                    let done=state.shared["deletions"][id]["done"].as_array().is_some_and(|v|v.iter().any(|v|v==&local));
                    let busy=job.lock().unwrap();
                    (!done || audit_due && state.shared["deletions"][id]["global"]!=false) && !(busy["status"]=="running" && busy["chat_id"]==*id) &&
                        attempts.get(id).is_none_or(|t|t.elapsed()>=Duration::from_secs(30))
                }).collect();
                if pending.is_empty(){return Ok(());}
                for id in &pending {attempts.insert(id.clone(),Instant::now());}
                let present=local_ids(&codex)?;if audit_due{audit=Instant::now();}
                for id in pending {
                    let done=state.shared["deletions"][&id]["done"].as_array().is_some_and(|v|v.iter().any(|v|v==&local));
                    if done && !present.contains(&id){continue;}
                    remove_local(&home,&codex,&id,present.contains(&id))?;
                    if state.shared["deletions"][&id]["global"]!=false {crate::mobile::forget_chat(&config,&id)?;}
                    state_store::change(&config,|state|{
                        let done=&mut Arc::make_mut(&mut state.shared)["deletions"][&id]["done"];
                        if !done.as_array().unwrap().iter().any(|v|v==&local){done.as_array_mut().unwrap().push(json!(local));}
                        Ok(())
                    })?;
                    crate::state_transport::log(format!("CHAT DELETE {id} completed"));
                }
                Ok(())
            })();
            if let Err(error)=result{crate::state_transport::log(format!("CHAT DELETE pending: {}",crate::environment_analysis::redact(&error.to_string())));}
            thread::sleep(Duration::from_secs(1));
        }
    });
}
