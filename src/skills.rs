// skills.rs v1.7.83
//! Startup skill checks, cached reports and opt-in native plugin updates.
use serde_json::{Value,json};
use std::{collections::BTreeMap,fs,path::Path,sync::{Arc,Mutex,OnceLock},time::{Duration,SystemTime,UNIX_EPOCH}};
#[derive(Default)]
struct Cache { checked:bool, running:bool, value:Value }
static CACHE:OnceLock<Mutex<Cache>>=OnceLock::new();
pub(crate) fn snapshot(config:&Path)->Value {
    let cache=CACHE.get_or_init(Default::default).lock().unwrap();
    let mut value=if cache.value.is_null(){json!({"state":"checking","items":[]})}else{cache.value.clone()};
    value["auto_update"]=json!(auto_update(config));value
}
pub(crate) fn refresh(home:&Path,root:&Path) {
    let mut cache=CACHE.get_or_init(Default::default).lock().unwrap();
    if cache.running||cache.checked{return;}
    cache.running=true;drop(cache);
    let home=home.to_owned();let root=root.to_owned();
    std::thread::spawn(move||{
        let config=home.join(".config/oaclis");
        let result=crate::catalog::skills(&home,&root).map(|inventory|collect(&inventory)).map(|items|{
            let candidates=update_candidates(&items);
            if !auto_update(&config)||candidates.is_empty(){return (items,false,false);}
            CACHE.get().unwrap().lock().unwrap().value=json!({"state":"updating","items":items});
            let failed=install_updates(&home,&root,&candidates).is_err();
            let fresh=crate::catalog::skills(&home,&root).map(|inventory|collect(&inventory)).unwrap_or(items);
            let failed=failed||!update_candidates(&fresh).is_empty();
            (fresh,true,failed)
        });
        let mut cache=CACHE.get().unwrap().lock().unwrap();cache.running=false;cache.checked=true;
        match result {
            Ok((items,updated,failed))=>cache.value=json!({"state":"ready","updated":updated,"update_failed":failed,"checked_at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),"items":items}),
            Err(error)=>{if cache.value.is_null(){cache.value=json!({"items":[]});}cache.value["state"]=json!("unavailable");cache.value["error"]=json!(error.to_string());}
        }
    });
}
pub(crate) fn auto_update(config:&Path)->bool {
    crate::state_store::snapshot(config).ok().and_then(|state|state.appearance.get("device:master").and_then(|p|p.sync["skills_auto_update"].as_bool())).unwrap_or(false)
}
pub(crate) fn set_auto_update(config:&Path,on:bool)->std::io::Result<()> {
    crate::state_store::change(config,|state|{
        let record=Arc::make_mut(Arc::make_mut(&mut state.appearance).entry("device:master".into()).or_default());
        record.sync["skills_auto_update"]=json!(on);Ok(())
    })
}
fn update_candidates(items:&[Value])->std::collections::BTreeSet<String> {
    items.iter().filter(|item|item["enabled"]==true&&item["update"]=="outdated").filter_map(|item|item["plugin_id"].as_str()).filter(|id|id.split_once('@').is_some_and(|(name,market)|crate::devices::identifier(name)&&crate::devices::identifier(market))).map(String::from).collect()
}
fn install_updates(home:&Path,root:&Path,plugins:&std::collections::BTreeSet<String>)->std::io::Result<()> {
    let native=root.join("packages/app-server-daemon/current/bin/codex");
    let executable=if native.is_file(){native}else{crate::terminal::codex(home)?};
    let markets:std::collections::BTreeSet<_>=plugins.iter().filter_map(|id|id.split_once('@').map(|(_,market)|market)).collect();
    for market in markets {
        crate::app_update::run(std::process::Command::new(&executable).env("CODEX_HOME",root).args(["plugin","marketplace","upgrade",market,"--json"]),Duration::from_secs(60))?;
    }
    for plugin in plugins {
        crate::app_update::run(std::process::Command::new(&executable).env("CODEX_HOME",root).args(["plugin","add",plugin,"--json"]),Duration::from_secs(60))?;
    }
    Ok(())
}
fn upstream(manifest:&Value)->Option<Value> {
    let repository=manifest["repository"].as_str()?.trim_end_matches('/').trim_end_matches(".git");
    let path=repository.strip_prefix("https://github.com/")?;
    let parts:Vec<_>=path.split('/').collect();
    if parts.len()!=2||parts.iter().any(|part|part.is_empty()||!part.bytes().all(|b|b.is_ascii_alphanumeric()||b"-_.".contains(&b))){return None;}
    let agent:ureq::Agent=ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(5))).build().into();
    let body=agent.get(format!("https://raw.githubusercontent.com/{path}/HEAD/.codex-plugin/plugin.json")).call().ok()?.body_mut().with_config().limit(65536).read_to_string().ok()?;
    serde_json::from_str(&body).ok()
}
fn update_state(local:&str,latest:&str)->&'static str {
    fn parts(value:&str)->Option<Vec<u64>> {value.trim_start_matches('v').split('.').map(|n|n.parse().ok()).collect()}
    match (parts(local),parts(latest)) {
        (Some(mut a),Some(mut b))=>{let len=a.len().max(b.len());a.resize(len,0);b.resize(len,0);if a<b {"outdated"}else{"current"}},
        _ if local==latest&&!local.is_empty()=>"current",
        _=>"unknown",
    }
}
fn collect(inventory:&Value)->Vec<Value> {
    let mut plugins:BTreeMap<String,(String,Option<String>)>=BTreeMap::new();
    let mut rows=Vec::new();
    for entry in inventory["data"].as_array().into_iter().flatten() {
        for skill in entry["skills"].as_array().into_iter().flatten() {
            let path=Path::new(skill["path"].as_str().unwrap_or(""));
            let plugin=path.ancestors().find_map(|parent|{let file=parent.join(".codex-plugin/plugin.json");file.is_file().then_some(file)});
            let (version,latest)=if let Some(file)=plugin {
                plugins.entry(file.to_string_lossy().into_owned()).or_insert_with(||{
                    let manifest=fs::read(&file).ok().and_then(|bytes|serde_json::from_slice::<Value>(&bytes).ok()).unwrap_or(Value::Null);
                    (manifest["version"].as_str().unwrap_or("").to_owned(),upstream(&manifest).and_then(|v|v["version"].as_str().map(String::from)))
                }).clone()
            }else{
                let version=fs::read_to_string(path).ok().and_then(|text|text.lines().take(35).find_map(|line|line.trim().strip_prefix("version:").map(|v|v.trim().trim_matches(['\'', '"']).to_owned()))).unwrap_or_default();
                (version,None)
            };
            rows.push(json!({"name":skill["name"],"plugin_id":skill["pluginId"],"enabled":skill["enabled"],"version":version,"latest":latest,"update":latest.as_deref().map_or("unknown",|v|update_state(&version,v))}));
        }
        for error in entry["errors"].as_array().into_iter().flatten() {
            let name=Path::new(error["path"].as_str().unwrap_or("")).parent().and_then(Path::file_name).unwrap_or_default().to_string_lossy();
            rows.push(json!({"name":name,"enabled":false,"invalid":true,"update":"unknown"}));
        }
    }
    rows.sort_by_key(|row|row["name"].as_str().unwrap_or("").to_lowercase());rows
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_update_is_opt_in_and_preserves_existing_settings(){
        let root=std::env::temp_dir().join(format!("oaclis-skill-setting-{}",std::process::id()));
        fs::create_dir_all(&root).unwrap();
        assert!(!auto_update(&root));
        crate::state_store::change(&root,|state|{let record=Arc::make_mut(Arc::make_mut(&mut state.appearance).entry("device:master".into()).or_default());record.value=json!({"label":"Keep this name"});Ok(())}).unwrap();
        set_auto_update(&root,true).unwrap();assert!(auto_update(&root));
        set_auto_update(&root,false).unwrap();assert!(!auto_update(&root));
        assert_eq!(crate::state_store::snapshot(&root).unwrap().appearance["device:master"].value["label"],"Keep this name");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn only_enabled_outdated_managed_plugins_are_updated_once(){
        let rows=vec![json!({"enabled":true,"update":"outdated","plugin_id":"sample@source"}),json!({"enabled":true,"update":"outdated","plugin_id":"sample@source"}),json!({"enabled":false,"update":"outdated","plugin_id":"disabled@source"}),json!({"enabled":true,"update":"unknown","plugin_id":"unknown@source"}),json!({"enabled":true,"update":"outdated","plugin_id":"bad@source;cmd"})];
        assert_eq!(update_candidates(&rows),std::collections::BTreeSet::from(["sample@source".to_owned()]));
    }
    #[cfg(unix)]
    #[test]
    fn updater_uses_the_native_marketplace_and_install_commands(){
        use std::os::unix::fs::PermissionsExt;
        let root=std::env::temp_dir().join(format!("oaclis-skill-updater-{}",std::process::id()));
        let executable=root.join("packages/app-server-daemon/current/bin/codex");fs::create_dir_all(executable.parent().unwrap()).unwrap();
        let log=root.join("commands");fs::write(&executable,format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\n",log.display())).unwrap();fs::set_permissions(&executable,fs::Permissions::from_mode(0o700)).unwrap();
        install_updates(&root,&root,&std::collections::BTreeSet::from(["sample@source".to_owned()])).unwrap();
        assert_eq!(fs::read_to_string(log).unwrap(),"plugin marketplace upgrade source --json\nplugin add sample@source --json\n");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn versions_are_compared_numerically_and_unknown_is_not_current(){
        assert_eq!(update_state("1.9.0","1.10.0"),"outdated");
        assert_eq!(update_state("v2.0","2.0.0"),"current");
        assert_eq!(update_state("3.0","2.9"),"current");
        assert_eq!(update_state("custom","2.0"),"unknown");
    }
    #[test]
    fn inventory_preserves_disabled_and_invalid_skills(){
        let rows=collect(&json!({"data":[{"skills":[{"name":"Zulu","enabled":false,"path":"/missing/SKILL.md"},{"name":"Alpha","enabled":true,"path":"/missing/SKILL.md"}],"errors":[{"path":"/broken/SKILL.md"}]}]}));
        assert_eq!(rows[0]["name"],"Alpha");assert_eq!(rows[0]["update"],"unknown");assert_eq!(rows[2]["enabled"],false);assert_eq!(rows[1]["invalid"],true);
    }
}
