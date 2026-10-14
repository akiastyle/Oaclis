// app_inventory.rs v1.4.76
//! Define synchronization requirements from the master's actual project environment.
use crate::{environment::{self,Manifest},invalid};
use serde_json::{Value,json};
use std::{io,path::Path,time::{SystemTime,UNIX_EPOCH}};
const SCRIPT:&str=include_str!("../tools/app_inventory.py");
fn now()->u64 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}

pub(crate) fn recover_interrupted(config: &Path) -> io::Result<()> {
    let records = environment::database(config, "", "inventory-list", None)?;
    if let Some(records) = records.as_object() {
        for (key, record) in records {
            if key.starts_with("master:") && record["status"] == "pending" {
                let mut recovered = record.clone();
                recovered["status"] = json!("failed");
                recovered["error"] = json!("Rilevamento interrotto dal riavvio di Oaclis. Nessuna analisi in corso.");
                environment::database(config, key, "inventory-save", Some(&recovered))?;
            }
        }
    }
    Ok(())
}

pub(crate) fn request(config:&Path,value:&Value)->io::Result<Value> {
    let source=value["source_device_id"].as_str().unwrap_or("master");
    if source!="master" {return Ok(json!({"status":"unavailable","error":"Definizione disponibile per le chat locali del master"}));}
    let chat=value["chat_id"].as_str().ok_or_else(||invalid("Chat richiesta"))?;
    let key=environment::key(source,chat)?;
    let previous=environment::database(config,&key,"inventory-get",None)?;
    if previous["status"] == "pending" || (value["force"] != true && matches!(previous["status"].as_str(), Some("ready" | "failed"))) {return Ok(previous);}
    let analysis=environment::database(config,&key,"analysis-get",None)?;
    if analysis["status"]!="ready" {return Ok(json!({"status":"waiting"}));}
    let manifest:Manifest=serde_json::from_value(analysis["manifest"].clone()).map_err(io::Error::other)?;
    manifest.validate()?;
    let signature=analysis["revision"].clone();
    let pending=json!({"status":"pending","signature":signature,"checked_at":now()});
    environment::database(config,&key,"inventory-save",Some(&pending))?;
    let config=config.to_path_buf();
    std::thread::spawn(move||{
        let input=json!({"paths":manifest.paths,"configs":manifest.configs,"plugins":manifest.plugins});
        let mut result=match environment::python(None,SCRIPT,&["source",&input.to_string()]) {
            Ok(profile)=>json!({"status":"ready","paths":profile["paths"],"requirements":profile["requirements"],"configs":manifest.configs}),
            Err(error)=>json!({"status":"failed","error":error.to_string()})
        };
        result["signature"]=signature;result["checked_at"]=json!(now());
        let _=environment::database(&config,&key,"inventory-save",Some(&result));
    });
    Ok(pending)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_check_does_not_expire_or_require_another_analysis() {
        let dir=std::env::temp_dir().join(format!("oaclis-check-cache-{}-{}",std::process::id(),now()));
        std::fs::create_dir_all(&dir).unwrap();
        let chat="019f99ae-6802-7b30-902b-f73dbd2459b4";
        let key=environment::key("master",chat).unwrap();
        let saved=json!({"status":"ready","checked_at":1,"requirements":[{"label":"Python 3","version":"3.12.3"}]});
        environment::database(&dir,&key,"inventory-save",Some(&saved)).unwrap();
        assert_eq!(request(&dir,&json!({"chat_id":chat})).unwrap(),saved);
        let pending=json!({"status":"pending","checked_at":1,"requirements":saved["requirements"]});
        environment::database(&dir,&key,"inventory-save",Some(&pending)).unwrap();
        recover_interrupted(&dir).unwrap();
        let recovered=environment::database(&dir,&key,"inventory-get",None).unwrap();
        assert_eq!(recovered["status"], "failed");
        assert_eq!(recovered["requirements"], saved["requirements"]);
        assert_eq!(request(&dir,&json!({"chat_id":chat})).unwrap(), recovered);
        recover_interrupted(&dir).unwrap();
        assert_eq!(environment::database(&dir,&key,"inventory-get",None).unwrap(), recovered);
        assert_eq!(request(&dir,&json!({"chat_id":chat,"force":true})).unwrap()["status"], "waiting");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
