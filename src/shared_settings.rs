// shared_settings.rs v1.13.644
//! Native settings replication over existing, mutually approved SSH connections.
use crate::{
    invalid, shared_records as records,
    state_store::{self, read_json, save_changed},
};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn start(home: PathBuf) {
    thread::spawn(move || {
        let config = home.join(".config/oaclis");
        let (send, receive) = mpsc::channel::<(String, String, io::Result<Value>)>();
        let mut active = BTreeSet::new();
        let mut checked = BTreeMap::new();
        loop {
            if let Ok((peer, device, result)) = receive.recv_timeout(Duration::from_secs(1)) {
                active.remove(&peer);
                checked.insert(peer, Instant::now());
                if let Err(error) = publish_runtime(&config, &device, result) {
                    eprintln!("Oaclis: stato condiviso: {error}");
                }
            }
            let ready = (|| -> io::Result<_> {
                let devices = crate::devices::load(config.join("devices.json"))?;
                let pairing = read_json(&config.join("pairing.json"), json!({}))?;
                crate::jobs::retain(&pairing["approved"].as_object().into_iter().flat_map(|peers|peers.values()).filter_map(|peer|peer["device_id"].as_str().map(String::from)).collect());
                let mut ready = Vec::new();
                let mut retained = active.clone();
                for (peer, approval) in pairing["approved"].as_object().into_iter().flatten() {
                    retained.insert(peer.clone());
                    let Some(device) = devices.iter().find(|d| approval["device_id"] == d.id)
                    else {
                        continue;
                    };
                    if !active.contains(peer)
                        && checked
                            .get(peer)
                            .is_none_or(|time: &Instant| time.elapsed() >= Duration::from_secs(5))
                    {
                        ready.push((checked.get(peer).copied(), peer.clone(), device.clone()));
                    }
                }
                checked.retain(|peer, _| retained.contains(peer));
                ready.sort_by_key(|(time, _, _)| *time);
                Ok(ready)
            })();
            match ready {
                Ok(ready) => {
                    for (_, peer, device) in ready.into_iter().take(4 - active.len()) {
                        active.insert(peer.clone());
                        let config = config.clone();
                        let send = send.clone();
                        thread::spawn(move || {
                            let result = sync_peer(&config, &peer, &device);
                            let _ = send.send((peer, device.id, result));
                        });
                    }
                }
                Err(error) => eprintln!("Oaclis: impostazioni condivise: {error}"),
            }
        }
    });
}
// Runtime telemetry is volatile; each peer keeps its own 30-second freshness.
static RUNTIME:OnceLock<Mutex<BTreeMap<PathBuf,Value>>>=OnceLock::new();
pub(crate) fn runtime(config:&Path)->io::Result<Value> {
    let mut cache=RUNTIME.get_or_init(Default::default).lock().map_err(|_|io::Error::other("Stato device non disponibile"))?;
    let states=cache.entry(config.into()).or_insert_with(||json!({}));
    let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    states.as_object_mut().unwrap().retain(|_,status|status["received_at"].as_u64().is_some_and(|time|now.saturating_sub(time)<30_000));
    Ok(states.clone())
}
pub(crate) fn publish_runtime(config:&Path,device:&str,result:io::Result<Value>)->io::Result<()> {
    let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut value=result.unwrap_or_else(|error|json!({"available":false,"error":error.to_string(),"observed_at":now}));
    value["received_at"]=json!(now);
    let mut cache=RUNTIME.get_or_init(Default::default).lock().map_err(|_|io::Error::other("Stato device non disponibile"))?;
    let states=cache.entry(config.into()).or_insert_with(||json!({}));
    let previous=&states[device];
    if value["device_name"].is_null(){value["device_name"]=previous["device_name"].clone();}
    if value["update_target"].is_null(){value["update_target"]=previous["update_target"].clone();}
    if value["update_target"].is_string() {
        let outcome=(if value["available"]==false {"temporaneamente non raggiungibile"}
            else if value["build"]["revision"]==value["update_target"] && value["update"]["status"]!="running" {"aggiornamento completato e verificato"}
            else if value["update"]["status"]=="failed" {"aggiornamento non riuscito"}
            else {value["update"]["phase"].as_str().unwrap_or("aggiornamento in corso")}).to_owned();
        if previous["update_log"]!=outcome {
            crate::state_transport::log(format!("Aggiornamento · {} · {outcome}",value["device_name"].as_str().unwrap_or("Dispositivo")));
        }
        value["update_log"]=json!(outcome);
        if outcome=="aggiornamento completato e verificato" || outcome=="aggiornamento non riuscito" {value.as_object_mut().unwrap().remove("update_target");}
    }
    states[device]=value;
    Ok(())
}
pub fn read_positions(config: &Path, file: &str) -> io::Result<Value> {
    read_json(&config.join(file), json!({}))
}
pub fn positions(config: &Path) -> io::Result<Value> {
    read_positions(config, "positions.json")
}
pub fn write_positions(config: &Path, operation: &str, value: &Value) -> io::Result<()> {
    let owner = state_store::store(config)?;
    let _guard = owner
        .shared_lock
        .lock()
        .map_err(|_| io::Error::other("Impostazioni non disponibili"))?;
    match operation {
        "positions" => save_changed(&config.join("positions.json"), value),
        "link-position" => {
            for key in ["id", "source", "target"] {
                if !value[key].as_str().is_some_and(records::identifier) {
                    return Err(invalid("Collegamento non valido"));
                }
            }
            if value["source"] == value["target"] {
                return Err(invalid("Collegamento non valido"));
            }
            let point = json!({"x":value["x"],"y":value["y"]});
            records::validate_point(&point, true)?;
            let mut pair = [
                value["source"].as_str().unwrap(),
                value["target"].as_str().unwrap(),
            ];
            pair.sort();
            let mut points = read_positions(config, "link-positions.json")?;
            points[format!("{}:{}:{}", value["id"].as_str().unwrap(), pair[0], pair[1])] = point;
            save_changed(&config.join("link-positions.json"), &points)
        }
        _ => Err(invalid("Operazione posizioni non valida")),
    }
}
pub fn save_positions(config: &Path, value: &Value) -> io::Result<()> {
    let entries = value
        .as_object()
        .filter(|v| v.len() <= 1000)
        .ok_or_else(|| invalid("Posizioni non valide"))?;
    for (id, point) in entries {
        if !crate::devices::identifier(id) {
            return Err(invalid("Posizioni non valide"));
        }
        records::validate_point(point, false)?;
    }
    write_positions(config, "positions", value)
}
pub(crate) fn exchange(config: &Path, packet: Option<&Value>) -> io::Result<Value> {
    let owner = state_store::store(config)?;
    let _guard = owner
        .shared_lock
        .lock()
        .map_err(|_| io::Error::other("Impostazioni non disponibili"))?;
    let id = state_store::identity(config)?;
    if let Some(packet) = packet {
        let pairing = read_json(&config.join("pairing.json"), json!({}))?;
        let sender = packet["sender"]
            .as_str()
            .ok_or_else(|| invalid("Identita' mittente mancante"))?;
        if packet["schema"] != 1 || !pairing["approved"][sender]["device_id"].is_string() {
            return Err(invalid(
                "Dispositivo non autorizzato a sincronizzare le impostazioni",
            ));
        }
        records::validate(&packet["records"])?;
        crate::chat_deletion::validate(&packet["deletions"])?;
    }
    let local_name = std::env::var("HOSTNAME").unwrap_or_else(|_| "Oaclis".into());
    let master = std::fs::read_to_string(config.join("role"))
        .unwrap_or_default()
        .trim()
        == "master";
    state_store::change_with_files(config, |state, files| {
        if let Some(packet)=packet {crate::chat_deletion::merge(state,&packet["deletions"]);}
        crate::chat_deletion::validate(&state.shared["deletions"])?;
        crate::chat_deletion::purge(state,files);
        let deletions=state.shared.get("deletions").cloned().unwrap_or(json!({}));
        let devices: Vec<Value> =
            serde_json::from_value(read_json(&config.join("devices.json"), json!([]))?)?;
        let saved_positions = positions(config)?;
        let link_positions = read_positions(config, "link-positions.json")?;

        let mut mapping = state_store::mapping(config, &state.shared)?;
        for device in &devices {
            if let Some(alias) = device["id"].as_str() {
                mapping
                    .entry(alias.into())
                    .or_insert(format!("manual-{id}-{alias}"));
            }
        }
        let current = records::snapshot(
            state,
            &mapping,
            &devices,
            &saved_positions,
            &link_positions,
            &local_name,
        );
        let baseline = state.shared.get("baseline").cloned().unwrap_or(json!({}));
        let mut values = state.shared.get("records").cloned().unwrap_or(json!({}));
        let mut clock = values
            .as_object()
            .into_iter()
            .flatten()
            .map(|(_, v)| records::clock(v).0)
            .max()
            .unwrap_or(if master { 1000000 } else { 0 });
        let keys: BTreeSet<_> = current
            .as_object()
            .unwrap()
            .keys()
            .chain(baseline.as_object().into_iter().flat_map(|v| v.keys()))
            .cloned()
            .collect();
        for key in keys {
            if current.get(&key) != baseline.get(&key) {
                clock += 1;
                values[&key] =
                    json!({"clock":[clock,id],"value":current.get(&key).unwrap_or(&Value::Null)});
            }
        }
        if let Some(packet) = packet {
            for (key, incoming) in packet["records"].as_object().unwrap() {
                if values
                    .get(key)
                    .is_none_or(|old| records::clock(incoming) > records::clock(old))
                {
                    values[key] = incoming.clone();
                }
            }
        }
        crate::chat_deletion::purge_records(&mut values,&crate::chat_deletion::ids(state));
        records::validate(&values)?;
        let (devices, mut positions, link_positions) =
            records::apply(state, &values, &mut mapping, &devices, &id)?;
        for (key, point) in saved_positions.as_object().into_iter().flatten() {
            if key.strip_prefix("mobile-").is_some_and(|id| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit())) {
                positions[key] = point.clone();
            }
        }
        state.shared = Arc::new(json!({"records":values,"mapping":mapping,"baseline":{},"deletions":deletions}));
        let baseline = records::snapshot(
            state,
            &mapping,
            &devices,
            &positions,
            &link_positions,
            &local_name,
        );
        Arc::make_mut(&mut state.shared)["baseline"] = baseline;
        files["devices.json"] = json!(devices);
        files["positions.json"] = positions;
        files["link-positions.json"] = link_positions;
        let outgoing:serde_json::Map<String,Value>=state.shared["deletions"].as_object().into_iter().flatten()
            .filter(|(_,entry)|entry["global"]!=false).map(|(id,entry)|(id.clone(),entry.clone())).collect();
        Ok(json!({"schema":1,"sender":id,"records":values,"deletions":outgoing}))
    })
}
fn sync_peer(config: &Path, peer: &str, device: &crate::devices::Device) -> io::Result<Value> {
    let packet = exchange(config, None)?;
    let response =
        crate::state_transport::remote(device, &json!({"kind":"shared-exchange","value":packet}))?;
    if response["sender"] != peer {
        return Err(invalid("Identita' SSH diversa da quella autorizzata"));
    }
    if !response["runtime"]["active"].is_boolean() && response["runtime"]["available"] != false {
        return Err(invalid("Stato Oaclis non valido"));
    }
    exchange(config, Some(&response))?;
    let _=crate::jobs::receive(device,&response["jobs"]);
    let mut runtime=response["runtime"].clone();
    runtime.as_object_mut().unwrap().remove("update_target");runtime.as_object_mut().unwrap().remove("update_log");
    runtime["device_name"]=json!(device.name);Ok(runtime)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn remote_update_is_complete_only_when_the_expected_build_returns() {
        let root=std::env::temp_dir().join(format!("oaclis-update-log-{}",std::process::id()));
        publish_runtime(&root,"kj",Ok(json!({"device_name":"KJetson","available":true,"update_target":"expected","update":{"status":"running"},"build":{"revision":"old"}}))).unwrap();
        assert_eq!(runtime(&root).unwrap()["kj"]["update_log"],"aggiornamento in corso");
        publish_runtime(&root,"kj",Err(io::Error::other("offline"))).unwrap();
        assert!(runtime(&root).unwrap()["kj"]["update_target"].is_string());
        publish_runtime(&root,"kj",Ok(json!({"available":true,"update":{"status":"running","phase":"Compilazione e installazione"},"build":{"revision":"old"}}))).unwrap();
        assert_eq!(runtime(&root).unwrap()["kj"]["update_log"],"Compilazione e installazione");
        publish_runtime(&root,"kj",Ok(json!({"available":true,"update":{},"build":{"revision":"expected"}}))).unwrap();
        let done=runtime(&root).unwrap();assert_eq!(done["kj"]["update_log"],"aggiornamento completato e verificato");assert!(done["kj"].get("update_target").is_none());
    }
    #[test]
    fn peer_results_are_published_independently_and_expire_individually() {
        let path = std::env::temp_dir().join(format!(
            "oaclis-peer-status-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&path).unwrap();
        publish_runtime(
            &path,
            "fast",
            Ok(json!({"available":true,"active":false,"observed_at":1})),
        )
        .unwrap();
        let first=runtime(&path).unwrap()["fast"].clone();
        publish_runtime(&path, "slow", Err(io::Error::other("timeout"))).unwrap();
        let saved=runtime(&path).unwrap();
        assert_eq!(saved["fast"],first);
        assert!(!path.join("shared-status.json").exists());
        assert_eq!(runtime(&path).unwrap()["fast"]["available"], true);
        assert_eq!(runtime(&path).unwrap()["slow"]["available"], false);
        RUNTIME.get().unwrap().lock().unwrap().get_mut(&path).unwrap()["slow"]["received_at"]=json!(0);
        publish_runtime(&path, "fast", Ok(json!({"available":true,"active":true}))).unwrap();
        assert!(runtime(&path).unwrap().get("slow").is_none());
        publish_runtime(&path, "slow", Ok(json!({"available":true,"active":false}))).unwrap();
        assert!(runtime(&path).unwrap()["slow"].get("error").is_none());
        assert_eq!(runtime(&path).unwrap()["slow"]["available"], true);
        std::fs::remove_dir_all(path).unwrap();
    }
}
