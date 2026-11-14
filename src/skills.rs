// skills.rs v1.0.2
//! Read-only skill availability and upstream version checks, cached between background scans.
use serde_json::{Value,json};
use std::{collections::BTreeMap,fs,path::Path,sync::{Mutex,OnceLock},time::{Duration,Instant,SystemTime,UNIX_EPOCH}};
#[derive(Default)]
struct Cache { checked:Option<Instant>, running:bool, value:Value }
static CACHE:OnceLock<Mutex<Cache>>=OnceLock::new();
pub(crate) fn snapshot()->Value {
    let cache=CACHE.get_or_init(Default::default).lock().unwrap();
    if cache.value.is_null(){json!({"state":"checking","items":[]})}else{cache.value.clone()}
}
pub(crate) fn refresh(home:&Path,root:&Path) {
    let mut cache=CACHE.get_or_init(Default::default).lock().unwrap();
    if cache.running||cache.checked.is_some_and(|time|time.elapsed()<Duration::from_secs(3600)){return;}
    cache.running=true;drop(cache);
    let home=home.to_owned();let root=root.to_owned();
    std::thread::spawn(move||{
        let result=crate::catalog::skills(&home,&root).map(|inventory|collect(&inventory));
        let mut cache=CACHE.get().unwrap().lock().unwrap();cache.running=false;cache.checked=Some(Instant::now());
        match result {
            Ok(items)=>cache.value=json!({"state":"ready","checked_at":SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),"items":items}),
            Err(error)=>{if cache.value.is_null(){cache.value=json!({"items":[]});}cache.value["state"]=json!("unavailable");cache.value["error"]=json!(error.to_string());}
        }
    });
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
            rows.push(json!({"name":skill["name"],"enabled":skill["enabled"],"version":version,"latest":latest,"update":latest.as_deref().map_or("unknown",|v|update_state(&version,v))}));
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
