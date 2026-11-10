// engines.rs v1.10.118
//! Private provider settings. Native Codex authentication remains owned by Codex.
use crate::invalid;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, path::Path, sync::{Arc, Mutex, OnceLock}};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Settings {
    configured: bool,
    default_engine: String,
    enabled: Vec<String>,
    models: BTreeMap<String, String>,
    keys: BTreeMap<String, String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self { configured:false, default_engine:"codex".into(), enabled:vec!["codex".into()], models:BTreeMap::new(), keys:BTreeMap::new() }
    }
}
struct Store { settings: Mutex<Settings> }

fn store(home: &Path) -> io::Result<Arc<Store>> {
    static STORES: OnceLock<Mutex<BTreeMap<std::path::PathBuf, Arc<Store>>>> = OnceLock::new();
    let mut stores = STORES.get_or_init(Default::default).lock().unwrap();
    if let Some(store) = stores.get(home) { return Ok(store.clone()); }
    let path = home.join(".config/oaclis/engines.json");
    let settings = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| invalid("Configurazione motori non valida"))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Settings::default(),
        Err(error) => return Err(error),
    };
    let store = Arc::new(Store { settings:Mutex::new(settings) });
    stores.insert(home.into(),store.clone());
    Ok(store)
}
fn known(engine: &str) -> bool { matches!(engine,"codex"|"claude"|"llamacpp") }
fn key(settings: &Settings, home: &Path, engine: &str) -> String {
    settings.keys.get(engine).cloned().unwrap_or_else(|| {
        if engine == "llamacpp" { std::fs::read_to_string(home.join(".config/ai-lab/api.key")).unwrap_or_default().trim().into() }
        else { String::new() }
    })
}
fn public(settings: &Settings, home: &Path) -> Value {
    let codex_home = std::env::var_os("CODEX_HOME").map(std::path::PathBuf::from).unwrap_or_else(||home.join(".codex"));
    json!({"configured":settings.configured,"default_engine":settings.default_engine,"providers":[
        {"id":"codex","enabled":settings.enabled.iter().any(|id|id=="codex"),"available":crate::terminal::codex(home).is_ok(),"credentials":codex_home.join("auth.json").is_file(),"model":settings.models.get("codex").map(String::as_str).unwrap_or("")},
        {"id":"claude","enabled":settings.enabled.iter().any(|id|id=="claude"),"available":false,"credentials":!key(settings,home,"claude").is_empty(),"model":settings.models.get("claude").map(String::as_str).unwrap_or("")},
        {"id":"llamacpp","enabled":settings.enabled.iter().any(|id|id=="llamacpp"),"available":false,"credentials":!key(settings,home,"llamacpp").is_empty(),"model":settings.models.get("llamacpp").map(String::as_str).unwrap_or("")}
    ]})
}
pub(crate) fn settings(home: &Path) -> io::Result<Value> {
    let store = store(home)?;
    let settings = store.settings.lock().unwrap();
    Ok(public(&settings,home))
}
pub(crate) fn save(home: &Path, value: &Value) -> io::Result<Value> {
    let engine = value["default_engine"].as_str().filter(|id|known(id)).ok_or_else(||invalid("Motore non valido"))?;
    let enabled = value["enabled"].as_array().filter(|items|!items.is_empty() && items.len()<=3).ok_or_else(||invalid("Scegli almeno un motore"))?;
    let enabled = enabled.iter().map(|id|id.as_str().filter(|id|known(id)).map(String::from).ok_or_else(||invalid("Motore non valido"))).collect::<io::Result<Vec<_>>>()?;
    if !enabled.iter().any(|id|id==engine) { return Err(invalid("Abilita il motore predefinito")); }
    let models: BTreeMap<String,String> = serde_json::from_value(value["models"].clone()).map_err(|_|invalid("Modelli non validi"))?;
    for (id, model) in &models { if !known(id) {return Err(invalid("Motore non valido"));} crate::terminal::validate_model(model)?; }
    let keys: BTreeMap<String,String> = serde_json::from_value(value["keys"].clone()).map_err(|_|invalid("Chiavi non valide"))?;
    for (id, key) in &keys {
        if !matches!(id.as_str(),"claude"|"llamacpp") || key.len()>4096 || key.chars().any(char::is_control) { return Err(invalid("Chiave non valida")); }
    }
    let store = store(home)?;
    let mut saved = store.settings.lock().unwrap();
    let mut next = saved.clone();
    next.configured=true; next.default_engine=engine.into(); next.enabled=enabled; next.models=models;
    for (id,key) in keys { if !key.is_empty() { next.keys.insert(id,key); } }
    let directory = home.join(".config/oaclis");
    std::fs::create_dir_all(&directory)?;
    crate::staging::save_json(&directory.join("engines.json"), &next)?;
    *saved=next;
    Ok(public(&saved,home))
}
pub(crate) fn llama_key(home: &Path) -> io::Result<String> {
    let store=store(home)?;
    let settings=store.settings.lock().unwrap();
    let key=key(&settings,home,"llamacpp");
    if key.is_empty(){return Err(io::Error::other("Collegamento llama.cpp non configurato"));}
    Ok(key)
}
pub(crate) fn creation(home: &Path, value: &Value) -> io::Result<String> {
    let store=store(home)?;
    let settings=store.settings.lock().unwrap();
    let engine=value.get("engine").map(|v|v.as_str().filter(|id|known(id)).ok_or_else(||invalid("Motore non valido"))).transpose()?.unwrap_or("codex");
    // Older callers remain Codex; an unsupported provider must never create a Codex archive.
    if engine!="codex" {return Err(invalid("Chat per questo motore non ancora disponibili"));}
    if value.get("engine").is_some() && settings.configured && !settings.enabled.iter().any(|id|id==engine) {return Err(invalid("Motore disattivato"));}
    Ok(settings.models.get(engine).cloned().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_settings_preserve_keys_and_never_publish_them() {
        use std::os::unix::fs::PermissionsExt;
        let home=std::env::temp_dir().join(format!("oaclis-engines-{}",std::process::id()));
        std::fs::create_dir(&home).unwrap();
        let input=json!({"default_engine":"codex","enabled":["codex","llamacpp"],"models":{"codex":"gpt-6.1-sol"},"keys":{"llamacpp":"fixture-secret"}});
        let result=save(&home,&input).unwrap();
        assert!(!result.to_string().contains("fixture-secret"));
        assert!(result["providers"][2]["credentials"].as_bool().unwrap());
        assert_eq!(std::fs::metadata(home.join(".config/oaclis/engines.json")).unwrap().permissions().mode()&0o777,0o600);
        let mut empty=input.clone();empty["keys"]=json!({"llamacpp":""});save(&home,&empty).unwrap();
        assert_eq!(llama_key(&home).unwrap(),"fixture-secret");
        assert!(creation(&home,&json!({"engine":"llamacpp"})).is_err());
        assert!(creation(&home,&json!({"engine":"unknown"})).is_err());
        assert_eq!(creation(&home,&json!({})).unwrap(),"gpt-6.1-sol");
        empty["keys"]=json!({"llamacpp":"bad\nkey"});assert!(save(&home,&empty).is_err());
        assert_eq!(llama_key(&home).unwrap(),"fixture-secret");
        std::fs::remove_dir_all(home).unwrap();
    }
}
