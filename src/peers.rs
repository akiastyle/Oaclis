// peers.rs v1.15.409
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
    committed: BTreeMap<String, Value>,
    network: Value,
}
pub type Peers = Arc<Mutex<State>>;
pub(crate) fn local_address(ip: std::net::IpAddr) -> bool {
    matches!(ip, std::net::IpAddr::V4(ip) if ip.is_private() || ip.is_loopback() || ip.is_link_local())
}

pub(crate) fn helper(home: &Path, operation: &str, value: &Value) -> io::Result<Value> {
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
    if local["ssh"]["address"] == local["wsl_ip"] { return Ok(None); }
    let address = local["wsl_ip"].as_str().ok_or_else(|| invalid("Indirizzo WSL assente"))?;
    address.parse::<std::net::Ipv4Addr>().map_err(io::Error::other)?;
    let instance = local["instance"].as_str().unwrap_or("");
    if !instance.chars().all(|c| c.is_ascii_hexdigit() || c == '-') { return Err(invalid("Sessione WSL non valida")); }
    let script = include_str!("../scripts/wsl-discovery.ps1").replacen("param([string]$WslIp, [string]$Instance)", &format!("$WslIp = '{address}'; $Instance = '{instance}'"), 1);
    Ok(Some(powershell(&script).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?))
}
// External work never holds the snapshot lock used by HTTP and automation.
fn announce(
    shared: &Peers, home: &Path, next_probe: &mut Instant,
    discovery_bridge: &mut Option<std::process::Child>,
    run: &impl Fn(&Path, &str, &Value) -> io::Result<Value>,
) -> io::Result<Value> {
    let mut local = shared.lock().unwrap().local.clone();
    if local.is_null() || Instant::now() >= *next_probe {
        let mut prepared = run(home, "prepare", &Value::Null)?;
        if !local.is_null() { prepared["instance"] = local["instance"].clone(); }
        if local["wsl_ip"] != prepared["wsl_ip"] {
            if let Some(mut child) = discovery_bridge.take() { let _ = child.kill(); let _ = child.wait(); }
        }
        local = prepared;
        shared.lock().unwrap().local = local.clone();
        *next_probe = Instant::now() + Duration::from_secs(30);
    }
    if discovery_bridge.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some()) { *discovery_bridge = None; }
    if discovery_bridge.is_none() { *discovery_bridge = bridge(&local)?; }
    let approvals = run(home, "state", &Value::Null)?;
    let packet = run(home, "sign", &local)?;
    let cards: Vec<Value> = {
        let mut state = shared.lock().unwrap();
        state.approvals = approvals;
        state.packet = packet.clone();
        state.error = None;
        state.peers.values().filter(|(card, seen)| {
            let id = card["id"].as_str().unwrap_or("");
            seen.elapsed() < Duration::from_secs(20)
                && state.approvals["approved"][id]["public_key"] == card["public_key"]
                && card["approved"].as_array().is_some_and(|ids| ids.contains(&local["id"]))
                && state.committed.get(id) != Some(&card["ssh"])
        }).map(|(card, _)| card.clone()).collect()
    };
    for card in cards {
        let result = run(home, "commit", &card);
        let mut state = shared.lock().unwrap();
        match result {
            Ok(_) => { state.committed.insert(card["id"].as_str().unwrap().into(), card["ssh"].clone()); }
            Err(error) => { state.error = Some(error.to_string()); }
        }
    }
    Ok(packet)
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
        let sender = match socket.try_clone() {
            Ok(sender) => sender,
            Err(error) => { shared.lock().unwrap().error = Some(error.to_string()); return; }
        };
        let announcements = shared.clone();
        let announcement_home = home.clone();
        thread::spawn(move || {
            let mut next_probe = Instant::now();
            let mut discovery_bridge = None;
            loop {
                let result = announce(&announcements, &announcement_home, &mut next_probe, &mut discovery_bridge, &helper);
                match result {
                    Ok(packet) => {
                        let bytes = serde_json::to_vec(&packet).unwrap();
                        let targets = announcements.lock().unwrap().local["broadcasts"].clone();
                        for address in targets.as_array().into_iter().flatten().filter_map(Value::as_str) {
                            let _ = sender.send_to(&bytes, format!("{address}:23232"));
                        }
                    }
                    Err(error) => { announcements.lock().unwrap().error = Some(error.to_string()); }
                }
                thread::sleep(Duration::from_secs(5));
            }
        });
        let mut budget = 0;
        let mut budget_reset = Instant::now();
        loop {
            if budget_reset.elapsed() >= Duration::from_secs(5) {
                budget = 0;
                budget_reset = Instant::now();
            }
            let mut bytes = [0u8; 8193];
            if let Ok((size, sender)) = socket.recv_from(&mut bytes) {
                if !local_address(sender.ip()) || size > 8192 || budget >= 64 { continue; }
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
                "ssh_ready":card["ssh"]["host_key"].is_string(),
                "accepted":state.approvals["approved"][id]["public_key"] == card["public_key"],
                "remote_accepted":card["approved"].as_array().unwrap().contains(&state.local["id"]),
                "device_id":state.approvals["approved"][id]["device_id"],
                "connected":state.committed.contains_key(id)
                    && state.approvals["approved"][id]["public_key"] == card["public_key"]
                    && card["approved"].as_array().unwrap().contains(&state.local["id"])})
        }).collect();
    json!({"local_id":state.local["id"],"wsl":state.local["wsl"],"setup_needed":state.local["setup_needed"],"setup_kind":state.local["setup_kind"],"port":23232,"peers":list,"error":state.error,"network":state.network,"ignored":state.approvals["ignored"].as_array().map_or(0, Vec::len)})
}
pub fn forget(peers: &Peers, home: &Path, id: &str) -> io::Result<()> {
    helper(home, "forget", &json!({"id":id}))?;
    let approvals = helper(home, "state", &Value::Null)?;
    let mut state = peers.lock().unwrap();
    state.approvals = approvals;
    let ignored = state.approvals["ignored"].clone();
    state.committed.retain(|peer, _| !ignored.as_array().is_some_and(|ids| ids.contains(&json!(peer))));
    Ok(())
}
pub fn restore(peers: &Peers, home: &Path) -> io::Result<Value> {
    let result = helper(home, "restore", &Value::Null)?;
    let approvals = helper(home, "state", &Value::Null)?;
    peers.lock().unwrap().approvals = approvals;
    Ok(result)
}
pub fn hello(peers: &Peers) -> Value { peers.lock().unwrap().packet.clone() }
pub fn approve(peers: &Peers, home: &Path, value: &Value) -> io::Result<Value> {
    let id = value["id"].as_str().ok_or_else(|| invalid("Dispositivo richiesto"))?;
    let accept = value["accept"].as_bool().ok_or_else(|| invalid("Conferma richiesta"))?;
    let peer = {
        let state = peers.lock().unwrap();
        let (peer, seen) = state.peers.get(id).ok_or_else(|| invalid("Dispositivo non rilevato"))?;
        if seen.elapsed() >= Duration::from_secs(20) { return Err(invalid("Dispositivo non più raggiungibile")); }
        peer.clone()
    };
    let result = helper(home, "approve", &json!({"peer":peer,"accept":accept}))?;
    let approvals = helper(home, "state", &Value::Null)?;
    peers.lock().unwrap().approvals = approvals;
    Ok(result)
}
pub fn network(peers: &Peers) -> io::Result<Value> {
    let mut state = peers.lock().unwrap();
    if state.network["status"] == "pending" { return Err(invalid("Configurazione Windows già in attesa")); }
    if state.local["wsl"] != true { return Err(invalid("Questo comando riguarda WSL")); }
    if state.local["ssh"]["address"] == state.local["wsl_ip"] { return Err(invalid("WSL con rete condivisa: configurare il firewall senza portproxy, come indicato nel README")); }
    let lan = state.local["ssh"]["address"].as_str().unwrap();
    let wsl = state.local["wsl_ip"].as_str().unwrap();
    lan.parse::<std::net::Ipv4Addr>().map_err(io::Error::other)?;
    wsl.parse::<std::net::Ipv4Addr>().map_err(io::Error::other)?;
    let script = include_str!("../scripts/wsl-network.ps1").replacen("param([Parameter(Mandatory=$true)][string]$LanIp, [Parameter(Mandatory=$true)][string]$WslIp)", &format!("$LanIp = '{lan}'; $WslIp = '{wsl}'"), 1);
    // PowerShell performs the UTF-16/base64 encoding; the script is carried on stdin.
    let launcher = "$ErrorActionPreference = 'Stop'; $code = [Console]::In.ReadToEnd(); $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($code)); $process = Start-Process powershell.exe -Verb RunAs -ArgumentList @('-NoProfile','-EncodedCommand',$encoded) -Wait -PassThru; if ($process.ExitCode -ne 0) { throw 'Configurazione rete non riuscita: verificare la finestra PowerShell.' }";
    state.network = json!({"status":"pending"});
    drop(state);
    let launched = (|| -> io::Result<std::process::Child> {
        let mut child = powershell(launcher).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn()?;
        if let Err(error) = child.stdin.take().unwrap().write_all(script.as_bytes()) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        Ok(child)
    })();
    let child = match launched {
        Ok(child) => child,
        Err(error) => {
            peers.lock().unwrap().network = json!({"status":"failed","error":error.to_string()});
            return Err(error);
        }
    };
    let shared = peers.clone();
    thread::spawn(move || {
        let result = match child.wait_with_output() {
            Ok(output) if output.status.success() => json!({"status":"ready"}),
            Ok(output) => json!({"status":"failed","error":String::from_utf8_lossy(&output.stderr)}),
            Err(error) => json!({"status":"failed","error":error.to_string()}),
        };
        shared.lock().unwrap().network = result;
    });
    Ok(json!({"ok":true,"message":"Conferma la richiesta amministrativa di Windows su questo dispositivo"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(local: &str, remote: &str, remote_accepted: bool) -> Peers {
        let mut state = State::default();
        state.local = json!({"id":local,"public_key":local});
        state.approvals = json!({"approved":{remote:{"public_key":remote}},"ignored":[]});
        state.peers.insert(remote.into(), (json!({"id":remote,"public_key":remote,"name":"Peer","role":"client","ssh":{"address":"192.168.1.1"},"approved":if remote_accepted {vec![local]} else {vec![]}}), Instant::now()));
        state.committed.insert(remote.into(), json!({}));
        Arc::new(Mutex::new(state))
    }
    #[test]
    fn both_screens_share_code_and_single_approval_is_pending() {
        let a = fixture("a", "b", false);
        let b = fixture("b", "a", true);
        assert_eq!(snapshot(&a)["peers"][0]["code"], snapshot(&b)["peers"][0]["code"]);
        assert_eq!(snapshot(&a)["peers"][0]["connected"], false);
        assert_eq!(snapshot(&b)["peers"][0]["connected"], true);
        a.lock().unwrap().peers.get_mut("b").unwrap().1 = Instant::now() - Duration::from_secs(21);
        assert_eq!(snapshot(&a)["peers"], json!([]));
    }
    #[test]
    fn slow_network_probe_does_not_block_snapshots() {
        use std::sync::mpsc;
        let peers = fixture("a", "b", false);
        let worker_peers = peers.clone();
        let (started, ready) = mpsc::channel();
        let (release, wait) = mpsc::channel();
        let worker = thread::spawn(move || {
            let wait = Mutex::new(wait);
            announce(&worker_peers, Path::new("."), &mut Instant::now(), &mut None, &|_, operation, _| {
                if operation == "prepare" {
                    started.send(()).unwrap();
                    wait.lock().unwrap().recv().unwrap();
                    return Err(io::Error::other("simulated timeout"));
                }
                unreachable!()
            })
        });
        ready.recv_timeout(Duration::from_secs(2)).unwrap();
        let (send, result) = mpsc::channel();
        let reader = thread::spawn(move || send.send(snapshot(&peers)).unwrap());
        let observed = result.recv_timeout(Duration::from_millis(500));
        release.send(()).unwrap();
        assert!(worker.join().unwrap().is_err());
        reader.join().unwrap();
        assert_eq!(observed.expect("Network probe blocked the UI")["local_id"], "a");
    }

}
