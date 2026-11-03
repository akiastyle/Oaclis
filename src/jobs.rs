// jobs.rs v1.21.176
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
    fn expire(&self) {
        self.jobs.lock().unwrap().retain(|_,record|record.received.elapsed().as_secs()<3600);
        let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        self.peers.lock().unwrap().retain(|_,peer| {
            peer.jobs.retain(|(_,updated_at,_)|now.saturating_sub(*updated_at)<3_600_000);
            peer.received.elapsed().as_secs()<3600
        });
    }
    fn update(&self,value:Value)->io::Result<Value> {
        if value.as_object().is_some_and(|object|object.is_empty()) {
            return Ok(json!({
                "protocol":"oaclis-jobs-v1","method":"POST","endpoint":"/api/jobs/status","content_type":"application/json",
                "identity":["device","chat_id","id"],"additional_fields":false,
                "fields":{
                    "chat_id":{"type":"string","required":true,"max_length":160,"pattern":"^[A-Za-z0-9_-]+$","description":"ID reale della chat che configura il tool, lo stesso usato per riprenderla. La chat lo verifica e lo inserisce direttamente nella configurazione del tool."},
                    "id":{"type":"string","required":true,"max_length":160,"pattern":"^[A-Za-z0-9_-]+$","description":"Nome identificativo stabile del tool. Usa ID diversi per tool diversi nella stessa chat."},
                    "text":{"type":"string","required":true,"max_length":2000,"description":"Testo libero, anche su piu righe. Il tool decide cosa scrivere; viene visualizzato senza interpretazione. Non includere credenziali."},
                    "progress":{"type":["number","null"],"minimum":0,"maximum":100,"default":null,"description":"Percentuale reale; null se non quantificabile. Usa 100 soltanto a conclusione riuscita."}
                },
                "reporting":{"max_updates_per_second":1,"heartbeat_seconds":15,"timeout_seconds":2,"failure_must_not_stop_tool":true,"replace_previous":true,"device_is_implicit":true,"stop_after_progress_100":true,"completion_response":{"ok":true,"stop":true},"delivery":"Invia a Oaclis sullo stesso device. Non inviare al sito pubblico o al relay mobile."},
                "example":{"chat_id":"ID_REALE_DELLA_PROPRIA_CHAT","id":"nome-tool","text":"Elaborazione in corso","progress":42},
                "response":{"ok":true}
            }));
        }
        let update=Update::parse(value)?;self.expire();let key=(update.chat_id.clone(),update.id.clone());
        let mut jobs=self.jobs.lock().unwrap();
        if jobs.len()>=256&&!jobs.contains_key(&key){return Err(invalid("Troppi lavori monitorati"));}
        let finished=update.progress==Some(100.0);
        if finished&&jobs.get(&key).is_some_and(|record|record.update.progress==Some(100.0)){return Ok(json!({"ok":true,"stop":true}));}
        let record=Record{update,received:Instant::now(),updated_at:SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64};
        jobs.insert(key,record);Ok(if finished{json!({"ok":true,"stop":true})}else{json!({"ok":true})})
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
pub(crate) fn reports()->Vec<Value>{local().collected(None)}
pub(crate) fn expire(){local().expire()}
pub(crate) fn receive(device:&crate::devices::Device,value:&Value)->io::Result<()>{local().receive(device,value)}
pub(crate) fn all(devices:&[crate::devices::Device])->Value{json!({"jobs":local().collected(Some(devices))})}
pub(crate) fn for_chat(chat:&str)->Value{local().for_chat(chat)}
pub(crate) fn retain(approved:&std::collections::BTreeSet<String>){local().peers.lock().unwrap().retain(|device,_|approved.contains(device));}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expiry_discards_silent_tools_and_peers_but_keeps_heartbeats() {
        let monitor=Monitor::default();
        for id in ["old","active"] {monitor.update(json!({"chat_id":"chat","id":id,"text":"Running","progress":42})).unwrap();}
        monitor.jobs.lock().unwrap().get_mut(&("chat".into(),"old".into())).unwrap().received=Instant::now()-std::time::Duration::from_secs(3601);
        monitor.peers.lock().unwrap().insert("old".into(),PeerJobs{name:"Old".into(),jobs:vec![],received:Instant::now()-std::time::Duration::from_secs(3601)});
        let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64;
        monitor.peers.lock().unwrap().insert("connected".into(),PeerJobs{name:"Connected".into(),jobs:vec![(Update{chat_id:"chat".into(),id:"silent-peer-tool".into(),text:"Old".into(),progress:Some(100.0)},now-3_600_001,false)],received:Instant::now()});
        monitor.expire();
        assert!(monitor.peers.lock().unwrap()["connected"].jobs.is_empty());
        assert_eq!(monitor.snapshot()["jobs"].as_array().unwrap().len(),1);
        assert_eq!(monitor.snapshot()["jobs"][0]["id"],"active");
        assert!(!monitor.peers.lock().unwrap().contains_key("old"));
        monitor.update(json!({"chat_id":"chat","id":"old","text":"Resumed","progress":43})).unwrap();
        assert_eq!(monitor.snapshot()["jobs"].as_array().unwrap().len(),2);
    }
    #[test]
    fn empty_request_describes_protocol_without_registering_a_tool() {
        let monitor=Monitor::default();
        let contract=monitor.update(json!({})).unwrap();
        assert_eq!(contract["endpoint"],"/api/jobs/status");
        assert_eq!(contract["fields"]["id"]["max_length"],160);
        assert!(monitor.snapshot()["jobs"].as_array().unwrap().is_empty());
        let mut report=contract["example"].clone();report["chat_id"]=json!("chat-a");
        assert_eq!(monitor.update(report.clone()).unwrap(),contract["response"]);
        report["id"]=json!("other-tool");monitor.update(report).unwrap();
        assert_eq!(monitor.for_chat("chat-a").as_array().unwrap().len(),2);
        assert_eq!(monitor.update(json!({})).unwrap(),contract);
        assert_eq!(monitor.snapshot()["jobs"].as_array().unwrap().len(),2);
        assert!(monitor.update(json!(null)).is_err());
        assert!(monitor.update(json!({"chat_id":"chat-a"})).is_err());
    }
    #[test]
    fn completion_requests_stop_and_duplicate_reports_do_not_restart_the_event() {
        let monitor=Monitor::default();let mut report=json!({"chat_id":"chat","id":"tool","text":"Done","progress":100});
        assert_eq!(monitor.update(report.clone()).unwrap(),json!({"ok":true,"stop":true}));
        let snapshot=monitor.snapshot();report["text"]=json!("Repeated");monitor.update(report.clone()).unwrap();assert_eq!(monitor.snapshot(),snapshot);
        report["progress"]=json!(0);assert_eq!(monitor.update(report).unwrap(),json!({"ok":true}));
        assert_eq!(monitor.snapshot()["jobs"][0]["progress"].as_f64(),Some(0.0));
    }
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
