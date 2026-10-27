// catalog.rs v2.33.1538
//! Read Codex archives; title edits use native Codex, Oaclis metadata stays in memory.
use crate::{devices::Device, state_store};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::{self, BufRead, BufReader},
    path::Path,
    time::UNIX_EPOCH,
};

pub(crate) type Catalogs = HashMap<String, Result<std::sync::Arc<Vec<Value>>, String>>;

// One snapshot (including failures) per device in a comparison cycle.
pub(crate) fn cached<'a>(catalogs: &'a mut Catalogs, id: &str, load: impl FnOnce() -> io::Result<Vec<Value>>) -> io::Result<&'a [Value]> {
    match catalogs.entry(id.into()).or_insert_with(|| load().map(std::sync::Arc::new).map_err(|error| error.to_string())) {
        Ok(chats) => Ok(chats),
        Err(error) => Err(io::Error::other(error.clone())),
    }
}

pub(crate) fn local(root: &Path) -> io::Result<Vec<Value>> {
    let home = std::env::var_os("HOME").ok_or_else(|| crate::invalid("HOME non definita"))?;
    local_at(root, &Path::new(&home).join(".config/oaclis"))
}
pub(crate) fn local_at(root: &Path, config: &Path) -> io::Result<Vec<Value>> {
    let root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()?.join(root)
    };
    let mut names = BTreeMap::new();
    let mut hidden = BTreeSet::new();
    let database = if root.join("state_5.sqlite").exists() {
        root.join("state_5.sqlite")
    } else {
        root.join("sqlite/state_5.sqlite")
    };
    if database.is_file() {
        let db = rusqlite::Connection::open_with_flags(
            database,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )
        .map_err(io::Error::other)?;
        let mut query = db
            .prepare("SELECT id,name,title,source FROM threads")
            .map_err(io::Error::other)?;
        let rows = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })
            .map_err(io::Error::other)?;
        for row in rows {
            let (id, name, title, source) = row.map_err(io::Error::other)?;
            if let Some(source) = source {
                if source.starts_with('{')
                    && serde_json::from_str::<Value>(&source)?
                        .get("subagent")
                        .is_some()
                {
                    hidden.insert(id.clone());
                }
            }
            let title = name
                .filter(|s| !s.is_empty())
                .or(title)
                .unwrap_or_default()
                .trim()
                .to_owned();
            if !title.is_empty() {
                names.insert(id, title);
            }
        }
    }
    if let Ok((file, _)) = crate::open_regular(&root.join("session_index.jsonl")) {
        for line in BufReader::new(file).lines() {
            let line = line?;
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                if let (Some(id), Some(name)) = (
                    value["id"].as_str(),
                    value["thread_name"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty()),
                ) {
                    names.entry(id.to_owned()).or_insert(name.into());
                }
            }
        }
    }
    let mut chats = Vec::new();
    walk(&root, &root.join("sessions"), &names, &hidden, &mut chats)?;
    let state = state_store::snapshot(config)?;
    let id = state_store::identity(config)?;
    for chat in &mut chats {
        let chat_id = chat["id"].as_str().unwrap().to_owned();
        chat["analysis"] = state
            .analysis
            .get(&format!("master:{chat_id}"))
            .cloned()
            .unwrap_or(json!({"status":"missing"}));
        let mut data = json!({});
        if let Some(record) = state.appearance.get(&format!("chat:{chat_id}")) {
            for name in state_store::MODULES {
                if record.sync.get(name).is_some() {
                    data[name] = record.sync[name]["sources"][&id].clone();
                }
            }
        }
        state_store::strip_entries(&mut data);
        chat["sync_data"] = data;
    }
    chats.retain(|chat| state.shared["deletions"].get(chat["id"].as_str().unwrap_or("")).is_none());
    chats.sort_by(|a, b| b["path"].as_str().cmp(&a["path"].as_str()));
    Ok(chats)
}
fn walk(
    root: &Path,
    directory: &Path,
    names: &BTreeMap<String, String>,
    hidden: &BTreeSet<String>,
    chats: &mut Vec<Value>,
) -> io::Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() {
            walk(root, &path, names, hidden, chats)?;
            continue;
        }
        if !kind.is_file() {
            continue;
        }
        let filename = entry.file_name();
        let Some(filename) = filename.to_str() else {
            continue;
        };
        if !filename.starts_with("rollout-") || !filename.ends_with(".jsonl") || filename.len() < 42
        {
            continue;
        }
        let Some(id) = filename.get(filename.len() - 42..filename.len() - 6) else {
            continue;
        };
        if hidden.contains(id) {
            continue;
        }
        let metadata = entry.metadata()?;
        chats.push(json!({"id":id,"name":names.get(id).map(String::as_str).unwrap_or("Senza titolo"),"path":path,"relative_path":path.strip_prefix(root).map_err(io::Error::other)?,"bytes":metadata.len(),"modified_ns":metadata.modified()?.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos().to_string()}));
    }
    Ok(())
}
// Keep one native Codex connection for explicit title edits, outside the state locks.
struct NameClient {
    child: std::process::Child,
    input: std::process::ChildStdin,
    replies: std::sync::mpsc::Receiver<Value>,
    next: u64,
}
impl Drop for NameClient {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill").args(["-KILL", "--", &format!("-{}", self.child.id())]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl NameClient {
    fn start(root: &Path, executable: &Path) -> io::Result<Self> {
        use std::{os::unix::process::CommandExt, process::{Command, Stdio}};
        let mut child = Command::new(executable).args(["app-server", "--listen", "stdio://"])
            .env("CODEX_HOME", root).process_group(0).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let (send, replies) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                let Ok(value) = serde_json::from_str::<Value>(&line) else { break };
                if value.get("id").is_some() && send.send(value).is_err() { break; }
            }
        });
        let mut client = Self { child, input, replies, next: 0 };
        client.request("initialize", json!({"clientInfo":{"name":"oaclis","version":env!("OACLIS_APP_VERSION")}}))?;
        use std::io::Write;
        client.input.write_all(b"{\"method\":\"initialized\",\"params\":{}}\n")?;
        Ok(client)
    }
    fn request(&mut self, method: &str, params: Value) -> io::Result<Value> {
        use std::io::Write;
        let id = self.next;
        self.next += 1;
        serde_json::to_writer(&mut self.input, &json!({"id":id,"method":method,"params":params}))?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            let value = self.replies.recv_timeout(until.saturating_duration_since(std::time::Instant::now()))
                .map_err(|_|io::Error::new(io::ErrorKind::TimedOut,"Codex non risponde alla rinomina"))?;
            if value["id"] != id { continue; }
            if let Some(error) = value.get("error") { return Err(io::Error::other(error["message"].as_str().unwrap_or("Rinomina Codex non riuscita"))); }
            return value.get("result").cloned().ok_or_else(||crate::invalid("Risposta rinomina Codex non valida"));
        }
    }
}
pub(crate) fn rename(home: &Path, root: &Path, id: &str, name: &str) -> io::Result<()> {
    if id.len() != 36 || !id.bytes().enumerate().all(|(i,b)|if [8,13,18,23].contains(&i) {b==b'-'} else {b.is_ascii_hexdigit()}) {
        return Err(crate::invalid("Identificativo chat non valido"));
    }
    if name.trim().is_empty() || name != name.trim() {
        return Err(crate::invalid("Nome chat non valido"));
    }
    crate::appearance::Appearance { label: name.into(), ..Default::default() }.validate()?;
    static CLIENT: std::sync::OnceLock<std::sync::Mutex<Option<(std::path::PathBuf, NameClient)>>> = std::sync::OnceLock::new();
    let mut saved = CLIENT.get_or_init(Default::default).lock().map_err(|_|io::Error::other("Rinomina Codex non disponibile"))?;
    if saved.as_ref().is_some_and(|(path, _)|path != root) { *saved = None; }
    let result = (|| {
        if saved.is_none() {
            let native = root.join("packages/app-server-daemon/current/bin/codex");
            let executable = if native.is_file() { native } else { crate::terminal::codex(home)? };
            *saved = Some((root.into(), NameClient::start(root, &executable)?));
        }
        saved.as_mut().unwrap().1.request("thread/name/set", json!({"threadId":id,"name":name})).map(|_|())
    })();
    if let Err(error) = result {
        *saved = None;
        crate::state_transport::log(format!("Codex rename {id}: {error}"));
        return Err(io::Error::other("Impossibile rinominare la chat in Codex. Riprova."));
    }
    Ok(())
}

pub(crate) fn remote(device: &Device) -> io::Result<Value> {
    crate::state_transport::remote(device, &json!({"kind":"catalog"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn catalog_skips_links_and_uses_latest_title() {
        let root = std::env::temp_dir().join(format!("oaclis-catalog-{}", std::process::id()));
        fs::create_dir_all(root.join("sessions")).unwrap();
        fs::write(
            root.join("session_index.jsonl"),
            "{\"id\":\"01a0568c-cf6c-7d62-bb6d-12c20b4d9b16\",\"thread_name\":\"Titolo\"}\n",
        )
        .unwrap();
        fs::write(
            root.join(
                "sessions/rollout-2026-08-31T08-41-03-01a0568c-cf6c-7d62-bb6d-12c20b4d9b16.jsonl",
            ),
            "secret content",
        )
        .unwrap();
        let entries = local_at(&root, &root.join("config")).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "Titolo");
        assert!(!entries[0].to_string().contains("secret content"));
        std::os::unix::fs::symlink(
            root.join("session_index.jsonl"),
            root.join(
                "sessions/rollout-2026-08-31T08-41-03-01a0568c-cf6c-7d62-bb6d-12c20b4d9b17.jsonl",
            ),
        )
        .unwrap();
        fs::write(root.join("session_index.jsonl"), "").unwrap();
        let unindexed = local_at(&root, &root.join("config")).unwrap();
        assert_eq!(unindexed.len(), 1);
        assert_eq!(unindexed[0]["name"], "Senza titolo");
        let database=rusqlite::Connection::open(root.join("state_5.sqlite")).unwrap();
        database.execute_batch("CREATE TABLE threads(id TEXT,name TEXT,title TEXT,source TEXT)").unwrap();
        database.execute("INSERT INTO threads VALUES(?1,?2,?3,?4)",rusqlite::params!["01a0568c-cf6c-7d62-bb6d-12c20b4d9b16","Titolo Codex","Primo messaggio","cli"]).unwrap();
        database.execute("INSERT INTO threads VALUES(?1,?2,?3,?4)",rusqlite::params!["01a0568c-cf6c-7d62-bb6d-12c20b4d9b18",Option::<String>::None,"",r#"{"subagent":{}}"#]).unwrap();
        drop(database);
        fs::write(
            root.join(
                "sessions/rollout-2026-08-31T08-41-03-01a0568c-cf6c-7d62-bb6d-12c20b4d9b18.jsonl",
            ),
            "internal",
        )
        .unwrap();
        let titled = local_at(&root, &root.join("config")).unwrap();
        assert_eq!(titled.len(), 1);
        assert_eq!(titled[0]["name"], "Titolo Codex");
        rusqlite::Connection::open(root.join("state_5.sqlite")).unwrap().execute("UPDATE threads SET name='Titolo aggiornato' WHERE source='cli'",[]).unwrap();
        assert_eq!(
            local_at(&root, &root.join("config")).unwrap()[0]["name"],
            "Titolo aggiornato"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cycle_reuses_each_devices_catalog_and_errors_then_reloads_next_cycle() {
        let mut catalogs = Catalogs::new();
        let calls = std::cell::Cell::new(0);
        for id in ["master", "peer-a", "peer-b", "master", "peer-a", "peer-b"] {
            let chats = cached(&mut catalogs, id, || {
                calls.set(calls.get()+1);
                Ok(vec![json!({"id":id})])
            }).unwrap();
            assert_eq!(chats[0]["id"], id);
        }
        assert_eq!(calls.get(), 3);
        let shared = catalogs.clone();
        assert!(std::sync::Arc::ptr_eq(catalogs["master"].as_ref().unwrap(), shared["master"].as_ref().unwrap()));
        for _ in 0..2 {
            assert!(cached(&mut catalogs,"offline",|| {
                calls.set(calls.get()+1);
                Err(io::Error::other("offline"))
            }).is_err());
        }
        assert_eq!(calls.get(), 4);
        catalogs.clear();
        assert!(cached(&mut catalogs,"offline",||Ok(vec![])).unwrap().is_empty());
        assert_eq!(cached(&mut catalogs,"master",||Ok(vec![json!({"id":"updated"})])).unwrap()[0]["id"],"updated");
    }

    #[test]
    fn rename_rejects_invalid_identity_and_name_before_starting_codex() {
        let home = Path::new("/oaclis-missing-home");
        let id = "01a12618-f84f-76f3-a2f0-0db43170e78e";
        for name in ["", " ", " title", "title\nline"] {
            assert!(rename(home,home,id,name).is_err());
        }
        assert!(rename(home,home,id,&"a".repeat(161)).is_err());
        assert!(rename(home,home,"../archive","Title").is_err());
    }
    #[test]
    fn name_client_reuses_connection_ignores_notifications_and_reports_errors() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("oaclis-name-client-{}",std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let program = root.join("codex");
        fs::write(&program, r#"#!/bin/sh
n=0
while IFS= read -r line; do
 case "$line" in
  *'"method":"initialize"'*) printf '{"id":0,"result":{}}\n';;
  *'"method":"thread/name/set"'*) n=$((n+1)); printf '{"method":"thread/name/updated","params":{}}\n{"id":%s,"result":{}}\n' "$n";;
  *'"method":"fail"'*) n=$((n+1)); printf '{"id":%s,"error":{"message":"name rejected"}}\n' "$n";;
 esac
done
"#).unwrap();
        fs::set_permissions(&program,fs::Permissions::from_mode(0o700)).unwrap();
        let mut client = NameClient::start(&root,&program).unwrap();
        let pid = client.child.id();
        for name in ["First", "Second"] {
            assert_eq!(client.request("thread/name/set",json!({"threadId":"id","name":name})).unwrap(),json!({}));
            assert_eq!(client.child.id(),pid);
        }
        assert!(client.request("fail",json!({})).unwrap_err().to_string().contains("name rejected"));
        drop(client);
        fs::remove_dir_all(root).unwrap();
    }

}
