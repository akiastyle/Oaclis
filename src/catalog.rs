// catalog.rs v2.43.1899
//! Read Codex archives; title edits use native Codex, Oaclis metadata stays in memory.
use crate::{devices::Device, state_store};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    io::{self, BufRead, BufReader},
    path::{Path, PathBuf},
    os::unix::fs::MetadataExt,
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

type Stamp = (u64, i64, i64, i64, i64, u64);
fn stamp(metadata: &fs::Metadata) -> Stamp {
    (metadata.ino(), metadata.mtime(), metadata.mtime_nsec(), metadata.ctime(), metadata.ctime_nsec(), metadata.len())
}
#[derive(Default)]
struct ArchiveIndex {
    touched: Option<std::time::Instant>,
    titles: Vec<Option<Stamp>>,
    names: BTreeMap<String, String>,
    hidden: BTreeSet<String>,
    directories: HashMap<PathBuf, (Stamp, Vec<(PathBuf, bool)>)>,
}
static INDEXES: std::sync::OnceLock<std::sync::Mutex<HashMap<PathBuf,std::sync::Arc<std::sync::Mutex<ArchiveIndex>>>>> = std::sync::OnceLock::new();
pub(crate) fn cached_name(chat: &str) -> Option<String> {
    let indexes=INDEXES.get()?.lock().ok()?;
    indexes.values().find_map(|index|index.try_lock().ok()?.names.get(chat).cloned())
}
pub(crate) fn expire() {
    if let Some(indexes)=INDEXES.get() {
        indexes.lock().unwrap().retain(|_,index|std::sync::Arc::strong_count(index)>1 || index.try_lock().map(|index|index.touched.is_some_and(|time|time.elapsed().as_secs()<3600)).unwrap_or(true));
    }
}
pub(crate) fn number_chats(config: &Path, chats: &[Value]) -> io::Result<()> {
    let snapshot=state_store::snapshot(config)?;
    let mut entries: BTreeMap<String,String>=chats.iter().chain(snapshot.links.values().filter_map(|link|link.get("entry")))
        .filter_map(|chat|chat["id"].as_str().map(|id|(id.to_owned(),chat["name"].as_str().unwrap_or("").to_owned()))).collect();
    entries.retain(|id,_|snapshot.shared["deletions"].get(id).is_none() && snapshot.appearance.get(&format!("chat:{id}")).is_none_or(|record|record.sync["chat_number"].as_u64().is_none()));
    if entries.is_empty(){return Ok(());}
    let mut entries:Vec<_>=entries.into_iter().collect();
    entries.sort_by_key(|(id,name)|(snapshot.appearance.get(&format!("chat:{id}")).and_then(|r|r.value["label"].as_str()).filter(|s|!s.is_empty()).unwrap_or(name).to_lowercase(),id.clone()));
    state_store::change(config,|state|{
        let mut next=state.appearance.values().filter_map(|record|record.sync["chat_number"].as_u64().or(record.sync["next_chat_number"].as_u64())).max().unwrap_or(0);
        for (id,_) in entries {
            let record=state_store::chat_record(state,&id);
            if record.sync["chat_number"].as_u64().is_none(){next+=1;record.sync["chat_number"]=json!(next);}
        }
        let counter=std::sync::Arc::make_mut(&mut state.appearance).entry("device:master".into()).or_default();
        std::sync::Arc::make_mut(counter).sync["next_chat_number"]=json!(next);
        Ok(())
    })
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
    let index = INDEXES.get_or_init(Default::default).lock().map_err(|_|io::Error::other("Catalogo non disponibile"))?.entry(root.clone()).or_default().clone();
    let mut index = index.lock().map_err(|_|io::Error::other("Catalogo non disponibile"))?;
    index.touched=Some(std::time::Instant::now());
    let mut names = BTreeMap::new();
    let mut hidden = BTreeSet::new();
    let database = if root.join("state_5.sqlite").exists() {
        root.join("state_5.sqlite")
    } else {
        root.join("sqlite/state_5.sqlite")
    };
    let titles = [database.clone(), database.with_file_name("state_5.sqlite-wal"), root.join("session_index.jsonl")]
        .iter().map(|path|fs::metadata(path).ok().map(|m|stamp(&m))).collect::<Vec<_>>();
    if index.titles != titles {
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
        index.names = names;
        index.hidden = hidden;
        index.titles = titles;
    }
    let mut chats = Vec::new();
    let mut visited = BTreeSet::new();
    let ArchiveIndex { names, hidden, directories, .. } = &mut *index;
    walk(&root, &root.join("sessions"), names, hidden, directories, &mut visited, &mut chats)?;
    directories.retain(|path, _|visited.contains(path));
    drop(index);
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
        chat["number"] = state.appearance.get(&format!("chat:{chat_id}")).map(|r|r.sync["chat_number"].clone()).unwrap_or(Value::Null);
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
    directories: &mut HashMap<PathBuf, (Stamp, Vec<(PathBuf, bool)>)>,
    visited: &mut BTreeSet<PathBuf>,
    chats: &mut Vec<Value>,
) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.is_dir() => metadata,
        Ok(_) => return Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    visited.insert(directory.to_path_buf());
    let signature = stamp(&metadata);
    if !directories.get(directory).is_some_and(|(old,_)|*old == signature) {
        let mut entries = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_dir() || kind.is_file() { entries.push((entry.path(), kind.is_dir())); }
        }
        directories.insert(directory.to_path_buf(), (signature, entries));
    }
    let entries = directories[directory].1.clone();
    for (path, directory_entry) in entries {
        if directory_entry {
            walk(root, &path, names, hidden, directories, visited, chats)?;
            continue;
        }
        let filename = path.file_name().unwrap();
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
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => continue,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e),
        };
        chats.push(json!({"id":id,"name":names.get(id).map(String::as_str).unwrap_or("Senza titolo"),"path":path,"relative_path":path.strip_prefix(root).map_err(io::Error::other)?,"bytes":metadata.len(),"modified_ns":metadata.modified()?.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos().to_string()}));
    }
    Ok(())
}
// One native Codex connection for chat creation and title edits, outside state locks.
struct NativeClient {
    child: std::process::Child,
    input: std::process::ChildStdin,
    replies: std::sync::mpsc::Receiver<Value>,
    next: u64,
}
impl Drop for NativeClient {
    fn drop(&mut self) {
        let _ = std::process::Command::new("kill").args(["-KILL", "--", &format!("-{}", self.child.id())]).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl NativeClient {
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
        client.request("initialize", json!({"clientInfo":{"name":"oaclis","version":env!("OACLIS_APP_VERSION")},"capabilities":{"experimentalApi":true}}))?;
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
                .map_err(|_|io::Error::new(io::ErrorKind::TimedOut,"Codex non risponde"))?;
            if value["id"] != id { continue; }
            if let Some(error) = value.get("error") { return Err(io::Error::other(error["message"].as_str().unwrap_or("Operazione Codex non riuscita"))); }
            return value.get("result").cloned().ok_or_else(||crate::invalid("Risposta Codex non valida"));
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
    let params=json!({"threadId":id,"name":name});
    match crate::assistant_stream::connect(root) {
        Ok(mut socket)=>crate::assistant_stream::rpc(&mut socket,1,"thread/name/set",params).map(|_|()),
        Err(_)=>native(home,root,false,|client|client.request("thread/name/set",params).map(|_|())),
    }.map_err(|_|io::Error::other("Impossibile rinominare la chat in Codex. Riprova."))
}
fn native<T>(home:&Path,root:&Path,release:bool,work:impl FnOnce(&mut NativeClient)->io::Result<T>)->io::Result<T> {
    static CLIENT:std::sync::OnceLock<std::sync::Mutex<Option<(std::path::PathBuf,NativeClient)>>>=std::sync::OnceLock::new();
    let mut saved=CLIENT.get_or_init(Default::default).lock().map_err(|_|io::Error::other("Codex non disponibile"))?;
    if saved.as_ref().is_some_and(|(path,_)|path!=root){*saved=None;}
    let result=(||{
        if saved.is_none(){
            let native=root.join("packages/app-server-daemon/current/bin/codex");
            let executable=if native.is_file(){native}else{crate::terminal::codex(home)?};
            *saved=Some((root.into(),NativeClient::start(root,&executable)?));
        }
        work(&mut saved.as_mut().unwrap().1)
    })();
    if let Err(error)=&result{*saved=None;crate::state_transport::log(format!("Codex: {error}"));}
    // The creator must release thread ownership before the terminal resumes it.
    if release{*saved=None;}
    result
}
pub(crate) fn skills(home:&Path,root:&Path)->io::Result<Value> {
    let executable=root.join("packages/app-server-daemon/current/bin/codex");
    let executable=if executable.is_file(){executable}else{crate::terminal::codex(home)?};
    let mut client=NativeClient::start(root,&executable)?;
    client.request("skills/list",json!({"cwds":[home],"forceReload":true}))
}
pub(crate) fn create(home:&Path,root:&Path,cwd:&Path,name:&str,model:&str)->io::Result<Value> {
    if name.trim().is_empty()||name!=name.trim()||!cwd.is_absolute(){return Err(crate::invalid("Nome o cartella chat non validi"));}
    crate::appearance::Appearance{label:name.into(),..Default::default()}.validate()?;
    crate::terminal::validate_model(model)?;
    native(home,root,true,|client|{
        let config=client.request("config/read",json!({"includeLayers":false}))?;
        let available=client.request("model/list",json!({"includeHidden":false,"limit":100}))?;
        let models=available["data"].as_array().ok_or_else(||crate::invalid("Elenco modelli non disponibile"))?;
        let selected=if model.is_empty(){config["config"]["model"].as_str().or_else(||models.iter().find(|m|m["isDefault"]==true).and_then(|m|m["model"].as_str())).ok_or_else(||crate::invalid("Scegli il modello per la nuova chat"))?}else{model};
        if !models.iter().any(|m|m["model"]==selected){return Err(crate::invalid("Modello non disponibile in Codex"));}
        fs::create_dir_all(cwd)?;
        let result=client.request("thread/start",json!({"cwd":cwd,"model":selected,"ephemeral":false,"historyMode":"legacy","persistExtendedHistory":true}))?;
        let thread=&result["thread"];let id=thread["id"].as_str().filter(|id|id.len()==36&&crate::devices::identifier(id)).ok_or_else(||crate::invalid("Identificativo nuova chat non valido"))?;
        client.request("thread/name/set",json!({"threadId":id,"name":name}))?;
        Ok(json!({"id":id,"name":name,"path":thread["path"],"cwd":cwd,"model":selected}))
    })
}

pub(crate) fn remote(device: &Device) -> io::Result<Value> {
    crate::state_transport::remote(device, &json!({"kind":"catalog"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    #[test]
    fn creation_makes_the_requested_directory_before_starting_the_chat() {
        use std::os::unix::fs::PermissionsExt;
        let root=std::env::temp_dir().join(format!("oaclis-create-directory-{}",std::process::id()));
        let cwd=root.join("new/project");let binary=root.join("packages/app-server-daemon/current/bin/codex");
        fs::create_dir_all(binary.parent().unwrap()).unwrap();
        let reply=json!({"id":3,"result":{"thread":{"id":"11111111-1111-4111-8111-111111111111","path":root.join("sessions/fixture.jsonl")}}});
        let script=format!(r#"#!/bin/sh
while IFS= read -r line; do
 case "$line" in
  *'"method":"initialize"'*) echo '{{"id":0,"result":{{}}}}' ;;
  *'"method":"config/read"'*) echo '{{"id":1,"result":{{"config":{{"model":"test-model"}}}}}}' ;;
  *'"method":"model/list"'*) echo '{{"id":2,"result":{{"data":[{{"model":"test-model"}}]}}}}' ;;
  *'"method":"thread/start"'*) if [ -d '{}' ]; then echo '{}'; else echo '{{"id":3,"error":{{"message":"Directory missing at thread/start"}}}}'; fi ;;
  *'"method":"thread/name/set"'*) echo '{{"id":4,"result":{{}}}}' ;;
 esac
done
"#,cwd.display(),reply);
        fs::write(&binary,script).unwrap();fs::set_permissions(&binary,fs::Permissions::from_mode(0o700)).unwrap();
        let result=create(&root,&root,&cwd,"New project","").unwrap();
        assert_eq!(result["cwd"],json!(cwd));assert!(cwd.is_dir());
        let file=root.join("existing-file");fs::write(&file,"keep").unwrap();
        assert!(create(&root,&root,&file,"Another project","").is_err());
        assert_eq!(fs::read_to_string(&file).unwrap(),"keep");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn chat_numbers_survive_renaming_filtering_and_deletion() {
        let root=std::env::temp_dir().join(format!("oaclis-chat-numbers-{}",std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let chats=vec![json!({"id":"b","name":"Zeta"}),json!({"id":"a","name":"Alfa"})];
        number_chats(&root,&chats).unwrap();
        assert_eq!(state_store::snapshot(&root).unwrap().appearance["chat:a"].sync["chat_number"],1);
        assert_eq!(state_store::snapshot(&root).unwrap().appearance["chat:b"].sync["chat_number"],2);
        number_chats(&root,&[json!({"id":"b","name":"A nuova posizione"}),json!({"id":"c","name":"AAA"})]).unwrap();
        assert_eq!(state_store::snapshot(&root).unwrap().appearance["chat:b"].sync["chat_number"],2);
        assert_eq!(state_store::snapshot(&root).unwrap().appearance["chat:c"].sync["chat_number"],3);
        state_store::change(&root,|state|{std::sync::Arc::make_mut(&mut state.appearance).remove("chat:c");Ok(())}).unwrap();
        number_chats(&root,&[json!({"id":"d","name":"Nuova"})]).unwrap();
        assert_eq!(state_store::snapshot(&root).unwrap().appearance["chat:d"].sync["chat_number"],4);
        let preferences=state_store::database(&root,"","preferences-get",None).unwrap();
        assert_eq!(preferences["preferences"]["a"]["number"],1);
        assert_eq!(preferences["preferences"]["b"]["number"],2);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn expiry_releases_idle_catalog_but_never_an_in_use_index() {
        use std::sync::{Arc,Mutex};
        let root=std::env::temp_dir().join(format!("oaclis-expiry-{}",std::process::id()));
        let index=Arc::new(Mutex::new(ArchiveIndex{touched:Some(std::time::Instant::now()-std::time::Duration::from_secs(3601)),..Default::default()}));
        let weak=Arc::downgrade(&index);
        INDEXES.get_or_init(Default::default).lock().unwrap().insert(root,index.clone());
        expire();assert!(weak.upgrade().is_some());
        drop(index);expire();assert!(weak.upgrade().is_none());
    }
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
        let path = PathBuf::from(titled[0]["path"].as_str().unwrap());
        fs::write(&path, "changed archive contents").unwrap();
        assert_eq!(local_at(&root, &root.join("config")).unwrap()[0]["bytes"], 24);
        fs::remove_file(path).unwrap();
        assert!(local_at(&root, &root.join("config")).unwrap().is_empty());
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
        let mut client = NativeClient::start(&root,&program).unwrap();
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
