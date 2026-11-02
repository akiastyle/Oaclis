// mobile_auth.rs v1.48.698
//! Local mobile authority: QR-bound keys, chat grants and signed, non-replayable requests.
use crate::invalid;
use ring::{hmac, rand::{SecureRandom, SystemRandom}, signature};
use serde::{Deserialize, Serialize};
use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use std::{collections::{BTreeMap,BTreeSet}, io, path::{Path,PathBuf}, sync::{Arc,Mutex,OnceLock,RwLock,atomic::{AtomicU64,Ordering}}, time::{SystemTime,UNIX_EPOCH}};

#[derive(Clone,Default,Deserialize,Serialize,PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Authority {
    #[serde(default)] invitations:BTreeMap<String,Invitation>,
    #[serde(default)] pub phones:BTreeMap<String,Phone>,
}
#[derive(Clone,Deserialize,Serialize,PartialEq)]
#[serde(deny_unknown_fields)]
struct Invitation { secret:String, until:u64 }
#[derive(Clone,Deserialize,Serialize,PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Phone { pub name:String, public_key:String, pub chats:BTreeSet<String> }
#[derive(Clone,Deserialize,Serialize,PartialEq)]
#[serde(deny_unknown_fields)]
struct Settings { admin_key:String, #[serde(default)] authorization:Authority }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedRequest { purpose:String, epoch:String, device:String, id:String, issued:u64, chat_id:String, action:String, text:String, key:String, #[serde(default)] speech:bool, #[serde(default)] speech_cursor:Option<u64>, #[serde(default)] image:Option<String>, #[serde(default)] icon:String, #[serde(default)] model:String, #[serde(default)] engine:String }

pub(crate) struct Store {
    config:PathBuf,
    settings:RwLock<Arc<Settings>>,
    writer:Mutex<()>,
    epoch:String,
    seen:Mutex<BTreeMap<String,u64>>,
    last_time:AtomicU64,
}
pub(crate) fn now()->u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
pub(crate) fn hex(bytes:&[u8])->String { bytes.iter().map(|b|format!("{b:02x}")).collect() }
fn unhex(value:&str,length:usize)->io::Result<Vec<u8>> {
    if value.len()!=length*2 || !value.bytes().all(|b|b.is_ascii_hexdigit()) {return Err(invalid("Prova mobile non valida"));}
    value.as_bytes().chunks_exact(2).map(|part|u8::from_str_radix(std::str::from_utf8(part).unwrap(),16).map_err(io::Error::other)).collect()
}
fn random()->io::Result<String> {let mut bytes=[0u8;32];SystemRandom::new().fill(&mut bytes).map_err(|_|io::Error::other("Generazione sicurezza mobile non riuscita"))?;Ok(hex(&bytes))}
fn verify(public_key:&str,payload:&str,proof:&str)->io::Result<()> {
    let key=unhex(public_key,65)?;let proof=unhex(proof,64)?;
    signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED,key).verify(payload.as_bytes(),&proof)
        .map_err(|_|invalid("Firma mobile non valida"))
}
pub(crate) fn at(config:&Path)->io::Result<Arc<Store>> {
    static STORES:OnceLock<Mutex<BTreeMap<PathBuf,Arc<Store>>>>=OnceLock::new();
    let config=config.canonicalize()?;
    let mut stores=STORES.get_or_init(Default::default).lock().unwrap();
    if let Some(store)=stores.get(&config){return Ok(store.clone());}
    let settings:Settings=serde_json::from_reader(crate::open_regular(&config.join("mobile-relay.json"))?.0).map_err(|_|invalid("Collegamento mobile da configurare"))?;
    unhex(&settings.admin_key,32)?;
    let store=Arc::new(Store{config:config.clone(),settings:RwLock::new(Arc::new(settings)),writer:Mutex::new(()),epoch:random()?,seen:Mutex::new(BTreeMap::new()),last_time:AtomicU64::new(now())});
    stores.insert(config,store.clone());Ok(store)
}
impl Store {
    pub(crate) fn admin_key(&self)->String {self.settings.read().unwrap().admin_key.clone()}
    pub(crate) fn authority(&self)->Authority {self.settings.read().unwrap().authorization.clone()}
    fn change<T>(&self,edit:impl FnOnce(&mut Authority)->io::Result<T>)->io::Result<T> {
        let _writer=self.writer.lock().unwrap();
        let old=self.settings.read().unwrap().clone();let mut next=(*old).clone();
        let result=edit(&mut next.authorization)?;
        if next!=*old {crate::staging::save_json(&self.config.join("mobile-relay.json"),&next)?;*self.settings.write().unwrap()=Arc::new(next);}
        Ok(result)
    }
    pub(crate) fn security(&self)->Value {json!({"epoch":self.epoch,"time":now()})}
    pub(crate) fn invitation(&self)->io::Result<(String,String)> {
        self.change(|auth|{
            auth.invitations.retain(|_,invite|invite.until>now());
            if auth.invitations.len()>=20{return Err(invalid("Troppi inviti mobile in attesa"));}
            let secret=random()?;let hash=hex(&Sha256::digest(secret.as_bytes()));
            auth.invitations.insert(hash.clone(),Invitation{secret:secret.clone(),until:now()+600});Ok((hash,secret))
        })
    }
    pub(crate) fn discard_invitation(&self,hash:&str)->io::Result<()> {self.change(|auth|{auth.invitations.remove(hash);Ok(())})}
    fn pairing(&self,id:&str,name:&str,binding:&Value)->io::Result<(String,String)> {
        let hash=binding["invite_hash"].as_str().ok_or_else(||invalid("Nuovo abbinamento mobile richiesto"))?;
        let payload=binding["payload"].as_str().filter(|s|s.len()<=2048).ok_or_else(||invalid("Prova abbinamento mancante"))?;
        let (purpose,device,label,key):(String,String,String,String)=serde_json::from_str(payload).map_err(|_|invalid("Prova abbinamento non valida"))?;
        if purpose!="oaclis-pair-v1" || device!=id || label!=name {return Err(invalid("Abbinamento mobile non corrispondente"));}
        let authority=self.authority();
        let invitation=authority.invitations.get(hash).filter(|invite|invite.until>=now()).ok_or_else(||invalid("Invito mobile scaduto"))?;
        let proof=unhex(binding["proof"].as_str().unwrap_or(""),32)?;
        hmac::verify(&hmac::Key::new(hmac::HMAC_SHA256,invitation.secret.as_bytes()),payload.as_bytes(),&proof).map_err(|_|invalid("Prova abbinamento non valida"))?;
        verify(&key,payload,binding["key_proof"].as_str().unwrap_or(""))?;
        Ok((hash.into(),key))
    }
    pub(crate) fn pending(&self,id:&str,name:&str,binding:&Value)->bool {self.pairing(id,name,binding).is_ok()}
    pub(crate) fn approve(&self,id:&str,name:&str,binding:&Value)->io::Result<()> {
        let (hash,key)=self.pairing(id,name,binding)?;
        self.change(|auth|{
            if !auth.invitations.get(&hash).is_some_and(|invite|invite.until>=now()) || auth.phones.contains_key(id){return Err(invalid("Invito mobile non piu' disponibile"));}
            auth.phones.insert(id.into(),Phone{name:name.into(),public_key:key,chats:BTreeSet::new()});auth.invitations.remove(&hash);Ok(())
        })
    }
    pub(crate) fn revoke(&self,id:&str)->io::Result<()> {self.change(|auth|{auth.phones.remove(id);Ok(())})}
    pub(crate) fn grants(&self,id:&str,chats:BTreeSet<String>)->io::Result<()> {
        self.change(|auth|{auth.phones.get_mut(id).ok_or_else(||invalid("Nuovo abbinamento mobile richiesto"))?.chats=chats;Ok(())})
    }
    pub(crate) fn allow_created(&self,device:&str,parent:&str,chat:&str)->io::Result<()> {
        self.change(|auth|{let phone=auth.phones.get_mut(device).filter(|phone|phone.chats.contains(parent)).ok_or_else(||invalid("Telefono non autorizzato dal master"))?;phone.chats.insert(chat.into());Ok(())})
    }
    pub(crate) fn forget_chat(&self,id:&str)->io::Result<()> {self.change(|auth|{for phone in auth.phones.values_mut(){phone.chats.remove(id);}Ok(())})}
    pub(crate) fn command(&self,envelope:&Value)->io::Result<Value> {
        let device=envelope["device"].as_str().unwrap_or("");
        let settings=self.settings.read().unwrap().clone();let phone=settings.authorization.phones.get(device).ok_or_else(||invalid("Telefono non autorizzato dal master"))?;
        let signed=&envelope["signed"];
        let payload=signed["payload"].as_str().filter(|s|s.len()<=786432).ok_or_else(||invalid("Richiesta mobile non firmata"))?;
        verify(&phone.public_key,payload,signed["signature"].as_str().unwrap_or(""))?;
        let current=now();
        if current<self.last_time.fetch_max(current,Ordering::Relaxed){return Err(invalid("Orologio locale non allineato"));}
        let command:SignedRequest=serde_json::from_str(payload).map_err(|_|invalid("Richiesta mobile non valida"))?;
        if command.purpose!="oaclis-console-v1" || command.epoch!=self.epoch || command.device!=device
            || Some(command.id.as_str())!=envelope["id"].as_str() || unhex(&command.id,16).is_err()
            || command.issued>current+5 || current.saturating_sub(command.issued)>30
            || !phone.chats.contains(&command.chat_id) || command.text.chars().count()>8000 || command.text.contains('\0')
            || !matches!(command.action.as_str(),"open"|"read"|"input"|"create"|"engines"|"models")
            || !matches!(command.key.as_str(),""|"Enter"|"Escape"|"Tab"|"Up"|"Down"|"Left"|"Right"|"BSpace"|"C-c"|"1"|"2"|"3")
            || (!matches!(command.action.as_str(),"input"|"create") && (!command.text.is_empty() || !command.key.is_empty() || command.image.is_some()))
            || (command.action=="create" && (!command.key.is_empty() || command.image.is_some() || command.text.trim().is_empty() || command.text.chars().count()>160))
            || (command.action!="create"&&(!command.icon.is_empty()||!command.model.is_empty()||!command.engine.is_empty())) {
            return Err(invalid("Richiesta mobile non autorizzata dal master"));
        }
        crate::terminal::validate_model(&command.model)?;
        if !matches!(command.engine.as_str(),""|"codex"|"claude"|"llamacpp") {return Err(invalid("Motore non valido"));}
        if command.action=="create"{crate::appearance::Appearance{label:command.text.clone(),icon:command.icon.clone(),..Default::default()}.validate()?;}
        if let Some(image)=&command.image {crate::terminal::image_bytes(image)?;if command.key!="Enter" {return Err(invalid("Invio immagine non valido"));}}
        let mut seen=self.seen.lock().unwrap();seen.retain(|_,until|*until>=now());
        let id=format!("{device}:{}",command.id);
        if seen.contains_key(&id) || seen.len()>=8192{return Err(invalid("Richiesta mobile gia' utilizzata"));}
        seen.insert(id,command.issued+30);
        Ok(json!({"chat_id":command.chat_id,"action":command.action,"text":command.text,"key":command.key,"speech":command.speech,"speech_cursor":command.speech_cursor,"image":command.image,"icon":command.icon,"model":command.model,"engine":if command.engine.is_empty(){"codex"}else{&command.engine}}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::signature::{EcdsaKeyPair,KeyPair};
    fn fixture()->(PathBuf,Arc<Store>,EcdsaKeyPair,Value) {
        let root=std::env::temp_dir().join(format!("oaclis-mobile-auth-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();std::fs::write(root.join("mobile-relay.json"),json!({"admin_key":"a".repeat(64)}).to_string()).unwrap();
        let store=at(&root).unwrap();let (hash,secret)=store.invitation().unwrap();
        let rng=SystemRandom::new();let document=EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING,&rng).unwrap();
        let key=EcdsaKeyPair::from_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING,document.as_ref(),&rng).unwrap();
        let payload=json!(["oaclis-pair-v1","b".repeat(64),"Phone",hex(key.public_key().as_ref())]).to_string();
        let binding=json!({"invite_hash":hash,"payload":payload,"proof":hex(hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256,secret.as_bytes()),payload.as_bytes()).as_ref()),"key_proof":hex(key.sign(&rng,payload.as_bytes()).unwrap().as_ref())});
        (root,store,key,binding)
    }
    fn command(store:&Store,key:&EcdsaKeyPair,id:&str,chat:&str,issued:u64)->Value {
        let payload=json!({"purpose":"oaclis-console-v1","epoch":store.epoch,"device":"b".repeat(64),"id":id,"issued":issued,"chat_id":chat,"action":"input","text":"hello","key":"Enter"}).to_string();
        json!({"device":"b".repeat(64),"id":id,"signed":{"signature":hex(key.sign(&SystemRandom::new(),payload.as_bytes()).unwrap().as_ref()),"payload":payload}})
    }
    #[test]
    fn creation_is_bound_to_an_authorized_parent_and_keeps_other_grants() {
        let (root,store,key,binding)=fixture();let device="b".repeat(64);
        store.approve(&device,"Phone",&binding).unwrap();store.grants(&device,BTreeSet::from(["allowed".into()])).unwrap();
        let packet=command(&store,&key,&"5".repeat(32),"allowed",now());
        let mut payload:Value=serde_json::from_str(packet["signed"]["payload"].as_str().unwrap()).unwrap();payload["action"]=json!("create");payload["key"]=json!("");payload["text"]=json!("Nuova chat");payload["icon"]=json!("code");payload["model"]=json!("gpt-6.1-sol");payload["engine"]=json!("codex");
        let signed=|payload:&Value|{let payload=payload.to_string();json!({"device":device,"id":serde_json::from_str::<Value>(&payload).unwrap()["id"],"signed":{"signature":hex(key.sign(&SystemRandom::new(),payload.as_bytes()).unwrap().as_ref()),"payload":payload}})};
        let mut forged=payload.clone();forged["chat_id"]=json!("not-allowed");assert!(store.command(&signed(&forged)).is_err());
        forged=payload.clone();forged["cwd"]=json!("/root");assert!(store.command(&signed(&forged)).is_err());
        let checked=store.command(&signed(&payload)).unwrap();assert_eq!(checked["icon"],"code");assert_eq!(checked["model"],"gpt-6.1-sol");assert_eq!(checked["engine"],"codex");
        assert!(store.allow_created(&device,"not-allowed","new-chat").is_err());
        store.allow_created(&device,"allowed","new-chat").unwrap();assert_eq!(store.authority().phones[&device].chats,BTreeSet::from(["allowed".into(),"new-chat".into()]));
        assert!(store.command(&signed(&payload)).is_err());
        for (action,nonce) in [("engines","6"),("models","7")] {
            let mut query=payload.clone();query["action"]=json!(action);query["id"]=json!(nonce.repeat(32));
            assert!(store.command(&signed(&query)).is_err());
            for field in ["text","icon","model","engine"] {query[field]=json!("");}
            assert_eq!(store.command(&signed(&query)).unwrap()["action"],action);
            query["chat_id"]=json!("not-allowed");assert!(store.command(&signed(&query)).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn pairing_requires_the_local_qr_secret_and_phone_key_possession() {
        let (root,store,_,binding)=fixture();let id="b".repeat(64);
        assert!(store.pending(&id,"Phone",&binding));assert!(!store.pending(&"c".repeat(64),"Phone",&binding));assert!(!store.pending(&id,"Renamed",&binding));
        let mut forged=binding.clone();forged["proof"]=json!("00".repeat(32));assert!(!store.pending(&id,"Phone",&forged));
        forged=binding.clone();forged["key_proof"]=json!("00".repeat(64));assert!(!store.pending(&id,"Phone",&forged));
        store.approve(&id,"Phone",&binding).unwrap();assert!(store.approve(&id,"Phone",&binding).is_err());
        assert!(store.authority().phones[&id].chats.is_empty());
        let persisted:Value=serde_json::from_slice(&std::fs::read(root.join("mobile-relay.json")).unwrap()).unwrap();assert_eq!(persisted["admin_key"],"a".repeat(64));assert!(persisted["authorization"]["invitations"].as_object().unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn commands_require_local_grants_valid_signatures_freshness_and_no_replay() {
        let (root,store,key,binding)=fixture();let device="b".repeat(64);
        store.approve(&device,"Phone",&binding).unwrap();
        let id="1".repeat(32);let packet=command(&store,&key,&id,"allowed",now());assert!(store.command(&packet).is_err());
        store.grants(&device,BTreeSet::from(["allowed".into()])).unwrap();
        let mut forged=packet.clone();forged["signed"]["signature"]=json!("00".repeat(64));assert!(store.command(&forged).is_err());
        forged=packet.clone();forged["id"]=json!("2".repeat(32));assert!(store.command(&forged).is_err());
        assert!(store.command(&command(&store,&key,&"3".repeat(32),"forbidden",now())).is_err());
        assert!(store.command(&command(&store,&key,&"4".repeat(32),"allowed",now()-31)).is_err());
        assert!(store.command(&command(&store,&key,&"5".repeat(32),"allowed",now()+60)).is_err());
        forged=packet.clone();forged["value"]=json!({"action":"input","text":"relay injection"});
        assert_eq!(store.command(&forged).unwrap()["text"],"hello");assert!(store.command(&packet).is_err());
        store.forget_chat("allowed").unwrap();assert!(store.command(&command(&store,&key,&"6".repeat(32),"allowed",now())).is_err());
        store.grants(&device,BTreeSet::from(["allowed".into()])).unwrap();store.revoke(&device).unwrap();
        assert!(store.command(&command(&store,&key,&"7".repeat(32),"allowed",now())).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn restarting_changes_the_epoch_but_preserves_only_locally_approved_keys() {
        let (root,store,key,binding)=fixture();let device="b".repeat(64);
        store.approve(&device,"Phone",&binding).unwrap();store.grants(&device,BTreeSet::from(["allowed".into()])).unwrap();
        let packet=command(&store,&key,&"8".repeat(32),"allowed",now());
        let restarted=Store{config:root.clone(),settings:RwLock::new(store.settings.read().unwrap().clone()),writer:Mutex::new(()),epoch:random().unwrap(),seen:Mutex::new(BTreeMap::new()),last_time:AtomicU64::new(now())};
        assert!(restarted.command(&packet).is_err());
        let packet=command(&restarted,&key,&"9".repeat(32),"allowed",now());assert!(restarted.command(&packet).is_ok());
        restarted.last_time.store(now()+60,Ordering::Relaxed);assert!(restarted.command(&command(&restarted,&key,&"a".repeat(32),"allowed",now())).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires the local browser interoperability fixture"]
    fn browser_signatures_are_verified_by_the_native_master() {
        let input:Value=serde_json::from_slice(&std::fs::read(std::env::var_os("OACLIS_MOBILE_FIXTURE").expect("browser fixture path")).unwrap()).unwrap();
        let id=input["device"].as_str().unwrap();let binding=&input["binding"];
        let mut settings=Settings{admin_key:"a".repeat(64),authorization:Authority::default()};
        settings.authorization.invitations.insert(binding["invite_hash"].as_str().unwrap().into(),Invitation{secret:input["secret"].as_str().unwrap().into(),until:now()+60});
        let store=Store{config:PathBuf::new(),settings:RwLock::new(Arc::new(settings)),writer:Mutex::new(()),epoch:input["epoch"].as_str().unwrap().into(),seen:Mutex::new(BTreeMap::new()),last_time:AtomicU64::new(now())};
        let (_,public_key)=store.pairing(id,"Phone",binding).unwrap();
        let mut pinned=(*store.settings.read().unwrap().clone()).clone();
        pinned.authorization.phones.insert(id.into(),Phone{name:"Phone".into(),public_key,chats:BTreeSet::from(["fixture-chat".into()])});
        *store.settings.write().unwrap()=Arc::new(pinned);
        let packet=&input["packet"];assert_eq!(store.command(packet).unwrap()["text"],"Ciao è 🇮🇹");
        assert!(store.command(packet).is_err());
        if let Some(packet)=input.get("image_packet") {
            let value=store.command(packet).unwrap();assert!(value["image"].as_str().unwrap().starts_with("data:image/jpeg;base64,"));
            if let Some(log)=std::env::var_os("OACLIS_PASTE_LOG") {
                let mut sent=value.clone();sent["text"]=json!("image prompt");crate::terminal::local(&sent).unwrap();
                let pasted=std::fs::read_to_string(log).unwrap();let lines:Vec<_>=pasted.lines().collect();
                assert!(lines[0].ends_with(".jpg"));assert_eq!(lines[1],"image prompt");assert_eq!(lines[2],"KEY:Enter");
                let path=std::path::Path::new(lines[0]);assert!(path.is_file());
                assert_eq!(std::fs::read(path).unwrap(),crate::terminal::image_bytes(value["image"].as_str().unwrap()).unwrap());
                use std::os::unix::fs::PermissionsExt;assert_eq!(std::fs::metadata(path).unwrap().permissions().mode()&0o777,0o600);
                assert!(crate::terminal::image_bytes("data:image/jpeg;base64,invalid").is_err());
                assert!(crate::terminal::image_bytes(&format!("data:image/jpeg;base64,{}","A".repeat(700000))).is_err());
            }

            let mut tampered=packet.clone();let mut body:Value=serde_json::from_str(tampered["signed"]["payload"].as_str().unwrap()).unwrap();body["image"]=json!("data:image/jpeg;base64,changed");tampered["signed"]["payload"]=json!(body.to_string());assert!(store.command(&tampered).is_err());
        }

        if let Some(packets)=input["number_packets"].as_array() {
            for (packet,key) in packets.iter().zip(["1","2","3"]) {
                let value=store.command(packet).unwrap();assert_eq!(value["key"],key);assert_eq!(value["text"],"");
            }
        }
        let mut tampered=packet.clone();let mut body:Value=serde_json::from_str(packet["signed"]["payload"].as_str().unwrap()).unwrap();body["text"]=json!("forged by relay");tampered["signed"]["payload"]=json!(body.to_string());assert!(store.command(&tampered).is_err());
    }

    #[test]
    fn speech_subscription_and_cursor_are_covered_by_phone_signature_and_grants() {
        let (root,store,key,binding)=fixture();let device="b".repeat(64);
        store.approve(&device,"Phone",&binding).unwrap();store.grants(&device,BTreeSet::from(["allowed".into()])).unwrap();
        let mut packet=command(&store,&key,&"c".repeat(32),"allowed",now());
        let mut payload:Value=serde_json::from_str(packet["signed"]["payload"].as_str().unwrap()).unwrap();
        payload["speech"]=json!(true);payload["speech_cursor"]=json!(7);
        let text=payload.to_string();packet["signed"]["payload"]=json!(text);
        packet["signed"]["signature"]=json!(hex(key.sign(&SystemRandom::new(),text.as_bytes()).unwrap().as_ref()));
        let mut forged=packet.clone();payload["speech_cursor"]=json!(0);forged["signed"]["payload"]=json!(payload.to_string());
        assert!(store.command(&forged).is_err());
        let checked=store.command(&packet).unwrap();assert_eq!(checked["speech"],true);assert_eq!(checked["speech_cursor"],7);
        std::fs::remove_dir_all(root).unwrap();
    }

}
