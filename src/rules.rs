use crate::{
    devices::{self, Device},
    invalid, open_regular,
    paths::{ProjectPaths, validate_absolute},
    progress::Event,
    ssh::{self, quote},
    staging,
    sync::{self, Direction, Kind, Outcome},
    transfer,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashSet},
    fs, io,
    os::unix::fs::PermissionsExt,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Copy,
    Sync,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub device_id: String,
    pub local_root: PathBuf,
    pub remote_root: PathBuf,
    pub operation: Operation,
    pub direction: Direction,
    pub include: Vec<PathBuf>,
    pub exclude: Vec<PathBuf>,
}

fn relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && !path.as_os_str().as_encoded_bytes().contains(&0)
}
impl Rule {
    pub fn validate(&self) -> io::Result<()> {
        if !devices::identifier(&self.id)
            || !devices::identifier(&self.device_id)
            || self.name.trim().is_empty()
        {
            return Err(invalid("ID, nome o dispositivo della regola non valido"));
        }
        for root in [&self.local_root, &self.remote_root] {
            let parts: Vec<_> = root.components().map(|p| p.as_os_str()).collect();
            if parts.iter().any(|p| *p == ".ssh")
                || parts.iter().position(|p| *p == ".codex").is_some_and(|i| {
                    !((parts.get(i + 1).is_some_and(|p| *p == "skills") && parts.len() >= i + 3)
                        || (parts.get(i + 1).is_some_and(|p| *p == "plugins")
                            && parts.get(i + 2).is_some_and(|p| *p == "cache")
                            && parts.len() >= i + 6))
                })
            {
                return Err(invalid(
                    "Archivi SSH/Codex non utilizzabili come cartelle di progetto",
                ));
            }
        }
        ProjectPaths::new(self.local_root.clone(), self.remote_root.clone())?;
        if self.operation == Operation::Copy && self.direction == Direction::Both {
            return Err(invalid("La copia è soltanto verso il destinatario"));
        }
        if self
            .include
            .iter()
            .chain(&self.exclude)
            .any(|path| !relative(path))
        {
            return Err(invalid(
                "Selezioni relative senza '..', '.', radici o percorsi vuoti",
            ));
        }
        Ok(())
    }
    fn selected(&self, path: &Path) -> bool {
        if reserved_path(path) {
            return false;
        }
        (self.include.is_empty() || self.include.iter().any(|prefix| path.starts_with(prefix)))
            && !self.exclude.iter().any(|prefix| path.starts_with(prefix))
    }
}

fn validate(rules: &[Rule]) -> io::Result<()> {
    let mut ids = HashSet::new();
    for rule in rules {
        rule.validate()?;
        if !ids.insert(&rule.id) {
            return Err(invalid("ID regola duplicato"));
        }
    }
    Ok(())
}
pub fn load(path: impl AsRef<Path>) -> io::Result<Vec<Rule>> {
    let file = match open_regular(path.as_ref()) {
        Ok((file, _)) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let rules: Vec<Rule> = serde_json::from_reader(file)?;
    validate(&rules)?;
    Ok(rules)
}
pub fn save(path: impl AsRef<Path>, rules: &[Rule]) -> io::Result<()> {
    validate(rules)?;
    staging::save_json(path.as_ref(), &rules)
}
pub fn upsert(rules: &mut Vec<Rule>, rule: Rule) -> io::Result<()> {
    rule.validate()?;
    if let Some(old) = rules.iter_mut().find(|old| old.id == rule.id) {
        *old = rule;
    } else {
        rules.push(rule);
    }
    Ok(())
}

pub(crate) fn ensure_local(directory: &Path) -> io::Result<()> {
    validate_absolute(directory)?;
    for path in directory.ancestors().collect::<Vec<_>>().into_iter().rev() {
        match path.symlink_metadata() {
            Ok(meta) if meta.is_dir() => {}
            Ok(_) => return Err(invalid("Directory locale non regolare o link")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(path)?,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
pub(crate) fn ensure_remote(device: &Device, directory: &Path) -> io::Result<()> {
    validate_absolute(directory)?;
    let directory = directory
        .to_str()
        .ok_or_else(|| invalid("Directory remota non UTF-8"))?;
    ssh::run(
        device,
        &format!(
            "p={d}; while [ \"$p\" != / ]; do [ ! -L \"$p\" ] && {{ [ ! -e \"$p\" ] || [ -d \"$p\" ]; }} || exit 2; p=$(dirname -- \"$p\"); done; mkdir -p -- {d}",
            d = quote(directory)
        ),
    )?;
    Ok(())
}
fn local_files(rule: &Rule, directory: &Path, files: &mut BTreeSet<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let rel = path
            .strip_prefix(&rule.local_root)
            .map_err(io::Error::other)?;
        let kind = entry.file_type()?;
        if kind.is_dir() {
            // Si può attraversare una cartella non inclusa per raggiungere file selezionati.
            if !rule.exclude.iter().any(|p| rel.starts_with(p)) && !reserved_path(rel) {
                local_files(rule, &path, files)?;
            }
        } else if rule.selected(rel) {
            if !kind.is_file() {
                return Err(invalid(format!(
                    "File speciale o link selezionato: {}",
                    rel.display()
                )));
            }
            files.insert(rel.into());
        }
    }
    Ok(())
}
fn reserved_path(path: &Path) -> bool {
    path.components().any(|part| {
        let Component::Normal(name) = part else {
            return true;
        };
        let Some(name) = name.to_str() else {
            return true;
        };
        matches!(
            name,
            ".git"
                | ".ssh"
                | ".codex"
                | "node_modules"
                | "target"
                | "__pycache__"
                | ".venv"
                | "venv"
                | "gpg.txt"
                | ".env"
                | ".htaccess"
        ) || (name.starts_with("gpg") && name.ends_with(".txt"))
            || name.starts_with(".oaclis-")
            || name.starts_with(".env.")
            || name.ends_with(".pem")
            || name.ends_with(".key")
            || name.ends_with(".pyc")
    })
}
fn remote_files(device: &Device, rule: &Rule) -> io::Result<BTreeSet<PathBuf>> {
    let output = ssh::run(
        device,
        &format!(
            "find -- {} -printf '%y\\0%P\\0'",
            quote(
                rule.remote_root
                    .to_str()
                    .ok_or_else(|| invalid("Radice non UTF-8"))?
            )
        ),
    )?;
    let parts: Vec<_> = output.split(|b| *b == 0).collect();
    if parts.last() != Some(&&b""[..]) || parts.len() % 2 != 1 {
        return Err(invalid("Elenco remoto non valido"));
    }
    let mut files = BTreeSet::new();
    for pair in parts[..parts.len() - 1].as_chunks::<2>().0 {
        let path = PathBuf::from(std::str::from_utf8(pair[1]).map_err(io::Error::other)?);
        if path.as_os_str().is_empty() {
            continue;
        }
        if !relative(&path) {
            return Err(invalid("Percorso remoto fuori progetto"));
        }
        if pair[0] == b"d" || !rule.selected(&path) {
            continue;
        }
        if pair[0] != b"f" {
            return Err(invalid(format!(
                "File remoto speciale o link selezionato: {}",
                path.display()
            )));
        }
        files.insert(path);
    }
    Ok(files)
}

#[derive(Debug, Serialize)]
pub struct FileResult {
    pub path: PathBuf,
    pub outcome: Outcome,
}
#[derive(Debug, Serialize)]
pub struct ItemProgress {
    pub path: PathBuf,
    pub index: usize,
    pub total: usize,
    pub event: Event,
}

pub fn execute(
    device: &Device,
    rule: &Rule,
    state: &Path,
    mut notify: impl FnMut(ItemProgress),
) -> io::Result<Vec<FileResult>> {
    rule.validate()?;
    device.validate()?;
    if device.id != rule.device_id {
        return Err(invalid("Dispositivo della regola diverso"));
    }
    validate_absolute(state)?;
    if state.starts_with(&rule.local_root) {
        return Err(invalid("Lo stato deve stare fuori dal progetto"));
    }
    ensure_local(&rule.local_root)?;
    ensure_remote(device, &rule.remote_root)?;
    ensure_local(state)?;
    let mut files = BTreeSet::new();
    if rule.direction != Direction::Receive {
        local_files(rule, &rule.local_root, &mut files)?;
    }
    if rule.direction != Direction::Send {
        files.extend(remote_files(device, rule)?);
    }
    let mapping = ProjectPaths::new(rule.local_root.clone(), rule.remote_root.clone())?;
    let total = files.len();
    let mut results = Vec::new();
    for (index, path) in files.into_iter().enumerate() {
        let local = rule.local_root.join(&path);
        let remote = mapping.map(&local)?;
        ensure_local(local.parent().unwrap())?;
        ensure_remote(device, remote.parent().unwrap())?;
        let mut callback = |event| {
            notify(ItemProgress {
                path: path.clone(),
                index: index + 1,
                total,
                event,
            })
        };
        callback(Event::Preparing);
        let outcome = if rule.operation == Operation::Copy && rule.direction == Direction::Receive {
            if let Ok((mut file, _)) = open_regular(&local) {
                let receipt = transfer::copy_hash(&mut file, &mut io::sink())?;
                if transfer::inspect_remote(device, &remote)?
                    .is_some_and(|(peer, _)| peer == receipt)
                {
                    Outcome::Identical
                } else {
                    Outcome::Conflict
                }
            } else {
                sync::synchronize_with_direction(
                    device,
                    &local,
                    &remote,
                    &state.join(format!(
                        "copy-{}.json",
                        Sha256::digest(path.as_os_str().as_encoded_bytes())
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect::<String>()
                    )),
                    Kind::File,
                    Direction::Receive,
                    &mut callback,
                )?
            }
        } else if rule.operation == Operation::Copy {
            let (mut file, metadata) = open_regular(&local)?;
            let receipt = transfer::copy_hash(&mut file, &mut io::sink())?;
            crate::check_unchanged(&file, &metadata)?;
            match transfer::inspect_remote(device, &remote)? {
                Some((peer, _)) if peer == receipt => Outcome::Identical,
                Some(_) => Outcome::Conflict,
                None => {
                    transfer::write_file(
                        device,
                        &local,
                        &remote,
                        None,
                        Some(&receipt),
                        &mut callback,
                    )?;
                    Outcome::Sent
                }
            }
        } else {
            let binding = serde_json::to_vec(&(
                device.id.as_str(),
                device.ssh_alias.as_str(),
                &local,
                &remote,
            ))?;
            let key: String = Sha256::digest(binding)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            sync::synchronize_with_direction(
                device,
                &local,
                &remote,
                &state.join(format!("{key}.json")),
                Kind::File,
                rule.direction,
                callback,
            )?
        };
        if rule.operation == Operation::Copy && outcome == Outcome::Identical {
            notify(ItemProgress {
                path: path.clone(),
                index: index + 1,
                total,
                event: Event::Completed,
            });
        }
        if outcome != Outcome::Conflict {
            if rule.direction == Direction::Receive || outcome == Outcome::Received {
                let (_, mode) = transfer::inspect_remote(device, &remote)?
                    .ok_or_else(|| invalid("File sorgente rimosso"))?;
                fs::set_permissions(&local, fs::Permissions::from_mode(mode & 0o777))?;
            } else {
                let mode = fs::metadata(&local)?.permissions().mode() & 0o777;
                ssh::run(
                    device,
                    &format!(
                        "[ ! -L {p} ] && chmod {mode:o} -- {p}",
                        p = quote(
                            remote
                                .to_str()
                                .ok_or_else(|| invalid("Percorso non UTF-8"))?
                        )
                    ),
                )?;
            }
        }
        results.push(FileResult { path, outcome });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rule() -> Rule {
        Rule {
            id: "project".into(),
            name: "Project".into(),
            device_id: "laptop".into(),
            local_root: "/work/project".into(),
            remote_root: "/dest/project".into(),
            operation: Operation::Sync,
            direction: Direction::Both,
            include: vec![],
            exclude: vec![],
        }
    }
    #[test]
    fn selection_uses_path_components_and_protects_private_files() {
        let mut rule = rule();
        rule.include = vec!["src".into(), "docs/hello world.md".into()];
        rule.exclude = vec!["src/generated".into()];
        for path in [
            "src/main.rs",
            "src/generated2/file.rs",
            "docs/hello world.md",
        ] {
            assert!(rule.selected(Path::new(path)));
        }
        for path in [
            "src2/main.rs",
            "src/generated/code.rs",
            "docs/other.md",
            "src/.env",
            "src/.env.local",
            "src/private.key",
            "src/.oaclis-backups/hash/file",
            "src/.git/config",
        ] {
            assert!(!rule.selected(Path::new(path)));
        }
    }
    #[test]
    fn rejects_roots_traversal_and_ambiguous_copy_direction() {
        let mut r = rule();
        r.local_root = "/".into();
        assert!(r.validate().is_err());
        for root in ["/root/.ssh", "/root/.codex/sessions"] {
            let mut r = rule();
            r.local_root = root.into();
            assert!(r.validate().is_err());
        }
        for value in ["../outside", "/root", "src/../outside", ".", ""] {
            let mut r = rule();
            r.include = vec![value.into()];
            assert!(r.validate().is_err());
        }
        let mut r = rule();
        r.operation = Operation::Copy;
        assert!(r.validate().is_err());
        r.direction = Direction::Send;
        assert!(r.validate().is_ok());
    }
    #[test]
    fn local_scan_skips_cache_and_rejects_selected_symlinks() {
        let root = std::env::temp_dir().join(format!(
            "oaclis-rule-scan-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target/cache"), b"cache").unwrap();
        fs::write(root.join("main.rs"), b"source").unwrap();
        let mut r = rule();
        r.local_root = root.clone();
        let mut files = BTreeSet::new();
        local_files(&r, &root, &mut files).unwrap();
        assert_eq!(files, BTreeSet::from([PathBuf::from("main.rs")]));
        std::os::unix::fs::symlink("main.rs", root.join("link")).unwrap();
        assert!(local_files(&r, &root, &mut files).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
