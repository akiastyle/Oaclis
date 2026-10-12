//! Signed LAN discovery; pairing requires an explicit approval on both devices.
use crate::invalid;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, io::{self, Write}, net::UdpSocket, path::{Path, PathBuf}, process::{Command, Stdio}, sync::{Arc, Mutex}, thread, time::{Duration, Instant}};

#[derive(Default)]
pub struct State {
    local: Value,
    packet: Value,
    peers: BTreeMap<String, (Value, Instant)>,
    approvals: Value,
    error: Option<String>,
    bridge: Option<std::process::Child>,
    committed: BTreeMap<String, Value>,
}
pub type Peers = Arc<Mutex<State>>;

fn helper(home: &Path, operation: &str, value: &Value) -> io::Result<Value> {
    let mut child = Command::new("python3").args(["-c", include_str!("../tools/pairing.py")])
        .arg(home).arg(home.join(".config/oaclis")).arg(operation)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&json!({"value":value,"ssh_source":include_str!("../tools/ssh_settings.py")}))?)?;
    let output = child.wait_with_output()?;
    let result: Value = serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
    if !output.status.success() { return Err(invalid(result["error"].as_str().unwrap_or("Collegamento non riuscito"))); }
    Ok(result)
}
fn powershell(script: &str) -> Command {
    // SSH sessions do not inherit the Windows PATH from an interactive WSL shell.
    let executable = Path::new("/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe");
    let mut command = Command::new(if executable.exists() { executable.as_os_str() } else { std::ffi::OsStr::new("powershell.exe") });
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    command
}
fn bridge(local: &Value) -> io::Result<Option<std::process::Child>> {
    if local["wsl"] != true { return Ok(None); }
    let address = local["wsl_ip"].as_str().ok_or_else(|| invalid("Indirizzo WSL assente"))?;
    address.parse::<std::net::Ipv4Addr>().map_err(io::Error::other)?;
    let instance = local["instance"].as_str().unwrap_or("");
    if !instance.chars().all(|c| c.is_ascii_hexdigit() || c == '-') { return Err(invalid("Sessione WSL non valida")); }
    let script = include_str!("../scripts/wsl-discovery.ps1").replacen("param([string]$WslIp, [string]$Instance)", &format!("$WslIp = '{address}'; $Instance = '{instance}'"), 1);
    Ok(Some(powershell(&script).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?))
}
pub fn start(home: PathBuf) -> Peers {
    let state = Arc::new(Mutex::new(State::default()));
    let shared = state.clone();
    thread::spawn(move || {
        let socket = match UdpSocket::bind("0.0.0.0:23232") {
            Ok(socket) => socket,
            Err(error) => { shared.lock().unwrap().error = Some(error.to_string()); return; }
        };
        let _ = socket.set_broadcast(true);
        let _ = socket.set_read_timeout(Some(Duration::from_millis(250)));
        let mut next_announcement = Instant::now();
        let mut budget = 0;
        loop {
            if Instant::now() >= next_announcement {
                next_announcement = Instant::now() + Duration::from_secs(5);
                budget = 0;
                let result = (|| -> io::Result<Value> {
                    let mut state = shared.lock().unwrap();
                    if state.local.is_null() { state.local = helper(&home, "prepare", &Value::Null)?; }
                    if state.bridge.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some()) { state.bridge = None; }
                    if state.bridge.is_none() { state.bridge = bridge(&state.local)?; }
                    state.approvals = helper(&home, "state", &Value::Null)?;
                    let packet = helper(&home, "sign", &state.local)?;
                    state.packet = packet.clone();
                    state.error = None;
                    let cards: Vec<Value> = state.peers.values().filter(|(_, seen)| seen.elapsed() < Duration::from_secs(20)).map(|(card, _)| card.clone()).collect();
                    for card in cards {
                        let id = card["id"].as_str().unwrap();
                        if state.approvals["approved"][id]["public_key"] == card["public_key"]
                            && card["approved"].as_array().unwrap().contains(&state.local["id"])
                            && state.committed.get(id) != Some(&card["ssh"])
                        {
                            match helper(&home, "commit", &card) {
                                Ok(_) => { state.committed.insert(id.into(), card["ssh"].clone()); }
                                Err(error) => { state.error = Some(error.to_string()); }
                            }
                        }
                    }
                    Ok(packet)
                })();
                match result {
                    Ok(packet) => { let _ = socket.send_to(&serde_json::to_vec(&packet).unwrap(), "255.255.255.255:23232"); }
                    Err(error) => { shared.lock().unwrap().error = Some(error.to_string()); }
                }
            }
            let mut bytes = [0u8; 8193];
            if let Ok((size, _)) = socket.recv_from(&mut bytes) {
                if size > 8192 || budget >= 64 { continue; }
                let Ok(packet) = serde_json::from_slice::<Value>(&bytes[..size]) else { continue; };
                let Ok(unverified) = serde_json::from_str::<Value>(packet["hello"].as_str().unwrap_or("")) else { continue; };
                {
                    let state = shared.lock().unwrap();
                    if unverified["id"] == state.local["id"] || state.peers.get(unverified["id"].as_str().unwrap_or("")).is_some_and(|(card,_)| card["time"].as_i64() >= unverified["time"].as_i64()) { continue; }
                }
                budget += 1;
                if let Ok(card) = helper(&home, "verify", &packet) {
                    let mut state = shared.lock().unwrap();
                    state.peers.retain(|_, (_, seen)| seen.elapsed() < Duration::from_secs(300));
                    if state.peers.len() >= 64 && !state.peers.contains_key(card["id"].as_str().unwrap()) { continue; }
                    state.peers.insert(card["id"].as_str().unwrap().into(), (card, Instant::now()));
                }
            }
        }
    });
    state
}
pub fn snapshot(peers: &Peers) -> Value {
    let state = peers.lock().unwrap();
    let list: Vec<Value> = state.peers.values().filter(|(card, seen)| seen.elapsed() < Duration::from_secs(20)
        && !state.approvals["ignored"].as_array().is_some_and(|ids| ids.contains(&card["id"])))
        .map(|(card, _)| {
            let mut keys = [state.local["public_key"].as_str().unwrap_or(""), card["public_key"].as_str().unwrap()];
            keys.sort();
            let digest: String = Sha256::digest(keys.join("\n").as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
            let id = card["id"].as_str().unwrap();
            json!({"id":id,"name":card["name"],"role":card["role"],"address":card["ssh"]["address"],
                "code":format!("{} {} {}", &digest[..4], &digest[4..8], &digest[8..12]),
                "accepted":state.approvals["approved"][id]["public_key"] == card["public_key"],
                "remote_accepted":card["approved"].as_array().unwrap().contains(&state.local["id"]),
                "device_id":state.approvals["approved"][id]["device_id"],
                "connected":state.committed.contains_key(id)
                    && state.approvals["approved"][id]["public_key"] == card["public_key"]
                    && card["approved"].as_array().unwrap().contains(&state.local["id"])})
        }).collect();
    json!({"local_id":state.local["id"],"wsl":state.local["wsl"],"peers":list,"error":state.error,"ignored":state.approvals["ignored"].as_array().map_or(0, Vec::len)})
}
pub fn forget(peers: &Peers, home: &Path, id: &str) -> io::Result<()> {
    let mut state = peers.lock().unwrap();
    helper(home, "forget", &json!({"id":id}))?;
    state.approvals = helper(home, "state", &Value::Null)?;
    let ignored = state.approvals["ignored"].clone();
    state.committed.retain(|peer, _| !ignored.as_array().is_some_and(|ids| ids.contains(&json!(peer))));
    Ok(())
}
pub fn restore(peers: &Peers, home: &Path) -> io::Result<Value> {
    let mut state = peers.lock().unwrap();
    let result = helper(home, "restore", &Value::Null)?;
    state.approvals = helper(home, "state", &Value::Null)?;
    Ok(result)
}
pub fn hello(peers: &Peers) -> Value { peers.lock().unwrap().packet.clone() }
pub fn approve(peers: &Peers, home: &Path, value: &Value) -> io::Result<Value> {
    let id = value["id"].as_str().ok_or_else(|| invalid("Dispositivo richiesto"))?;
    let accept = value["accept"].as_bool().ok_or_else(|| invalid("Conferma richiesta"))?;
    let mut state = peers.lock().unwrap();
    let (peer, seen) = state.peers.get(id).ok_or_else(|| invalid("Dispositivo non rilevato"))?;
    if seen.elapsed() >= Duration::from_secs(20) { return Err(invalid("Dispositivo non più raggiungibile")); }
    let result = helper(home, "approve", &json!({"peer":peer,"accept":accept}))?;
    state.approvals = helper(home, "state", &Value::Null)?;
    Ok(result)
}
pub fn network(peers: &Peers) -> io::Result<Value> {
    let state = peers.lock().unwrap();
    if state.local["wsl"] != true { return Err(invalid("Questo comando riguarda WSL")); }
    let lan = state.local["ssh"]["address"].as_str().unwrap();
    let wsl = state.local["wsl_ip"].as_str().unwrap();
    lan.parse::<std::net::Ipv4Addr>().map_err(io::Error::other)?;
    wsl.parse::<std::net::Ipv4Addr>().map_err(io::Error::other)?;
    let script = include_str!("../scripts/wsl-network.ps1").replacen("param([Parameter(Mandatory=$true)][string]$LanIp, [Parameter(Mandatory=$true)][string]$WslIp)", &format!("$LanIp = '{lan}'; $WslIp = '{wsl}'"), 1);
    // PowerShell performs the UTF-16/base64 encoding; the script is carried on stdin.
    let launcher = "$code = [Console]::In.ReadToEnd(); $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code)); Start-Process powershell.exe -Verb RunAs -ArgumentList @('-NoProfile','-EncodedCommand',$encoded)";
    let mut child = powershell(launcher).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(script.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() { return Err(invalid(String::from_utf8_lossy(&output.stderr))); }
    Ok(json!({"ok":true,"message":"Conferma la richiesta amministrativa di Windows su questo dispositivo"}))
}
