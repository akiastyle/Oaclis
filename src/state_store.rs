// state_store.rs v2.35.1099
//! Oaclis metadata: one in-process owner, memory reads and atomic JSON persistence.
use crate::{invalid, staging};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) const MODULES: [&str; 3] = ["chat", "paths", "apps"];
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct Presentation {
    pub value: Value,
    pub hidden: bool,
    pub sync: Value,
}
impl Default for Presentation {
    fn default() -> Self {
        Self {
            value: json!({}),
            hidden: false,
            sync: json!({}),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct State {
    pub schema: u32,
    pub appearance: BTreeMap<String, Presentation>,
    pub analysis: BTreeMap<String, Value>,
    pub links: BTreeMap<String, Value>,
    pub transfers: BTreeMap<String, Value>,
    #[serde(default)]
    pub shared: Value,
}
impl Default for State {
    fn default() -> Self {
        Self {
            schema: 1,
            appearance: Default::default(),
            analysis: Default::default(),
            links: Default::default(),
            transfers: Default::default(),
            shared: json!({"records":{},"baseline":{},"mapping":{}}),
        }
    }
}
pub(crate) struct Store {
    pub state: Mutex<State>,
    pub shared_lock: Mutex<()>,
    persisted: AtomicBool,
    _lock: File,
}
static STORES: OnceLock<Mutex<BTreeMap<PathBuf, Arc<Store>>>> = OnceLock::new();

pub(crate) fn read_json(path: &Path, default: Value) -> io::Result<Value> {
    match crate::open_regular(path) {
        Ok((file, _)) => serde_json::from_reader(file).map_err(io::Error::other),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(default),
        Err(error) => Err(error),
    }
}
pub(crate) fn save_changed(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let next = serde_json::to_value(value)?;
    if read_json(path, Value::Null)? != next {
        staging::save_json(path, &next)?;
    }
    Ok(())
}
pub(crate) fn store(config: &Path) -> io::Result<Arc<Store>> {
    let mut stores = STORES
        .get_or_init(Default::default)
        .lock()
        .map_err(|_| io::Error::other("Archivio non disponibile"))?;
    if let Some(store) = stores.get(config) {
        return Ok(store.clone());
    }
    crate::rules::ensure_local(config)?;
    let path = config.join("state.lock");
    if path.symlink_metadata().is_ok_and(|m| !m.is_file()) {
        return Err(invalid("Lock non regolare"));
    }
    use std::os::unix::fs::OpenOptionsExt;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    lock.try_lock()
        .map_err(|_| io::Error::other("Lo stato Oaclis e' gia' gestito da un'altra istanza"))?;
    recover_commit(config)?;
    let native = [
        "appearance.json",
        "analysis.json",
        "chat_links.json",
        "transfers.json",
    ]
    .iter()
    .all(|name| config.join(name).is_file());
    let state = if native {
        read_tables(config)?
    } else {
        crate::state_migration::load(config)?
    };
    if state
        .appearance
        .values()
        .any(|r| !r.value.is_object() || !r.sync.is_object())
    {
        return Err(invalid("Record personalizzazioni JSON non valido"));
    }
    if state.appearance.values().any(|r| {
        MODULES
            .iter()
            .any(|name| r.sync.get(*name).is_some_and(|v| !v.is_object()))
    }) {
        return Err(invalid("Moduli JSON non validi"));
    }
    let result = Arc::new(Store {
        state: Mutex::new(state),
        shared_lock: Mutex::new(()),
        persisted: AtomicBool::new(native),
        _lock: lock,
    });
    stores.insert(config.into(), result.clone());
    Ok(result)
}
fn read_tables(config: &Path) -> io::Result<State> {
    Ok(State {
        schema: 1,
        appearance: serde_json::from_value(read_json(&config.join("appearance.json"), json!({}))?)?,
        analysis: serde_json::from_value(read_json(&config.join("analysis.json"), json!({}))?)?,
        links: serde_json::from_value(read_json(&config.join("chat_links.json"), json!({}))?)?,
        transfers: serde_json::from_value(read_json(&config.join("transfers.json"), json!({}))?)?,
        shared: read_json(
            &config.join("shared-settings.json"),
            json!({"records":{},"baseline":{},"mapping":{}}),
        )?,
    })
}
fn external(config: &Path) -> bool {
    let owned = STORES
        .get_or_init(Default::default)
        .lock()
        .map(|v| v.contains_key(config))
        .unwrap_or(false);
    !owned && std::os::unix::net::UnixStream::connect(config.join("state.sock")).is_ok()
}
pub(crate) fn snapshot(config: &Path) -> io::Result<State> {
    if external(config) {
        return serde_json::from_value(crate::state_transport::local(
            config,
            &json!({"kind":"snapshot"}),
        )?)
        .map_err(io::Error::other);
    }
    Ok(store(config)?
        .state
        .lock()
        .map_err(|_| io::Error::other("Stato non disponibile"))?
        .clone())
}
pub(crate) fn change<T>(
    config: &Path,
    action: impl FnOnce(&mut State) -> io::Result<T>,
) -> io::Result<T> {
    let store = store(config)?;
    let mut current = store
        .state
        .lock()
        .map_err(|_| io::Error::other("Stato non disponibile"))?;
    if config.join(".state-commit.json").exists() {
        recover_commit(config)?;
        *current = read_tables(config)?;
    }
    let mut next = current.clone();
    let result = action(&mut next)?;
    if next != *current || !store.persisted.load(Ordering::Relaxed) {
        persist(
            config,
            &current,
            &next,
            !store.persisted.load(Ordering::Relaxed),
        )?;
        *current = next;
        store.persisted.store(true, Ordering::Relaxed);
    }
    Ok(result)
}
fn recover_commit(config: &Path) -> io::Result<()> {
    let journal = config.join(".state-commit.json");
    let pending = read_json(&journal, Value::Null)?;
    if pending.is_null() {
        return Ok(());
    }
    let files = pending
        .as_object()
        .ok_or_else(|| invalid("Salvataggio JSON interrotto non valido"))?;
    if files.keys().any(|name| {
        ![
            "appearance.json",
            "analysis.json",
            "chat_links.json",
            "transfers.json",
            "shared-settings.json",
        ]
        .contains(&name.as_str())
    }) {
        return Err(invalid("File stato non valido"));
    }
    for (name, value) in files {
        staging::save_json(&config.join(name), value)?;
    }
    std::fs::remove_file(journal)?;
    File::open(config)?.sync_all()
}
fn persist(config: &Path, before: &State, after: &State, force: bool) -> io::Result<()> {
    let old = [
        serde_json::to_value(&before.appearance)?,
        serde_json::to_value(&before.analysis)?,
        serde_json::to_value(&before.links)?,
        serde_json::to_value(&before.transfers)?,
        before.shared.clone(),
    ];
    let next = [
        serde_json::to_value(&after.appearance)?,
        serde_json::to_value(&after.analysis)?,
        serde_json::to_value(&after.links)?,
        serde_json::to_value(&after.transfers)?,
        after.shared.clone(),
    ];
    let mut files = json!({});
    for (index, name) in [
        "appearance.json",
        "analysis.json",
        "chat_links.json",
        "transfers.json",
        "shared-settings.json",
    ]
    .iter()
    .enumerate()
    {
        if force || old[index] != next[index] {
            files[*name] = next[index].clone();
        }
    }
    if files.as_object().unwrap().len() == 1 {
        let (name, value) = files.as_object().unwrap().iter().next().unwrap();
        return staging::save_json(&config.join(name), value);
    }
    if !files.as_object().unwrap().is_empty() {
        staging::save_json(&config.join(".state-commit.json"), &files)?;
        recover_commit(config)?;
    }
    Ok(())
}
pub(crate) fn identity(config: &Path) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    match std::fs::read_to_string(config.join("pairing_ed25519.pub")) {
        Ok(key) => Ok(Sha256::digest(
            key.split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ")
                .as_bytes(),
        )
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()[..32]
            .to_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok("local".into()),
        Err(error) => Err(error),
    }
}
pub(crate) fn mapping(config: &Path, shared: &Value) -> io::Result<BTreeMap<String, String>> {
    let mut result: BTreeMap<String, String> = serde_json::from_value(
        shared["mapping"]
            .as_object()
            .cloned()
            .map(Value::Object)
            .unwrap_or(json!({})),
    )?;
    result.insert("master".into(), identity(config)?);
    let pairing = read_json(&config.join("pairing.json"), json!({}))?;
    for (peer, approval) in pairing["approved"].as_object().into_iter().flatten() {
        if let Some(id) = approval["device_id"].as_str() {
            result.insert(id.into(), peer.clone());
        }
    }
    Ok(result)
}
pub(crate) fn scopes(record: &Presentation) -> Value {
    MODULES
        .iter()
        .map(|&name| {
            (
                name.to_owned(),
                json!(match &record.sync[name]["enabled"] {
                    Value::Null => !record.sync[name]
                        .as_object()
                        .is_some_and(|v| v.contains_key("enabled")),
                    Value::Bool(v) => *v,
                    Value::Number(v) => v.as_i64() != Some(0),
                    _ => true,
                }),
            )
        })
        .collect()
}
fn preferences(state: &State) -> Value {
    state
        .appearance
        .iter()
        .filter_map(|(key, record)| {
            key.strip_prefix("chat:").map(|id| {
                (
                    id.to_owned(),
                    json!({"hidden":record.hidden,"scopes":scopes(record)}),
                )
            })
        })
        .collect()
}
fn measurements(config: &Path, state: &State) -> io::Result<(Value, Value)> {
    let reverse: BTreeMap<_, _> = mapping(config, &state.shared)?
        .into_iter()
        .map(|(a, b)| (b, a))
        .collect();
    let mut inventories = json!({});
    let mut preparations = json!({});
    for (key, record) in &state.appearance {
        let Some(chat) = key.strip_prefix("chat:") else {
            continue;
        };
        for (source, value) in record.sync["apps"]["sources"]
            .as_object()
            .into_iter()
            .flatten()
        {
            inventories[format!("{}:{chat}", reverse.get(source).unwrap_or(source))] =
                value.clone();
        }
        for (source, value) in record.sync["paths"]["sources"]
            .as_object()
            .into_iter()
            .flatten()
        {
            let c = &record.sync["chat"]["sources"][source];
            if !c.is_object() {
                continue;
            }
            preparations[format!("{}:{chat}", reverse.get(source).unwrap_or(source))] = json!({"status":value["status"],"chat_bytes":c["bytes"],"chat_modified_ns":c["modified_ns"],"measured_at":value["measured_at"],"paths":value["items"],"paths_bytes":value["bytes"],"path_files":value["files"]});
        }
    }
    Ok((inventories, preparations))
}
fn read_operation(config: &Path, state: &State, key: &str, operation: &str) -> io::Result<Value> {
    let missing = || json!({"status":"missing"});
    match operation {
        "load" => Ok(state
            .appearance
            .iter()
            .map(|(key, record)| (key.clone(), record.value.clone()))
            .collect()),
        "links-get" => Ok(json!(state.links.values().collect::<Vec<_>>())),
        "preferences-get" => Ok(
            json!({"preferences":preferences(state),"links":state.links.values().collect::<Vec<_>>()}),
        ),
        "analysis-list" => Ok(json!(state.analysis)),
        "transfer-list" => Ok(json!(state.transfers)),
        "analysis-get" => {
            let mut result = state.analysis.get(key).cloned().unwrap_or_else(missing);
            let chat = key.split_once(':').map(|(_, v)| v).unwrap_or(key);
            result["scopes"] = state
                .appearance
                .get(&format!("chat:{chat}"))
                .map(scopes)
                .unwrap_or(json!({"chat":true,"paths":true,"apps":true}));
            Ok(result)
        }
        "inventory-list" | "inventory-get" | "preparation-get" | "status-read" => {
            let (inventories, mut preparations) = measurements(config, state)?;
            if operation == "inventory-list" {
                return Ok(inventories);
            }
            if operation == "inventory-get" {
                return Ok(inventories.get(key).cloned().unwrap_or_else(missing));
            }
            if operation == "preparation-get" {
                return Ok(preparations.get(key).cloned().unwrap_or_else(missing));
            }
            strip_entries(&mut preparations);
            Ok(
                json!({"preferences":preferences(state),"links":state.links.values().collect::<Vec<_>>(),"analyses":state.analysis,"app_checks":inventories,"preparations":preparations,"transfers":state.transfers}),
            )
        }
        _ => Err(invalid("Operazione di lettura non valida")),
    }
}
pub(crate) fn strip_entries(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("entries");
            for v in map.values_mut() {
                strip_entries(v);
            }
        }
        Value::Array(values) => {
            for v in values {
                strip_entries(v);
            }
        }
        _ => {}
    }
}
pub(crate) fn chat_record<'a>(state: &'a mut State, chat: &str) -> &'a mut Presentation {
    state.appearance.entry(format!("chat:{chat}")).or_default()
}
pub(crate) fn set_scopes(record: &mut Presentation, value: &Value) -> io::Result<()> {
    if !value.as_object().is_some_and(|v| {
        v.len() == 3
            && MODULES
                .iter()
                .all(|k| v.get(*k).is_some_and(Value::is_boolean))
    }) {
        return Err(invalid("Moduli non validi"));
    }
    for module in MODULES {
        record.sync[module]["enabled"] = json!(u8::from(value[module] == true));
    }
    Ok(())
}
fn write_operation(
    config: &Path,
    state: &mut State,
    key: &str,
    operation: &str,
    value: &Value,
) -> io::Result<Value> {
    match operation {
        "migrate" => Ok(json!({"ok":true})),
        "save" => {
            let entries = value
                .as_object()
                .ok_or_else(|| invalid("Personalizzazioni non valide"))?;
            for (key, value) in entries {
                state.appearance.entry(key.clone()).or_default().value = value.clone();
            }
            Ok(Value::Null)
        }
        "chat-observed" | "inventory-save" | "preparation-save" => {
            let (source, chat) = key
                .split_once(':')
                .ok_or_else(|| invalid("Sorgente chat non valida"))?;
            let sources = mapping(config, &state.shared)?;
            let source = sources.get(source).map(String::as_str).unwrap_or(source);
            let record = chat_record(state, chat);
            match operation {
                "inventory-save" => record.sync["apps"]["sources"][source] = value.clone(),
                "chat-observed" => {
                    for field in ["bytes", "modified_ns", "sha256"] {
                        record.sync["chat"]["sources"][source][field] = value[field].clone();
                    }
                }
                _ => {
                    record.sync["chat"]["sources"][source] = json!({"bytes":value["chat_bytes"],"modified_ns":value["chat_modified_ns"],"measured_at":value["measured_at"]});
                    record.sync["paths"]["sources"][source] = json!({"status":value["status"],"items":value["paths"],"bytes":value["paths_bytes"],"files":value["path_files"],"measured_at":value["measured_at"]});
                }
            }
            Ok(value.clone())
        }
        "analysis-save" | "analysis-scopes" => {
            let chat = key.split_once(':').map(|(_, v)| v).unwrap_or(key);
            if operation == "analysis-save" {
                state.analysis.insert(key.into(), value.clone());
                let record = chat_record(state, chat);
                for name in MODULES {
                    record.sync[name]
                        .as_object_mut()
                        .map(|v| v.entry("enabled").or_insert(json!(1)));
                    if record.sync[name].is_null() {
                        record.sync[name] = json!({"enabled":1});
                    }
                }
            } else {
                set_scopes(chat_record(state, chat), value)?;
            }
            read_operation(config, state, key, "analysis-get")
        }
        "transfer-save" => {
            state.transfers.insert(key.into(), value.clone());
            Ok(json!(state.transfers))
        }
        "links-save" => {
            let mut link = value.clone();
            if let Some(saved) = state.links.get(key) {
                if ["source", "target", "operation"]
                    .iter()
                    .any(|&name| saved[name] != link[name])
                {
                    return Err(invalid(
                        "Elimina e ricrea il collegamento per cambiarne la direzione",
                    ));
                }
                if saved.get("flow").is_some() {
                    link["flow"] = saved["flow"].clone();
                }
            }
            if let Some(record) = state.appearance.get(&format!(
                "chat:{}",
                link["entry"]["id"]
                    .as_str()
                    .ok_or_else(|| invalid("Chat mancante"))?
            )) {
                link["scopes"] = scopes(record);
            }
            if link["flow"].is_object() {
                link["transfer_pending"] = json!(link["flow"]["initialized"] != true);
            }
            state.links.insert(key.into(), link);
            read_operation(config, state, key, "links-get")
        }
        "links-flow" => {
            if let Some(link) = state.links.get_mut(key) {
                if link["flow"]["generation"] == value["generation"] {
                    link["flow"] = value.clone();
                    link["transfer_pending"] = json!(value["initialized"] != true);
                }
            }
            read_operation(config, state, key, "links-get")
        }
        "links-delete" => {
            state.links.remove(key);
            read_operation(config, state, key, "links-get")
        }
        "chat-visibility" | "chat-hide" | "chat-scopes" | "device-forget" => {
            if operation != "device-forget" {
                let record = chat_record(state, key);
                record.sync.as_object_mut().map(|v| v.remove("removed"));
                if operation == "chat-scopes" {
                    set_scopes(record, value)?;
                } else {
                    record.hidden = if operation == "chat-hide" {
                        true
                    } else {
                        value["hidden"]
                            .as_bool()
                            .ok_or_else(|| invalid("Visibilita' non valida"))?
                    };
                }
            }
            state.links.retain(|_, link| {
                let matched = if operation == "device-forget" {
                    link["source"] == key || link["target"] == key
                } else {
                    link["entry"]["id"] == key
                };
                if matched && operation == "chat-scopes" {
                    link["scopes"] = value.clone();
                }
                !matched || !matches!(operation, "device-forget" | "chat-hide")
            });
            if operation == "device-forget" {
                state.appearance.remove(&format!("device:{key}"));
                state
                    .analysis
                    .retain(|k, _| k.split_once(':').is_none_or(|(source, _)| source != key));
            }
            read_operation(config, state, key, "preferences-get")
        }
        _ => Err(invalid("Operazione stato non valida")),
    }
}
pub fn database(
    config: &Path,
    key: &str,
    operation: &str,
    value: Option<&Value>,
) -> io::Result<Value> {
    if external(config) {
        return crate::state_transport::local(
            config,
            &json!({"kind":"database","operation":operation,"key":key,"value":value}),
        );
    }
    let reads = [
        "load",
        "links-get",
        "preferences-get",
        "analysis-list",
        "analysis-get",
        "inventory-list",
        "inventory-get",
        "preparation-get",
        "status-read",
        "transfer-list",
    ];
    if reads.contains(&operation) {
        let owner = store(config)?;
        let state = owner
            .state
            .lock()
            .map_err(|_| io::Error::other("Stato non disponibile"))?;
        read_operation(config, &state, key, operation)
    } else {
        change(config, |state| {
            write_operation(config, state, key, operation, value.unwrap_or(&Value::Null))
        })
    }
}
