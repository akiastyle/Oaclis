// compare.rs v1.5.100
use crate::{chat::Chat, check_unchanged, open_regular};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum ChatComparison {
    DestinationMissing,
    Identical,
    SourceAhead,
    DestinationAhead,
    Conflict,
    DifferentChat,
}

pub fn chats(source: &Chat, destination: Option<&Chat>) -> ChatComparison {
    let Some(destination) = destination else {
        return ChatComparison::DestinationMissing;
    };
    if source.id != destination.id {
        return ChatComparison::DifferentChat;
    }
    if source.records == destination.records {
        ChatComparison::Identical
    } else if source.records.starts_with(&destination.records) {
        ChatComparison::SourceAhead
    } else if destination.records.starts_with(&source.records) {
        ChatComparison::DestinationAhead
    } else {
        ChatComparison::Conflict
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum FileComparison {
    DestinationMissing,
    Identical,
    Different,
}

pub fn files(
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> io::Result<FileComparison> {
    let source = fingerprint(source.as_ref())?;
    match fingerprint(destination.as_ref()) {
        Ok(destination) if source == destination => Ok(FileComparison::Identical),
        Ok(_) => Ok(FileComparison::Different),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok(FileComparison::DestinationMissing)
        }
        Err(error) => Err(error),
    }
}

fn fingerprint(path: &Path) -> io::Result<[u8; 32]> {
    let (mut file, metadata) = open_regular(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    check_unchanged(&file, &metadata)?;
    Ok(digest.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn chat(id: &str, records: &[u8]) -> Chat {
        Chat {
            id: id.into(),
            history_mode: "legacy".into(),
            project_paths: BTreeSet::new(),
            parent_id: None,
            records: records.iter().map(|value| [*value; 32]).collect(),
        }
    }

    #[test]
    fn distinguishes_every_chat_comparison() {
        let original = chat("a", &[1, 2]);
        assert_eq!(chats(&original, None), ChatComparison::DestinationMissing);
        for (other, expected) in [
            (chat("a", &[1, 2]), ChatComparison::Identical),
            (chat("a", &[1]), ChatComparison::SourceAhead),
            (chat("a", &[1, 2, 3]), ChatComparison::DestinationAhead),
            (chat("a", &[1, 9]), ChatComparison::Conflict),
            (chat("b", &[1, 2]), ChatComparison::DifferentChat),
        ] {
            assert_eq!(chats(&original, Some(&other)), expected);
        }
    }
}
