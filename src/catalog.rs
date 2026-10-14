// catalog.rs v2.23.1181
//! Read local and SSH catalogs without depending on the HTTP adapter.
use crate::devices::Device;
use serde_json::Value;
use std::{io,path::Path};

const CATALOG: &str = include_str!("../tools/chat_catalog.py");
pub(crate) fn local(root: &Path) -> io::Result<Vec<Value>> {
    let output = std::process::Command::new("python3")
        .args(["-c", CATALOG])
        .arg(root)
        .output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("Impossibile leggere il catalogo Codex: {}", detail.lines().last().unwrap_or("errore catalogo"))));
    }
    let value: Value = serde_json::from_slice(&output.stdout).map_err(io::Error::other)?;
    serde_json::from_value(value["chats"].clone()).map_err(io::Error::other)
}
pub(crate) fn remote(device: &Device) -> io::Result<Value> {
    let command = format!("python3 -c {}", crate::ssh::quote(CATALOG));
    serde_json::from_slice(&crate::ssh::run(device, &command)?).map_err(io::Error::other)
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
        let entries = local(&root).unwrap();
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
        let unindexed = local(&root).unwrap();
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
        let titled = local(&root).unwrap();
        assert_eq!(titled.len(), 1);
        assert_eq!(titled[0]["name"], "Titolo Codex");
        assert!(std::process::Command::new("python3").args(["-c","import sqlite3,sys; db=sqlite3.connect(sys.argv[1]); db.execute(\"UPDATE threads SET name='Titolo aggiornato' WHERE source='cli'\"); db.commit()"]).arg(root.join("state_5.sqlite")).status().unwrap().success());
        assert_eq!(local(&root).unwrap()[0]["name"], "Titolo aggiornato");
        fs::remove_dir_all(root).unwrap();
    }
}
