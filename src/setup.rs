// setup.rs v1.7.141
//! First-run role selection. Discovery never authorizes SSH or copies chats.
use crate::{invalid, peers, staging};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io::{self, IsTerminal, Read, Write}, net::{Ipv4Addr, SocketAddr, TcpStream, UdpSocket}, path::Path, time::{Duration, Instant}};

fn announce(address: &str) -> io::Result<Value> {
    let ip: Ipv4Addr = address.parse().map_err(io::Error::other)?;
    if !peers::local_address(ip.into()) { return Err(invalid("Indirizzo LAN richiesto")); }
    let mut stream = TcpStream::connect_timeout(&SocketAddr::from((ip, 23232)), Duration::from_millis(300))?;
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    write!(stream, "GET /peer/hello HTTP/1.0\r\nHost: {ip}:23232\r\nConnection: close\r\n\r\n")?;
    let mut bytes = Vec::new();
    stream.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 { return Err(invalid("Annuncio troppo grande")); }
    let split = bytes.windows(4).position(|b| b == b"\r\n\r\n").ok_or_else(|| invalid("Risposta HTTP non valida"))?;
    let status = std::str::from_utf8(&bytes[..split]).map_err(io::Error::other)?;
    if !status.lines().next().is_some_and(|line| line.split_whitespace().nth(1) == Some("200")) { return Err(invalid("Oaclis non pronto")); }
    serde_json::from_slice(&bytes[split+4..]).map_err(io::Error::other)
}
fn remember(found: &mut BTreeMap<String, Value>, home: &Path, packet: &Value, own_id: &Value) {
    if let Ok(card) = peers::helper(home, "verify", packet) {
        if card["role"] == "master" && card["id"] != *own_id {
            found.insert(card["id"].as_str().unwrap().into(), card);
        }
    }
}
fn discover(home: &Path, settings: &Value) -> io::Result<Vec<Value>> {
    let local = peers::helper(home, "prepare", &Value::Null)?;
    let socket = UdpSocket::bind("0.0.0.0:23232").map_err(|error| invalid(format!("Porta UDP 23232 non disponibile ({error}): se Oaclis è già avviato, arrestarlo prima di setup")))?;
    socket.set_read_timeout(Some(Duration::from_millis(250)))?;
    let mut addresses = Vec::new();
    if let Some(address) = settings["master"]["address"].as_str() { addresses.push(address.to_owned()); }
    // ARP is a source of candidate addresses; only a signed Oaclis reply identifies a master.
    match peers::helper(home, "neighbors", &Value::Null) {
        Ok(value) => for address in value.as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !addresses.iter().any(|a| a == address) { addresses.push(address.into()); }
        },
        Err(error) => eprintln!("Tabella vicini: {error}; continuo con gli annunci LAN."),
    }
    let mut found = BTreeMap::new();
    for address in addresses {
        if let Ok(packet) = announce(&address) { remember(&mut found, home, &packet, &local["id"]); }
    }
    let deadline = Instant::now() + Duration::from_secs(if found.is_empty() { 12 } else { 2 });
    let mut budget = 0;
    while Instant::now() < deadline {
        let mut bytes = [0u8; 8193];
        if let Ok((size, sender)) = socket.recv_from(&mut bytes) {
            if size > 8192 || budget >= 64 || !peers::local_address(sender.ip()) { continue; }
            budget += 1;
            if let Ok(packet) = serde_json::from_slice(&bytes[..size]) { remember(&mut found, home, &packet, &local["id"]); }
        }
    }
    Ok(found.into_values().collect())
}
fn input() -> io::Result<String> {
    let mut value = String::new();
    if io::stdin().read_line(&mut value)? == 0 { return Ok("q".into()); }
    Ok(value.trim().to_lowercase())
}
fn save(home: &Path, mut settings: Value, role: &str, master: Option<&Value>) -> io::Result<()> {
    let config = home.join(".config/oaclis");
    settings["role"] = json!(role);
    if let Some(master) = master {
        settings["master"] = json!({"id":master["id"],"name":master["name"],"address":master["ssh"]["address"],"port":23232,"authorized":false});
    }
    let role_path = config.join("role");
    if role_path.symlink_metadata().is_ok_and(|metadata| !metadata.is_file()) { return Err(invalid("File ruolo non regolare")); }
    staging::save_json(&config.join("settings.json"), &settings)?;
    let mut staged = staging::StagedFile::new(&config)?;
    writeln!(staged.file, "{role}")?;
    staged.file.sync_all()?;
    fs::rename(&staged.path, role_path)?;
    println!("Ruolo salvato: {role}. Impostazioni: {}", config.join("settings.json").display());
    if master.is_some() { println!("Master trovato. L'autorizzazione reciproca non è ancora eseguita."); }
    Ok(())
}
pub fn configure(home: &Path) -> io::Result<()> {
    let config = home.join(".config/oaclis");
    fs::create_dir_all(&config)?;
    let path = config.join("settings.json");
    let settings: Value = if path.exists() { serde_json::from_reader(crate::open_regular(&path)?.0)? } else { json!({"schema_version":1,"control_port":23232}) };
    if !settings.is_object() { return Err(invalid("Settings non validi: file conservato")); }
    let role = match fs::read_to_string(config.join("role")) {
        Ok(role) => role.trim().to_owned(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => settings["role"].as_str().unwrap_or("").to_owned(),
        Err(error) => return Err(error),
    };
    if matches!(role.as_str(), "master" | "client") {
        println!("Ruolo già configurato: {role}; conservato.");
        return Ok(());
    }
    if !role.is_empty() { return Err(invalid("Ruolo salvato non valido")); }
    loop {
        println!("Ricerca dei master Oaclis nella LAN sulla porta 23232…");
        let masters = discover(home, &settings)?;
        if masters.len() == 1 { return save(home, settings, "client", masters.first()); }
        if !io::stdin().is_terminal() {
            println!("Ruolo non configurato: eseguire oaclis setup da un terminale interattivo.");
            return Ok(());
        }
        if masters.len() > 1 {
            for (index, master) in masters.iter().enumerate() { println!("{}: {} ({})", index+1, master["name"].as_str().unwrap(), master["ssh"]["address"].as_str().unwrap()); }
            println!("Scegli il numero del master, oppure q per rimandare:");
            let choice = input()?;
            if choice == "q" { return Ok(()); }
            if let Ok(index) = choice.parse::<usize>() {
                if index > 0 && index <= masters.len() { return save(home, settings, "client", masters.get(index-1)); }
            }
        } else {
            println!("Nessun master trovato. r: riprova, m: configura questo dispositivo come master, q: rimanda");
            match input()?.as_str() {
                "m" => return save(home, settings, "master", None),
                "q" => return Ok(()),
                _ => {},
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn role_and_last_master_are_saved_without_authorizing_access() {
        let home = std::env::temp_dir().join(format!("oaclis-setup-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(home.join(".config/oaclis")).unwrap();
        let master = json!({"id":"known-id","name":"ROG","ssh":{"address":"192.168.1.138"}});
        save(&home,json!({"environment":{"os":"linux"},"tools":{"codex":{"detected":true}}}),"client",Some(&master)).unwrap();
        let stored: Value = serde_json::from_slice(&fs::read(home.join(".config/oaclis/settings.json")).unwrap()).unwrap();
        assert_eq!(stored["role"],"client");
        assert_eq!(stored["master"]["port"],23232);
        assert_eq!(stored["master"]["authorized"],false);
        assert_eq!(stored["environment"]["os"],"linux");
        assert!(!home.join(".ssh/authorized_keys").exists());
        configure(&home).unwrap();
        assert_eq!(fs::read_to_string(home.join(".config/oaclis/role")).unwrap(),"client\n");
        fs::remove_dir_all(home).unwrap();
    }
}
