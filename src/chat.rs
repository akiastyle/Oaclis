// chat.rs v1.8.177
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
    pub(crate) activity: Option<String>,
}

/// Canonical UTC event time, never the archive's filesystem modification time.
pub(crate) fn event_time(record: &Value) -> Option<String> {
    let value=record["timestamp"].as_str()?;
    let prefix=value.get(..19)?;
    if prefix.bytes().enumerate().any(|(i,b)| match i {4|7=>b!=b'-',10=>b!=b'T',13|16=>b!=b':',_=>!b.is_ascii_digit()}) {return None;}
    let rest=value.get(19..)?.strip_suffix('Z')?;
    let fraction=if rest.is_empty() {""} else {rest.strip_prefix('.')?};
    if fraction.len()>9 || !fraction.bytes().all(|b|b.is_ascii_digit()) {return None;}
    Some(format!("{prefix}.{fraction:0<9}Z"))
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
            activity: None,
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
            if let Some(time)=event_time(&record) {
                if chat.activity.as_ref().is_none_or(|previous|time>*previous) {chat.activity=Some(time);}
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

pub(crate) fn proof(path: &Path, id: &str, prefix_bytes: u64) -> io::Result<Value> {
    use std::io::Seek;
    use std::time::UNIX_EPOCH;
    let(file,metadata)=open_regular(path)?;
    let mut reader=BufReader::new(file);
    let chat=Chat::parse(&mut reader)?;
    if chat.id!=id{return Err(invalid("Identita' chat diversa da quella richiesta"));}
    reader.rewind()?;
    let mut digest=Sha256::new();let mut prefix=Sha256::new();let mut bytes=0u64;let mut buffer=[0u8;65536];
    loop{let count=reader.read(&mut buffer)?;if count==0{break;}digest.update(&buffer[..count]);let amount=prefix_bytes.saturating_sub(bytes).min(count as u64) as usize;prefix.update(&buffer[..amount]);bytes+=count as u64;}
    check_unchanged(reader.get_ref(),&metadata)?;
    Ok(serde_json::json!({"activity":chat.activity,"sha256":digest.finalize().iter().map(|b|format!("{b:02x}")).collect::<String>(),"prefix":prefix.finalize().iter().map(|b|format!("{b:02x}")).collect::<String>(),"bytes":bytes,"modified_ns":metadata.modified()?.duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos().to_string(),"records":chat.record_count()}))
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
