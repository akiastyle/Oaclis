use crate::{
    check_unchanged,
    devices::Device,
    invalid, open_regular,
    paths::validate_absolute,
    progress::{self, Event},
    ssh::{self, quote},
    staging::StagedFile,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Receipt {
    pub bytes: u64,
    pub sha256: String,
}

pub(crate) fn copy_hash(reader: &mut impl Read, writer: &mut impl Write) -> io::Result<Receipt> {
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    let mut bytes = 0;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        writer.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
        bytes += count as u64;
    }
    Ok(Receipt {
        bytes,
        sha256: digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    })
}

struct RemoteStage<'a> {
    device: &'a Device,
    path: String,
}
impl Drop for RemoteStage<'_> {
    fn drop(&mut self) {
        // shortcut: offline durante la pulizia lascia una cartella privata, aggiungere recupero con le regole persistenti.
        let _ = ssh::run(self.device, &format!("rm -rf -- {}", quote(&self.path)));
    }
}

pub fn send_file(
    device: &Device,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> io::Result<Receipt> {
    send_file_with_progress(device, source, destination, |_| {})
}

pub fn send_file_with_progress(
    device: &Device,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
    mut notify: impl FnMut(Event),
) -> io::Result<Receipt> {
    write_file(
        device,
        source.as_ref(),
        destination.as_ref(),
        None,
        None,
        &mut notify,
    )
}

pub(crate) fn write_file(
    device: &Device,
    source: &Path,
    destination: &Path,
    previous: Option<&Receipt>,
    required_source: Option<&Receipt>,
    notify: &mut impl FnMut(Event),
) -> io::Result<Receipt> {
    notify(Event::Preparing);
    device.validate()?;
    validate_absolute(destination)?;
    if destination.file_name().is_none() {
        return Err(invalid("Destinazione file mancante"));
    }
    let target = destination
        .to_str()
        .ok_or_else(|| invalid("Percorso remoto non UTF-8"))?;
    if target.ends_with('/') {
        return Err(invalid("Serve una destinazione file"));
    }
    let parent = destination
        .parent()
        .unwrap()
        .to_str()
        .ok_or_else(|| invalid("Cartella non UTF-8"))?;
    let (mut source, metadata) = open_regular(source)?;
    let mut snapshot = StagedFile::new(&std::env::temp_dir())?;
    let expected = copy_hash(&mut source, &mut snapshot.file)?;
    check_unchanged(&source, &metadata)?;
    if required_source.is_some_and(|receipt| receipt != &expected) {
        return Err(invalid("Origine modificata dopo il confronto"));
    }
    snapshot
        .file
        .set_times(std::fs::FileTimes::new().set_modified(metadata.modified()?))?;
    let prefix = format!("{}/.oaclis-", parent.trim_end_matches('/'));
    let existence = if let Some(receipt) = previous {
        verify_command(target, receipt)?
    } else {
        format!("[ ! -e {} ] && [ ! -L {} ]", quote(target), quote(target))
    };
    let output = ssh::run(
        device,
        &format!(
            "p={}; while [ \"$p\" != / ]; do [ -d \"$p\" ] && [ ! -L \"$p\" ] || exit 2; p=$(dirname -- \"$p\"); done; {} || exit 3; mktemp -d -- {}",
            quote(parent),
            existence,
            quote(&format!("{prefix}XXXXXXXXXX"))
        ),
    )?;
    let stage = String::from_utf8(output)
        .map_err(io::Error::other)?
        .trim_end_matches('\n')
        .to_owned();
    let suffix = stage
        .strip_prefix(&prefix)
        .ok_or_else(|| invalid("Cartella temporanea remota inattesa"))?;
    if suffix.len() != 10 || !suffix.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(invalid("Cartella temporanea remota non valida"));
    }
    let stage = RemoteStage {
        device,
        path: stage,
    };
    let remote_file = format!("{}/file", stage.path);
    if let Some(receipt) = previous {
        ssh::run(
            device,
            &format!(
                "cp --reflink=auto -- {} {} && {} && cp --reflink=auto -- {} {}",
                quote(target),
                quote(&format!("{}/previous", stage.path)),
                verify_command(&format!("{}/previous", stage.path), receipt)?,
                quote(&format!("{}/previous", stage.path)),
                quote(&remote_file)
            ),
        )?;
    }
    let mut command = Command::new("rsync");
    command
        .args([
            "--checksum",
            "--info=progress2,name0",
            "--outbuf=N",
            "--secluded-args",
            "--perms",
            "--times",
            "--fsync",
            "--timeout=30",
            "--rsh",
            ssh::OPTIONS,
        ])
        .arg(format!(
            "--chmod=F{:o}",
            metadata.permissions().mode() & 0o777
        ))
        .arg("--")
        .arg(&snapshot.path)
        .arg(format!("{}:{}", device.ssh_alias, remote_file));
    progress::run(&mut command, notify)?;
    notify(Event::Verifying);
    ssh::run(
        device,
        &publish_command(target, &remote_file, previous, &expected)?,
    )?;
    notify(Event::Completed);
    Ok(expected)
}

fn verify_command(path: &str, receipt: &Receipt) -> io::Result<String> {
    if receipt.sha256.len() != 64 || !receipt.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Impronta non valida"));
    }
    Ok(format!(
        "[ -f {p} ] && [ ! -L {p} ] && h=$(sha256sum < {p}) && n=$(wc -c < {p}) && [ \"${{h%% *}}\" = {hash} ] && [ \"$n\" -eq {bytes} ]",
        p = quote(path),
        hash = quote(&receipt.sha256),
        bytes = receipt.bytes
    ))
}

pub(crate) fn publish_command(
    target: &str,
    staged: &str,
    previous: Option<&Receipt>,
    incoming: &Receipt,
) -> io::Result<String> {
    let parent = Path::new(target)
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| invalid("Cartella mancante"))?;
    let incoming_check = verify_command(staged, incoming)?;
    let Some(previous) = previous else {
        return Ok(format!(
            "{incoming_check} && ln -T -- {} {} && sync -f {}",
            quote(staged),
            quote(target),
            quote(parent)
        ));
    };
    let key: String = Sha256::digest(target.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let backups = format!("{}/.oaclis-backups", parent.trim_end_matches('/'));
    let directory = format!("{backups}/{key}");
    let backup = format!("{directory}/{}", previous.sha256);
    let temporary = format!("{staged}.previous");
    let lock = format!("{directory}/lock");
    let script = format!(
        "{} && {incoming_check} && cp --reflink=auto -- {} {} && {} && {{ ln -T -- {} {} || {}; }} && mv -T -- {} {} && sync -f {}",
        verify_command(target, previous)?,
        quote(target),
        quote(&temporary),
        verify_command(&temporary, previous)?,
        quote(&temporary),
        quote(&backup),
        verify_command(&backup, previous)?,
        quote(staged),
        quote(target),
        quote(parent)
    );
    Ok(format!(
        "[ ! -L {b} ] && mkdir -m 700 -p -- {b} && [ ! -L {d} ] && mkdir -m 700 -p -- {d} && chmod 700 -- {b} {d} && [ ! -L {l} ] && flock -n {l} sh -c {script}",
        b = quote(&backups),
        d = quote(&directory),
        l = quote(&lock),
        script = quote(&script)
    ))
}

pub(crate) fn inspect_remote(device: &Device, target: &Path) -> io::Result<Option<(Receipt, u32)>> {
    validate_absolute(target)?;
    let target = target
        .to_str()
        .ok_or_else(|| invalid("Percorso remoto non UTF-8"))?;
    let parent = Path::new(target)
        .parent()
        .and_then(Path::to_str)
        .ok_or_else(|| invalid("Cartella mancante"))?;
    let output = ssh::run(
        device,
        &format!(
            "p={}; while [ \"$p\" != / ]; do [ -d \"$p\" ] && [ ! -L \"$p\" ] || exit 2; p=$(dirname -- \"$p\"); done; if [ ! -e {t} ] && [ ! -L {t} ]; then echo missing; else [ -f {t} ] && [ ! -L {t} ] && sha256sum < {t} && wc -c < {t} && stat -c %a -- {t}; fi",
            quote(parent),
            t = quote(target)
        ),
    )?;
    let output = String::from_utf8(output).map_err(io::Error::other)?;
    if output == "missing\n" {
        return Ok(None);
    }
    let fields: Vec<_> = output.split_whitespace().collect();
    if fields.len() != 4 {
        return Err(invalid("Metadati remoti inattesi"));
    }
    let receipt = Receipt {
        sha256: fields[0].into(),
        bytes: fields[2].parse().map_err(io::Error::other)?,
    };
    verify_command(target, &receipt)?;
    let mode = u32::from_str_radix(fields[3], 8).map_err(io::Error::other)? & 0o777;
    Ok(Some((receipt, mode)))
}

pub(crate) fn fetch_file(
    device: &Device,
    target: &Path,
    local: &Path,
    notify: &mut impl FnMut(Event),
) -> io::Result<Option<(Receipt, u32)>> {
    let Some((receipt, mode)) = inspect_remote(device, target)? else {
        return Ok(None);
    };
    let target = target
        .to_str()
        .ok_or_else(|| invalid("Percorso remoto non UTF-8"))?;
    let mut command = Command::new("rsync");
    command
        .args([
            "--checksum",
            "--secluded-args",
            "--times",
            "--perms",
            "--chmod=F600",
            "--fsync",
            "--timeout=30",
            "--info=progress2,name0",
            "--outbuf=N",
            "--rsh",
            ssh::OPTIONS,
        ])
        .arg("--")
        .arg(format!("{}:{}", device.ssh_alias, target))
        .arg(local);
    progress::run(&mut command, notify)?;
    let (mut file, metadata) = open_regular(local)?;
    let actual = copy_hash(&mut file, &mut io::sink())?;
    check_unchanged(&file, &metadata)?;
    if actual != receipt {
        return Err(invalid("Destinazione modificata durante il confronto"));
    }
    Ok(Some((receipt, mode)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn publication_checks_previous_content_and_keeps_an_independent_backup() {
        let directory = std::env::temp_dir().join(format!(
            "oaclis-publish-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let target = directory.join("target ' $(literal)");
        let staged = directory.join("incoming");
        let old = copy_hash(&mut &b"old"[..], &mut io::sink()).unwrap();
        let incoming = copy_hash(&mut &b"new"[..], &mut io::sink()).unwrap();
        fs::write(&target, b"changed after comparison").unwrap();
        fs::write(&staged, b"new").unwrap();
        let command = publish_command(
            target.to_str().unwrap(),
            staged.to_str().unwrap(),
            Some(&old),
            &incoming,
        )
        .unwrap();
        let rejected = Command::new("sh").args(["-c", &command]).output().unwrap();
        assert!(!rejected.status.success());
        assert_eq!(fs::read(&target).unwrap(), b"changed after comparison");
        fs::write(&target, b"old").unwrap();
        let result = Command::new("sh").args(["-c", &command]).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(fs::read(&target).unwrap(), b"new");
        let key: String = Sha256::digest(target.to_str().unwrap().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let backup = directory.join(".oaclis-backups").join(key).join(old.sha256);
        fs::write(&target, b"later write").unwrap();
        assert_eq!(fs::read(backup).unwrap(), b"old");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn publication_rejects_invalid_receipts_and_corrupt_incoming_data() {
        let directory = std::env::temp_dir().join(format!(
            "oaclis-corrupt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let staged = directory.join("staged");
        let target = directory.join("target");
        fs::write(&staged, b"wrong").unwrap();
        let expected = copy_hash(&mut &b"right"[..], &mut io::sink()).unwrap();
        let command = publish_command(
            target.to_str().unwrap(),
            staged.to_str().unwrap(),
            None,
            &expected,
        )
        .unwrap();
        assert!(
            !Command::new("sh")
                .args(["-c", &command])
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(!target.exists());
        assert!(
            publish_command(
                "/tmp/target",
                "/tmp/staged",
                None,
                &Receipt {
                    bytes: 0,
                    sha256: "invalid;command".into()
                }
            )
            .is_err()
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
