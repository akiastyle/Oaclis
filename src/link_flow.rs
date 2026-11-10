// link_flow.rs v1.21.399
//! Link lifecycle: compare immutable proofs, persist intent, never transfer from a poll.
use crate::{devices::Device, environment, invalid, catalog};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io, path::Path, time::{SystemTime, UNIX_EPOCH}};

fn identity(config: &Path) -> io::Result<String> {
    let public=std::fs::read_to_string(config.join("pairing_ed25519.pub"))?;
    let material=public.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
    Ok(Sha256::digest(material.as_bytes()).iter().map(|byte|format!("{byte:02x}")).collect::<String>()[..32].to_owned())
}

pub(crate) fn key(link: &Value) -> io::Result<String> {
    let id = link["entry"]["id"].as_str().ok_or_else(|| invalid("Chat richiesta"))?;
    let a = link["source"].as_str().ok_or_else(|| invalid("Sorgente richiesta"))?;
    let b = link["target"].as_str().ok_or_else(|| invalid("Destinazione richiesta"))?;
    let mut pair = [a,b]; pair.sort();
    Ok(format!("{id}:{}:{}", pair[0], pair[1]))
}

pub(crate) fn initial(executor: &str) -> Value {
    json!({"executor":executor,"generation":format!("{executor}-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
           "initialized":false,"phase":"waiting","requested":false})
}

pub(crate) fn save(config: &Path, link: &Value, flow: Value) -> io::Result<()> {
    let storage=key(link)?;
    let saved=environment::database(config, &storage, "links-flow", Some(&flow))?;
    if !saved.as_array().is_some_and(|all|all.iter().any(|link|key(link).is_ok_and(|key|key==storage) && link["flow"]["generation"]==flow["generation"])) {
        return Err(invalid("Collegamento cancellato o sostituito"));
    }
    Ok(())
}

pub(crate) fn compare_proofs(source: Option<&Value>, target: Option<&Value>) -> (&'static str, Option<&'static str>) {
    match (source, target) {
        (Some(_), None) => ("ready", Some("source")),
        (None, Some(_)) => ("ready", Some("target")),
        (Some(a), Some(b)) if a["sha256"] == b["sha256"] => ("aligned", None),
        (Some(a), Some(b)) if a["prefix"] == b["prefix"] => {
            if a["bytes"].as_u64() > b["bytes"].as_u64() { ("ready", Some("source")) }
            else if b["bytes"].as_u64() > a["bytes"].as_u64() { ("ready", Some("target")) }
            else { ("conflict", None) }
        },
        (Some(a),Some(b)) => match (a["activity"].as_str(),b["activity"].as_str()) {
            (Some(a),Some(b)) if a>b => ("ready",Some("source")),
            (Some(a),Some(b)) if b>a => ("ready",Some("target")),
            _ => ("conflict",None),
        },
        _ => ("conflict", None),
    }
}

fn module_signature(chat: Option<&Value>, module: &str) -> Option<Value> {
    let chat=chat?;let record=&chat["sync_data"][module];
    if record["status"]!="ready" {return None;}
    let home=chat["analysis"]["manifest"]["home"].as_str().unwrap_or("");
    let path_key=|path:&str| if !home.is_empty() {path.strip_prefix(&format!("{home}/")).map(|v|format!("~/{v}")).unwrap_or_else(||path.to_owned())} else {path.to_owned()};
    let mut items=Vec::new();
    if module=="paths" {
        for item in record["items"].as_array().into_iter().flatten() {
            let hash=item["sha256"].as_str()?;
            items.push(json!([path_key(item["path"].as_str()?),hash]));
        }
    } else {
        for item in record["requirements"].as_array().into_iter().flatten() {items.push(json!([item["kind"],item["name"],item["version"]]));}
        for item in record["configs"].as_array().into_iter().flatten() {items.push(json!([item["service"],path_key(item["path"].as_str().unwrap_or("")),item["modules"]]));}
    }
    items.sort_by_key(Value::to_string);
    Some(json!(items))
}

fn remember_delivery(config: &Path, link: &Value, flow: &mut Value) -> io::Result<()> {
    let transfers = environment::database(config,"","transfer-list",None)?;
    let storage = key(link)?;
    let Some(record) = transfers.as_object().and_then(|all|all.values().find(|record|
        record["link_key"]==storage && record["generation"]==flow["generation"] && record["status"]=="completed")) else {return Ok(());};
    let side = if record["source_device_id"]==link["source"] {"source"}
        else if record["source_device_id"]==link["target"] {"target"} else {return Ok(());};
    let other = if side=="source" {"target"} else {"source"};
    let modules = if record["modules"].is_object() {record["modules"].clone()} else {flow["modules"].clone()};
    for name in ["paths","apps"] {
        if record["scopes"][name]!=true || record["module_progress"][name]["progress"].as_f64()!=Some(1.0)
            || record["results"][name].is_null() || !modules[name][side].is_array() {continue;}
        if name=="apps" && !record["results"][name].as_array().is_some_and(|items|items.iter().any(|item|item["installation"]["status"]=="completed")) {continue;}
        flow["delivered"][name]=json!({"source":side,"signature":modules[name][side],"target_before":modules[name][other]});
        if name=="apps" {flow["delivered"][name]["installed"]=json!(true);}
    }
    Ok(())
}

fn match_modules(link: &Value, flow: &mut Value, source: Option<&Value>, target: Option<&Value>, chat_phase: &str, newer: Option<&str>) {
    let winner=newer.or_else(||flow["newer"].as_str()).unwrap_or("source").to_owned();
    let selected=if winner=="target" {target} else {source};
    let mut modules=json!({});let mut waiting=false;let mut changed=false;
    for name in ["chat","paths","apps"] {
        let enabled=link["scopes"][name]!=false;
        let (a,b)=if name=="chat" {(Some(flow["proofs"]["source"]["sha256"].clone()),Some(flow["proofs"]["target"]["sha256"].clone()))}
            else {(module_signature(source,name),module_signature(target,name))};
        let winner_ready=if winner=="target" {b.is_some()} else {a.is_some()};
        let available=if name=="chat" {selected.is_some()} else {winner_ready};
        let empty=if winner=="target" {b.as_ref()} else {a.as_ref()}.is_some_and(|v|v==&json!([]));
        let receipt=&flow["delivered"][name];
        let (selected_signature,other_signature)=if winner=="target" {(&b,&a)} else {(&a,&b)};
        let delivered=name!="chat" && (name!="apps" || receipt["installed"]==true) && receipt["source"]==winner
            && selected_signature.as_ref().is_some_and(|signature|signature==&receipt["signature"])
            && other_signature.as_ref().is_some_and(|signature|signature==&receipt["target_before"] || signature==&receipt["signature"]);
        let different=enabled && !empty && a!=b && !delivered;
        if enabled && !available {waiting=true;}
        changed|=different;
        modules[name]=json!({"enabled":enabled,"changed":different,"ready":available,"source":a,"target":b});
    }
    flow["modules"]=modules;
    flow["newer"]=json!(winner);
    flow["phase"]=json!(if chat_phase=="conflict" {"conflict"} else if waiting {"waiting_inventory"} else if changed {"ready"} else {"aligned"});
    flow["error"]=if waiting {json!("In attesa del check locale dei moduli attivi sulla sorgente")} else {Value::Null};
}

pub(crate) fn inspect(config: &Path, codex: &Path, device: &Device, link: &Value, catalogs: &mut catalog::Catalogs) -> io::Result<Value> {
    let id = link["entry"]["id"].as_str().ok_or_else(||invalid("Chat richiesta"))?;
    let find = |list: &[Value]| list.iter().find(|chat|chat["id"] == id).cloned();
    let local = find(catalog::cached(catalogs, "master", || catalog::local_at(codex, config))?);
    let remote = find(catalog::cached(catalogs, &device.id, || {
        serde_json::from_value(catalog::remote(device)?["chats"].clone()).map_err(io::Error::other)
    })?);
    let (source,target) = if link["source"] == "master" { (local,remote) } else { (remote,local) };
    let stamp = |chat: &Option<Value>| chat.as_ref().map(|chat|json!([chat["path"],chat["bytes"],chat["modified_ns"]])).unwrap_or(Value::Null);
    let signatures = json!({"source":stamp(&source),"target":stamp(&target)});
    let mut flow = link["flow"].clone();
    if !flow["delivered"].is_object() {remember_delivery(config,link,&mut flow)?;}
    if flow["signatures"] == signatures && flow["comparison"].is_string() {
        let phase=flow["comparison"].as_str().unwrap().to_owned();
        match_modules(link,&mut flow,source.as_ref(),target.as_ref(),&phase,None);
        save(config,link,flow.clone())?;return Ok(flow);
    }
    let mut checking=flow.clone();checking["phase"]=json!("checking");save(config,link,checking)?;
    let prefix = source.as_ref().and_then(|s|s["bytes"].as_u64()).unwrap_or(0).min(target.as_ref().and_then(|t|t["bytes"].as_u64()).unwrap_or(0));
    let proof = |chat: &Option<Value>, incoming: bool| -> io::Result<Option<Value>> {
        let Some(chat) = chat else { return Ok(None); };
        let path = chat["path"].as_str().ok_or_else(||invalid("Archivio chat mancante"))?;
        let value = if incoming {crate::state_transport::remote(device,&json!({"kind":"chat-proof","path":path,"chat_id":id,"prefix_bytes":prefix}))?}else{crate::chat::proof(Path::new(path),id,prefix)?};
        if value["bytes"] != chat["bytes"] || value["modified_ns"] != chat["modified_ns"] { return Err(invalid("Chat cambiata durante il confronto; attendo il prossimo controllo")); }
        Ok(Some(value))
    };
    let a = proof(&source, link["source"] != "master")?;
    let b = proof(&target, link["target"] != "master")?;
    for (alias,proof) in [(link["source"].as_str().unwrap(), &a),(link["target"].as_str().unwrap(), &b)] {
        if let Some(proof)=proof { environment::database(config,&environment::key(alias,id)?,"chat-observed",Some(proof))?; }
    }
    let (mut phase,newer) = compare_proofs(a.as_ref(),b.as_ref());
    if link["operation"] != "sync" && newer == Some("target") { phase = "conflict"; }
    flow["comparison"] = json!(phase); flow["phase"] = json!(phase); flow["signatures"] = signatures;
    flow["proofs"] = json!({"source":a,"target":b}); flow["error"] = Value::Null;
    match_modules(link,&mut flow,source.as_ref(),target.as_ref(),phase,newer);
    flow["checked_at"] = json!(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
    save(config,link,flow.clone())?;
    Ok(flow)
}

pub(crate) fn request(link: &Value, flow: &Value) -> io::Result<Value> {
    if flow["phase"] == "conflict" { return Err(invalid("Le copie divergono: nessuna sovrascrittura automatica")); }
    let reverse = flow["newer"] == "target";
    if reverse && link["operation"] != "sync" { return Err(invalid("Direzione opposta al trasferimento previsto")); }
    Ok(json!({"chat_id":link["entry"]["id"],"source_device_id":if reverse {&link["target"]} else {&link["source"]},
      "device_id":if reverse {&link["source"]} else {&link["target"]},"operation":"send","scopes":link["scopes"],
      "modules":flow["modules"],"link_key":key(link)?,"generation":flow["generation"]}))
}

pub(crate) fn should_transfer(link: &Value, flow: &Value, enabled: bool) -> bool {
    enabled && flow["requested"] == true
        && (flow["initialized"] != true || link["operation"] == "sync")
        && flow["phase"] == "ready"
}

pub(crate) fn finish(config: &Path, key: &str, generation: &Value, success: bool, error: &str) -> io::Result<()> {
    let links=environment::database(config,"","links-get",None)?;
    if let Some(link)=links.as_array().and_then(|all|all.iter().find(|link|self::key(link).is_ok_and(|k|k==key) && link["flow"]["generation"]==*generation)) {
        let mut flow=link["flow"].clone();
        flow["requested"]=json!(false);
        flow["phase"]=json!(if success {if link["operation"]=="sync" {"aligned"} else {"transferred"}} else {"failed"});
        if success {remember_delivery(config,link,&mut flow)?;}
        if success {flow["initialized"]=json!(true);flow["common"]=flow["proofs"][flow["newer"].as_str().unwrap_or("source")]["sha256"].clone();flow["signatures"]=Value::Null;}
        flow["error"]=if success {Value::Null} else {json!(error)};
        save(config,link,flow)?;
    }
    Ok(())
}

pub(crate) fn recover(config: &Path) -> io::Result<()> {
    let links=environment::database(config,"","links-get",None)?;
    let transfers=environment::database(config,"","transfer-list",None)?;
    let local=identity(config).unwrap_or_default();
    for link in links.as_array().into_iter().flatten() {
        if link["flow"]["executor"] != local {continue;}
        if link["flow"]["phase"] == "transferring" {
            let storage=key(link)?;
            let completed=transfers.as_object().is_some_and(|all|all.values().any(|record| record["link_key"]==storage && record["generation"]==link["flow"]["generation"] && record["status"]=="completed"));
            finish(config,&storage,&link["flow"]["generation"],completed,"Trasferimento interrotto dal riavvio; riprovare manualmente")?;
        } else if link["flow"]["phase"] == "checking" {
            let mut flow=link["flow"].clone();flow["phase"]=json!("waiting");save(config,link,flow)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn proofs_choose_content_not_device_time() {
        let a=json!({"sha256":"a","prefix":"common","bytes":20});
        let b=json!({"sha256":"b","prefix":"common","bytes":10});
        assert_eq!(compare_proofs(Some(&a),Some(&b)),("ready",Some("source")));
        assert_eq!(compare_proofs(Some(&b),Some(&a)),("ready",Some("target")));
        assert_eq!(compare_proofs(Some(&a),Some(&a)),("aligned",None));
        assert_eq!(compare_proofs(Some(&a),None),("ready",Some("source")));
        assert_eq!(compare_proofs(None,None),("conflict",None));
        let old=json!({"sha256":"old","prefix":"diverged-old","activity":"2026-10-14T10:00:00.000000000Z"});
        let new=json!({"sha256":"new","prefix":"diverged-new","activity":"2026-10-15T10:00:00.000000000Z"});
        assert_eq!(compare_proofs(Some(&new),Some(&old)),("ready",Some("source")));
        assert_eq!(compare_proofs(Some(&old),Some(&new)),("ready",Some("target")));

        assert_eq!(compare_proofs(Some(&a),Some(&json!({"sha256":"b","prefix":"other","bytes":30}))), ("conflict",None));
    }
    #[test]
    fn chat_winner_controls_all_modules_and_switches_remain_independent() {
        let link=json!({"operation":"sync","scopes":{"chat":true,"paths":true,"apps":false}});
        let source=json!({"analysis":{"manifest":{"home":"/root"}},"sync_data":{
            "paths":{"status":"ready","items":[{"path":"/root/project","sha256":"old"}]}}});
        let target=json!({"analysis":{"manifest":{"home":"/home/kim"}},"sync_data":{
            "paths":{"status":"ready","items":[{"path":"/home/kim/project","sha256":"new"}]}}});
        let mut flow=json!({"proofs":{"source":{"sha256":"older-chat"},"target":{"sha256":"newer-chat"}}});
        match_modules(&link,&mut flow,Some(&source),Some(&target),"ready",Some("target"));
        assert_eq!(flow["phase"],"ready");assert_eq!(flow["newer"],"target");
        assert_eq!(flow["modules"]["paths"]["changed"],true);
        assert_eq!(flow["modules"]["apps"]["enabled"],false);
        assert_eq!(flow["modules"]["apps"]["changed"],false);
        let equal=json!({"proofs":{"source":{"sha256":"same"},"target":{"sha256":"same"}},"newer":"target"});
        let mut aligned=equal.clone();
        match_modules(&link,&mut aligned,Some(&target),Some(&target),"aligned",None);
        assert_eq!(aligned["phase"],"aligned");
        let mut only_paths=equal;
        match_modules(&link,&mut only_paths,Some(&source),Some(&target),"aligned",None);
        assert_eq!(only_paths["phase"],"ready");assert_eq!(only_paths["newer"],"target");
        assert_eq!(only_paths["modules"]["chat"]["changed"],false);
        let mut missing=json!({"proofs":{}});
        match_modules(&link,&mut missing,Some(&json!({})),None,"ready",Some("source"));
        assert_eq!(missing["phase"],"waiting_inventory");
        assert!(!should_transfer(&link,&missing,true));
    }
    #[test]
    fn completed_delivery_survives_matching_without_claiming_apps_are_installed() {
        let root=std::env::temp_dir().join(format!("oaclis-delivery-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();
        let source=json!({"sync_data":{
            "paths":{"status":"ready","items":[{"path":"/var/www/project","sha256":"files-v1"}]},
            "apps":{"status":"ready","requirements":[{"kind":"command","name":"node","version":"24"}]}}});
        let target=json!({"sync_data":{"paths":{"status":"ready","items":[]},"apps":{"status":"ready","requirements":[]}}});
        let mut flow=json!({"generation":"one","proofs":{"source":{"sha256":"same-chat"},"target":{"sha256":"same-chat"}},"newer":"source"});
        let mut link=json!({"entry":{"id":"chat","name":"Chat"},"source":"master","target":"peer","operation":"sync","scopes":{"chat":true,"paths":true,"apps":true}});
        match_modules(&link,&mut flow,Some(&source),Some(&target),"aligned",None);
        assert_eq!(flow["phase"],"ready");
        link["flow"]=flow.clone();
        let storage=key(&link).unwrap();
        let mut record=json!({"link_key":storage,"generation":"one","source_device_id":"master","status":"completed",
            "scopes":{"paths":true,"apps":true},"module_progress":{"paths":{"progress":1.0},"apps":{"progress":1.0}},"results":{"paths":[],"apps":[]}});
        // Recover the already completed transfer written by the previous build.
        environment::database(&root,"master:chat","transfer-save",Some(&record)).unwrap();
        remember_delivery(&root,&link,&mut flow).unwrap();
        match_modules(&link,&mut flow,Some(&source),Some(&target),"aligned",None);
        assert_eq!(flow["phase"],"ready");
        assert_eq!(flow["modules"]["paths"]["changed"],false);
        assert_eq!(flow["modules"]["apps"]["changed"],true);
        assert_eq!(flow["modules"]["apps"]["target"],json!([]));
        assert!(flow["delivered"]["apps"].get("installed").is_none());
        record["results"]["apps"]=json!([{"installation":{"status":"completed"}}]);
        environment::database(&root,"master:chat","transfer-save",Some(&record)).unwrap();
        remember_delivery(&root,&link,&mut flow).unwrap();
        match_modules(&link,&mut flow,Some(&source),Some(&target),"aligned",None);
        assert_eq!(flow["phase"],"aligned");
        assert_eq!(flow["delivered"]["apps"]["installed"],true);
        let mut changed=source.clone();changed["sync_data"]["paths"]["items"][0]["sha256"]=json!("files-v2");
        match_modules(&link,&mut flow,Some(&changed),Some(&target),"aligned",None);
        assert_eq!(flow["phase"],"ready");
        assert_eq!(flow["modules"]["paths"]["changed"],true);
        let mut edited=target.clone();edited["sync_data"]["paths"]["items"]=json!([{"path":"/var/www/project","sha256":"remote-edit"}]);
        match_modules(&link,&mut flow,Some(&source),Some(&edited),"aligned",None);
        assert_eq!(flow["modules"]["paths"]["changed"],true);
        let mut stale=link["flow"].clone();stale["generation"]=json!("new-link");
        remember_delivery(&root,&link,&mut stale).unwrap();assert!(stale["delivered"].is_null());
        record["status"]=json!("failed");environment::database(&root,"master:chat","transfer-save",Some(&record)).unwrap();
        let mut failed=link["flow"].clone();remember_delivery(&root,&link,&mut failed).unwrap();assert!(failed["delivered"].is_null());
        // Future transfers bind their receipt to the captured input, not a later inventory.
        record["status"]=json!("completed");record["modules"]=link["flow"]["modules"].clone();
        environment::database(&root,"master:chat","transfer-save",Some(&record)).unwrap();
        let mut fresh=flow.clone();fresh["delivered"]=Value::Null;
        remember_delivery(&root,&link,&mut fresh).unwrap();
        match_modules(&link,&mut fresh,Some(&changed),Some(&target),"aligned",None);
        assert_eq!(fresh["modules"]["paths"]["changed"],true);
        link["flow"]=fresh.clone();environment::database(&root,&storage,"links-save",Some(&link)).unwrap();
        let saved=environment::database(&root,"","links-get",None).unwrap();
        assert_eq!(saved[0]["flow"]["delivered"],fresh["delivered"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_transfer_requires_play_and_a_difference() {
        let link=json!({"operation":"sync"});
        let mut flow=json!({"initialized":false,"phase":"ready","requested":false});
        assert!(!should_transfer(&link,&flow,true));
        flow["requested"]=json!(true);assert!(should_transfer(&link,&flow,true));assert!(!should_transfer(&link,&flow,false));
        flow["phase"]=json!("aligned");assert!(!should_transfer(&link,&flow,true));
        flow["phase"]=json!("ready");flow["requested"]=json!(false);
        flow["initialized"]=json!(true);assert!(!should_transfer(&link,&flow,true));
        flow["requested"]=json!(true);assert!(should_transfer(&link,&flow,true));
        assert!(!should_transfer(&json!({"operation":"send"}),&flow,true));
        flow["phase"]=json!("conflict");assert!(!should_transfer(&link,&flow,true));
    }
    #[test]
    fn completion_survives_restart_and_never_resurrects_a_deleted_generation() {
        let root=std::env::temp_dir().join(format!("oaclis-link-flow-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&root).unwrap();std::fs::write(root.join("pairing_ed25519.pub"),"ssh-ed25519 fixture").unwrap();
        let owner=identity(&root).unwrap();
        let mut link=json!({"entry":{"id":"chat","name":"Chat"},"source":"master","target":"peer","operation":"send","flow":initial(&owner)});
        let storage=key(&link).unwrap();let old=link["flow"]["generation"].clone();
        environment::database(&root,&storage,"links-save",Some(&link)).unwrap();
        finish(&root,&storage,&old,true,"").unwrap();
        let links=environment::database(&root,"","links-get",None).unwrap();
        assert_eq!(links[0]["flow"]["phase"],"transferred");assert_eq!(links[0]["transfer_pending"],false);
        recover(&root).unwrap();assert_eq!(environment::database(&root,"","links-get",None).unwrap(),links);
        environment::database(&root,&storage,"links-delete",None).unwrap();finish(&root,&storage,&old,true,"").unwrap();
        assert_eq!(environment::database(&root,"","links-get",None).unwrap(),json!([]));
        link["flow"]=initial(&owner);environment::database(&root,&storage,"links-save",Some(&link)).unwrap();
        finish(&root,&storage,&old,true,"").unwrap();
        assert_eq!(environment::database(&root,"","links-get",None).unwrap()[0]["flow"]["initialized"],false);
        let mut foreign=link["flow"].clone();foreign["executor"]=json!("other");foreign["phase"]=json!("transferring");save(&root,&link,foreign).unwrap();
        recover(&root).unwrap();assert_eq!(environment::database(&root,"","links-get",None).unwrap()[0]["flow"]["phase"],"transferring");
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn manual_direction_follows_the_newer_copy_and_respects_one_way() {
        let mut link=json!({"entry":{"id":"chat"},"source":"master","target":"peer","operation":"sync","scopes":{"chat":true,"paths":true,"apps":true}});
        let flow=json!({"phase":"ready","newer":"target","generation":"one"});
        let run=request(&link,&flow).unwrap();
        assert_eq!(run["source_device_id"],"peer");assert_eq!(run["device_id"],"master");assert_eq!(run["operation"],"send");
        link["operation"]=json!("send");assert!(request(&link,&flow).is_err());
        assert!(request(&link,&json!({"phase":"conflict"})).is_err());
    }
}
