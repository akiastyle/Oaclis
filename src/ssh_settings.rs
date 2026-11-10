// ssh_settings.rs v1.23.371
//! Managed OpenSSH hosts; preserve unrelated configuration and user keys.
use crate::{invalid, staging::StagedFile};
use serde_json::{Value, json};
use std::{env, fs::{self, File, OpenOptions}, io::{self, Read, Write}, net::{IpAddr, Ipv4Addr}, os::{fd::AsRawFd, unix::fs::{DirBuilderExt, OpenOptionsExt}}, path::Path, process::{Command, Stdio}, thread, time::{Duration, Instant}};

pub(crate) fn command(command: &mut Command, input: Option<&[u8]>, timeout: Duration) -> io::Result<Vec<u8>> {
    use std::os::unix::process::CommandExt;
    command.process_group(0).stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().unwrap(); let stderr = child.stderr.take().unwrap();
    let output = thread::spawn(move || { let mut b = Vec::new(); stdout.take(262145).read_to_end(&mut b).map(|_|b) });
    let error = thread::spawn(move || { let mut b = Vec::new(); stderr.take(262145).read_to_end(&mut b).map(|_|b) });
    let sent = match (input, child.stdin.take()) {
        (Some(input), Some(mut stdin)) => { let input=input.to_vec();Some(thread::spawn(move ||stdin.write_all(&input))) },
        _ => None,
    };
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? { break Some(status); }
        if Instant::now() >= deadline {
            let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}",child.id())]).status();
            let _ = child.kill(); let _ = child.wait(); break None;
        }
        thread::sleep(Duration::from_millis(10));
    };
    let stdout = output.join().map_err(|_|io::Error::other("Lettura comando interrotta"))??;
    let stderr = error.join().map_err(|_|io::Error::other("Lettura errore interrotta"))??;
    let status = status.ok_or_else(||io::Error::new(io::ErrorKind::TimedOut,"Tempo massimo del comando superato"))?;
    if !status.success() { return Err(io::Error::other(String::from_utf8_lossy(&stderr).trim().to_owned() + " (comando non riuscito)")); }
    if let Some(sent)=sent {sent.join().map_err(|_|io::Error::other("Invio comando interrotto"))??;}
    if stdout.len() > 262144 || stderr.len() > 262144 { return Err(invalid("Risposta del comando troppo grande")); }
    Ok(stdout)
}

pub(crate) fn text(command: &mut Command, timeout: u64) -> io::Result<String> {
    String::from_utf8(self::command(command,None,Duration::from_secs(timeout))?).map(|s|s.trim_start_matches('\u{feff}').trim().to_owned()).map_err(io::Error::other)
}

pub(crate) fn atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if path.symlink_metadata().is_ok_and(|m| !m.is_file()) { return Err(invalid("Configurazione SSH non regolare")); }
    let parent = path.parent().ok_or_else(||invalid("Cartella mancante"))?;
    let mut staged = StagedFile::new(parent)?;
    staged.file.write_all(bytes)?; staged.file.sync_all()?;
    fs::rename(&staged.path,path)?;
    File::open(parent)?.sync_all()
}

pub(crate) fn read(path: &Path, default: Value) -> io::Result<Value> {
    match crate::open_regular(path) {
        Ok((file,_)) => serde_json::from_reader(file).map_err(io::Error::other),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(default),
        Err(e) => Err(e),
    }
}

pub(crate) fn lock(config: &Path) -> io::Result<File> {
    unsafe extern "C" { fn flock(fd: std::ffi::c_int, operation: std::ffi::c_int) -> std::ffi::c_int; }
    let path = config.join("pairing.lock");
    if path.symlink_metadata().is_ok_and(|m|!m.is_file()) { return Err(invalid("Lock pairing non regolare")); }
    let file = OpenOptions::new().append(true).create(true).mode(0o600).open(path)?;
    // Closing the descriptor releases the same advisory lock used by legacy clients.
    if unsafe { flock(file.as_raw_fd(),2) } != 0 { return Err(io::Error::last_os_error()); }
    Ok(file)
}

pub(crate) fn user(user: &str) -> bool {
    !user.is_empty() && user.len() <= 64 && (user.as_bytes()[0].is_ascii_alphanumeric() || user.starts_with('_'))
        && user.bytes().all(|b|b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

pub(crate) fn validate(value: &Value) -> io::Result<Value> {
    let host = value["address"].as_str().ok_or_else(||invalid("Indirizzo non valido"))?;
    if host.is_empty() || host.len() > 253 || (host.parse::<IpAddr>().is_err() && !(host.as_bytes()[0].is_ascii_alphanumeric()
        && host.as_bytes()[host.len()-1].is_ascii_alphanumeric() && host.bytes().all(|b|b.is_ascii_alphanumeric() || b".-".contains(&b)))) { return Err(invalid("Indirizzo non valido")); }
    let user = value["user"].as_str().filter(|s|self::user(s)).ok_or_else(||invalid("Utente SSH non valido"))?;
    let port = value["port"].as_u64().filter(|p|(1..=65535).contains(p)).ok_or_else(||invalid("Porta SSH non valida"))?;
    let key = value["identity_file"].as_str().ok_or_else(||invalid("Percorso della chiave non valido"))?;
    if (!key.is_empty() && !(key.starts_with('/') || key.starts_with("~/"))) || key.contains(['\r','\n','\0','"','\\','%']) { return Err(invalid("Percorso della chiave non valido")); }
    Ok(json!({"address":host,"user":user,"port":port,"identity_file":key}))
}

pub(crate) fn wsl() -> bool {
    cfg!(target_os="linux") && (env::var_os("WSL_DISTRO_NAME").is_some() || fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default().to_lowercase().contains("microsoft"))
}

pub(crate) fn networks() -> Vec<(Ipv4Addr,u32)> {
    let mut networks = Vec::new();
    if let Ok(text) = text(Command::new("ip").args(["-j","-4","addr","show","up"]),2) {
        if let Ok(Value::Array(interfaces)) = serde_json::from_str::<Value>(&text) {
            for interface in interfaces { for a in interface["addr_info"].as_array().into_iter().flatten() {
                if a["scope"] == "global" { if let (Some(address),Some(prefix)) = (a["local"].as_str().and_then(|s|s.parse().ok()),a["prefixlen"].as_u64().filter(|p|*p<=32)) { networks.push((address,prefix as u32)); } }
            } }
        }
    }
    if wsl() {
        let script = "Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notmatch 'vEthernet|Loopback' -and $_.AddressState -eq 'Preferred' -and $_.IPAddress -notlike '169.254.*' } | Select-Object IPAddress,PrefixLength | ConvertTo-Json -Compress";
        if let Ok(output) = text(&mut crate::peers::powershell(script),5) {
            if let Ok(value) = serde_json::from_str::<Value>(&output) {
                let values = if value.is_array() { value.as_array().unwrap().clone() } else { vec![value] };
                for v in values { if let (Some(ip),Some(prefix)) = (v["IPAddress"].as_str().and_then(|s|s.parse().ok()),v["PrefixLength"].as_u64().filter(|p|*p<=32)) { networks.push((ip,prefix as u32)); } }
            }
        }
    }
    networks
}

fn same_lan(address: &str, networks: &[(Ipv4Addr,u32)]) -> Option<bool> {
    let ip: IpAddr = address.parse().ok()?;
    if networks.is_empty() { return None; }
    Some(match ip { IpAddr::V4(ip) => networks.iter().any(|(network,prefix)| {
        let mask = if *prefix == 0 { 0 } else { u32::MAX << (32-prefix) };
        u32::from(ip)&mask == u32::from(*network)&mask
    }), IpAddr::V6(_) => false })
}

pub(crate) fn ssh_directory(home: &Path) -> io::Result<std::path::PathBuf> {
    let directory = home.join(".ssh");
    if directory.symlink_metadata().is_ok_and(|m|!m.is_dir()) { return Err(invalid("Cartella SSH non regolare")); }
    fs::DirBuilder::new().mode(0o700).create(&directory).or_else(|e|if e.kind()==io::ErrorKind::AlreadyExists { Ok(()) } else { Err(e) })?;
    Ok(directory)
}

pub(crate) fn settings(home: &Path, config: &Path, operation: &str, value: &Value) -> io::Result<Value> {
    let _lock = if operation == "list" { None } else { Some(lock(config)?) };
    apply(home,config,operation,value)
}

pub(crate) fn connections(config: &Path, value: &Value) -> io::Result<Value> {
    let saved = read(&config.join("ssh_settings.json"),json!({}))?;
    if !saved.is_object() { return Err(invalid("Impostazioni SSH non valide")); }
    let mut result = json!({});
    for device in value.as_array().ok_or_else(||invalid("Dispositivi non validi"))? {
        let id = device["id"].as_str().ok_or_else(||invalid("ID non valido"))?;
        if let Some(entry) = saved.get(id) { result[id] = validate(&entry["settings"])?; continue; }
        let alias = device["ssh_alias"].as_str().filter(|s|crate::devices::identifier(s)).ok_or_else(||invalid("Alias non valido"))?;
        if let Ok(output) = text(Command::new("ssh").args(["-G",alias]),3) {
            let mut fields = std::collections::BTreeMap::new();
            for line in output.lines() { if let Some((key,value)) = line.split_once(' ') { fields.entry(key).or_insert(value); } }
            result[id] = json!({"address":fields.get("hostname").unwrap_or(&""),"user":fields.get("user").unwrap_or(&""),"port":fields.get("port").unwrap_or(&"22").parse::<u16>().map_err(io::Error::other)?,"identity_file":fields.get("identityfile").unwrap_or(&"")});
        }
    }
    Ok(result)
}

pub(crate) fn apply(home: &Path, config: &Path, operation: &str, value: &Value) -> io::Result<Value> {
    if operation == "list" {
        let mut result = connections(config,value)?;
        let networks = networks();
        for connection in result.as_object_mut().unwrap().values_mut() { connection["in_lan"] = json!(same_lan(connection["address"].as_str().unwrap_or(""),&networks)); }
        return Ok(result);
    }
    let path = config.join("ssh_settings.json");
    let mut saved = read(&path,json!({}))?;
    if !saved.is_object() { return Err(invalid("Impostazioni SSH non valide")); }
    if !matches!(operation,"save"|"delete") { return Err(invalid("Operazione SSH non valida")); }
    let id = value["id"].as_str().filter(|s|s.len()<=160 && crate::devices::identifier(s)).ok_or_else(||invalid("Dispositivo non valido"))?;
    let alias = value["ssh_alias"].as_str().filter(|s|s.len()<=160 && crate::devices::identifier(s)).ok_or_else(||invalid("Alias non valido"))?;
    if operation == "delete" { if saved.as_object_mut().unwrap().remove(id).is_none() { return Ok(json!({"ok":true})); } }
    else { saved[id] = json!({"ssh_alias":alias,"settings":validate(&value["settings"])?,"paired":value["paired"]==true}); }
    let directory = ssh_directory(home)?;
    let ssh_config = directory.join("config");
    let previous = match crate::open_regular(&ssh_config) { Ok((mut file,_)) => {let mut s=String::new();file.read_to_string(&mut s)?;s}, Err(e) if e.kind()==io::ErrorKind::NotFound => String::new(), Err(e)=>return Err(e) };
    for path in [config.join("ssh_config"),config.join("known_hosts")] { if path.to_string_lossy().contains(['\n','\r','\0','"','\\','%']) { return Err(invalid("Percorso configurazione SSH non valido")); } }
    let include = format!("Include \"{}\"\n",config.join("ssh_config").display());
    let mut hosts = Vec::new();
    for entry in saved.as_object().unwrap().values() {
        let data = validate(&entry["settings"])?;
        let alias = entry["ssh_alias"].as_str().filter(|s|s.len()<=160 && crate::devices::identifier(s)).ok_or_else(||invalid("Alias salvato non valido"))?;
        let mut host = format!("Host {alias}\n  HostName {}\n  Port {}\n  User {}\n  StrictHostKeyChecking yes\n",data["address"].as_str().unwrap(),data["port"],data["user"].as_str().unwrap());
        if entry["paired"] == true { host+=&format!("  UserKnownHostsFile \"{}\" ~/.ssh/known_hosts\n",config.join("known_hosts").display()); }
        if let Some(key) = data["identity_file"].as_str().filter(|s|!s.is_empty()) { host+=&format!("  IdentityFile \"{key}\"\n  IdentitiesOnly yes\n"); }
        hosts.push(host);
    }
    atomic(&config.join("ssh_config"),hosts.join("\n").as_bytes())?;
    if !previous.starts_with(&include) { atomic(&ssh_config,(include+&previous).as_bytes())?; }
    crate::staging::save_json(&path,&saved)?;
    Ok(json!({"ok":true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn step5_command_timeout_also_bounds_a_blocked_input_writer(){
        let start=Instant::now();
        let error=command(Command::new("sh").args(["-c","sleep 10"]),Some(&vec![b'x';1024*1024]),Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.kind(),io::ErrorKind::TimedOut);assert!(start.elapsed()<Duration::from_secs(3));
    }
    #[test]
    fn step3_ssh_rejects_injection_and_estimates_subnets() {
        let value=json!({"address":"example.com","user":"root","port":22,"identity_file":""});
        for (key,invalid) in [("address",json!("example.com\nProxyCommand bad")),("user",json!("-bad")),("port",json!(0)),("port",json!(true)),("identity_file",json!("/tmp/key\"\nHost bad"))] {
            let mut v=value.clone();v[key]=invalid;assert!(validate(&v).is_err());
        }
        let networks=[("192.168.1.138".parse().unwrap(),24)];
        assert_eq!(same_lan("192.168.1.167",&networks),Some(true));
        assert_eq!(same_lan("192.168.2.167",&networks),Some(false));
        assert_eq!(same_lan("example.org",&networks),None);
        assert_eq!(same_lan("192.168.1.167",&[]),None);
    }
    #[test]
    fn step3_ssh_preserves_unmanaged_hosts_and_rejects_symlinks() {
        let home=env::temp_dir().join(format!("oaclis-ssh-step3-{}",std::process::id()));fs::create_dir(&home).unwrap();
        let config=home.join("config");fs::create_dir(&config).unwrap();fs::create_dir(home.join(".ssh")).unwrap();
        let original="Host legacy\n HostName 192.168.1.2\n User root\n UserKnownHostsFile /tmp/original-known-hosts\n\nHost unrelated\n HostName example.org\n";
        fs::write(home.join(".ssh/config"),original).unwrap();
        let mut value=json!({"id":"client","ssh_alias":"legacy","settings":{"address":"203.0.113.10","user":"kim","port":2222,"identity_file":"/tmp/key with space"}});
        settings(&home,&config,"save",&value).unwrap();
        let effective=text(Command::new("ssh").args(["-G","-F"]).arg(home.join(".ssh/config")).arg("legacy"),3).unwrap();
        for expected in ["hostname 203.0.113.10\n","port 2222\n","user kim\n","identityfile /tmp/key with space\n","userknownhostsfile /tmp/original-known-hosts\n"] { assert!(effective.contains(expected),"{expected}"); }
        assert_eq!(connections(&config,&json!([{"id":"client","ssh_alias":"legacy"}])).unwrap()["client"]["address"],"203.0.113.10");
        value["settings"]["port"]=json!(2223);settings(&home,&config,"save",&value).unwrap();
        assert_eq!(fs::read_to_string(home.join(".ssh/config")).unwrap().matches("Include ").count(),1);
        settings(&home,&config,"delete",&value).unwrap();
        assert_eq!(read(&config.join("ssh_settings.json"),Value::Null).unwrap(),json!({}));
        assert_eq!(fs::read_to_string(config.join("ssh_config")).unwrap(),"");
        assert!(fs::read_to_string(home.join(".ssh/config")).unwrap().ends_with(original));
        let target=home.join("untouched");fs::write(&target,"preserve").unwrap();
        fs::remove_file(config.join("ssh_config")).unwrap();std::os::unix::fs::symlink(&target,config.join("ssh_config")).unwrap();
        assert!(settings(&home,&config,"save",&value).is_err());assert_eq!(fs::read_to_string(target).unwrap(),"preserve");
        fs::remove_dir_all(home).unwrap();
    }
}
