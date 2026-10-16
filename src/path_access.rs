// path_access.rs v1.12.354
//! One-use authorization for creating a missing project directory; passwords never enter state.
use crate::{devices::Device, invalid};
use serde_json::{Value, json};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex, mpsc},
};

#[derive(Clone)]
struct Pending {
    id: String,
    device: Option<Device>,
    path: PathBuf,
    done: mpsc::SyncSender<bool>,
    busy: bool,
}
static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn validate(path: &Path) -> io::Result<()> {
    crate::paths::validate_absolute(path)?;
    if path.components().count() < 3
        || [
            "/etc", "/usr", "/bin", "/sbin", "/lib", "/lib64", "/boot", "/dev", "/proc", "/sys",
            "/run",
        ]
        .iter()
        .any(|p| path.starts_with(p))
        || path.components().any(|part| {
            [".ssh", ".codex", ".agents", ".config", ".cache"]
                .iter()
                .any(|name| part.as_os_str() == *name)
        })
    {
        return Err(invalid("Percorso non autorizzato alla preparazione sudo"));
    }
    Ok(())
}
fn number(flag: &str) -> io::Result<u32> {
    let out = Command::new("id").arg(flag).output()?;
    if !out.status.success() {
        return Err(io::Error::other("Identita' utente non disponibile"));
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .map_err(io::Error::other)
}
fn check_ancestors(path: &Path) -> io::Result<()> {
    for parent in path.ancestors() {
        match parent.symlink_metadata() {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(invalid(
                    "Il percorso contiene un link o non e' una cartella",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
// Called only by sudo. Existing folders and their ownership are never changed.
pub fn create_privileged(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    validate(path)?;
    if path.starts_with("/root") {
        return Err(invalid(
            "La home root non e' una destinazione sudo autorizzata",
        ));
    }
    if number("-u")? != 0 || uid == 0 {
        return Err(invalid("Identita' amministrativa non valida"));
    }
    let mut current = PathBuf::from("/");
    let parts: Vec<_> = path.components().skip(1).collect();
    for (index, part) in parts.iter().enumerate() {
        current.push(part.as_os_str());
        let last = index + 1 == parts.len();
        match current.symlink_metadata() {
            Ok(meta) => {
                if last {
                    return Err(invalid(
                        "La cartella esiste gia': proprietario e permessi non modificati",
                    ));
                }
                if !meta.is_dir()
                    || meta.file_type().is_symlink()
                    || meta.uid() != 0
                    || meta.permissions().mode() & 0o022 != 0
                {
                    return Err(invalid(
                        "Il percorso amministrativo contiene una cartella non protetta o un link",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::DirBuilder::new().mode(0o755).create(&current)?;
                fs::set_permissions(&current, fs::Permissions::from_mode(0o755))?;
                if last {
                    if let Err(error) = std::os::unix::fs::chown(&current, Some(uid), Some(gid)) {
                        let _ = fs::remove_dir(&current);
                        return Err(error);
                    }
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
pub(crate) fn prepare(mut request: Value) -> io::Result<Value> {
    let path = PathBuf::from(
        request["path"]
            .as_str()
            .ok_or_else(|| invalid("Percorso mancante"))?,
    );
    validate(&path)?;
    check_ancestors(&path)?;
    let user = String::from_utf8_lossy(&Command::new("id").arg("-un").output()?.stdout)
        .trim()
        .to_owned();
    match fs::create_dir_all(&path) {
        Ok(()) => return match crate::staging::StagedFile::new(&path) {
            Ok(_) => Ok(json!({"ready":true})),
            Err(e) if e.kind()==io::ErrorKind::PermissionDenied => Err(io::Error::other("La cartella esiste ma non e' scrivibile: proprietario e permessi non modificati")),
            Err(e) => Err(e),
        },
        Err(error) if error.kind()==io::ErrorKind::PermissionDenied => {
            if request.get("password").is_none() { return Ok(json!({"ready":false,"user":user})); }
        },
        Err(error) => return Err(error),
    }
    let mut password = request["password"]
        .take()
        .as_str()
        .ok_or_else(|| invalid("Password richiesta"))?
        .as_bytes()
        .to_vec();
    if password.is_empty()
        || password.len() > 1024
        || password.iter().any(|b| matches!(b, 0 | 10 | 13))
    {
        password.fill(0);
        return Err(invalid("Password non valida"));
    }
    let uid = number("-u")?;
    let gid = number("-g")?;
    let mut child = Command::new("sudo")
        .args(["-S", "-k", "-p", "", "--"])
        .arg(std::env::current_exe()?)
        .arg("prepare-path-root")
        .arg(&path)
        .arg(uid.to_string())
        .arg(gid.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let sent = (|| {
        let mut input = child.stdin.take().unwrap();
        input.write_all(&password)?;
        input.write_all(b"\n")
    })();
    password.fill(0);
    if let Err(error) = sent {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(io::Error::other(format!(
            "Autorizzazione non riuscita: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    check_ancestors(&path)?;
    drop(crate::staging::StagedFile::new(&path)?);
    Ok(json!({"ready":true}))
}
fn target(device: Option<&Device>, value: &Value) -> io::Result<Value> {
    if let Some(device) = device {
        crate::state_transport::remote(device, value)
    } else {
        prepare(value.clone())
    }
}
pub(crate) fn ensure(
    device: Option<&Device>,
    path: &Path,
    job: &Arc<Mutex<Value>>,
) -> io::Result<()> {
    let status = target(device, &json!({"kind":"prepare-path","path":path}))?;
    if status["ready"] == true {
        return Ok(());
    }
    let mut random = [0u8; 24];
    fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
    let id: String = random.iter().map(|b| format!("{b:02x}")).collect();
    let (done, receive) = mpsc::sync_channel(1);
    {
        let mut pending = PENDING.lock().unwrap();
        if pending.is_some() {
            return Err(io::Error::other("Un'autorizzazione e' gia' in attesa"));
        }
        *pending = Some(Pending {
            id: id.clone(),
            device: device.cloned(),
            path: path.into(),
            done,
            busy: false,
        });
    }
    job.lock().unwrap()["authorization"] = json!({"id":id,"device":device.map(|d|d.name.as_str()).unwrap_or("Questo dispositivo"),"path":path,"user":status["user"]});
    let result = receive.recv();
    job.lock()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("authorization");
    if result == Ok(true) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::Interrupted,
            "Preparazione cartella annullata: trasferimento fermato",
        ))
    }
}
pub(crate) fn authorize(mut request: Value) -> io::Result<Value> {
    let id = request["id"]
        .as_str()
        .ok_or_else(|| invalid("Autorizzazione mancante"))?;
    let pending = {
        let mut guard = PENDING.lock().unwrap();
        let pending = guard
            .as_mut()
            .filter(|p| p.id == id)
            .ok_or_else(|| invalid("Autorizzazione non piu' attiva"))?;
        if pending.busy {
            return Err(invalid("Autorizzazione gia' in corso"));
        }
        pending.busy = true;
        pending.clone()
    };
    if request["cancel"] == true {
        PENDING.lock().unwrap().take();
        let _ = pending.done.send(false);
        return Ok(json!({"ok":true}));
    }
    let password = request["password"].take();
    let result = target(
        pending.device.as_ref(),
        &json!({"kind":"prepare-path","path":pending.path,"password":password}),
    );
    let mut guard = PENDING.lock().unwrap();
    match result {
        Ok(value) if value["ready"] == true => {
            guard.take();
            let _ = pending.done.send(true);
            Ok(json!({"ok":true}))
        }
        Ok(_) => {
            if let Some(p) = guard.as_mut() {
                p.busy = false;
            }
            Err(io::Error::other("Cartella non scrivibile"))
        }
        Err(error) => {
            if let Some(p) = guard.as_mut() {
                p.busy = false;
            }
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "oaclis-path-access-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn ordinary_preparation_creates_only_the_requested_directory() {
        let root = fixture();
        let path = root.join("project/nested");
        assert_eq!(prepare(json!({"path":path})).unwrap()["ready"], true);
        assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
        assert_eq!(prepare(json!({"path":path})).unwrap()["ready"], true);
        assert_eq!(prepare(json!({"path":path,"password":"unused-fixture"})).unwrap()["ready"], true);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn protected_paths_and_symlinks_are_refused_before_sudo() {
        for path in [
            "/",
            "/etc/project",
            "/usr/project",
            "/var/../etc/project",
            "/home/user/.ssh/project",
        ] {
            assert!(prepare(json!({"path":path,"password":"unused-fixture"})).is_err());
        }
        let root = fixture();
        let alias = root.join("alias");
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        assert!(prepare(json!({"path":alias.join("project")})).is_err());
        assert!(!root.join("project").exists());
        // /tmp is writable by other users: a privileged traversal must refuse it.
        assert!(create_privileged(&root.join("privileged"), 1000, 1000).is_err());
        assert!(!root.join("privileged").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn authorization_is_bound_to_one_request_and_cancel_releases_the_worker() {
        let (done, receive) = mpsc::sync_channel(1);
        *PENDING.lock().unwrap() = Some(Pending {
            id: "active".into(),
            device: None,
            path: "/var/www/html/project".into(),
            done,
            busy: false,
        });
        assert!(authorize(json!({"id":"stale","password":"unused-fixture"})).is_err());
        assert!(
            PENDING
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|p| p.id == "active")
        );
        assert_eq!(
            authorize(json!({"id":"active","cancel":true})).unwrap()["ok"],
            true
        );
        assert_eq!(receive.recv().unwrap(), false);
        assert!(PENDING.lock().unwrap().is_none());
        assert!(authorize(json!({"id":"active","cancel":true})).is_err());
    }
}
