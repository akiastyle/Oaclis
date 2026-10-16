// catalog.rs v2.27.1398
//! Read Codex archives natively; Oaclis metadata comes from its in-memory JSON state.
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
        let script = r#"import sqlite3,sys
with sqlite3.connect(sys.argv[1]) as db:
    db.execute('CREATE TABLE threads(id TEXT,name TEXT,title TEXT,source TEXT)')
    db.execute('INSERT INTO threads VALUES(?,?,?,?)',('01a0568c-cf6c-7d62-bb6d-12c20b4d9b16','Titolo Codex','Primo messaggio','cli'))
    db.execute('INSERT INTO threads VALUES(?,?,?,?)',('01a0568c-cf6c-7d62-bb6d-12c20b4d9b18',None,'','{"subagent":{}}'))"#;
        assert!(
            std::process::Command::new("python3")
                .args(["-c", script])
                .arg(root.join("state_5.sqlite"))
                .status()
                .unwrap()
                .success()
        );
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
        assert!(std::process::Command::new("python3").args(["-c","import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); db.execute(\"UPDATE threads SET name='Titolo aggiornato' WHERE source='cli'\"); db.commit()"]).arg(root.join("state_5.sqlite")).status().unwrap().success());
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

}
