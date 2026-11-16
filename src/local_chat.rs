// local_chat.rs v1.0.0
//! llama.cpp conversations, owned by the existing state store and shared console transport.
use crate::{invalid, state_store};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,VecDeque},io::{self,BufRead,BufReader,Read},path::Path,sync::{Arc,Mutex,OnceLock,atomic::{AtomicBool,Ordering}},time::{SystemTime,UNIX_EPOCH,Duration}};

#[derive(Default)]
struct Output { turn:String, touched:Option<std::time::Instant>, running:bool, partial:String, error:String, sequence:u64, events:VecDeque<Value> }
#[derive(Default)]
struct Session { stopped:AtomicBool, output:Mutex<Output> }
static SESSIONS:OnceLock<Mutex<BTreeMap<String,Arc<Session>>>>=OnceLock::new();
static INFERENCE:Mutex<()>=Mutex::new(());
fn now()->u64 {SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64}
fn identifier()->io::Result<String> {
    use ring::rand::SecureRandom;
    let mut bytes=[0u8;16];ring::rand::SystemRandom::new().fill(&mut bytes).map_err(|_|invalid("Identificativo chat non disponibile"))?;
    bytes[6]=(bytes[6]&15)|64;bytes[8]=(bytes[8]&63)|128;
    let hex=bytes.iter().map(|b|format!("{b:02x}")).collect::<String>();
    Ok(format!("{}-{}-{}-{}-{}",&hex[..8],&hex[8..12],&hex[12..16],&hex[16..20],&hex[20..]))
}
pub(crate) fn contains(config:&Path,id:&str)->io::Result<bool> {Ok(state_store::snapshot(config)?.conversations.contains_key(id))}
pub(crate) fn entry(chat:&Value)->Value {
    json!({"id":chat["id"],"name":chat["name"],"engine":"llamacpp","model":chat["model"],"cwd":chat["cwd"],"path":"","relative_path":"","bytes":0,"modified_ns":format!("{}000000",chat["updated"].as_u64().unwrap_or(0))})
}
pub(crate) fn create(home:&Path,cwd:&Path,name:&str,model:&str)->io::Result<Value> {
    if name.is_empty()||!cwd.is_absolute(){return Err(invalid("Nome o cartella chat non validi"));}
    crate::terminal::validate_model(model)?;
    let catalog=crate::llama::models(home)?;
    if !catalog["models"].as_array().unwrap().iter().any(|row|row["id"]==model){return Err(invalid("Scegli un modello llama.cpp disponibile"));}
    let id=identifier()?;std::fs::create_dir_all(cwd)?;
    let chat=json!({"id":id,"name":name,"model":model,"cwd":cwd,"updated":now(),"messages":[]});
    state_store::change(&home.join(".config/oaclis"),|state|{Arc::make_mut(&mut state.conversations).insert(id,Arc::new(chat.clone()));Ok(())})?;
    Ok(entry(&chat))
}
pub(crate) fn rename(config:&Path,id:&str,name:&str)->io::Result<()> {
    if name.trim().is_empty(){return Err(invalid("Nome chat non valido"));}
    crate::appearance::Appearance{label:name.into(),..Default::default()}.validate()?;
    state_store::change(config,|state|{
        let chat=Arc::make_mut(Arc::make_mut(&mut state.conversations).get_mut(id).ok_or_else(||invalid("Chat non disponibile"))?);
        chat["name"]=json!(name);chat["updated"]=json!(now());Ok(())
    })
}
pub(crate) fn stop(id:&str) {
    if let Some(sessions)=SESSIONS.get(){if let Some(session)=sessions.lock().unwrap().remove(id){session.stopped.store(true,Ordering::Relaxed);}}
}
impl Output {
    fn event(&mut self,turn:&str,kind:&str,text:&str) {
        let mut rest=text;
        loop {
            let mut end=rest.len().min(800);while !rest.is_char_boundary(end){end-=1;}
            self.sequence+=1;self.events.push_back(json!({"seq":self.sequence,"item":turn,"kind":kind,"text":&rest[..end]}));
            if end==rest.len(){break;}rest=&rest[end..];
        }
        while self.events.len()>256 {self.events.pop_front();}
    }
    fn speech(&self,after:Option<u64>)->Value {
        let reset=after.is_some_and(|cursor|cursor>self.sequence||self.events.front().is_some_and(|e|cursor.saturating_add(1)<e["seq"].as_u64().unwrap()));
        let mut events=Vec::new();let mut size=0;let mut cursor=after.unwrap_or(self.sequence);
        if after.is_some()&&!reset {
            for event in self.events.iter().filter(|e|e["seq"].as_u64().unwrap()>after.unwrap()) {
                size+=event.to_string().len();if size>5000{break;}
                cursor=event["seq"].as_u64().unwrap();events.push(event.clone());
            }
        }else{cursor=self.sequence;}
        json!({"ready":true,"cursor":cursor,"reset":reset,"events":events,"activity":if self.running {Some("Working")}else{None},"waiting":false})
    }
}
fn session(id:&str)->Arc<Session> {SESSIONS.get_or_init(Default::default).lock().unwrap().entry(id.into()).or_default().clone()}
fn context(chat:&Value)->Vec<Value> {
    let mut messages=Vec::new();let mut size=0;
    for message in chat["messages"].as_array().unwrap().iter().rev() {
        let text=message["content"].as_str().unwrap_or("");if text.is_empty(){continue;}
        if size+text.len()>24000 {break;}size+=text.len();messages.push(json!({"role":message["role"],"content":text}));
    }
    messages.reverse();messages.insert(0,json!({"role":"system","content":"Sei un assistente conversazionale. Rispondi in italiano. Non hai accesso a file, terminale o strumenti: non dichiarare di aver eseguito operazioni."}));messages
}
// Templates can split reasoning tags across SSE chunks; never forward reasoning to speech.
#[derive(Default)]
struct Visible { pending:String, thinking:bool }
impl Visible {
    fn push(&mut self,text:&str)->String {
        self.pending.push_str(text);let mut visible=String::new();
        loop {
            let tag=if self.thinking {"</think>"}else{"<think>"};
            if let Some(at)=self.pending.find(tag) {
                if !self.thinking {visible.push_str(&self.pending[..at]);}
                self.pending.drain(..at+tag.len());self.thinking=!self.thinking;
            }else{
                let keep=(1..tag.len()).rev().find(|n|self.pending.ends_with(&tag[..*n])).unwrap_or(0);
                let end=self.pending.len()-keep;
                if !self.thinking {visible.push_str(&self.pending[..end]);}
                self.pending.drain(..end);break;
            }
        }
        visible
    }
}
pub(crate) fn expire() {
    if let Some(sessions)=SESSIONS.get(){sessions.lock().unwrap().retain(|_,session|Arc::strong_count(session)>1||session.output.lock().unwrap().touched.is_some_and(|t|t.elapsed()<Duration::from_secs(3600)));}
}
fn stream(endpoint:&str,key:&str,model:&str,messages:Vec<Value>,session:&Session,turn:&str)->io::Result<()> {
    let client:ureq::Agent=ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(180))).timeout_connect(Some(Duration::from_secs(3))).max_redirects(0).build().into();
    let body=json!({"model":model,"messages":messages,"stream":true,"max_tokens":1024,"chat_template_kwargs":{"enable_thinking":false}}).to_string();
    let mut response=client.post(endpoint).header("Authorization",&format!("Bearer {key}")).header("Content-Type","application/json")
        .send(body.as_bytes()).map_err(|_|io::Error::other("Collegamento llama.cpp non disponibile"))?;
    let mut reader=BufReader::new(response.body_mut().as_reader());let mut done=false;let mut filter=Visible::default();
    loop {
        if session.stopped.load(Ordering::Relaxed){return Ok(());}
        let mut line=String::new();let count=reader.by_ref().take(65537).read_line(&mut line)?;
        if count==0 {break;}
        if line.len()>65536 {return Err(invalid("Risposta llama.cpp troppo grande"));}
        let Some(data)=line.trim().strip_prefix("data: ") else {continue;};
        if data=="[DONE]" {done=true;break;}
        let value:Value=serde_json::from_str(data).map_err(|_|invalid("Risposta llama.cpp non valida"))?;
        if value.get("error").is_some(){return Err(invalid("Generazione llama.cpp non riuscita"));}
        let text=value["choices"][0]["delta"]["content"].as_str().unwrap_or("");
        let visible=filter.push(text);
        if !visible.is_empty(){let mut output=session.output.lock().unwrap();
            if output.partial.len()+visible.len()>262144{return Err(invalid("Risposta llama.cpp troppo lunga"));}
            output.partial.push_str(&visible);output.event(turn,"text",&visible);
        }
    }
    if !done {return Err(io::Error::other("Risposta llama.cpp interrotta"));}
    if session.output.lock().unwrap().partial.trim().is_empty(){return Err(io::Error::other("Risposta llama.cpp vuota"));}
    Ok(())
}
fn generate(home:std::path::PathBuf,id:String,turn:String,model:String,messages:Vec<Value>,session:Arc<Session>) {
    let result=(||{
        let _slot=INFERENCE.lock().unwrap();
        if session.stopped.load(Ordering::Relaxed){return Ok(());}
        let key=crate::engines::llama_key(&home)?;
        stream("http://127.0.0.1:18085/v1/chat/completions",key.trim(),&model,messages,&session,&turn)
    })();
    let mut output=session.output.lock().unwrap();let text=output.partial.clone();
    let error=result.err().map(|e|e.to_string()).unwrap_or_default();
    let status=if session.stopped.load(Ordering::Relaxed){"interrupted"}else if error.is_empty(){"completed"}else{"failed"};
    let saved=state_store::change(&home.join(".config/oaclis"),|state|{
        let Some(chat)=Arc::make_mut(&mut state.conversations).get_mut(&id) else {return Ok(());};
        let chat=Arc::make_mut(chat);
        if let Some(message)=chat["messages"].as_array_mut().unwrap().iter_mut().find(|m|m["id"]==turn){message["content"]=json!(text);message["status"]=json!(status);message["error"]=json!(error);}
        chat["updated"]=json!(now());Ok(())
    });
    output.error=if saved.is_err(){"Risposta non salvata. Riprova.".into()}else{String::new()};
    output.running=false;output.event(&turn,"done","");
}
pub(crate) fn request(home:&Path,value:&Value)->io::Result<Value> {
    let config=home.join(".config/oaclis");let id=value["chat_id"].as_str().unwrap();let action=value["action"].as_str().unwrap();
    if action=="stop" {stop(id);return Ok(json!({"ok":true}));}
    if action=="reset" {return Ok(json!({"ok":true}));}
    if crate::chat_deletion::blocked(&config,id)? {return Err(invalid("Chat eliminata definitivamente"));}
    let session=session(id);
    if session.stopped.load(Ordering::Relaxed){return Ok(json!({"stopped":true}));}
    if action=="open"&&!session.output.lock().unwrap().running {
        state_store::change(&config,|state|{
            let chat=Arc::make_mut(Arc::make_mut(&mut state.conversations).get_mut(id).ok_or_else(||invalid("Chat non disponibile"))?);
            for message in chat["messages"].as_array_mut().unwrap(){if message["status"]=="running"{message["status"]=json!("interrupted");}}
            Ok(())
        })?;
    }
    if action=="input" {
        if value["image"].is_string(){return Err(invalid("Questo collegamento llama.cpp accetta solo testo"));}
        let text=value["text"].as_str().unwrap_or("").trim();let key=value["key"].as_str().unwrap_or("");
        if key=="Escape" {stop(id);return request(home,&json!({"chat_id":id,"action":"read","speech":value["speech"],"speech_cursor":value["speech_cursor"]}));}
        if !text.is_empty() {
            let mut output=session.output.lock().unwrap();
            if output.running{return Err(invalid("llama.cpp sta rispondendo: attendi o interrompi la sessione"));}
            if text=="/model" {
                let catalog=crate::llama::models(home)?;
                let list=catalog["models"].as_array().unwrap().iter().filter_map(|m|m["id"].as_str()).collect::<Vec<_>>().join("\n");
                output.error=format!("Modelli disponibili:\n{list}\n\nPer cambiare: /model NOME-MODELLO");
            }else if let Some(model)=text.strip_prefix("/model ") {
                crate::terminal::validate_model(model)?;let catalog=crate::llama::models(home)?;
                if !catalog["models"].as_array().unwrap().iter().any(|m|m["id"]==model){return Err(invalid("Modello llama.cpp non disponibile"));}
                state_store::change(&config,|state|{Arc::make_mut(Arc::make_mut(&mut state.conversations).get_mut(id).ok_or_else(||invalid("Chat non disponibile"))?)["model"]=json!(model);Ok(())})?;
                output.error=format!("Modello selezionato: {model}");
            }else {
                let turn=identifier()?;
                let (model,messages)=state_store::change(&config,|state|{
                    let chat=Arc::make_mut(Arc::make_mut(&mut state.conversations).get_mut(id).ok_or_else(||invalid("Chat non disponibile"))?);
                    let messages=chat["messages"].as_array_mut().unwrap();
                    messages.push(json!({"role":"user","content":text}));messages.push(json!({"id":turn,"role":"assistant","content":"","status":"running"}));
                    chat["updated"]=json!(now());Ok((chat["model"].as_str().unwrap().to_owned(),context(chat)))
                })?;
                output.turn=turn.clone();output.running=true;output.partial.clear();output.error.clear();output.event(&turn,"start","");drop(output);
                let (home,id,session)=(home.to_owned(),id.to_owned(),session.clone());
                std::thread::spawn(move||generate(home,id,turn,model,messages,session));
            }
        }
    }
    let state=state_store::snapshot(&config)?;let chat=state.conversations.get(id).ok_or_else(||invalid("Chat non disponibile"))?;let mut output=session.output.lock().unwrap();output.touched=Some(std::time::Instant::now());
    let mut screen=format!("llama.cpp · {}\n\n",chat["model"].as_str().unwrap());
    for message in chat["messages"].as_array().unwrap() {
        let text=if message["status"]=="running"&&output.running&&message["id"]==output.turn {output.partial.as_str()}else{message["content"].as_str().unwrap_or("")};
        if !text.is_empty(){screen.push_str(if message["role"]=="user"{"› "}else{"• "});screen.push_str(text);screen.push_str("\n\n");}
        if let Some(error)=message["error"].as_str().filter(|e|!e.is_empty()){screen.push_str(error);screen.push_str("\n\n");}
        if message["status"]=="running"&&(!output.running||message["id"]!=output.turn) {screen.push_str("Generazione interrotta. Puoi inviare un nuovo messaggio.\n\n");}
    }
    if output.running {screen.push_str("Working · llama.cpp\n");}
    if !output.error.is_empty(){screen.push_str(&output.error);}
    let start=screen.char_indices().rev().nth(13999).map_or(0,|(i,_)|i);let screen=&screen[start..];
    let mut response=crate::terminal::screen_response(screen,screen,&value["screen_revision"],action=="read");
    response["active_access"]=json!("read-only");response["jobs"]=crate::jobs::for_chat(id);
    if value["speech"]==true {response["speech"]=output.speech(value["speech_cursor"].as_u64());}
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streaming_filters_split_reasoning_and_uses_shared_speech_protocol() {
        let server=tiny_http::Server::http("127.0.0.1:0").unwrap();let endpoint=format!("http://{}/v1/chat/completions",server.server_addr());
        let worker=std::thread::spawn(move||{
            let mut request=server.recv().unwrap();let mut body=String::new();request.as_reader().read_to_string(&mut body).unwrap();
            let body:Value=serde_json::from_str(&body).unwrap();assert_eq!(body["model"],"fixture");assert_eq!(body["stream"],true);
            assert!(request.headers().iter().any(|h|h.field.equiv("Authorization")&&h.value.as_str()=="Bearer fixture-key"));
            let mut data=String::new();for part in ["<th","ink>privato</th","ink>Ri","sposta"] {data.push_str(&format!("data: {}\n\n",json!({"choices":[{"delta":{"content":part}}]})));}data.push_str("data: [DONE]\n\n");
            request.respond(tiny_http::Response::from_string(data)).unwrap();
        });
        let session=Session::default();session.output.lock().unwrap().event("turn","start","");
        stream(&endpoint,"fixture-key","fixture",vec![],&session,"turn").unwrap();worker.join().unwrap();
        let output=session.output.lock().unwrap();assert_eq!(output.partial,"Risposta");let speech=output.speech(Some(0));
        assert_eq!(speech["events"][0]["kind"],"start");assert!(!speech.to_string().contains("privato"));
        let mut long=Output::default();long.event("turn","text",&"è".repeat(4000));assert!(!long.speech(Some(0))["events"].as_array().unwrap().is_empty());
    }
    #[test]
    fn local_archive_is_persisted_renamed_and_purged_without_codex() {
        let root=std::env::temp_dir().join(format!("oaclis-local-{}",identifier().unwrap()));let config=root.join(".config/oaclis");let id=identifier().unwrap();
        state_store::change(&config,|state|{Arc::make_mut(&mut state.conversations).insert(id.clone(),Arc::new(json!({"id":id,"name":"Prova","model":"fixture","cwd":root,"updated":now(),"messages":[{"role":"user","content":"Codice LUNA42"},{"id":"pending","role":"assistant","content":"","status":"running"}]})));Ok(())}).unwrap();
        let reply=request(&root,&json!({"chat_id":id,"action":"open"})).unwrap();assert!(reply["screen"].as_str().unwrap().contains("Codice LUNA42"));
        rename(&config,&id,"Rinominata").unwrap();
        let disk:Value=serde_json::from_slice(&std::fs::read(config.join("llama-chats.json")).unwrap()).unwrap();assert_eq!(disk[&id]["name"],"Rinominata");assert_eq!(disk[&id]["messages"][1]["status"],"interrupted");
        let mut chat=disk[&id].clone();chat["messages"].as_array_mut().unwrap().push(json!({"role":"user","content":"Quale codice?"}));assert_eq!(context(&chat)[1]["content"],"Codice LUNA42");
        state_store::change(&config,|state|{crate::chat_deletion::merge(state,&json!({id.clone():{"targets":[],"done":[],"global":false}}));Ok(())}).unwrap();
        assert!(!contains(&config,&id).unwrap());assert!(!root.join(".codex").exists());std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore="Explicit real Jetson lifecycle, isolated chat store and no Codex archive"]
    fn installed_jetson_chat_lifecycle() {
        let real_home=std::path::PathBuf::from(std::env::var_os("HOME").unwrap());let key=crate::engines::llama_key(&real_home).unwrap();
        let root=std::env::temp_dir().join(format!("oaclis-jetson-chat-{}",identifier().unwrap()));let config=root.join(".config/oaclis");
        crate::engines::save(&root,&json!({"default_engine":"llamacpp","enabled":["llamacpp"],"models":{"llamacpp":"Qwen3-4B-Q4_K_M"},"keys":{"llamacpp":key}})).unwrap();
        let entry=create(&root,&root.join("project"),"Chat test locale","Qwen3-4B-Q4_K_M").unwrap();let id=entry["id"].as_str().unwrap();
        request(&root,&json!({"chat_id":id,"action":"open"})).unwrap();
        for text in ["Memorizza il codice LUNA42. Rispondi solo OK.","Quale codice ti ho dato? Rispondi solo con il codice."] {
            request(&root,&json!({"chat_id":id,"action":"input","key":"Enter","text":text})).unwrap();
            let until=std::time::Instant::now()+Duration::from_secs(180);
            while session(id).output.lock().unwrap().running {assert!(std::time::Instant::now()<until);std::thread::sleep(Duration::from_millis(100));}
            let state=state_store::snapshot(&config).unwrap();let last=state.conversations[id]["messages"].as_array().unwrap().last().unwrap();assert_eq!(last["status"],"completed","{last}");println!("llama.cpp: {}",last["content"]);
        }
        let before=request(&root,&json!({"chat_id":id,"action":"read"})).unwrap();assert!(before["screen"].as_str().unwrap().contains("LUNA42"));
        let unchanged=request(&root,&json!({"chat_id":id,"action":"read","screen_revision":before["screen_revision"]})).unwrap();assert_eq!(unchanged["unchanged"],true);
        stop(id);let reopened=request(&root,&json!({"chat_id":id,"action":"open"})).unwrap();assert!(reopened["screen"].as_str().unwrap().contains("LUNA42"));
        let disk:Value=serde_json::from_slice(&std::fs::read(config.join("llama-chats.json")).unwrap()).unwrap();assert_eq!(disk[id]["messages"].as_array().unwrap().len(),4);assert!(!root.join(".codex").exists());
        let canceled=session(id);
        request(&root,&json!({"chat_id":id,"action":"input","key":"Enter","text":"Scrivi un racconto molto lungo su una nave spaziale."})).unwrap();
        let until=std::time::Instant::now()+Duration::from_secs(30);
        while canceled.output.lock().unwrap().partial.trim().is_empty() {assert!(std::time::Instant::now()<until);std::thread::sleep(Duration::from_millis(50));}
        stop(id);
        while canceled.output.lock().unwrap().running {assert!(std::time::Instant::now()<until);std::thread::sleep(Duration::from_millis(50));}
        let state=state_store::snapshot(&config).unwrap();assert_eq!(state.conversations[id]["messages"].as_array().unwrap().last().unwrap()["status"],"interrupted");
        println!("Interruzione dello streaming confermata; storico conservato.");
        std::fs::remove_dir_all(root).unwrap();
    }
}
