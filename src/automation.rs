// automation.rs v1.16.525
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

fn connection_states(config: &std::path::Path, snapshot: &Value) -> std::io::Result<Value> {
    let online = snapshot["peers"].as_array().cloned().unwrap_or_default();
    let runtime = crate::shared_settings::runtime(config)?;
    let pairing = crate::state_store::read_json(&config.join("pairing.json"), Value::Null)?;
    let mut states = json!({});
    for device in devices::load(config.join("devices.json"))? {
        let approved = pairing["approved"].as_object().is_some_and(|all| all.values().any(|v| v["device_id"] == device.id));
        let present = online.iter().any(|p| p["device_id"] == device.id && p["connected"] == true)
            || approved && runtime[&device.id]["available"] == true;
        let blocked = runtime[&device.id]["available"] == false;
        states[&device.id] = json!(if present && !blocked { "online" } else { "offline" });
    }
    Ok(states)
}

fn launch(active: &mut HashMap<String, thread::JoinHandle<()>>, device: &str, work: impl FnOnce() + Send + 'static) -> bool {
    active.retain(|_, worker| !worker.is_finished());
    if active.len() >= 4 || active.contains_key(device) { return false; }
    active.insert(device.into(), thread::spawn(work));
    true
}

pub(crate) fn start(codex: PathBuf, config: PathBuf, job: Arc<Mutex<Value>>, connections: Arc<Mutex<Value>>, discovery: peers::Peers, transfers: bool) {
    let monitor_config = config.clone();
    let monitor_connections = connections.clone();
    let monitor_discovery = discovery.clone();
    thread::spawn(move || loop {
        match connection_states(&monitor_config, &peers::snapshot(&monitor_discovery)) {
            Ok(states) => *monitor_connections.lock().unwrap() = states,
            Err(error) => eprintln!("Oaclis: disponibilita' device: {error}"),
        }
        thread::sleep(Duration::from_secs(1));
    });
    thread::spawn(move || {
        let mut checked: HashMap<String, Instant> = HashMap::new();
        let mut active: HashMap<String, thread::JoinHandle<()>> = HashMap::new();
        let transfer_start = Arc::new(Mutex::new(()));
        loop {
            active.retain(|_, worker| !worker.is_finished());
            let snapshot = peers::snapshot(&discovery);
            let local = snapshot["local_id"].as_str().unwrap_or("");
            let settings: Value = std::fs::read(config.join("settings.json")).ok().and_then(|s|serde_json::from_slice(&s).ok()).unwrap_or(Value::Null);
            let interval = Duration::from_secs(settings["chat_match_interval_seconds"].as_u64().unwrap_or(60).clamp(10,3600));
            if let Ok(all_devices) = devices::load(config.join("devices.json")) {
                if let Ok(links) = environment::database(&config,"","links-get",None) {
                    let mut groups: HashMap<String, Vec<Value>> = HashMap::new();
                    for link in links.as_array().into_iter().flatten().filter(|link|eligible(link,local)) {
                        let Ok(key) = link_flow::key(link) else {continue;};
                        let remote = if link["source"] == "master" {&link["target"]} else {&link["source"]};
                        let Some(device) = all_devices.iter().find(|d|remote == &json!(d.id)) else {continue;};
                        if active.contains_key(&device.id) || link["flow"]["phase"] == "transferring" { continue; }
                        if connections.lock().unwrap()[&device.id] != "online" {
                            if link["flow"]["phase"] != "offline" {
                                let mut flow = link["flow"].clone();flow["phase"]=json!("offline");let _=link_flow::save(&config,link,flow);
                            }
                            continue;
                        }
                        if link["flow"]["requested"] != true && link["flow"]["phase"] != "offline" && checked.get(&key).is_some_and(|time|time.elapsed()<interval) {continue;}
                        groups.entry(device.id.clone()).or_default().push(link.clone());
                    }
                    let mut ready: Vec<_> = all_devices.into_iter().filter_map(|device| groups.remove(&device.id).map(|links|(device,links))).collect();
                    ready.sort_by_key(|(_,links)| links.iter().filter_map(|link|link_flow::key(link).ok().and_then(|key|checked.get(&key).copied())).min());
                    let mut local_catalog = None;
                    for (device, links) in ready.into_iter().take(4-active.len()) {
                        let mut catalogs = crate::catalog::Catalogs::new();
                        if links.iter().any(|link|link["flow"]["requested"] != true || link["flow"]["phase"] != "ready") {
                            let cached = local_catalog.get_or_insert_with(|| crate::catalog::local_at(&codex,&config).map(Arc::new).map_err(|error|error.to_string()));
                            catalogs.insert("master".into(),cached.clone());
                        }
                        let keys: Vec<_> = links.iter().filter_map(|link|link_flow::key(link).ok()).collect();
                        let device_id = device.id.clone();
                        let config = config.clone();let codex = codex.clone();let job = job.clone();let connections = connections.clone();let transfer_start = transfer_start.clone();
                        if launch(&mut active,&device_id,move || {
                            for planned in links {
                                let Ok(key) = link_flow::key(&planned) else {continue;};
                                let Some(link) = crate::state_store::snapshot(&config).ok().and_then(|state|state.links.get(&key).cloned()) else {continue;};
                                if link["flow"]["generation"] != planned["flow"]["generation"] || !eligible(&link, planned["flow"]["executor"].as_str().unwrap_or("")) || link["flow"]["phase"] == "transferring" {continue;}
                                if connections.lock().unwrap()[&device.id] != "online" {break;}
                                let mut flow = link["flow"].clone();
                                let mut flow = if flow["requested"] == true && flow["phase"] == "ready" {flow} else {match link_flow::inspect(&config,&codex,&device,&link,&mut catalogs) {
                                    Ok(flow)=>flow,
                                    Err(error)=>{
                                        flow["phase"]=json!("waiting");flow["error"]=json!(error.to_string());
                                        let _=link_flow::save(&config,&link,flow);continue;
                                    }
                                }};
                                if !link_flow::should_transfer(&link,&flow,transfers) {continue;}
                                let Ok(_start) = transfer_start.try_lock() else {continue;};
                                if job.lock().unwrap()["status"] == "running" {continue;}
                                let request=match link_flow::request(&link,&flow) {Ok(v)=>v,Err(_)=>continue};
                                flow["phase"]=json!("transferring");flow["requested"]=json!(false);
                                if link_flow::save(&config,&link,flow.clone()).is_err() {continue;}
                                if let Err(error) = transfer_job::start(request,&codex,&config,&job) {
                                    let busy=job.lock().unwrap()["status"]=="running";
                                    let waiting=error.to_string().starts_with("Attendi");
                                    flow["phase"]=json!(if busy {"ready"} else if waiting {"waiting_inventory"} else {"failed"});
                                    flow["requested"]=json!(busy);flow["error"]=if busy {Value::Null} else {json!(error.to_string())};
                                    let _=link_flow::save(&config,&link,flow);
                                }
                            }
                        }) {
                            for key in keys {checked.insert(key,Instant::now());}
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
    #[test]
    fn availability_uses_each_peers_fresh_result_without_transfer_state() {
        use std::{fs, time::{SystemTime, UNIX_EPOCH}};
        let path = std::env::temp_dir().join(format!("oaclis-monitor-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("devices.json"),json!([{"id":"peer","name":"Peer","ssh_alias":"alias"}]).to_string()).unwrap();
        fs::write(path.join("pairing.json"),json!({"approved":{"stable":{"device_id":"peer"}}}).to_string()).unwrap();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        fs::write(path.join("shared-status.json"),json!({"runtime":{"peer":{"available":true,"received_at":now}}}).to_string()).unwrap();
        assert_eq!(connection_states(&path,&json!({})).unwrap()["peer"],"online");
        fs::write(path.join("shared-status.json"),json!({"runtime":{"peer":{"available":false,"received_at":now}}}).to_string()).unwrap();
        let lan=json!({"peers":[{"device_id":"peer","connected":true}]});
        assert_eq!(connection_states(&path,&lan).unwrap()["peer"],"offline");
        fs::write(path.join("shared-status.json"),json!({"runtime":{"peer":{"available":true,"received_at":0}}}).to_string()).unwrap();
        assert_eq!(connection_states(&path,&json!({})).unwrap()["peer"],"offline");
        assert_eq!(connection_states(&path,&lan).unwrap()["peer"],"online");
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn device_comparisons_overlap_without_duplicates_and_have_four_slots() {
        use std::sync::{Condvar, mpsc};
        let mut active = HashMap::new();
        let gate = Arc::new((Mutex::new(false), Condvar::new()));
        let (done, results) = mpsc::channel();
        let slow_gate = gate.clone();
        assert!(launch(&mut active,"slow",move || {
            let (lock, ready) = &*slow_gate;
            let _ = ready.wait_timeout_while(lock.lock().unwrap(),Duration::from_secs(3),|released|!*released).unwrap();
        }));
        assert!(launch(&mut active,"fast",move || {done.send("fast").unwrap();}));
        let fast_finished = results.recv_timeout(Duration::from_millis(500));
        active.remove("fast").unwrap().join().unwrap();
        let duplicate = launch(&mut active,"slow",||{});
        let mut started = 0;
        for id in ["second","third","fourth"] {
            let gate = gate.clone();
            started += usize::from(launch(&mut active,id,move || {
                let (lock,ready)=&*gate;
                let _ = ready.wait_timeout_while(lock.lock().unwrap(),Duration::from_secs(3),|released|!*released).unwrap();
            }));
        }
        let overflow = launch(&mut active,"fifth",||{});
        *gate.0.lock().unwrap() = true;
        gate.1.notify_all();
        for (_,worker) in active.drain() {worker.join().unwrap();}
        assert_eq!(fast_finished.unwrap(),"fast");
        assert!(!duplicate,"Two workers for the same device");
        assert_eq!(started,3);
        assert!(!overflow,"More than four active device comparisons");
    }

}
