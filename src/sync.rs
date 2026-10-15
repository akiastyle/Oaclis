// sync.rs v1.7.302
use crate::{
    chat::Chat,
    check_unchanged,
    compare::{self, ChatComparison},
    devices::Device,
    invalid, open_regular,
    paths::validate_absolute,
    progress::Event,
    staging::{self, StagedFile},
    transfer::{self, Receipt},
};
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    File,
    Chat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Send,
    Receive,
    Both,
}

impl Direction {
    fn permits(self, outcome: &Outcome) -> bool {
        !matches!(
            (self, outcome),
            (Self::Send, Outcome::Received) | (Self::Receive, Outcome::Sent)
        )
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Identical,
    Sent,
    Received,
    Conflict,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    device_id: String,
    ssh_alias: String,
    local: PathBuf,
    remote: PathBuf,
    kind: Kind,
    common: Receipt,
}

fn file_direction(local: &Receipt, remote: Option<&Receipt>, common: Option<&Receipt>, direction: Direction) -> Outcome {
    if remote != Some(local) {
        match direction {
            Direction::Send=>return Outcome::Sent,
            Direction::Receive if remote.is_some()=>return Outcome::Received,
            _=>{},
        }
    }
    match remote {
        Some(remote) if local == remote => Outcome::Identical,
        None if common.is_none() => Outcome::Sent,
        Some(remote) if common == Some(remote) => Outcome::Sent,
        Some(_) if common == Some(local) => Outcome::Received,
        _ => Outcome::Conflict,
    }
}

pub fn synchronize(
    device: &Device,
    local: &Path,
    remote: &Path,
    checkpoint: &Path,
    kind: Kind,
    notify: impl FnMut(Event),
) -> io::Result<Outcome> {
    synchronize_with_direction(
        device,
        local,
        remote,
        checkpoint,
        kind,
        Direction::Both,
        notify,
    )
}

pub fn synchronize_with_direction(
    device: &Device,
    local: &Path,
    remote: &Path,
    checkpoint: &Path,
    kind: Kind,
    direction: Direction,
    mut notify: impl FnMut(Event),
) -> io::Result<Outcome> {
    device.validate()?;
    for path in [local, remote, checkpoint] {
        validate_absolute(path)?;
    }
    if local == checkpoint {
        return Err(invalid("Checkpoint e origine devono essere distinti"));
    }
    let previous: Option<Checkpoint> = match open_regular(checkpoint) {
        Ok((file, _)) => Some(serde_json::from_reader(file)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    if let Some(previous) = &previous
        && (previous.device_id != device.id
            || previous.ssh_alias != device.ssh_alias
            || previous.local != local
            || previous.remote != remote
            || previous.kind != kind)
    {
        return Err(invalid(
            "Il checkpoint appartiene a un'altra coppia di copie",
        ));
    }
    let checkpoint_probe = StagedFile::new(
        checkpoint
            .parent()
            .ok_or_else(|| invalid("Cartella checkpoint mancante"))?,
    )?;
    drop(checkpoint_probe);
    notify(Event::Preparing);
    let parent = local.parent().ok_or_else(|| invalid("Cartella mancante"))?;
    let mut snapshot = StagedFile::new(parent)?;
    let local_receipt = match open_regular(local) {
        Ok((mut source, metadata)) => {
            let receipt = transfer::copy_hash(&mut source, &mut snapshot.file)?;
            check_unchanged(&source, &metadata)?;
            Some(receipt)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let downloaded = StagedFile::new(parent)?;
    let peer = transfer::inspect_remote(device, remote)?;
    if kind == Kind::Chat && peer.is_some() {
        let fetched = transfer::fetch_file(device, remote, &downloaded.path, &mut notify)?;
        if fetched != peer {
            return Err(invalid("La copia remota è cambiata durante il confronto"));
        }
    }
    let peer_receipt = peer.as_ref().map(|(receipt, _)| receipt);
    let outcome = match local_receipt.as_ref() {
        None if peer.is_some() && previous.is_none() => {
            if kind == Kind::Chat {
                Chat::read(&downloaded.path)?;
            }
            Outcome::Received
        }
        None => Outcome::Conflict,
        Some(receipt) if kind == Kind::File => {
            file_direction(receipt, peer_receipt, previous.as_ref().map(|p| &p.common), direction)
        }
        Some(_) => {
            let local_chat = Chat::read(&snapshot.path)?;
            let remote_chat = if peer.is_some() {
                Some(Chat::read(&downloaded.path)?)
            } else {
                None
            };
            match compare::chats(&local_chat, remote_chat.as_ref()) {
                ChatComparison::Identical => Outcome::Identical,
                ChatComparison::SourceAhead => Outcome::Sent,
                ChatComparison::DestinationMissing if previous.is_none() => Outcome::Sent,
                ChatComparison::DestinationAhead => Outcome::Received,
                _ => Outcome::Conflict,
            }
        }
    };
    if !direction.permits(&outcome) {
        return Ok(Outcome::Conflict);
    }
    let common = match outcome {
        Outcome::Conflict => return Ok(outcome),
        Outcome::Sent => transfer::write_file(
            device,
            local,
            remote,
            peer_receipt,
            local_receipt.as_ref(),
            &mut |event| {
                if !matches!(event, Event::Completed) {
                    notify(event);
                }
            },
        )?,
        Outcome::Received => {
            if kind == Kind::File {
                let fetched = transfer::fetch_file(device, remote, &downloaded.path, &mut notify)?;
                if fetched != peer {
                    return Err(invalid("La copia remota è cambiata durante il confronto"));
                }
            }
            notify(Event::Verifying);
            let (receipt, mode) = peer.as_ref().unwrap();
            fs::set_permissions(&downloaded.path, fs::Permissions::from_mode(*mode))?;
            let command = transfer::publish_command(
                local.to_str().ok_or_else(|| invalid("Origine non UTF-8"))?,
                downloaded
                    .path
                    .to_str()
                    .ok_or_else(|| invalid("Snapshot non UTF-8"))?,
                local_receipt.as_ref(),
                receipt,
            )?;
            let result = Command::new("sh").args(["-c", &command]).output();
            let _ = fs::remove_file(format!("{}.previous", downloaded.path.display()));
            let result = result?;
            if !result.status.success() {
                return Err(invalid(format!(
                    "Pubblicazione locale: {}",
                    String::from_utf8_lossy(&result.stderr)
                )));
            }
            receipt.clone()
        }
        Outcome::Identical => local_receipt.unwrap(),
    };
    staging::save_json(
        checkpoint,
        &Checkpoint {
            device_id: device.id.clone(),
            ssh_alias: device.ssh_alias.clone(),
            local: local.into(),
            remote: remote.into(),
            kind,
            common,
        },
    )?;
    notify(Event::Completed);
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn receipt(value: &str) -> Receipt {
        Receipt {
            bytes: value.len() as u64,
            sha256: value.into(),
        }
    }
    #[test]
    fn directions_refuse_the_opposite_transfer() {
        assert!(!Direction::Send.permits(&Outcome::Received));
        assert!(!Direction::Receive.permits(&Outcome::Sent));
        assert!(Direction::Send.permits(&Outcome::Sent));
        assert!(Direction::Receive.permits(&Outcome::Received));
        assert!(Direction::Both.permits(&Outcome::Sent));
        assert!(Direction::Both.permits(&Outcome::Received));
    }

    #[test]
    fn decides_from_common_content_without_timestamps() {
        let chosen=receipt("winner");let older=receipt("older");
        assert_eq!(file_direction(&chosen,Some(&older),None,Direction::Send),Outcome::Sent);
        assert_eq!(file_direction(&older,Some(&chosen),None,Direction::Receive),Outcome::Received);
        let base = receipt("base");
        let a = receipt("aaaa");
        let b = receipt("bbbb");
        for (local, remote, common, expected) in [
            (&base, Some(&base), None, Outcome::Identical),
            (&a, None, None, Outcome::Sent),
            (&a, Some(&b), None, Outcome::Conflict),
            (&a, Some(&base), Some(&base), Outcome::Sent),
            (&base, Some(&b), Some(&base), Outcome::Received),
            (&a, Some(&b), Some(&base), Outcome::Conflict),
            (&base, None, Some(&base), Outcome::Conflict),
            (&a, Some(&a), Some(&base), Outcome::Identical),
        ] {
            assert_eq!(file_direction(local, remote, common, Direction::Both), expected);
        }
    }
}
