// environment.rs v1.13.476
//! Chat environment analysis and destination preparation, independent of the UI.
use crate::{
    devices::{self, Device},
    invalid,
    paths::validate_absolute,
    rules::{self, Operation, Rule},
    ssh, staging,
    sync::{Direction, Outcome},
    transfer,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const ANALYZER: &str = include_str!("../tools/environment_analysis.py");
const SETUP: &str = include_str!("../tools/environment_setup.py");
pub(crate) const STORE: &str = concat!(include_str!("../tools/chat_records.py"), "\n", include_str!("../tools/appearance_store.py"));

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Scopes {
    pub chat: bool,
    pub paths: bool,
    pub apps: bool,
}
impl Default for Scopes {
    fn default() -> Self {
        Self {
            chat: true,
            paths: true,
            apps: true,
        }
    }
}
impl Scopes {
    pub fn any(self) -> bool {
        self.chat || self.paths || self.apps
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub path: PathBuf,
    pub projects: Vec<PathBuf>,
    pub service: String,
    #[serde(default)]
    pub modules: Vec<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plugin {
    pub path: PathBuf,
    pub kind: String,
    pub name: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Fingerprint {
    pub path: PathBuf,
    pub bytes: Option<u64>,
    pub modified_ns: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub home: PathBuf,
    pub codex_root: PathBuf,
    pub paths: Vec<PathBuf>,
    pub packages: Vec<String>,
    pub configs: Vec<Config>,
    pub plugins: Vec<Plugin>,
    pub uncertainties: Vec<String>,
    #[serde(default)] pub path_notes: Vec<String>,
    #[serde(default)] pub apps: Vec<String>,
    #[serde(default)] pub app_notes: Vec<String>,
    #[serde(default)] pub read_notes: Vec<String>,
    #[serde(default)]
    pub fingerprints: Vec<Fingerprint>,
}
impl Manifest {
    pub fn validate(&self) -> io::Result<()> {
        for p in [&self.home, &self.codex_root] {
            validate_absolute(p)?;
        }
        for p in &self.paths {
            validate_absolute(p)?;
            if p == &self.home || p.components().count() < 3 || ["/etc","/proc","/sys","/dev","/run","/usr","/bin","/lib","/boot"].iter().any(|root| p.starts_with(root))
                || p.components().any(|c| matches!(c,std::path::Component::Normal(s) if [".ssh",".codex",".agents",".config",".cache"].iter().any(|n|s == *n))) {
                return Err(invalid("Radice del progetto non valida"));
            }
        }
        const PACKAGES: &[&str] = &[
            "cargo",
            "rustc",
            "nodejs",
            "npm",
            "python3",
            "python3-venv",
            "python3-pip",
            "php-cli",
            "composer",
            "ruby",
            "bundler",
            "golang-go",
            "docker.io",
            "docker-compose-v2",
            "apache2",
            "nginx",
            "libapache2-mod-php",
            "rsync",
            "openssh-client",
        ];
        if self
            .packages
            .iter()
            .any(|p| !PACKAGES.contains(&p.as_str()))
        {
            return Err(invalid("Pacchetto non supportato"));
        }
        for config in &self.configs {
            validate_absolute(&config.path)?;
            if config.modules.iter().any(|m| {
                ![
                    "rewrite",
                    "ssl",
                    "proxy",
                    "proxy_http",
                    "headers",
                    "expires",
                ]
                .contains(&m.as_str())
            }) {
                return Err(invalid("Modulo Apache non supportato"));
            }
            let parent = config.path.parent().and_then(Path::to_str).unwrap_or("");
            let local_web = config.path.file_name().is_some_and(|n| n == ".htaccess")
                && config.service == "apache2"
                && self.paths.iter().any(|root| config.path.starts_with(root));
            let server_web = matches!(
                (parent, config.service.as_str()),
                ("/etc/apache2/sites-available", "apache2")
                    | ("/etc/nginx/sites-available", "nginx")
                    | ("/etc/nginx/conf.d", "nginx")
            );
            if (!local_web && !server_web)
                || !config.projects.iter().any(|p| self.paths.contains(p))
            {
                return Err(invalid("Configurazione non associata al progetto"));
            }
        }
        for plugin in &self.plugins {
            validate_absolute(&plugin.path)?;
            if !devices::identifier(&plugin.name) {
                return Err(invalid("Nome plugin non valido"));
            }
            let valid = match plugin.kind.as_str() {
                "skill" => [
                    self.codex_root.join("skills"),
                    self.home.join(".agents/skills"),
                ]
                .iter()
                .any(|base| {
                    plugin.path.parent() == Some(base.as_path())
                        && plugin
                            .path
                            .file_name()
                            .is_some_and(|n| n == plugin.name.as_str())
                }),
                "plugin" => plugin
                    .path
                    .strip_prefix(self.codex_root.join("plugins/cache"))
                    .is_ok_and(|rel| {
                        rel.components().count() == 3
                            && rel
                                .components()
                                .nth(1)
                                .is_some_and(|c| c.as_os_str() == plugin.name.as_str())
                    }),
                _ => false,
            };
            if !valid {
                return Err(invalid("Percorso plugin non valido"));
            }
        }
        for item in &self.fingerprints {
            validate_absolute(&item.path)?;
            if !self.paths.iter().any(|root| item.path.starts_with(root))
                && !self.plugins.iter().any(|p| item.path.starts_with(&p.path))
                && !self.configs.iter().any(|c| c.path == item.path)
            {
                return Err(invalid("Manifest dipendenze esterno al progetto"));
            }
        }
        Ok(())
    }
}
pub fn key(source: &str, chat: &str) -> io::Result<String> {
    if !devices::identifier(source) || !devices::identifier(chat) {
        return Err(invalid("Chat o dispositivo non valido"));
    }
    Ok(format!("{source}:{chat}"))
}
pub fn revision(chat: &Value, device: Option<&Device>) -> String {
    format!(
        "environment-v3:{}:{}:{}:{}",
        device.map(|d| d.ssh_alias.as_str()).unwrap_or("master"),
        chat["path"],
        chat["bytes"],
        chat["modified_ns"]
    )
}
pub fn database(
    config: &Path,
    key: &str,
    operation: &str,
    value: Option<&Value>,
) -> io::Result<Value> {
    let path = config.join("appearance.sqlite");
    rules::ensure_local(config)?;
    match crate::open_regular(&path) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut command = Command::new("python3");
    command
        .args(["-c", STORE])
        .arg(&path)
        .arg(operation)
        .arg(key)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    if let Some(value) = value {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(value)?)?;
    } else {
        drop(child.stdin.take());
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!("Database ambiente non disponibile: {}", detail.lines().last().unwrap_or("errore SQLite"))));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
pub(crate) fn python(device: Option<&Device>, script: &str, args: &[&str]) -> io::Result<Value> {
    let bytes = if let Some(device) = device {
        ssh::run(
            device,
            &format!(
                "python3 -c {} {}",
                ssh::quote(script),
                args.iter()
                    .map(|a| ssh::quote(a))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
        )?
    } else {
        let output = Command::new("python3")
            .args(["-c", script])
            .args(args)
            .output()?;
        if !output.status.success() {
            return Err(invalid(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        output.stdout
    };
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn analyze(device: Option<&Device>, path: &str, chat: &str) -> io::Result<Manifest> {
    let manifest: Manifest = serde_json::from_value(python(device, ANALYZER, &[path, chat])?)
        .map_err(io::Error::other)?;
    manifest.validate()?;
    Ok(manifest)
}
pub fn fresh(device: Option<&Device>, manifest: &Manifest) -> io::Result<bool> {
    manifest.validate()?;
    Ok(python(
        device,
        SETUP,
        &["verify", &serde_json::to_string(&manifest.fingerprints)?],
    )?["fresh"]
        == true)
}
fn mapped(path: &Path, from: &Path, to: &Path) -> PathBuf {
    path.strip_prefix(from)
        .map(|p| to.join(p))
        .unwrap_or_else(|_| path.to_path_buf())
}
fn digest(value: &impl Serialize) -> io::Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(value)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

pub fn execute(
    device: &Device,
    manifest: &Manifest,
    scopes: Scopes,
    incoming: bool,
    operation: &str,
    state: &Path,
    mut notify: impl FnMut(Value),
) -> io::Result<Value> {
    manifest.validate()?;
    rules::ensure_local(state)?;
    let target_device = if incoming { None } else { Some(device) };
    let identity = python(target_device, SETUP, &["identity"])?;
    let target_home = PathBuf::from(
        identity["home"]
            .as_str()
            .ok_or_else(|| invalid("Home destinataria mancante"))?,
    );
    let target_codex = PathBuf::from(
        identity["codex_root"]
            .as_str()
            .ok_or_else(|| invalid("Cartella Codex destinataria mancante"))?,
    );
    validate_absolute(&target_home)?;
    validate_absolute(&target_codex)?;
    let destination_paths: Vec<_> = manifest
        .paths
        .iter()
        .map(|p| mapped(p, &manifest.home, &target_home))
        .collect();
    let mut results = Vec::new();
    let mut directories: Vec<(PathBuf, PathBuf)> = if scopes.paths {
        manifest
            .paths
            .iter()
            .cloned()
            .zip(destination_paths.iter().cloned())
            .collect()
    } else {
        vec![]
    };
    if scopes.apps {
        for plugin in &manifest.plugins {
            let target = if plugin.kind == "plugin" {
                target_home
                    .join(".config/oaclis/plugins")
                    .join(&plugin.name)
            } else if plugin
                .path
                .starts_with(manifest.home.join(".agents/skills"))
            {
                target_home.join(".agents/skills").join(&plugin.name)
            } else {
                target_codex.join("skills").join(&plugin.name)
            };
            directories.push((plugin.path.clone(), target));
        }
    }
    for (source, destination) in directories {
        let (local, remote) = if incoming {
            (destination, source)
        } else {
            (source, destination)
        };
        let id = digest(&(device.id.as_str(), &local, &remote))?;
        let rule = Rule {
            id: id.clone(),
            name: "Ambiente chat".into(),
            device_id: device.id.clone(),
            local_root: local,
            remote_root: remote,
            operation: if operation == "copy" {
                Operation::Copy
            } else {
                Operation::Sync
            },
            direction: if incoming && operation != "sync" {
                Direction::Receive
            } else if operation == "sync" {
                Direction::Both
            } else {
                Direction::Send
            },
            include: vec![],
            exclude: vec![],
        };
        let files = rules::execute(device, &rule, &state.join(&id), |p| {
            notify(json!({"event":p.event,"path":p.path,"index":p.index,"total":p.total}))
        })?;
        if files.iter().any(|f| f.outcome == Outcome::Conflict) {
            return Err(invalid("Conflitto nelle cartelle: copie conservate"));
        }
        results.push(json!(files));
    }
    if scopes.apps {
        notify(json!({"event":{"phase":"transferring"}}));
        let source_device = if incoming { Some(device) } else { None };
        let input = json!({"paths":manifest.paths,"configs":manifest.configs,"plugins":manifest.plugins});
        let inventory = python(source_device, include_str!("../tools/app_inventory.py"), &["source", &input.to_string()])?;
        let mut configs = Vec::new();
        for config in &manifest.configs {
            let mut staged = staging::StagedFile::new(state)?;
            if incoming {
                transfer::fetch_file(device, &config.path, &staged.path, &mut |_| {})?
                    .ok_or_else(|| invalid("Configurazione sorgente assente"))?;
            } else {
                let (mut source, _) = crate::open_regular(&config.path)?;
                io::copy(&mut source, &mut staged.file)?;
            }
            let mut content = String::new();
            fs::File::open(&staged.path)?.take(131073).read_to_string(&mut content)?;
            if content.len() > 131072 { return Err(invalid("Configurazione troppo grande")); }
            configs.push(json!({"path":config.path,"service":config.service,"modules":config.modules,"content":content}));
        }
        let definition = json!({"paths":manifest.paths,"requirements":inventory["requirements"],"configs":configs,"plugins":manifest.plugins});
        let directory = target_home.join(".config/oaclis/environments");
        let target = directory.join(format!("{}.json",state.file_name().and_then(|v|v.to_str()).ok_or_else(||invalid("Identificativo ambiente assente"))?));
        if incoming {
            rules::ensure_local(&directory)?;
            staging::save_json(&target, &definition)?;
        } else {
            rules::ensure_remote(device, &directory)?;
            let mut staged = staging::StagedFile::new(state)?;
            staged.file.write_all(&serde_json::to_vec_pretty(&definition)?)?;
            staged.file.sync_all()?;
            let previous = transfer::inspect_remote(device, &target)?.map(|(receipt,_)|receipt);
            transfer::write_file(device, &staged.path, &target, previous.as_ref(), None, &mut |event|notify(json!({"event":event})))?;
        }
        results.push(json!({"definition":target}));
    }
    Ok(json!(results))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_system_roots_and_arbitrary_packages() {
        let mut manifest = Manifest {
            home: "/root".into(),
            codex_root: "/root/.codex".into(),
            paths: vec!["/root/app".into()],
            packages: vec!["nodejs".into()],
            configs: vec![],
            plugins: vec![],
            uncertainties: vec![], path_notes: vec![], apps: vec![], app_notes: vec![], read_notes: vec![],
            fingerprints: vec![],
        };
        assert!(manifest.validate().is_ok());
        manifest.paths = vec!["/etc/apache2".into()];
        assert!(manifest.validate().is_err());
        manifest.paths = vec!["/root".into()];
        assert!(manifest.validate().is_err());
        manifest.paths = vec!["/root/app".into()];
        manifest.packages = vec!["curl; touch /tmp/x".into()];
        assert!(manifest.validate().is_err());
        assert_eq!(
            mapped(
                Path::new("/root/app"),
                Path::new("/root"),
                Path::new("/home/kim")
            ),
            Path::new("/home/kim/app")
        );
    }
}
