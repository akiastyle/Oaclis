// link_flow.rs v1.14.197
//! Link lifecycle: compare immutable proofs, persist intent, never transfer from a poll.
use crate::{devices::Device, environment, invalid, catalog};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{io, path::Path, time::{SystemTime, UNIX_EPOCH}};
const PROOF: &str = include_str!("../tools/chat_match.py");

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
        _ => ("conflict", None),
    }
}

pub(crate) fn inspect(config: &Path, codex: &Path, device: &Device, link: &Value, force: bool) -> io::Result<Value> {
    let id = link["entry"]["id"].as_str().ok_or_else(||invalid("Chat richiesta"))?;
    let local = catalog::local(codex)?;
    let remote = catalog::remote(device)?;
    let find = |list: &Value| list.as_array().and_then(|all|all.iter().find(|chat|chat["id"] == id)).cloned();
    let local = find(&json!(local));
    let remote = find(&remote["chats"]);
    let (source,target) = if link["source"] == "master" { (local,remote) } else { (remote,local) };
    let stamp = |chat: &Option<Value>| chat.as_ref().map(|chat|json!([chat["path"],chat["bytes"],chat["modified_ns"]])).unwrap_or(Value::Null);
    let signatures = json!({"source":stamp(&source),"target":stamp(&target)});
    let mut flow = link["flow"].clone();
    if !force && flow["signatures"] == signatures && flow["comparison"].is_string() {
        flow["phase"]=flow["comparison"].clone(); return Ok(flow);
    }
    let mut checking=flow.clone();checking["phase"]=json!("checking");save(config,link,checking)?;
    let prefix = source.as_ref().and_then(|s|s["bytes"].as_u64()).unwrap_or(0).min(target.as_ref().and_then(|t|t["bytes"].as_u64()).unwrap_or(0));
    let proof = |chat: &Option<Value>, incoming: bool| -> io::Result<Option<Value>> {
        let Some(chat) = chat else { return Ok(None); };
        let path = chat["path"].as_str().ok_or_else(||invalid("Archivio chat mancante"))?;
        let value = environment::python(if incoming {Some(device)} else {None}, PROOF, &[path,id,&prefix.to_string()])?;
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
    flow["comparison"] = json!(phase); flow["phase"] = json!(phase); flow["newer"] = json!(newer); flow["signatures"] = signatures;
    flow["proofs"] = json!({"source":a,"target":b}); flow["error"] = Value::Null;
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
      "link_key":key(link)?,"generation":flow["generation"]}))
}

pub(crate) fn should_transfer(link: &Value, flow: &Value, enabled: bool) -> bool {
    enabled && (flow["initialized"] != true || link["operation"] == "sync" && flow["requested"] == true)
        && matches!(flow["phase"].as_str(),Some("ready" | "aligned"))
}

pub(crate) fn finish(config: &Path, key: &str, generation: &Value, success: bool, error: &str) -> io::Result<()> {
    let links=environment::database(config,"","links-get",None)?;
    if let Some(link)=links.as_array().and_then(|all|all.iter().find(|link|self::key(link).is_ok_and(|k|k==key) && link["flow"]["generation"]==*generation)) {
        let mut flow=link["flow"].clone();
        flow["requested"]=json!(false);
        flow["phase"]=json!(if success {if link["operation"]=="sync" {"aligned"} else {"transferred"}} else {"failed"});
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
        assert_eq!(compare_proofs(Some(&a),Some(&json!({"sha256":"b","prefix":"other","bytes":30}))), ("conflict",None));
    }
    #[test]
    fn first_alignment_runs_once_then_waits_for_play() {
        let link=json!({"operation":"sync"});
        let mut flow=json!({"initialized":false,"phase":"ready","requested":false});
        assert!(should_transfer(&link,&flow,true));assert!(!should_transfer(&link,&flow,false));
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
