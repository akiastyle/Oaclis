//! Saved links run independently of the browser, through the same transfer entry point.
use crate::{devices, environment, web};
use serde_json::{Value, json};
use std::{collections::HashMap, path::PathBuf, sync::{Arc, Mutex}, thread, time::{Duration, Instant}};

fn eligible(link: &Value) -> bool {
    // The source owns recurring synchronization; only locally requested receives run here.
    if link["operation"] == "sync" {
        link["source"] == "master"
    } else {
        link["transfer_pending"] == true
    }
}

fn due(previous: Option<&(String, Instant, bool)>, signature: &str) -> bool {
    !previous.is_some_and(|(old, time, failed)| old == signature && (*failed || time.elapsed() < Duration::from_secs(60)))
}
fn conflict(value: &Value) -> bool {
    value == "conflict" || value.as_object().is_some_and(|v| v.values().any(conflict))
        || value.as_array().is_some_and(|v| v.iter().any(conflict))
}

pub(crate) fn start(codex: PathBuf, config: PathBuf, job: Arc<Mutex<Value>>, connections: Arc<Mutex<Value>>) {
    thread::spawn(move || {
        let mut attempts: HashMap<String, (String, Instant, bool)> = HashMap::new();
        let mut online_before: HashMap<String, bool> = HashMap::new();
        let mut epochs: HashMap<String, u64> = HashMap::new();
        loop {
            if let Ok(devices) = devices::load(config.join("devices.json")) {
                for device in devices {
                    let online = crate::ssh::run(&device, "true").is_ok();
                    connections.lock().unwrap()[&device.id] = json!(if online { "online" } else { "offline" });
                    if online && !online_before.get(&device.id).copied().unwrap_or(false) {
                        *epochs.entry(device.id.clone()).or_default() += 1;
                    }
                    online_before.insert(device.id, online);
                }
            }
            if job.lock().unwrap()["status"] != "running" {
                if let Ok(links) = environment::database(&config, "", "links-get", None) {
                    for link in links.as_array().into_iter().flatten().filter(|link| eligible(link)) {
                        let source = link["source"].as_str().unwrap_or("");
                        let target = link["target"].as_str().unwrap_or("");
                        let remote = if source == "master" { target } else { source };
                        if !online_before.get(remote).copied().unwrap_or(false) { continue; }
                        let id = link["entry"]["id"].as_str().unwrap_or("");
                        let Ok(key) = environment::key(source, id) else { continue; };
                        let Ok(analysis) = environment::database(&config, &key, "analysis-get", None) else { continue; };
                        if analysis["status"] != "ready" { continue; }
                        let attempt_key = format!("{key}:{target}");
                        let signature = format!("{}:{}:{}", epochs.get(remote).unwrap_or(&0), analysis["revision"], link["scopes"]);
                        if !due(attempts.get(&attempt_key), &signature) { continue; }
                        let request = json!({"source_device_id":source,"device_id":target,"operation":link["operation"],"chat_id":id,"scopes":link["scopes"]});
                        match web::start_transfer(request.clone(), &codex, &config, &job) {
                            Ok(_) => {
                                while job.lock().unwrap()["status"] == "running" { thread::sleep(Duration::from_millis(250)); }
                                let failed = { let state = job.lock().unwrap(); state["status"] == "failed" || conflict(&state["results"]) };
                                attempts.insert(attempt_key, (signature, Instant::now(), failed));
                                if !failed && link["transfer_pending"] == true {
                                    // Re-read: do not resurrect a deleted link or overwrite edits made during transfer.
                                    if let Ok(current) = environment::database(&config, "", "links-get", None) {
                                        if let Some(mut saved) = current.as_array().and_then(|all| all.iter().find(|v| v["source"] == link["source"] && v["target"] == link["target"] && v["entry"]["id"] == link["entry"]["id"])).cloned() {
                                            saved["transfer_pending"] = json!(false);
                                            let mut pair = [source, target]; pair.sort();
                                            let storage_key = format!("{id}:{}:{}",pair[0],pair[1]);
                                            let _ = environment::database(&config, &storage_key, "links-save", Some(&saved));
                                        }
                                    }
                                }
                            }
                            Err(error) => {
                                attempts.insert(attempt_key, (signature, Instant::now(), true));
                                let mut failed = request;
                                failed["status"] = json!("failed"); failed["message"] = json!(error.to_string());
                                let _ = environment::database(&config, &key, "transfer-save", Some(&failed));
                            }
                        }
                    }
                }
            }
            thread::sleep(Duration::from_secs(10));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_wait_for_a_change_or_reconnection_after_failure() {
        let previous = ("connection-1:revision-1".to_string(), Instant::now(), true);
        assert!(!due(Some(&previous), "connection-1:revision-1"));
        assert!(due(Some(&previous), "connection-2:revision-1"));
        assert!(due(Some(&previous), "connection-1:revision-2"));
        let completed = (previous.0.clone(), Instant::now() - Duration::from_secs(61), false);
        assert!(due(Some(&completed), &completed.0));
        assert!(conflict(&json!({"chat":"conflict"})));
        assert!(!conflict(&json!({"chat":"identical"})));
    }
    #[test]
    fn only_sync_repeats_after_the_first_transfer() {
        for operation in ["copy", "send", "sync"] {
            assert!(eligible(&json!({"source":"master","operation":operation,"transfer_pending":true})));
            assert_eq!(eligible(&json!({"source":"master","operation":operation,"transfer_pending":false})), operation == "sync");
        }
        assert!(!eligible(&json!({"source":"peer","operation":"sync","transfer_pending":false})));
        assert!(!eligible(&json!({"source":"peer","operation":"sync","transfer_pending":true})));
        assert!(eligible(&json!({"source":"peer","operation":"copy","transfer_pending":true})));
    }
}
