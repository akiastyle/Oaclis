// assistant_stream.rs v1.9.199
//! Bounded, on-demand observation of already loaded Codex threads; never starts a turn.
use serde_json::{Value,json};
use std::{collections::{BTreeMap,VecDeque},io,os::unix::net::UnixStream,path::Path,sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering}},thread,time::{Duration,Instant}};
use tungstenite::{Message,WebSocket};
#[derive(Default)]
struct Feed { sequence:u64, events:VecDeque<Value>, items:BTreeMap<String,(bool,String)>, work:BTreeMap<String,String>, busy:bool, ready:bool }
struct Reader { touched:Mutex<Instant>, started:Mutex<Instant>, running:AtomicBool, feed:Mutex<Feed> }
impl Feed {
    fn push(&mut self,id:&str,kind:&str,text:&str) {
        if kind=="text" && text.is_empty(){return;}
        if text.is_empty() {self.sequence+=1;self.events.push_back(json!({"seq":self.sequence,"item":id,"kind":kind}));}
        else {
            let mut rest=text;
            while !rest.is_empty() {
                let mut end=rest.len().min(800);while !rest.is_char_boundary(end){end-=1;}
                self.sequence+=1;self.events.push_back(json!({"seq":self.sequence,"item":id,"kind":kind,"text":&rest[..end]}));rest=&rest[end..];
            }
        }
        while self.events.len()>256 {self.events.pop_front();}
    }
    fn event(&mut self,chat:&str,value:&Value) {
        let params=&value["params"];if params["threadId"]!=chat {return;}
        let item=&params["item"];
        let eligible=item["type"]=="agentMessage";
        match value["method"].as_str() {
            Some("turn/started")=>{self.busy=true;},
            Some("turn/completed")=>{self.busy=false;self.work.clear();},
            Some("thread/status/changed")=>{self.busy=params["status"]["type"]=="active";if !self.busy {self.work.clear();}},
            Some("item/completed") if item["type"]!="agentMessage"=>{if let Some(id)=item["id"].as_str(){self.work.remove(id);}},
            Some("item/started") if matches!(item["type"].as_str(),Some("commandExecution"|"fileChange"|"mcpToolCall"|"dynamicToolCall"|"webSearch")) => {
                if let Some(id)=item["id"].as_str() {
                    if self.work.len()>=16 {self.work.clear();}
                    self.work.insert(id.into(),item["type"].as_str().unwrap().into());self.push(id,"working","");
                    self.events.back_mut().unwrap()["activity"]=item["type"].clone();
                }
            },
            Some("item/started") if item["type"]=="agentMessage" => {
                if let Some(id)=item["id"].as_str() {
                    if self.items.len()>=16 {self.items.clear();}
                    self.items.insert(id.into(),(eligible,String::new()));
                    if eligible {self.push(id,"start","");}
                }
            },
            Some("item/agentMessage/delta")=>{
                if let (Some(id),Some(delta))=(params["itemId"].as_str(),params["delta"].as_str()) {
                    if let Some((allowed,text))=self.items.get_mut(id) {
                        let allowed=*allowed;
                        if text.len()+delta.len()<=262144 {text.push_str(delta);if allowed {self.push(id,"text",delta);}}
                    }
                }
            },
            Some("item/completed") if item["type"]=="agentMessage"=>{
                if let Some(id)=item["id"].as_str() {
                    let previous=self.items.remove(id);
                    if eligible {
                        let final_text=item["text"].as_str().unwrap_or("");
                        let rest=match previous.as_ref() {Some((true,text))=>final_text.strip_prefix(text).unwrap_or(""),_=>{self.push(id,"start","");final_text}};
                        self.push(id,"text",rest);self.push(id,"done","");
                    }
                }
            },
            _=>{},
        }
    }
    fn snapshot(&self,after:Option<u64>)->Value {
        let activity=self.work.values().next().cloned().or_else(||self.busy.then(||"processing".into()));
        let reset=after.is_some_and(|after|after>self.sequence || self.events.front().is_some_and(|v|after.saturating_add(1)<v["seq"].as_u64().unwrap_or(0)));
        if after.is_none() || reset {return json!({"ready":self.ready,"cursor":self.sequence,"reset":reset,"events":[],"activity":activity});}
        let mut events=Vec::new();let mut bytes=0;let mut cursor=after.unwrap();
        for event in self.events.iter().filter(|v|v["seq"].as_u64().unwrap_or(0)>after.unwrap()) {
            let size=serde_json::to_vec(event).unwrap().len();if bytes+size>5000 {break;}
            bytes+=size;cursor=event["seq"].as_u64().unwrap();events.push(event);
        }
        json!({"ready":self.ready,"cursor":cursor,"events":events,"activity":activity})
    }
}
fn send(socket:&mut WebSocket<UnixStream>,value:Value)->io::Result<()> {
    socket.send(Message::Text(value.to_string().into())).map_err(io::Error::other)
}
fn rpc(socket:&mut WebSocket<UnixStream>,id:u64,method:&str,params:Value)->io::Result<Value> {
    send(socket,json!({"id":id,"method":method,"params":params}))?;
    let deadline=Instant::now()+Duration::from_secs(5);
    while Instant::now()<deadline {
        match socket.read() {
            Ok(Message::Text(text))=>{let value:Value=serde_json::from_str(&text)?;if value["id"]==id {
                return value.get("result").cloned().ok_or_else(||io::Error::other("Osservazione Codex non disponibile"));
            }},
            Err(tungstenite::Error::Io(error)) if matches!(error.kind(),io::ErrorKind::WouldBlock|io::ErrorKind::TimedOut)=>{},
            Err(error)=>return Err(io::Error::other(error)),_=>{},
        }
    }
    Err(io::Error::new(io::ErrorKind::TimedOut,"Osservazione Codex non disponibile"))
}
fn follow(root:&Path,chat:&str,reader:&Reader)->io::Result<()> {
    let stream=UnixStream::connect(root.join("app-server-control/app-server-control.sock"))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let config=tungstenite::protocol::WebSocketConfig::default().max_message_size(Some(1048576)).max_frame_size(Some(1048576));
    let (mut socket,_)=tungstenite::client::client_with_config("ws://localhost/",stream,Some(config)).map_err(io::Error::other)?;
    rpc(&mut socket,0,"initialize",json!({"clientInfo":{"name":"oaclis-speech","version":env!("OACLIS_APP_VERSION")}}))?;
    send(&mut socket,json!({"method":"initialized","params":{}}))?;
    let loaded=rpc(&mut socket,1,"thread/loaded/list",json!({}))?;
    if !loaded["data"].as_array().is_some_and(|ids|ids.iter().any(|id|id==chat)) {return Err(io::Error::other("Chat non attiva"));}
    let resumed=rpc(&mut socket,2,"thread/resume",json!({"threadId":chat,"excludeTurns":true}))?;
    {let mut feed=reader.feed.lock().unwrap();feed.work.clear();feed.busy=resumed["thread"]["status"]["type"]=="active";feed.ready=true;}
    while reader.touched.lock().unwrap().elapsed()<Duration::from_secs(30) {
        match socket.read() {
            Ok(Message::Text(text))=>{if let Ok(value)=serde_json::from_str(&text){reader.feed.lock().unwrap().event(chat,&value);}},
            Err(tungstenite::Error::Io(error)) if matches!(error.kind(),io::ErrorKind::WouldBlock|io::ErrorKind::TimedOut)=>{},
            Err(error)=>return Err(io::Error::other(error)),_=>{},
        }
    }
    let _=socket.close(None);Ok(())
}
pub(crate) fn snapshot(chat:&str,after:Option<u64>)->Value {
    static READERS:OnceLock<Mutex<BTreeMap<String,Arc<Reader>>>>=OnceLock::new();
    let mut readers=READERS.get_or_init(Default::default).lock().unwrap();
    readers.retain(|_,r|r.running.load(Ordering::Relaxed)||r.touched.lock().unwrap().elapsed()<Duration::from_secs(60));
    if !readers.contains_key(chat) {
        if readers.len()>=4 {return json!({"ready":false,"cursor":after.unwrap_or(0),"events":[]});}
        readers.insert(chat.into(),Arc::new(Reader{touched:Mutex::new(Instant::now()),started:Mutex::new(Instant::now()-Duration::from_secs(6)),running:AtomicBool::new(false),feed:Mutex::new(Feed::default())}));
    }
    let reader=readers[chat].clone();*reader.touched.lock().unwrap()=Instant::now();
    if !reader.running.load(Ordering::Relaxed) && reader.started.lock().unwrap().elapsed()>=Duration::from_secs(5) {
        *reader.started.lock().unwrap()=Instant::now();
        reader.running.store(true,Ordering::Relaxed);let worker=reader.clone();let chat=chat.to_owned();
        thread::spawn(move||{
            let root=std::env::var_os("CODEX_HOME").map(std::path::PathBuf::from).unwrap_or_else(||std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".codex"));
            if let Err(error)=follow(&root,&chat,&worker){crate::state_transport::log(format!("Speech {chat}: {error}"));}
            worker.feed.lock().unwrap().ready=false;worker.running.store(false,Ordering::Relaxed);
        });
    }
    reader.feed.lock().unwrap().snapshot(after)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn feed_keeps_only_answer_items_and_uses_bounded_replay_cursors() {
        let mut feed=Feed::default();let chat="chat";
        for (id,kind,phase) in [("tool","commandExecution",""),("status","agentMessage","commentary"),("answer","agentMessage","final_answer")] {
            feed.event(chat,&json!({"method":"item/started","params":{"threadId":chat,"item":{"id":id,"type":kind,"phase":phase}}}));
            feed.event(chat,&json!({"method":"item/agentMessage/delta","params":{"threadId":chat,"itemId":id,"delta":"hello. "}}));
            feed.event(chat,&json!({"method":"item/completed","params":{"threadId":chat,"item":{"id":id,"type":kind,"phase":phase,"text":"hello. final"}}}));
        }
        assert!(feed.events.iter().all(|v|(v["item"]=="answer" || v["item"]=="status") || (v["item"]=="tool" && v["kind"]=="working")));
        assert_eq!(feed.events.iter().filter(|v|v["kind"]=="working").count(),1);
        assert_eq!(feed.events.iter().filter_map(|v|v["text"].as_str()).collect::<String>(),"hello. finalhello. final");
        assert_eq!(feed.snapshot(None)["events"],json!([]));
        let result=feed.snapshot(Some(0));let cursor=result["cursor"].as_u64().unwrap();
        assert_eq!(feed.snapshot(Some(cursor))["events"],json!([]));
        for _ in 0..300 {feed.push("later","text","part");}
        assert_eq!(feed.events.len(),256);assert_eq!(feed.snapshot(Some(0))["reset"],true);
        feed.event(chat,&json!({"method":"turn/started","params":{"threadId":chat}}));
        assert_eq!(feed.snapshot(None)["activity"],"processing");
        feed.event(chat,&json!({"method":"item/started","params":{"threadId":chat,"item":{"id":"long","type":"commandExecution"}}}));
        assert_eq!(feed.snapshot(None)["activity"],"commandExecution");
        feed.event(chat,&json!({"method":"turn/completed","params":{"threadId":chat}}));
        assert!(feed.snapshot(None)["activity"].is_null());
    }
    #[test]
    fn observer_subscribes_only_to_loaded_thread_without_starting_turns() {
        let root=std::env::temp_dir().join(format!("oaclis-speech-test-{}",std::process::id()));
        std::fs::create_dir_all(root.join("app-server-control")).unwrap();
        let listener=std::os::unix::net::UnixListener::bind(root.join("app-server-control/app-server-control.sock")).unwrap();
        let server=thread::spawn(move||{
            let (stream,_)=listener.accept().unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut ws=tungstenite::accept(stream).unwrap();
            let read=|ws:&mut WebSocket<UnixStream>|->Value {loop {if let Message::Text(text)=ws.read().unwrap(){break serde_json::from_str(&text).unwrap();}}};
            let request=read(&mut ws);assert_eq!(request["method"],"initialize");send(&mut ws,json!({"id":0,"result":{}})).unwrap();
            assert_eq!(read(&mut ws)["method"],"initialized");
            assert_eq!(read(&mut ws)["method"],"thread/loaded/list");send(&mut ws,json!({"id":1,"result":{"data":["chat"]}})).unwrap();
            let request=read(&mut ws);assert_eq!(request["method"],"thread/resume");assert_eq!(request["params"]["excludeTurns"],true);
            send(&mut ws,json!({"id":2,"result":{}})).unwrap();
            for value in [json!({"method":"item/started","params":{"threadId":"chat","item":{"id":"answer","type":"agentMessage","phase":"final_answer"}}}),json!({"method":"item/agentMessage/delta","params":{"threadId":"chat","itemId":"answer","delta":"Hello."}})] {send(&mut ws,value).unwrap();}
            let _=ws.close(None);
        });
        let reader=Reader{touched:Mutex::new(Instant::now()),started:Mutex::new(Instant::now()),running:AtomicBool::new(true),feed:Mutex::new(Feed::default())};
        let _=follow(&root,"chat",&reader);server.join().unwrap();
        assert!(reader.feed.lock().unwrap().events.iter().any(|v|v["text"]=="Hello."));
        std::fs::remove_dir_all(root).unwrap();
    }
}
