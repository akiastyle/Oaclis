use crate::{check_unchanged, invalid, open_regular};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Chat {
    pub id: String,
    pub history_mode: String,
    pub project_paths: BTreeSet<PathBuf>,
    pub parent_id: Option<String>,
    pub(crate) records: Vec<[u8; 32]>,
}

impl Chat {
    pub fn read(path: impl AsRef<Path>) -> io::Result<Self> {
        let (file, metadata) = open_regular(path.as_ref())?;
        let mut reader = BufReader::new(file);
        let chat = Self::parse(&mut reader)?;
        check_unchanged(reader.get_ref(), &metadata)?;
        Ok(chat)
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    fn parse(reader: &mut impl BufRead) -> io::Result<Self> {
        let mut chat = Self {
            id: String::new(),
            history_mode: "legacy".into(),
            project_paths: BTreeSet::new(),
            parent_id: None,
            records: Vec::new(),
        };
        let mut line = Vec::new();
        loop {
            line.clear();
            // shortcut: massimo 64 MiB per record, aumentare solo per chat reali che lo richiedono.
            const LIMIT: u64 = 64 * 1024 * 1024;
            let size = reader
                .by_ref()
                .take(LIMIT + 1)
                .read_until(b'\n', &mut line)?;
            if size == 0 {
                break;
            }
            let number = chat.records.len() + 1;
            if size as u64 > LIMIT || line.last() != Some(&b'\n') {
                return Err(invalid(format!(
                    "Record {number} incompleto o troppo grande"
                )));
            }
            let record: Value = serde_json::from_slice(&line)
                .map_err(|_| invalid(format!("JSON non valido al record {number}")))?;
            let kind = record["type"]
                .as_str()
                .ok_or_else(|| invalid(format!("Tipo mancante al record {number}")))?;
            let payload = record["payload"]
                .as_object()
                .ok_or_else(|| invalid(format!("Payload non valido al record {number}")))?;
            if number == 1 {
                if kind != "session_meta" {
                    return Err(invalid("Il primo record deve essere session_meta"));
                }
                chat.id = payload
                    .get("id")
                    .and_then(Value::as_str)
                    .filter(|id| !id.trim().is_empty())
                    .ok_or_else(|| invalid("ID chat mancante"))?
                    .into();
                if let Some(mode) = payload.get("history_mode") {
                    chat.history_mode = mode
                        .as_str()
                        .filter(|mode| matches!(*mode, "legacy" | "paginated"))
                        .ok_or_else(|| invalid("Formato della cronologia non supportato"))?
                        .into();
                }
                chat.parent_id = record
                    .pointer("/payload/source/subagent/thread_spawn/parent_thread_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if !payload.contains_key("cwd") {
                    return Err(invalid("Directory di lavoro mancante"));
                }
            } else if kind == "session_meta" {
                return Err(invalid("session_meta duplicato"));
            }
            if matches!(kind, "session_meta" | "turn_context")
                && let Some(cwd) = payload.get("cwd")
            {
                let cwd = PathBuf::from(
                    cwd.as_str()
                        .ok_or_else(|| invalid("Directory di lavoro non valida"))?,
                );
                crate::paths::validate_absolute(&cwd)?;
                chat.project_paths.insert(cwd);
            }
            chat.records
                .push(Sha256::digest(serde_json::to_vec(&record)?).into());
        }
        if chat.records.is_empty() {
            return Err(invalid("Chat vuota"));
        }
        Ok(chat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn parse(text: &str) -> io::Result<Chat> {
        Chat::parse(&mut Cursor::new(text.as_bytes()))
    }

    #[test]
    fn reads_metadata_and_all_working_directories() {
        let chat = parse(concat!(
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"cwd\":\"/work/a\",\"history_mode\":\"paginated\",\"source\":{\"subagent\":{\"thread_spawn\":{\"parent_thread_id\":\"parent\"}}}}}\n",
            "{\"type\":\"turn_context\",\"payload\":{\"cwd\":\"/work/b\"}}\n"
        )).unwrap();
        assert_eq!(chat.id, "a");
        assert_eq!(chat.history_mode, "paginated");
        assert_eq!(chat.parent_id.as_deref(), Some("parent"));
        assert_eq!(chat.project_paths.len(), 2);
        assert_eq!(chat.record_count(), 2);
    }

    #[test]
    fn rejects_invalid_or_incomplete_chats() {
        for text in [
            "",
            "{}\n",
            "{broken}\n",
            "{\"type\":\"session_meta\",\"payload\":{\"cwd\":\"/work\"}}\n",
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"cwd\":\"../work\"}}\n",
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"cwd\":\"/work\",\"history_mode\":\"unknown\"}}\n",
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"cwd\":\"/work\"}}",
        ] {
            assert!(parse(text).is_err(), "Accettato: {text}");
        }
    }
}
