// shared_settings.rs v1.7.282
//! Native settings replication over existing, mutually approved SSH connections.
use crate::{
    invalid, shared_records as records,
    state_store::{self, read_json, save_changed},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
    thread,
    time::Duration,
};

pub fn start(home: PathBuf) {
    thread::spawn(move || {
        loop {
            if let Err(error) = sync(&home.join(".config/oaclis")) {
                eprintln!("Oaclis: impostazioni condivise: {error}");
            }
            thread::sleep(Duration::from_secs(5));
        }
    });
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
    }
    let devices: Vec<Value> =
        serde_json::from_value(read_json(&config.join("devices.json"), json!([]))?)?;
    let positions = positions(config)?;
    let link_positions = read_positions(config, "link-positions.json")?;
    let local_name = std::env::var("HOSTNAME").unwrap_or_else(|_| "Oaclis".into());
    let master = std::fs::read_to_string(config.join("role"))
        .unwrap_or_default()
        .trim()
        == "master";
    let (response, devices, positions, link_positions) = state_store::change(config, |state| {
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
            &positions,
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
        records::validate(&values)?;
        let (devices, positions, link_positions) =
            records::apply(state, &values, &mut mapping, &devices, &id)?;
        state.shared = json!({"records":values,"mapping":mapping,"baseline":{}});
        state.shared["baseline"] = records::snapshot(
            state,
            &mapping,
            &devices,
            &positions,
            &link_positions,
            &local_name,
        );
        Ok((
            json!({"schema":1,"sender":id,"records":values}),
            devices,
            positions,
            link_positions,
        ))
    })?;
    save_changed(&config.join("devices.json"), &devices)?;
    save_changed(&config.join("positions.json"), &positions)?;
    save_changed(&config.join("link-positions.json"), &link_positions)?;
    Ok(response)
}
fn sync(config: &Path) -> io::Result<()> {
    let mut packet = exchange(config, None)?;
    let devices = crate::devices::load(config.join("devices.json"))?;
    let pairing = read_json(&config.join("pairing.json"), json!({}))?;
    let mut errors = json!({});
    let mut runtime = json!({});
    for (peer, approval) in pairing["approved"].as_object().into_iter().flatten() {
        let Some(device) = devices.iter().find(|d| approval["device_id"] == d.id) else {
            continue;
        };
        let result = (|| -> io::Result<Value> {
            let response = crate::state_transport::remote(
                device,
                &json!({"kind":"shared-exchange","value":packet}),
            )?;
            if response["sender"] != peer.as_str() {
                return Err(invalid("Identita' SSH diversa da quella autorizzata"));
            }
            if !response["runtime"]["active"].is_boolean()
                && response["runtime"]["available"] != false
            {
                return Err(invalid("Stato Oaclis non valido"));
            }
            let status = response["runtime"].clone();
            packet = exchange(config, Some(&response))?;
            Ok(status)
        })();
        match result {
            Ok(value) => runtime[&device.id] = value,
            Err(error) => {
                errors[&device.id] = json!(error.to_string());
                runtime[&device.id] = json!({"available":false,"error":error.to_string(),"observed_at":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64});
            }
        }
    }
    save_changed(
        &config.join("shared-status.json"),
        &json!({"errors":errors,"runtime":runtime}),
    )
}
