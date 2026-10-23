// shared_records.rs v1.19.873
//! Shared settings retain the existing clocks, stable identities and ownership boundaries.
use std::sync::Arc;
use crate::{
    invalid,
    state_store::{MODULES, State, chat_record, scopes, set_scopes, strip_entries},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io};

pub(crate) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 200
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
pub(crate) fn validate(records: &Value) -> io::Result<()> {
    let records = records
        .as_object()
        .filter(|v| v.len() <= 10000)
        .ok_or_else(|| invalid("Impostazioni condivise non valide"))?;
    for (key, record) in records {
        let parts: Vec<_> = key.split(':').collect();
        let kind = parts[0];
        let size = match kind {
            "link" | "linkposition" => 4,
            "chatdata" => 3,
            "device" | "chat" | "preference" | "position" => 2,
            _ => 0,
        };
        if parts.len() != size || parts[1..].iter().any(|p| !identifier(p)) {
            return Err(invalid("Oggetto condiviso non valido"));
        }
        let clock = record["clock"]
            .as_array()
            .filter(|v| v.len() == 2)
            .ok_or_else(|| invalid("Revisione condivisa non valida"))?;
        if !clock[0].as_u64().is_some_and(|n| n < (1 << 53))
            || !clock[1].as_str().is_some_and(identifier)
        {
            return Err(invalid("Revisione condivisa non valida"));
        }
        let value = &record["value"];
        if value.is_null() {
            continue;
        }
        let object = value
            .as_object()
            .ok_or_else(|| invalid("Valore condiviso non valido"))?;
        if kind == "chatdata"
            && object
                .iter()
                .any(|(k, v)| !MODULES.contains(&k.as_str()) || !v.is_object())
        {
            return Err(invalid("Dati sorgenti non validi"));
        }
        if kind == "device" && !valid_text(&value["name"], 160, true) {
            return Err(invalid("Nome dispositivo non valido"));
        }
        if kind == "device" || kind == "chat" {
            let appearance = if kind == "device" {
                value.get("appearance").cloned().unwrap_or(json!({}))
            } else {
                value.clone()
            };
            let fields = appearance
                .as_object()
                .ok_or_else(|| invalid("Aspetto non valido"))?;
            if fields
                .iter()
                .any(|(k, v)| !["label", "icon", "color", "url"].contains(&k.as_str()) || !v.is_string())
            {
                return Err(invalid("Aspetto non valido"));
            }
            crate::appearance::Appearance {url:appearance["url"].as_str().unwrap_or("").into(),..Default::default()}.validate()?;
            let label = appearance.get("label").cloned().unwrap_or(json!(""));
            if !valid_text(&label, 320, false) {
                return Err(invalid("Etichetta non valida"));
            }
            // Keep the transport's existing icon limit; local editors validate their smaller upload limit.
            let icon = appearance["icon"].as_str().unwrap_or("");
            let color = appearance["color"].as_str().unwrap_or("");
            let predefined = [
                "", "chat", "code", "folder", "desktop", "laptop", "server", "terminal", "globe",
                "cloud", "box", "database", "git", "book", "star", "bolt", "shield", "bug",
                "settings", "cpu", "rocket", "link", "image", "wrench", "layers",
            ];
            if icon.len() > 2 * 1024 * 1024
                || !predefined.contains(&icon)
                    && !icon
                        .strip_prefix("data:image/png;base64,")
                        .is_some_and(|s| {
                            !s.is_empty()
                                && s.bytes()
                                    .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
                        })
            {
                return Err(invalid("Icona non valida"));
            }
            if !color.is_empty()
                && !(color.len() == 7
                    && color.starts_with('#')
                    && color[1..].bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Err(invalid("Colore non valido"));
            }
        }
        if matches!(kind, "preference" | "link")
            && value.get("scopes").is_some()
            && !value["scopes"].as_object().is_some_and(|v| {
                v.len() == 3
                    && MODULES
                        .iter()
                        .all(|n| v.get(*n).is_some_and(Value::is_boolean))
            })
        {
            return Err(invalid("Categorie non valide"));
        }
        if kind == "preference"
            && ["hidden", "list_hidden"]
                .iter()
                .any(|n| value.get(*n).is_some_and(|v| !v.is_boolean()))
        {
            return Err(invalid("Visibilita' non valida"));
        }
        if kind == "link" {
            if !valid_text(&value["entry"]["name"], 320, false)
                || !valid_text(value.get("label").unwrap_or(&json!("")), 160, false)
            {
                return Err(invalid("Titolo collegamento non valido"));
            }
            let a = value["source"].as_str().unwrap_or("");
            let b = value["target"].as_str().unwrap_or("");
            if !parts[2..].contains(&a)
                || !parts[2..].contains(&b)
                || a == b
                || value["entry"]["id"] != parts[1]
                || !["copy", "send", "sync"].contains(&value["operation"].as_str().unwrap_or(""))
            {
                return Err(invalid("Collegamento non valido"));
            }
        }
        if matches!(kind, "position" | "linkposition") {
            validate_point(value, kind == "linkposition")?;
        }
    }
    Ok(())
}
fn valid_text(value: &Value, limit: usize, required: bool) -> bool {
    value.as_str().is_some_and(|s| {
        s.chars().count() <= limit
            && !s.chars().any(|c| (c as u32) < 32)
            && (!required || !s.trim().is_empty())
    })
}
pub(crate) fn validate_point(value: &Value, offset: bool) -> io::Result<()> {
    if !value.as_object().is_some_and(|v| {
        v.len() == 2
            && ["x", "y"].iter().all(|k| {
                v.get(*k).and_then(Value::as_f64).is_some_and(|n| {
                    n.is_finite()
                        && ((-100000.0 * if offset { 1.0 } else { 0.0 })..=100000.0).contains(&n)
                })
            })
    }) {
        return Err(invalid("Posizione non valida"));
    }
    Ok(())
}
pub(crate) fn clock(record: &Value) -> (u64, &str) {
    (
        record["clock"][0].as_u64().unwrap_or(0),
        record["clock"][1].as_str().unwrap_or(""),
    )
}
pub(crate) fn snapshot(
    state: &State,
    mapping: &BTreeMap<String, String>,
    devices: &[Value],
    positions: &Value,
    link_positions: &Value,
    local_name: &str,
) -> Value {
    let mut out = json!({});
    let local = json!({"id":"master","name":local_name});
    for device in std::iter::once(&local).chain(devices) {
        let Some(alias) = device["id"].as_str() else {
            continue;
        };
        let Some(global) = mapping.get(alias) else {
            continue;
        };
        let key = format!("device:{global}");
        if state.shared["records"]
            .get(&key)
            .is_some_and(|r| r["value"].is_null())
        {
            continue;
        }
        let mut appearance = state
            .appearance
            .get(&format!("device:{alias}"))
            .map(|r| r.value.clone())
            .unwrap_or(json!({}));
        let name = if alias == "master" {
            appearance["label"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(local_name)
        } else {
            device["name"].as_str().unwrap_or("Oaclis")
        }
        .to_owned();
        appearance.as_object_mut().unwrap().remove("label");
        if appearance.get("icon").is_none() {
            appearance["icon"] = json!("desktop");
        }
        if appearance["color"].as_str().is_none_or(str::is_empty) {
            let index = Sha256::digest(global.as_bytes())
                .iter()
                .fold(0usize, |a, b| (a * 256 + *b as usize) % 5);
            appearance["color"] =
                json!(["#3d5a80", "#98c1d9", "#e0fbfc", "#ee6c4d", "#293241"][index]);
        }
        out[key] = json!({"name":name,"appearance":appearance});
    }
    for (key, record) in state.appearance.iter() {
        let Some(chat) = key.strip_prefix("chat:") else {
            continue;
        };
        if record.value.as_object().is_some_and(|v| !v.is_empty()) {
            out[key] = record.value.clone();
        }
        out[format!("preference:{chat}")] =
            json!({"hidden":false,"list_hidden":record.hidden,"scopes":scopes(record)});
        for module in MODULES {
            for (source, data) in record.sync[module]["sources"]
                .as_object()
                .into_iter()
                .flatten()
            {
                out[format!("chatdata:{chat}:{source}")][module] = data.clone();
            }
        }
    }
    for link in state.links.values() {
        let (Some(a), Some(b)) = (
            link["source"].as_str().and_then(|s| mapping.get(s)),
            link["target"].as_str().and_then(|s| mapping.get(s)),
        ) else {
            continue;
        };
        let mut value = link.clone();
        value.as_object_mut().unwrap().remove("transfer_pending");
        value["source"] = json!(a);
        value["target"] = json!(b);
        let mut pair = [a, b];
        pair.sort();
        out[format!(
            "link:{}:{}:{}",
            link["entry"]["id"].as_str().unwrap_or(""),
            pair[0],
            pair[1]
        )] = value;
    }
    for (alias, position) in positions.as_object().into_iter().flatten() {
        if let Some(global) = mapping.get(alias) {
            out[format!("position:{global}")] = position.clone();
        }
    }
    for (key, point) in link_positions.as_object().into_iter().flatten() {
        let parts: Vec<_> = key.split(':').collect();
        if parts.len() != 3 {
            continue;
        }
        if let (Some(a), Some(b)) = (mapping.get(parts[1]), mapping.get(parts[2])) {
            let mut pair = [a, b];
            pair.sort();
            out[format!("linkposition:{}:{}:{}", parts[0], pair[0], pair[1])] = point.clone();
        }
    }
    strip_entries(&mut out);
    out
}
pub(crate) fn apply(
    state: &mut State,
    records: &Value,
    mapping: &mut BTreeMap<String, String>,
    devices: &[Value],
    local_id: &str,
) -> io::Result<(Vec<Value>, Value, Value)> {
    let mut reverse: BTreeMap<String, String> = mapping
        .iter()
        .map(|(a, b)| (b.clone(), a.clone()))
        .collect();
    let mut devices: BTreeMap<String, Value> = devices
        .iter()
        .filter_map(|v| v["id"].as_str().map(|id| (id.into(), v.clone())))
        .collect();
    let mut positions = json!({});
    let mut link_positions = json!({});
    let objects = records
        .as_object()
        .ok_or_else(|| invalid("Impostazioni non valide"))?;
    for (key, record) in objects {
        let (kind, id) = key.split_once(':').unwrap();
        let value = &record["value"];
        match kind {
            "device" => {
                if !reverse.contains_key(id) && !value.is_null() {
                    let alias = format!(
                        "shared-{}",
                        Sha256::digest(id.as_bytes())
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()[..32]
                            .to_owned()
                    );
                    mapping.insert(alias.clone(), id.into());
                    reverse.insert(id.into(), alias);
                }
                let Some(alias) = reverse.get(id) else {
                    continue;
                };
                if value.is_null() {
                    devices.remove(alias);
                    Arc::make_mut(&mut state.appearance).remove(&format!("device:{alias}"));
                    continue;
                }
                let mut appearance = json!({"icon":"desktop","color":""});
                for (k, v) in value["appearance"].as_object().into_iter().flatten() {
                    appearance[k] = v.clone();
                }
                appearance["label"] = value["name"].clone();
                Arc::make_mut(Arc::make_mut(&mut state.appearance).entry(format!("device:{alias}")).or_default()).value = appearance;
                if alias != "master" {
                    let device = devices.entry(alias.clone()).or_insert(
                        json!({"id":alias,"ssh_alias":format!("oaclis-unconfigured-{alias}")}),
                    );
                    device["name"] = value["name"].clone();
                }
            }
            "chat" => {
                if !value.is_null() {
                    let mut appearance = json!({"label":"","icon":"","color":""});
                    for (k, v) in value.as_object().unwrap() {
                        appearance[k] = v.clone();
                    }
                    Arc::make_mut(Arc::make_mut(&mut state.appearance).entry(key.clone()).or_default()).value = appearance;
                }
            }
            "preference" => {
                let record = chat_record(state, id);
                record.sync.as_object_mut().map(|v| v.remove("removed"));
                set_scopes(
                    record,
                    value
                        .get("scopes")
                        .unwrap_or(&json!({"chat":true,"paths":true,"apps":true})),
                )?;
                record.hidden = value["hidden"] == true || value["list_hidden"] == true;
            }
            "chatdata" => {
                let (chat, source) = id.split_once(':').unwrap();
                let record = chat_record(state, chat);
                for module in MODULES {
                    if value.is_null() || value.get(module).is_none() {
                        if let Some(s) = record.sync[module]["sources"].as_object_mut() {
                            s.remove(source);
                        }
                        continue;
                    }
                    let mut incoming = value[module].clone();
                    if module == "paths" && source == local_id {
                        let cached: BTreeMap<String, Value> = record.sync[module]["sources"]
                            [source]["items"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v["path"].as_str().map(|p| (p.into(), v.clone())))
                            .collect();
                        for item in incoming["items"].as_array_mut().into_iter().flatten() {
                            if let Some(old) = item["path"].as_str().and_then(|p| cached.get(p)) {
                                if old["sha256"] == item["sha256"] && old.get("entries").is_some() {
                                    item["entries"] = old["entries"].clone();
                                }
                            }
                        }
                    }
                    record.sync[module]["sources"][source] = incoming;
                }
            }
            _ => {}
        }
    }
    for (key, record) in objects {
        let parts: Vec<_> = key.split(':').collect();
        let value = &record["value"];
        match parts[0] {
            "link" => {
                let (Some(a), Some(b)) = (reverse.get(parts[2]), reverse.get(parts[3])) else {
                    continue;
                };
                let mut pair = [a, b];
                pair.sort();
                let local_key = format!("{}:{}:{}", parts[1], pair[0], pair[1]);
                if value.is_null()
                    || parts[2..].iter().any(|id| {
                        records
                            .get(format!("device:{id}"))
                            .is_some_and(|r| r["value"].is_null())
                    })
                {
                    Arc::make_mut(&mut state.links).remove(&local_key);
                    continue;
                }
                let mut adapted = value.clone();
                adapted["source"] = json!(reverse[value["source"].as_str().unwrap()]);
                adapted["target"] = json!(reverse[value["target"].as_str().unwrap()]);
                adapted["transfer_pending"] = if value.get("flow").is_some() {
                    json!(value["flow"]["initialized"] != true)
                } else {
                    state
                        .links
                        .get(&local_key)
                        .map(|v| v["transfer_pending"].clone())
                        .unwrap_or(json!(false))
                };
                if let Some(scopes) =
                    records[format!("preference:{}", parts[1])]["value"].get("scopes")
                {
                    adapted["scopes"] = scopes.clone();
                }
                Arc::make_mut(&mut state.links).insert(local_key, adapted);
            }
            "position" => {
                if !value.is_null() {
                    if let Some(alias) = reverse.get(parts[1]) {
                        positions[alias] = value.clone();
                    }
                }
            }
            "linkposition" => {
                if !value.is_null()
                    && records
                        .get(format!("link:{}", parts[1..].join(":")))
                        .is_some_and(|r| !r["value"].is_null())
                {
                    if let (Some(a), Some(b)) = (reverse.get(parts[2]), reverse.get(parts[3])) {
                        let mut pair = [a, b];
                        pair.sort();
                        link_positions[format!("{}:{}:{}", parts[1], pair[0], pair[1])] =
                            value.clone();
                    }
                }
            }
            _ => {}
        }
    }
    Ok((devices.into_values().collect(), positions, link_positions))
}
