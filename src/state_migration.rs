// state_migration.rs v1.11.288
//! Read the former Oaclis SQLite archive once; never modify or delete its source.
use crate::state_store::{Presentation, State, read_json};
use rusqlite::{Connection, OpenFlags};
use serde_json::json;
use std::{collections::BTreeSet, io, path::Path};

pub(crate) fn load(config: &Path) -> io::Result<State> {
    let mut state = State::default();
    state.shared = read_json(&config.join("shared-settings.json"), state.shared.clone())?;
    let path = config.join("appearance.sqlite");
    match crate::open_regular(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            for (key, value) in read_json(&config.join("appearance.json"), json!({}))?
                .as_object()
                .into_iter()
                .flatten()
            {
                state.appearance.insert(
                    key.clone(),
                    Presentation {
                        value: value.clone(),
                        ..Default::default()
                    },
                );
            }
            return Ok(state);
        }
        Err(error) => return Err(error),
        Ok(_) => {}
    }
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(io::Error::other)?;
    db.execute_batch("BEGIN").map_err(io::Error::other)?;
    let mut query = db
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
        .map_err(io::Error::other)?;
    let tables = query
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(io::Error::other)?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(io::Error::other)?;
    let allowed = [
        "appearance",
        "analysis",
        "chat_links",
        "transfers",
        "chat_visibility",
        "chat_settings",
        "app_checks",
        "preparations",
    ];
    if tables.iter().any(|t| !allowed.contains(&t.as_str())) {
        return Err(crate::invalid(
            "Archivio SQLite con tabelle non riconosciute: conversione interrotta",
        ));
    }
    for (table, allowed) in [
        ("appearance", &["key", "value", "hidden", "sync"][..]),
        ("analysis", &["key", "value"][..]),
        ("chat_links", &["key", "value"][..]),
        ("transfers", &["key", "value"][..]),
    ] {
        if !tables.contains(table) {
            continue;
        }
        let mut query = db
            .prepare(&format!("PRAGMA table_info({table})"))
            .map_err(io::Error::other)?;
        let columns = query
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(io::Error::other)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?;
        if columns.iter().any(|c| !allowed.contains(&c.as_str())) {
            return Err(crate::invalid(
                "Schema SQLite non riconosciuto: nessun dato convertito",
            ));
        }
    }
    if tables.contains("appearance") {
        let mut query = db
            .prepare("PRAGMA table_info(appearance)")
            .map_err(io::Error::other)?;
        let columns = query
            .query_map([], |r| r.get::<_, String>(1))
            .map_err(io::Error::other)?
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(io::Error::other)?;
        let sql = format!(
            "SELECT key,value,{},{} FROM appearance",
            if columns.contains("hidden") {
                "hidden"
            } else {
                "0"
            },
            if columns.contains("sync") {
                "sync"
            } else {
                "'{}'"
            }
        );
        let mut query = db.prepare(&sql).map_err(io::Error::other)?;
        let rows = query
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, bool>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(io::Error::other)?;
        for row in rows {
            let (key, value, hidden, sync) = row.map_err(io::Error::other)?;
            state.appearance.insert(
                key,
                Presentation {
                    value: serde_json::from_str(&value)?,
                    hidden,
                    sync: serde_json::from_str(&sync)?,
                },
            );
        }
    }
    for (table, target) in [
        ("analysis", &mut state.analysis),
        ("chat_links", &mut state.links),
        ("transfers", &mut state.transfers),
    ] {
        if !tables.contains(table) {
            continue;
        }
        let mut query = db
            .prepare(&format!(
                "SELECT key,value FROM {table} WHERE value IS NOT NULL"
            ))
            .map_err(io::Error::other)?;
        for row in query
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(io::Error::other)?
        {
            let (key, value) = row.map_err(io::Error::other)?;
            target.insert(key, serde_json::from_str(&value)?);
        }
    }
    // Legacy schemas are refused rather than silently dropping fields during conversion.
    if tables.iter().any(|t| {
        [
            "chat_visibility",
            "chat_settings",
            "app_checks",
            "preparations",
        ]
        .contains(&t.as_str())
    }) {
        return Err(crate::invalid(
            "Schema SQLite precedente: completare la migrazione con la versione precedente prima della conversione JSON",
        ));
    }
    for record in state.appearance.values_mut() {
        record.sync.as_object_mut().map(|v| v.remove("removed"));
    }
    Ok(state)
}
