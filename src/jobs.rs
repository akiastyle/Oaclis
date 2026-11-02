// jobs.rs v1.15.103
//! Volatile tool telemetry, independent of AI execution and persistent chat state.
use crate::invalid;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, io, sync::{Mutex, OnceLock}, time::{Instant, SystemTime, UNIX_EPOCH}};

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Update { chat_id:String, id:String, text:String, progress:Option<f64> }
impl Update {
    fn parse(value:Value)->io::Result<Self> {
        let update:Self=serde_json::from_value(value).map_err(|_|invalid("Stato lavoro non valido"))?;
        if [&update.id,&update.chat_id].iter().any(|id|id.is_empty()||id.len()>160||!id.bytes().all(|b|b.is_ascii_alphanumeric()||b"_-".contains(&b)))
            || update.text.chars().count()>2000 || update.text.chars().any(|c|c.is_control()&&!matches!(c,'\n'|'\t'))
            || update.progress.is_some_and(|p|!p.is_finite()||!(0.0..=100.0).contains(&p)) {return Err(invalid("Stato lavoro non valido"));}
        Ok(update)
    }
}
struct Record { update:Update, received:Instant, updated_at:u64 }
struct PeerJobs { name:String, jobs:Vec<(Update,u64,bool)>, received:Instant }
#[derive(Default)]
struct Monitor { jobs:Mutex<BTreeMap<(String,String),Record>>, peers:Mutex<BTreeMap<String,PeerJobs>> }
impl Monitor {
    fn update(&self,value:Value)->io::Result<Value> {
        let update=Update::parse(value)?;let key=(update.chat_id.clone(),update.id.clone());
        let mut jobs=self.jobs.lock().unwrap();
        if jobs.len()>=256&&!jobs.contains_key(&key){return Err(invalid("Troppi lavori monitorati"));}
        let record=Record{update,received:Instant::now(),updated_at:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64};
        jobs.insert(key,record);Ok(json!({"ok":true}))
    }
    fn receive(&self,device:&crate::devices::Device,value:&Value)->io::Result<()> {
        if value.is_null(){self.peers.lock().unwrap().remove(&device.id);return Ok(());}
        let rows=value["jobs"].as_array().filter(|rows|rows.len()<=256).ok_or_else(||invalid("Stato strumenti non valido"))?;
        let mut jobs=Vec::new();let mut ids=std::collections::BTreeSet::new();
        for row in rows {
            let payload:serde_json::Map<String,Value>=["chat_id","id","text","progress"].into_iter().filter_map(|key|row.get(key).map(|v|(key.into(),v.clone()))).collect();
            let update=Update::parse(Value::Object(payload))?;
            if !ids.insert((update.chat_id.clone(),update.id.clone())){return Err(invalid("Strumento duplicato"));}
            jobs.push((update,row["updated_at"].as_u64().ok_or_else(||invalid("Data strumento non valida"))?,row["stale"].as_bool().ok_or_else(||invalid("Stato strumento non valido"))?));
        }
        self.peers.lock().unwrap().insert(device.id.clone(),PeerJobs{name:device.name.clone(),jobs,received:Instant::now()});Ok(())
    }
    fn snapshot(&self)->Value {
        json!({"jobs":self.jobs.lock().unwrap().values().map(|record|{
            let mut value=serde_json::to_value(&record.update).unwrap();
            value["updated_at"]=json!(record.updated_at);
            value["stale"]=json!(record.update.progress!=Some(100.0)&&record.received.elapsed().as_secs()>=90);
            value
        }).collect::<Vec<_>>()})
    }
    fn for_chat(&self,chat:&str)->Value{json!(self.collected(None).into_iter().filter(|row|row["chat_id"]==chat).collect::<Vec<_>>())}
    fn collected(&self,allowed:Option<&[crate::devices::Device]>)->Vec<Value> {
        let mut rows=self.snapshot()["jobs"].as_array().unwrap().clone();
        for row in &mut rows{row["device_id"]=json!("master");}
        for (device,peer) in self.peers.lock().unwrap().iter() {
            if allowed.is_some_and(|devices|!devices.iter().any(|item|&item.id==device)){continue;}
            for (update,updated_at,stale) in &peer.jobs {
                let mut row=serde_json::to_value(update).unwrap();
                row["device_id"]=json!(device);row["device_name"]=json!(peer.name);row["updated_at"]=json!(updated_at);
                row["stale"]=json!(*stale||update.progress!=Some(100.0)&&peer.received.elapsed().as_secs()>=30);
                rows.push(row);
            }
        }
        rows
    }
}
fn local()->&'static Monitor {static LOCAL:OnceLock<Monitor>=OnceLock::new();LOCAL.get_or_init(Default::default)}
pub(crate) fn update(value:Value)->io::Result<Value>{local().update(value)}
pub(crate) fn snapshot()->Value{local().snapshot()}
pub(crate) fn receive(device:&crate::devices::Device,value:&Value)->io::Result<()>{local().receive(device,value)}
pub(crate) fn all(devices:&[crate::devices::Device])->Value{json!({"jobs":local().collected(Some(devices))})}
pub(crate) fn for_chat(chat:&str)->Value{local().for_chat(chat)}
pub(crate) fn retain(approved:&std::collections::BTreeSet<String>){local().peers.lock().unwrap().retain(|device,_|approved.contains(device));}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replaces_status_and_keeps_same_tool_separate_for_each_chat() {
        let monitor=Monitor::default();
        monitor.update(json!({"chat_id":"chat-a","id":"tool","text":"First","progress":42})).unwrap();
        monitor.update(json!({"chat_id":"chat-b","id":"tool","text":"Other","progress":null})).unwrap();
        monitor.update(json!({"chat_id":"chat-a","id":"tool","text":"Next","progress":50})).unwrap();
        assert_eq!(monitor.for_chat("chat-a").as_array().unwrap().len(),1);assert!(monitor.for_chat("not-authorized").as_array().unwrap().is_empty());
        let snapshot=monitor.snapshot();assert_eq!(snapshot["jobs"].as_array().unwrap().len(),2);assert_eq!(snapshot["jobs"][0]["text"],"Next");assert_eq!(snapshot["jobs"][0]["progress"].as_f64(),Some(50.0));
        monitor.jobs.lock().unwrap().get_mut(&("chat-a".into(),"tool".into())).unwrap().received=Instant::now()-std::time::Duration::from_secs(91);
        assert_eq!(monitor.snapshot()["jobs"][0]["stale"],true);
        monitor.update(json!({"chat_id":"chat-a","id":"tool","text":"Done","progress":100})).unwrap();assert_eq!(monitor.snapshot()["jobs"][0]["stale"],false);
    }
    #[test]
    fn invalid_updates_are_atomic_and_peer_identity_cannot_be_forged() {
        let monitor=Monitor::default();let base=json!({"chat_id":"chat-a","id":"tool","text":"Working","progress":42});monitor.update(base.clone()).unwrap();
        for fields in [json!({"progress":101}),json!({"progress":-1}),json!({"device_id":"other"}),json!({"chat_id":"../chat"}),json!({"text":"bad\u{0}"})] {
            let mut value=base.clone();value.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());assert!(monitor.update(value).is_err());assert_eq!(monitor.snapshot()["jobs"][0]["text"],"Working");
        }
        let device=crate::devices::Device{id:"approved".into(),name:"Peer".into(),ssh_alias:"fixture".into()};
        let mut report=monitor.snapshot();report["jobs"][0]["device_id"]=json!("forged");monitor.receive(&device,&report).unwrap();
        let rows=monitor.collected(Some(std::slice::from_ref(&device)));assert_eq!(rows[1]["device_id"],"approved");assert_eq!(rows[1]["device_name"],"Peer");
        assert_eq!(monitor.collected(Some(&[])).len(),1);
        report["jobs"][0]["progress"]=json!(101);assert!(monitor.receive(&device,&report).is_err());assert_eq!(monitor.collected(None)[1]["progress"].as_f64(),Some(42.0));
        monitor.receive(&device,&Value::Null).unwrap();assert_eq!(monitor.collected(None).len(),1);
    }
}
