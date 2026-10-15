// automation.rs v1.12.280
//! Compare links periodically; transfers always require explicit Play.
use crate::{devices, environment, link_flow, peers, transfer_job};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, Mutex}, thread, time::{Duration, Instant}};

fn eligible(link: &Value, executor: &str) -> bool {
    link["flow"]["executor"] == executor && (link["flow"]["initialized"] != true || link["operation"] == "sync")
}

pub(crate) fn conflict(value: &Value) -> bool {
    value == "conflict" || value.as_object().is_some_and(|v| v.values().any(conflict))
        || value.as_array().is_some_and(|v| v.iter().any(conflict))
}

pub(crate) fn start(codex: PathBuf, config: PathBuf, job: Arc<Mutex<Value>>, connections: Arc<Mutex<Value>>, discovery: peers::Peers, transfers: bool) {
    thread::spawn(move || {
        let mut checked: HashMap<String, Instant> = HashMap::new();
        loop {
            let snapshot = peers::snapshot(&discovery);
            let local = snapshot["local_id"].as_str().unwrap_or("");
            let online = snapshot["peers"].as_array().cloned().unwrap_or_default();
            let status: Value = std::fs::read(config.join("shared-status.json")).ok().and_then(|s|serde_json::from_slice(&s).ok()).unwrap_or(Value::Null);
            let pairing: Value = std::fs::read(config.join("pairing.json")).ok().and_then(|s|serde_json::from_slice(&s).ok()).unwrap_or(Value::Null);
            let recent = std::fs::metadata(config.join("shared-status.json")).and_then(|m|m.modified()).ok().and_then(|t|t.elapsed().ok()).is_some_and(|age|age < Duration::from_secs(30));
            let settings: Value = std::fs::read(config.join("settings.json")).ok().and_then(|s|serde_json::from_slice(&s).ok()).unwrap_or(Value::Null);
            let interval = Duration::from_secs(settings["chat_match_interval_seconds"].as_u64().unwrap_or(60).clamp(10,3600));
            if let Ok(all_devices) = devices::load(config.join("devices.json")) {
                for device in &all_devices {
                    let approved = pairing["approved"].as_object().is_some_and(|all|all.values().any(|v|v["device_id"] == device.id));
                    let present = online.iter().any(|p|p["device_id"] == device.id && p["connected"] == true)
                        || approved && recent && status["errors"].is_object() && status["errors"][&device.id].is_null();
                    let blocked = recent && (!status["errors"][&device.id].is_null() || status["runtime"][&device.id]["available"] == false);
                    connections.lock().unwrap()[&device.id] = json!(if present && !blocked {"online"} else {"offline"});
                }
                if job.lock().unwrap()["status"] != "running" {
                    if let Ok(links) = environment::database(&config,"","links-get",None) {
                        for link in links.as_array().into_iter().flatten().filter(|link|eligible(link,local)) {
                            let Ok(key) = link_flow::key(link) else {continue;};
                            let remote = if link["source"] == "master" {&link["target"]} else {&link["source"]};
                            let Some(device) = all_devices.iter().find(|d|remote == &json!(d.id)) else {continue;};
                            let mut flow = link["flow"].clone();
                            if connections.lock().unwrap()[&device.id] != "online" {
                                if flow["phase"] != "offline" {flow["phase"]=json!("offline");let _=link_flow::save(&config,link,flow);}
                                continue;
                            }
                            let requested = flow["requested"] == true;
                            if !requested && flow["phase"] != "offline" && checked.get(&key).is_some_and(|time|time.elapsed()<interval) {continue;}
                            checked.insert(key.clone(),Instant::now());
                            let mut flow = match link_flow::inspect(&config,&codex,device,link,requested) {
                                Ok(flow)=>flow,
                                Err(error)=>{
                                    flow["phase"]=json!("waiting");flow["error"]=json!(error.to_string());
                                    let _=link_flow::save(&config,link,flow);continue;
                                }
                            };
                            if !link_flow::should_transfer(link,&flow,transfers) {continue;}
                            let request=match link_flow::request(link,&flow) {Ok(v)=>v,Err(_)=>continue};
                            flow["phase"]=json!("transferring");flow["requested"]=json!(false);
                            if link_flow::save(&config,link,flow.clone()).is_err() {continue;}
                            match transfer_job::start(request,&codex,&config,&job) {
                                Ok(_)=>{
                                    while job.lock().unwrap()["status"]=="running" {thread::sleep(Duration::from_millis(250));}
                                    // Completion updates the persisted flow, even if the browser is closed.
                                },
                                Err(error)=>{
                                    let waiting=error.to_string().starts_with("Attendi");
                                    flow["phase"]=json!(if waiting {"waiting_inventory"} else {"failed"});
                                    flow["error"]=json!(error.to_string());
                                    let _=link_flow::save(&config,link,flow);
                                }
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_the_executor_matches_and_one_way_finishes() {
        let mut link=json!({"operation":"send","flow":{"executor":"a","initialized":false}});
        assert!(eligible(&link,"a"));assert!(!eligible(&link,"b"));
        link["flow"]["initialized"]=json!(true);assert!(!eligible(&link,"a"));
        link["operation"]=json!("sync");assert!(eligible(&link,"a"));
        assert!(conflict(&json!({"chat":"conflict"})));assert!(!conflict(&json!({"chat":"identical"})));
    }
}
