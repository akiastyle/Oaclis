// pairing.rs v1.48.663
//! Signed discovery and mutually approved SSH; persistent identities stay unchanged.
use crate::{invalid, ssh_settings as ssh};
use serde_json::{Value, json};
use sha2::{Digest,Sha256};
use std::{env, fs::{self,File}, io::{self,Read}, net::Ipv4Addr, os::unix::fs::PermissionsExt, path::{Path,PathBuf}, process::Command, time::{Duration,SystemTime,UNIX_EPOCH}};

fn public_key(key: &str) -> io::Result<&str> {
    let encoded = key.strip_prefix("ssh-ed25519 ").filter(|s|s.len()==68).ok_or_else(||invalid("Chiave pubblica non valida"))?;
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut bytes = Vec::new();
    for chunk in encoded.as_bytes().chunks_exact(4) {
        let values = chunk.iter().map(|c|ALPHABET.iter().position(|b|b==c).map(|v|v as u32).ok_or_else(||invalid("Chiave pubblica non valida"))).collect::<io::Result<Vec<_>>>()?;
        let bits = values[0]<<18 | values[1]<<12 | values[2]<<6 | values[3];
        bytes.extend_from_slice(&[(bits>>16) as u8,(bits>>8) as u8,bits as u8]);
    }
    if !bytes.starts_with(b"\0\0\0\x0bssh-ed25519\0\0\0 ") { return Err(invalid("Chiave Ed25519 non valida")); }
    Ok(key)
}
fn identifier(key: &str) -> io::Result<String> {
    Ok(Sha256::digest(public_key(key)?.as_bytes()).iter().take(16).map(|b|format!("{b:02x}")).collect())
}
fn private_address(address: &str) -> io::Result<Ipv4Addr> {
    let ip: Ipv4Addr = address.parse().map_err(|_|invalid("Indirizzo IPv4 non valido"))?;
    if !crate::peers::local_address(ip.into()) { return Err(invalid("Il rilevamento richiede un indirizzo IPv4 locale")); }
    Ok(ip)
}
fn peer_id(value: &str) -> bool { value.len()==32 && value.bytes().all(|b|b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) }
fn now() -> io::Result<i64> { Ok(SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_secs() as i64) }
fn validate(card: &Value) -> io::Result<()> {
    let key = card["public_key"].as_str().ok_or_else(||invalid("Chiave assente"))?;
    if card["id"] != identifier(key)? { return Err(invalid("Identita non valida")); }
    let name = card["name"].as_str().ok_or_else(||invalid("Nome non valido"))?;
    if name.trim().is_empty() || name.chars().count()>160 || name.chars().any(|c|c<' ') { return Err(invalid("Nome non valido")); }
    if !matches!(card["role"].as_str(),Some("master"|"client"|"unconfigured")) { return Err(invalid("Ruolo non valido")); }
    let data=&card["ssh"];
    private_address(data["address"].as_str().ok_or_else(||invalid("Indirizzo assente"))?)?;
    if !data["port"].as_u64().is_some_and(|p|(1..=65535).contains(&p)) { return Err(invalid("Porta non valida")); }
    if !data["user"].as_str().is_some_and(ssh::user) { return Err(invalid("Utente non valido")); }
    if !data["host_key"].is_null() { public_key(data["host_key"].as_str().ok_or_else(||invalid("Chiave host non valida"))?)?; }
    let approved=card["approved"].as_array().ok_or_else(||invalid("Approvazioni non valide"))?;
    if approved.len()>64 || approved.iter().any(|v|!v.as_str().is_some_and(peer_id)) { return Err(invalid("Approvazioni non valide")); }
    if !card["time"].as_i64().is_some_and(|time|now().is_ok_and(|now|now.abs_diff(time)<=30)) { return Err(invalid("Annuncio scaduto: verificare gli orologi")); }
    Ok(())
}
fn state(config: &Path) -> io::Result<Value> {
    let saved=ssh::read(&config.join("pairing.json"),json!({"approved":{},"ignored":[]}))?;
    let approved=saved["approved"].as_object().ok_or_else(||invalid("Stato pairing non valido: file conservato"))?;
    let ignored=saved["ignored"].as_array().ok_or_else(||invalid("Stato pairing non valido: file conservato"))?;
    if approved.len()>64 || ignored.len()>64 || ignored.iter().any(|v|!v.as_str().is_some_and(peer_id))
        || approved.iter().any(|(id,v)|!peer_id(id) || !v.is_object() || !v["public_key"].as_str().is_some_and(|k|public_key(k).is_ok())) {
        return Err(invalid("Stato pairing non valido: file conservato"));
    }
    Ok(saved)
}
fn identity(config: &Path) -> io::Result<(PathBuf,String)> {
    let key=config.join("pairing_ed25519"); let public=key.with_extension("pub");
    for path in [&key,&public] { if path.symlink_metadata().is_ok_and(|m|!m.is_file()) { return Err(invalid("Chiave non regolare")); } }
    if !key.exists() {
        if public.exists() { return Err(invalid("Chiave privata mancante: chiave pubblica conservata")); }
        ssh::text(Command::new("ssh-keygen").args(["-q","-t","ed25519","-N","","-f"]).arg(&key).args(["-C","oaclis-pairing"]),15)?;
    }
    fs::set_permissions(&key,fs::Permissions::from_mode(0o600))?;
    let text=fs::read_to_string(public)?;
    let value=text.split_whitespace().take(2).collect::<Vec<_>>().join(" "); public_key(&value)?;
    Ok((key,value))
}
fn label(config: &Path) -> io::Result<String> {
    let appearance=ssh::read(&config.join("appearance.json"),json!({}))?;
    let record=&appearance["device:master"];
    if let Some(label)=record.get("value").unwrap_or(record)["label"].as_str().filter(|s|!s.is_empty()) { return Ok(label.into()); }
    ssh::text(&mut Command::new("hostname"),5)
}
fn linux_interfaces(interfaces: &Value, default: &str) -> io::Result<Vec<Value>> {
    let mut result=Vec::new();
    for interface in interfaces.as_array().ok_or_else(||invalid("Interfacce non valide"))? {
        if interface["operstate"]=="DOWN" || interface["flags"].as_array().is_some_and(|a|a.iter().any(|v|v=="LOOPBACK")) { continue; }
        for a in interface["addr_info"].as_array().into_iter().flatten() {
            if a["scope"]!="global" { continue; }
            let ip: Ipv4Addr=a["local"].as_str().ok_or_else(||invalid("Indirizzo interfaccia assente"))?.parse().map_err(io::Error::other)?;
            if ip.is_loopback() || ip.is_unspecified() { continue; }
            if !a["prefixlen"].as_u64().is_some_and(|p|p<=32) { return Err(invalid("Prefisso rete non valido")); }
            result.push(json!({"name":interface["ifname"],"address":ip.to_string(),"prefix":a["prefixlen"],"broadcast":a["broadcast"]}));
        }
    }
    result.sort_by_key(|nic|nic["name"]!=default); Ok(result)
}
fn mac_interfaces(text: &str) -> io::Result<Vec<Value>> {
    let mut blocks=Vec::<(String,String)>::new();
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            if let Some((name,_))=line.split_once(':') { blocks.push((name.into(),String::new())); }
        }
        if let Some((_,block))=blocks.last_mut() { block.push_str(line);block.push('\n'); }
    }
    let mut result=Vec::new();
    for (name,block) in blocks {
        if block.contains("LOOPBACK") || block.contains("status: inactive") { continue; }
        for line in block.lines() {
            let fields:Vec<_>=line.split_whitespace().collect();
            if fields.len()<4 || fields[0]!="inet" || fields[2]!="netmask" { continue; }
            let ip: Ipv4Addr=fields[1].parse().map_err(io::Error::other)?;
            let mask=if let Some(hex)=fields[3].strip_prefix("0x") { u32::from_str_radix(hex,16).map_err(io::Error::other)? } else { u32::from(fields[3].parse::<Ipv4Addr>().map_err(io::Error::other)?) };
            if mask.count_ones()!=mask.leading_ones() { return Err(invalid("Maschera rete non valida")); }
            result.push(json!({"name":name,"address":ip.to_string(),"prefix":mask.leading_ones(),"broadcast":Ipv4Addr::from(u32::from(ip)|!mask).to_string()}));
        }
    }
    Ok(result)
}
fn network_interfaces() -> io::Result<Vec<Value>> {
    if cfg!(target_os="macos") { return mac_interfaces(&ssh::text(Command::new("ifconfig").arg("-a"),15)?); }
    let interfaces:Value=serde_json::from_str(&ssh::text(Command::new("ip").args(["-j","-4","addr","show","up"]),15)?)?;
    let routes:Value=ssh::text(Command::new("ip").args(["-j","-4","route","show","default"]),15).ok().and_then(|s|serde_json::from_str(&s).ok()).unwrap_or(Value::Null);
    linux_interfaces(&interfaces,routes[0]["dev"].as_str().unwrap_or(""))
}
fn windows_network(ip: Ipv4Addr) -> io::Result<(String,bool)> {
    let script=r"$ErrorActionPreference='Stop'; $nic=Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -and $_.InterfaceAlias -notmatch 'vEthernet' } | Select-Object -First 1; $address=$nic.IPv4Address.IPAddress; $listeners=@(Get-NetTCPConnection -LocalPort 23232 -State Listen -ErrorAction SilentlyContinue); $udp=Get-NetFirewallRule -Name 'Oaclis-LAN-Discovery' -ErrorAction SilentlyContinue; $proxy=netsh interface portproxy show v4tov4; $mapping=$proxy | Where-Object { $_ -match ('^\s*'+[regex]::Escape($address)+'\s+23232\s+__VIRTUAL_IP__\s+23232\s*$') }; @{address=$address; ready=($mapping -and $listeners.Count -gt 0 -and $udp -and $udp.Enabled -eq 'True')} | ConvertTo-Json -Compress";
    let script=script.replace("__VIRTUAL_IP__",&ip.to_string().replace('.',"\\."));
    let value:Value=serde_json::from_str(&ssh::text(&mut crate::peers::powershell(&script),15)?)?;
    let address=value["address"].as_str().ok_or_else(||invalid("Indirizzo LAN Windows assente"))?.to_owned();
    private_address(&address)?; Ok((address,value["ready"]==true))
}
fn prepare(config: &Path) -> io::Result<Value> {
    let (_,key)=identity(config)?;
    let host_key=match fs::read_to_string("/etc/ssh/ssh_host_ed25519_key.pub") {
        Ok(text)=>Some(public_key(&text.split_whitespace().take(2).collect::<Vec<_>>().join(" "))?.to_owned()),
        Err(e) if e.kind()==io::ErrorKind::NotFound=>None, Err(e)=>return Err(e),
    };
    let interfaces=network_interfaces()?;
    let primary=interfaces.first().and_then(|v|v["address"].as_str()).ok_or_else(||invalid("Nessuna interfaccia IPv4 attiva: collegare il dispositivo alla rete"))?;
    let primary_ip:Ipv4Addr=primary.parse().map_err(io::Error::other)?;
    let wsl=ssh::wsl();
    let (address,ready)=if wsl { windows_network(primary_ip)? } else { (primary.into(),true) };
    private_address(&address)?;
    let mut bytes=[0u8;16];File::open("/dev/urandom")?.read_exact(&mut bytes)?;bytes[6]=(bytes[6]&15)|64;bytes[8]=(bytes[8]&63)|128;
    let hex:String=bytes.iter().map(|b|format!("{b:02x}")).collect();
    let instance=format!("{}-{}-{}-{}-{}",&hex[..8],&hex[8..12],&hex[12..16],&hex[16..20],&hex[20..]);
    let role=match fs::read_to_string(config.join("role")) { Ok(s)=>s.trim().to_owned(),Err(e) if e.kind()==io::ErrorKind::NotFound=>"unconfigured".into(),Err(e)=>return Err(e) };
    let user=ssh::text(Command::new("id").arg("-un"),5)?;
    Ok(json!({"id":identifier(&key)?,"public_key":key,"name":label(config)?,"instance":instance,"role":role,
        "ssh":{"address":address,"port":if wsl && address!=primary {2222} else {22},"user":user,"host_key":host_key},
        "broadcasts":interfaces.iter().filter_map(|v|v["broadcast"].as_str()).collect::<Vec<_>>(),"setup_needed":address!=primary && !ready,
        "setup_kind":if wsl && address!=primary {Some("wsl-nat")} else {None},"wsl":wsl,"wsl_ip":primary,"distro":env::var("WSL_DISTRO_NAME").unwrap_or_default()}))
}
fn sign(config: &Path, card: &Value) -> io::Result<Value> {
    let (key,_)=identity(config)?;let saved=state(config)?;
    let mut card=card.clone();
    if !card.is_object() { return Err(invalid("Annuncio non valido")); }
    card["name"]=json!(label(config)?);card["time"]=json!(now()?);card["approved"]=json!(saved["approved"].as_object().unwrap().keys().collect::<Vec<_>>());validate(&card)?;
    let message=serde_json::to_string(&card)?;
    let signature=ssh::command(Command::new("ssh-keygen").args(["-Y","sign","-f"]).arg(key).args(["-n","oaclis-lan"]),Some(message.as_bytes()),Duration::from_secs(15))?;
    Ok(json!({"hello":message,"signature":String::from_utf8(signature).map_err(io::Error::other)?}))
}
fn neighbors() -> io::Result<Value> {
    let mut addresses=Vec::<String>::new();
    if cfg!(target_os="macos") {
        let output=ssh::text(Command::new("arp").arg("-an"),15)?;
        for field in output.split_whitespace() { if let Some(ip)=field.strip_prefix('(').and_then(|s|s.strip_suffix(')')) { addresses.push(ip.into()); } }
    } else {
        let values:Value=serde_json::from_str(&ssh::text(Command::new("ip").args(["-j","-4","neigh","show"]),15)?)?;
        for v in values.as_array().ok_or_else(||invalid("Tabella vicini non valida"))? {
            if v["lladdr"].as_str().is_some_and(|s|!s.is_empty()) && !v["state"].as_array().is_some_and(|a|a.iter().any(|s|s=="FAILED" || s=="INCOMPLETE")) {
                if let Some(ip)=v["dst"].as_str() { addresses.push(ip.into()); }
            }
        }
        if ssh::wsl() {
            let script="Get-NetNeighbor -AddressFamily IPv4 | Where-Object { $_.State -in 'Reachable','Stale','Delay','Probe','Permanent' } | ForEach-Object { $_.IPAddress }";
            addresses.extend(ssh::text(&mut crate::peers::powershell(script),15)?.split_whitespace().map(str::to_owned));
        }
    }
    let mut result=Vec::new();
    for address in addresses { if let Ok(ip)=private_address(&address) { if !ip.is_loopback() && !result.contains(&ip.to_string()) { result.push(ip.to_string()); } } }
    result.truncate(32);Ok(json!(result))
}
fn verify(packet: &Value) -> io::Result<Value> {
    let message=packet["hello"].as_str().filter(|s|s.len()<=6000).ok_or_else(||invalid("Annuncio troppo grande"))?;
    let signature=packet["signature"].as_str().filter(|s|s.len()<=1500).ok_or_else(||invalid("Firma troppo grande"))?;
    let card:Value=serde_json::from_str(message)?;validate(&card)?;
    let mut signer=crate::staging::StagedFile::new(&env::temp_dir())?;
    let mut signed=crate::staging::StagedFile::new(&env::temp_dir())?;
    use std::io::Write;
    writeln!(signer.file,"oaclis {}",card["public_key"].as_str().unwrap())?;signed.file.write_all(signature.as_bytes())?;
    ssh::command(Command::new("ssh-keygen").args(["-Y","verify","-f"]).arg(&signer.path).args(["-I","oaclis","-n","oaclis-lan","-s"]).arg(&signed.path),Some(message.as_bytes()),Duration::from_secs(15))?;
    Ok(card)
}
fn approve(config: &Path, card: &Value, accept: bool) -> io::Result<Value> {
    validate(card)?;let mut saved=state(config)?;let id=card["id"].as_str().unwrap();
    if accept {
        if saved["approved"].get(id).is_none() && saved["approved"].as_object().unwrap().len()>=64 { return Err(invalid("Limite di 64 dispositivi raggiunto")); }
        let entry=saved["approved"].as_object_mut().unwrap().entry(id).or_insert(json!({}));entry["public_key"]=card["public_key"].clone();
        saved["ignored"].as_array_mut().unwrap().retain(|v|v!=id);
    } else {
        if saved["approved"][id]["device_id"].as_str().is_some_and(|s|!s.is_empty()) { return Err(invalid("Dispositivo gia collegato: eliminarlo dalla mappa")); }
        saved["approved"].as_object_mut().unwrap().remove(id);ignore(&mut saved,id);
    }
    crate::staging::save_json(&config.join("pairing.json"),&saved)?;Ok(json!({"ok":true}))
}
fn ignore(saved: &mut Value, id: &str) {
    let values=saved["ignored"].as_array_mut().unwrap();values.push(json!(id));if values.len()>64 { values.drain(..values.len()-64); }
}
fn append_unique(path: &Path, line: &str) -> io::Result<()> {
    let mut text=match crate::open_regular(path) { Ok((mut f,_))=>{let mut s=String::new();f.read_to_string(&mut s)?;s},Err(e) if e.kind()==io::ErrorKind::NotFound=>String::new(),Err(e)=>return Err(e) };
    if !text.lines().any(|s|s==line) { text=text.trim_end_matches('\n').to_owned()+if text.is_empty() {""} else {"\n"}+line+"\n";ssh::atomic(path,text.as_bytes())?; } Ok(())
}
fn forget(home: &Path, config: &Path, device: &str) -> io::Result<Value> {
    let mut saved=state(config)?;
    let approvals=saved["approved"].as_object().unwrap().clone();
    for (id,approval) in approvals {
        if approval["device_id"]!=device { continue; }
        let line=format!("{} oaclis-peer-{id}",approval["public_key"].as_str().ok_or_else(||invalid("Approvazione non valida"))?);
        let path=home.join(".ssh/authorized_keys");
        if path.symlink_metadata().is_ok() {
            let (mut f,_)=crate::open_regular(&path)?;let mut text=String::new();f.read_to_string(&mut text)?;
            ssh::atomic(&path,text.lines().filter(|s|*s!=line).map(|s|format!("{s}\n")).collect::<String>().as_bytes())?;
        }
        saved["approved"].as_object_mut().unwrap().remove(&id);ignore(&mut saved,&id);
    }
    crate::staging::save_json(&config.join("pairing.json"),&saved)?;Ok(json!({"ok":true}))
}
fn commit(home: &Path, config: &Path, card: &Value) -> io::Result<Value> {
    validate(card)?;let mut saved=state(config)?;let (_,key)=identity(config)?;let id=card["id"].as_str().unwrap();
    if saved["approved"][id]["public_key"]!=card["public_key"] || !card["approved"].as_array().unwrap().contains(&json!(identifier(&key)?)) { return Err(invalid("Sono richieste entrambe le autorizzazioni")); }
    let data=&card["ssh"];let host_key=data["host_key"].as_str().ok_or_else(||invalid(format!("SSH non pronto su {}: attivare il server SSH",card["name"].as_str().unwrap())))?;
    let directory=ssh::ssh_directory(home)?;let known=config.join("known_hosts");
    let address=data["address"].as_str().unwrap();let port=data["port"].as_u64().unwrap();let endpoint=if port==22 {address.into()} else {format!("[{address}]:{port}")};
    if known.symlink_metadata().is_ok() {
        crate::open_regular(&known)?;
        // ssh-keygen exits 1 for no matching entry. Existing lines are never replaced.
        let found=ssh::command(Command::new("ssh-keygen").args(["-F",&endpoint,"-f"]).arg(&known),None,Duration::from_secs(3));
        match found {
            Ok(output)=>for line in String::from_utf8_lossy(&output).lines().filter(|s|!s.starts_with('#') && !s.trim().is_empty()) {
                if line.split_whitespace().skip(1).take(2).collect::<Vec<_>>().join(" ")!=host_key { return Err(invalid("Chiave host cambiata: verificare il dispositivo prima di collegarlo")); }
            },
            Err(e) if e.kind()==io::ErrorKind::Other && e.to_string()==" (comando non riuscito)"=>{},
            Err(e)=>return Err(e),
        }
    }
    append_unique(&known,&format!("{endpoint} {host_key}"))?;
    let mut devices=crate::devices::load(config.join("devices.json"))?;
    let existing=ssh::connections(config,&serde_json::to_value(&devices)?)?;
    let device=if let Some(device)=devices.iter().find(|d|["address","port","user"].iter().all(|k|existing[&d.id][k]==data[k])) { device.clone() }
        else {let device=crate::devices::Device{id:format!("peer-{id}"),name:card["name"].as_str().unwrap().into(),ssh_alias:format!("oaclis-peer-{id}")};devices.push(device.clone());device};
    ssh::apply(home,config,"save",&json!({"id":device.id,"ssh_alias":device.ssh_alias,"paired":true,"settings":{"address":address,"port":port,"user":data["user"],"identity_file":config.join("pairing_ed25519")}}))?;
    crate::devices::save(config.join("devices.json"),&devices)?;
    saved["approved"][id]["device_id"]=json!(device.id);crate::staging::save_json(&config.join("pairing.json"),&saved)?;
    append_unique(&directory.join("authorized_keys"),&format!("{} oaclis-peer-{id}",card["public_key"].as_str().unwrap()))?;
    Ok(json!({"device_id":device.id}))
}
pub(crate) fn request(home: &Path, operation: &str, value: &Value) -> io::Result<Value> {
    if operation=="verify" { return verify(value); }
    if operation=="neighbors" { return neighbors(); }
    let config=home.join(".config/oaclis");let _lock=ssh::lock(&config)?;
    match operation {
        "prepare"=>prepare(&config),"sign"=>sign(&config,value),"state"=>state(&config),
        "approve"=>approve(&config,&value["peer"],value["accept"].as_bool().ok_or_else(||invalid("Conferma richiesta"))?),
        "commit"=>commit(home,&config,value),"forget"=>forget(home,&config,value["id"].as_str().ok_or_else(||invalid("Dispositivo richiesto"))?),
        "restore"=>{let mut saved=state(&config)?;saved["ignored"]=json!([]);crate::staging::save_json(&config.join("pairing.json"),&saved)?;Ok(json!({"ok":true}))},
        _=>Err(invalid("Operazione pairing sconosciuta")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Drop for Fixture { fn drop(&mut self) { let _=fs::remove_dir_all(&self.0); } }
    fn fixture(name: &str) -> Fixture {
        let root=env::temp_dir().join(format!("oaclis-pairing-step3-{name}-{}",std::process::id()));fs::create_dir(&root).unwrap();Fixture(root)
    }
    fn card(config: &Path, index: u16) -> Value {
        let (_,key)=identity(config).unwrap();
        json!({"id":identifier(&key).unwrap(),"public_key":key,"name":format!("Device {index}"),"role":if index==0 {"master"} else {"client"},"ssh":{"address":"127.0.0.1","port":2200+index,"user":"root","host_key":key},"approved":[],"time":now().unwrap()})
    }
    #[test]
    fn step3_pairing_requires_mutual_approval_and_preserves_existing_access() {
        let root=fixture("mutual");let homes=[root.0.join("a"),root.0.join("b")];let configs=homes.each_ref().map(|h|h.join(".config/oaclis"));
        for config in &configs { fs::create_dir_all(config).unwrap();fs::write(config.join("appearance.json"),"{\"device:master\":{\"value\":{\"label\":\"Périferica\"}}}").unwrap(); }
        let cards=[card(&configs[0],0),card(&configs[1],1)];
        let packet=request(&homes[1],"sign",&cards[1]).unwrap();
        assert_eq!(request(Path::new("/not-used"),"verify",&packet).unwrap()["name"],"Périferica");
        let escaped=packet["hello"].as_str().unwrap().replace('é',"\\u00e9");
        let signature=ssh::command(Command::new("ssh-keygen").args(["-Y","sign","-f"]).arg(configs[1].join("pairing_ed25519")).args(["-n","oaclis-lan"]),Some(escaped.as_bytes()),Duration::from_secs(15)).unwrap();
        assert_eq!(verify(&json!({"hello":escaped,"signature":String::from_utf8(signature).unwrap()})).unwrap()["name"],"Périferica");
        request(&homes[0],"approve",&json!({"peer":cards[1],"accept":true})).unwrap();
        assert!(request(&homes[0],"commit",&cards[1]).unwrap_err().to_string().contains("entrambe"));
        assert!(!homes[0].join(".ssh/authorized_keys").exists());
        request(&homes[1],"approve",&json!({"peer":cards[0],"accept":true})).unwrap();
        let approved=homes.each_ref().map(|h|verify(&request(h,"sign",&cards[if h==&homes[0] {0} else {1}]).unwrap()).unwrap());
        for i in 0..2 {
            fs::create_dir(homes[i].join(".ssh")).unwrap();fs::write(homes[i].join(".ssh/authorized_keys"),"existing-key\n").unwrap();fs::write(homes[i].join(".ssh/config"),"Host existing\n  HostName localhost\n").unwrap();
            for _ in 0..2 { request(&homes[i],"commit",&approved[1-i]).unwrap(); }
            let authorized=fs::read_to_string(homes[i].join(".ssh/authorized_keys")).unwrap();
            assert!(authorized.starts_with("existing-key\n"));assert_eq!(authorized.matches(cards[1-i]["public_key"].as_str().unwrap()).count(),1);
            assert!(fs::read_to_string(homes[i].join(".ssh/config")).unwrap().contains("Host existing"));assert_eq!(crate::devices::load(configs[i].join("devices.json")).unwrap().len(),1);
        }
        assert_ne!(fs::read(configs[0].join("pairing_ed25519")).unwrap(),fs::read(configs[1].join("pairing_ed25519")).unwrap());
        let id=cards[1]["id"].as_str().unwrap();let saved=state(&configs[0]).unwrap();let device=saved["approved"][id]["device_id"].clone();
        assert!(request(&homes[0],"approve",&json!({"peer":approved[1],"accept":false})).is_err());
        request(&homes[0],"approve",&json!({"peer":approved[1],"accept":true})).unwrap();assert_eq!(state(&configs[0]).unwrap()["approved"][id]["device_id"],device);
        let mut changed=approved[1].clone();changed["ssh"]["host_key"]=cards[0]["public_key"].clone();
        assert!(request(&homes[0],"commit",&changed).unwrap_err().to_string().contains("Chiave host cambiata"));
        request(&homes[0],"forget",&json!({"id":device})).unwrap();assert_eq!(fs::read_to_string(homes[0].join(".ssh/authorized_keys")).unwrap(),"existing-key\n");assert!(state(&configs[0]).unwrap()["ignored"].as_array().unwrap().contains(&cards[1]["id"]));
    }
    #[test]
    fn step3_pairing_rejects_tampering_expiry_bad_keys_and_missing_ssh() {
        let root=fixture("validate");let config=root.0.join(".config/oaclis");fs::create_dir_all(&config).unwrap();let mut card=card(&config,0);
        let mut packet=request(&root.0,"sign",&card).unwrap();let mut fake:Value=serde_json::from_str(packet["hello"].as_str().unwrap()).unwrap();fake["name"]=json!("Fake");packet["hello"]=json!(serde_json::to_string(&fake).unwrap());assert!(verify(&packet).is_err());
        card["time"]=json!(0);assert!(validate(&card).is_err());card["time"]=json!(now().unwrap());
        for (key,value) in [("name",json!("x\nHost evil")),("approved",json!(["bad"]))] { let mut invalid=card.clone();invalid[key]=value;assert!(validate(&invalid).is_err()); }
        let mut invalid=card.clone();invalid["ssh"]["address"]=json!("8.8.8.8");assert!(validate(&invalid).is_err());
        assert!(public_key("ssh-ed25519 not-a-key").is_err());
        card["ssh"]["host_key"]=Value::Null;assert!(validate(&card).is_ok());
        request(&root.0,"approve",&json!({"peer":card,"accept":true})).unwrap();card["approved"]=json!([card["id"]]);
        assert!(request(&root.0,"commit",&card).unwrap_err().to_string().contains("SSH non pronto"));assert!(!root.0.join(".ssh/authorized_keys").exists());
        let signed=request(&root.0,"sign",&card).unwrap();assert!(verify(&signed).unwrap()["ssh"]["host_key"].is_null());
    }
    #[test]
    fn step3_pairing_parses_linux_and_macos_interfaces_without_network_probes() {
        let interfaces=json!([
            {"ifname":"lo","flags":["LOOPBACK","UP"],"operstate":"UNKNOWN","addr_info":[{"local":"10.255.255.254","scope":"global","prefixlen":32}]},
            {"ifname":"docker0","flags":["UP"],"operstate":"DOWN","addr_info":[{"local":"172.17.0.1","scope":"global","prefixlen":16}]},
            {"ifname":"eth1","flags":["UP"],"operstate":"UP","addr_info":[{"local":"192.168.2.10","scope":"global","prefixlen":24,"broadcast":"192.168.2.255"}]},
            {"ifname":"eth0","flags":["UP"],"operstate":"UP","addr_info":[{"local":"172.29.128.195","scope":"global","prefixlen":20,"broadcast":"172.29.143.255"}]}
        ]);
        let result=linux_interfaces(&interfaces,"eth0").unwrap();assert_eq!(result.len(),2);assert_eq!(result[0]["address"],"172.29.128.195");
        let mac="lo0: flags=8049<UP,LOOPBACK,RUNNING>\n\tinet 127.0.0.1 netmask 0xff000000\nen0: flags=8863<UP,BROADCAST,RUNNING>\n\tinet 192.168.1.20 netmask 0xffffff00 broadcast 192.168.1.255\n\tstatus: active\nen1: flags=8863<UP,BROADCAST>\n\tinet 192.168.2.20 netmask 0xffffff00\n\tstatus: inactive\n";
        assert_eq!(mac_interfaces(mac).unwrap(),vec![json!({"name":"en0","address":"192.168.1.20","prefix":24,"broadcast":"192.168.1.255"})]);
    }
}
