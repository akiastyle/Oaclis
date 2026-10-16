// environment.rs v1.21.726
//! Chat environment analysis and destination preparation, independent of the UI.
use crate::{
    devices::{self, Device},
    invalid,
    paths::validate_absolute,
    rules::{self, Operation, Rule},
    staging,
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
};


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
    crate::state_store::database(config,key,operation,value)
}
pub(crate) fn request(device: Option<&Device>, operation: &str, value: Value) -> io::Result<Value> {
    let request=json!({"kind":"environment","operation":operation,"value":value});
    if let Some(device)=device {crate::state_transport::remote(device,&request)} else {native(&request)}
}
fn native(request:&Value)->io::Result<Value>{
    let value=&request["value"];
    match request["operation"].as_str(){
        Some("identity")=>crate::environment_setup::identity(),
        Some("verify")=>{let items:Vec<Fingerprint>=serde_json::from_value(value.clone()).map_err(io::Error::other)?;Ok(json!({"fresh":crate::environment_setup::fresh(&items)?}))},
        Some("analyze")=>serde_json::to_value(crate::environment_analysis::analyze(Path::new(value["path"].as_str().ok_or_else(||invalid("Archivio richiesto"))?),value["chat_id"].as_str().ok_or_else(||invalid("Chat richiesta"))?)?).map_err(io::Error::other),
        _=>Err(invalid("Operazione ambiente non valida")),
    }
}
pub fn forward()->io::Result<()>{
    let mut bytes=Vec::new();io::stdin().take(16*1024*1024+1).read_to_end(&mut bytes)?;
    if bytes.len()>16*1024*1024{return Err(invalid("Richiesta troppo grande"));}
    let request:Value=serde_json::from_slice(&bytes)?;
    if request["operation"]=="install" {
        let path=Path::new(request["value"]["definition"].as_str().ok_or_else(||invalid("Definizione richiesta"))?);
        let mut output=io::stdout().lock();
        let result=crate::app_install::saved(path,&mut |event| {
            let _=serde_json::to_writer(&mut output,&json!({"_oaclis_progress":event}));
            let _=output.write_all(b"\n");let _=output.flush();
        });
        let response=match result {Ok(value)=>json!({"_oaclis_result":value}),Err(error)=>json!({"_oaclis_error":error.to_string()})};
        serde_json::to_writer(&mut output,&response)?;output.write_all(b"\n")?;
    } else {serde_json::to_writer(io::stdout(),&native(&request)?)?;}
    Ok(())
}
pub fn analyze(device: Option<&Device>, path: &str, chat: &str) -> io::Result<Manifest> {
    let manifest:Manifest=serde_json::from_value(request(device,"analyze",json!({"path":path,"chat_id":chat}))?).map_err(io::Error::other)?;
    manifest.validate()?;Ok(manifest)
}
pub fn fresh(device: Option<&Device>, manifest: &Manifest) -> io::Result<bool> {
    manifest.validate()?;Ok(request(device,"verify",serde_json::to_value(&manifest.fingerprints)?)?["fresh"]==true)
}
fn digest(value: &impl Serialize) -> io::Result<String> {
    Ok(Sha256::digest(serde_json::to_vec(value)?)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

fn paths_command(device: &Device, list: &Path, incoming: bool) -> std::process::Command {
    let mut command = std::process::Command::new("rsync");
    command.args(["--recursive", "--links", "--safe-links", "--times", "--omit-dir-times",
        "--relative", "--no-implied-dirs", "--from0", "--info=progress2,name0", "--outbuf=L",
        "--partial", "--partial-dir=.oaclis-rsync", "--timeout=30", "--secluded-args"])
        .arg("--files-from").arg(list).arg("--rsh").arg(crate::ssh::OPTIONS);
    // Preserve the existing project exclusions without enumerating or comparing files ourselves.
    for pattern in [".git", ".ssh", ".codex", "node_modules", "target", "__pycache__", ".venv", "venv",
        "gpg*.txt", ".env", ".env.*", ".htaccess", ".oaclis-*", "*.pem", "*.key", "*.pyc"] {
        command.arg(format!("--exclude={pattern}"));
    }
    command.arg("--");
    if incoming { command.arg(format!("{}:/", device.ssh_alias)).arg("/"); }
    else { command.arg("/").arg(format!("{}:/", device.ssh_alias)); }
    command
}

fn transfer_paths(device: &Device, paths: &[PathBuf], incoming: bool, state: &Path,
    notify: &mut impl FnMut(Value)) -> io::Result<()> {
    if paths.is_empty() { return Ok(()); }
    device.validate()?;
    let mut list = staging::StagedFile::new(state)?;
    for path in paths {
        validate_absolute(path)?;
        list.file.write_all(path.strip_prefix("/").map_err(io::Error::other)?.as_os_str().as_encoded_bytes())?;
        list.file.write_all(b"/\0")?;
    }
    list.file.flush()?;
    let mut command = paths_command(device, &list.path, incoming);
    crate::state_transport::log(format!("EXEC {command:?}"));
    let result = crate::progress::run(&mut command, &mut |mut event| {
        if let crate::progress::Event::Transferring { percent, .. } = &mut event { *percent = (*percent).min(99); }
        notify(json!({"event":event}));
    });
    crate::state_transport::log(format!("EXIT rsync {}", match &result { Ok(()) => "OK".into(), Err(error) => error.to_string() }));
    result
}

pub fn execute(
    device: &Device,
    manifest: &Manifest,
    inventory: &Value,
    scopes: Scopes,
    incoming: bool,
    operation: &str,
    state: &Path,
    mut prepare_path: impl FnMut(Option<&Device>, &Path) -> io::Result<()>,
    mut notify: impl FnMut(Value),
) -> io::Result<Value> {
    manifest.validate()?;
    rules::ensure_local(state)?;
    let target_device = if incoming { None } else { Some(device) };
    let mut results = Vec::new();
    if scopes.paths {
        for path in &manifest.paths { prepare_path(target_device, path)?; }
        transfer_paths(device, &manifest.paths, incoming, state, &mut notify)?;
        results.push(json!({"paths":manifest.paths}));
    }
    if !scopes.apps { return Ok(json!(results)); }
    let identity = request(target_device,"identity",Value::Null)?;
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
    let mut directories: Vec<(PathBuf, PathBuf)> = Vec::new();
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
            crate::state_transport::log(format!("APPS FILE {} {}", p.path.display(),serde_json::to_string(&p.event).unwrap_or_default()))
        })?;
        if files.iter().any(|f| f.outcome == Outcome::Conflict) {
            return Err(invalid("Conflitto nelle cartelle: copie conservate"));
        }
        results.push(json!(files));
    }
    if scopes.apps {
        notify(json!({"event":{"phase":"transferring"}}));
        if inventory["status"]!="ready" {return Err(invalid("Inventario App sorgente non pronto"));}
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
        let definition = json!({"paths":manifest.paths,"wait_for_paths":inventory["paths_pending"]==true,"requirements":inventory["requirements"],"configs":configs,"plugins":manifest.plugins});
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
            transfer::write_file(device, &staged.path, &target, previous.as_ref(), None, &mut |event|crate::state_transport::log(format!("APPS DEFINITION {}",serde_json::to_string(&event).unwrap_or_default())))?;
        }
        results.push(json!({"definition":target}));
        let installation=if incoming {crate::app_install::saved(&target,&mut notify)} else {
            crate::state_transport::remote_progress(device,&json!({"kind":"environment","operation":"install","value":{"definition":target}}),&mut notify)
        }.map_err(|error| {crate::state_transport::log(format!("APP INSTALL FAILED {}",crate::environment_analysis::redact(&error.to_string())));invalid("Installazione delle app non riuscita. Dettagli nel log.")})?;
        if installation["status"]!="completed" {return Err(invalid("Installazione delle app non completata. Dettagli nel log."));}
        results.push(json!({"installation":installation}));
    }
    Ok(json!(results))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_rsync_preserves_absolute_layout_and_destination_only_files() {
        let root = std::env::temp_dir().join(format!("oaclis-paths-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();
        let first = root.join("first project"); let second = root.join("second\nproject");
        let destination = root.join("destination");
        for path in [&first,&second,&destination] {fs::create_dir(path).unwrap();}
        fs::write(first.join("file"),"new source content").unwrap();
        fs::write(second.join("file"),"second source").unwrap();
        fs::write(first.join(".env"),"secret").unwrap();
        let target = destination.join(first.strip_prefix("/").unwrap());
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("file"),"old").unwrap();
        fs::write(target.join("keep"),"destination only").unwrap();
        let mut list = staging::StagedFile::new(&root).unwrap();
        for path in [&first,&second] {list.file.write_all(path.strip_prefix("/").unwrap().as_os_str().as_encoded_bytes()).unwrap();list.file.write_all(b"/\0").unwrap();}
        list.file.flush().unwrap();
        let device = Device {id:"fixture".into(),name:"Fixture".into(),ssh_alias:"fixture".into()};
        let command = paths_command(&device,&list.path,false);
        let args: Vec<_> = command.get_args().map(|a|a.to_owned()).collect();
        assert!(!args.iter().any(|a|a=="--delete" || a=="--checksum"));
        let incoming = paths_command(&device,&list.path,true);
        let reverse: Vec<_> = incoming.get_args().map(|a|a.to_string_lossy().into_owned()).collect();
        assert_eq!(&reverse[reverse.len()-2..], &["fixture:/", "/"]);
        // Exercise the identical rsync arguments against isolated local directories.
        let mut local = std::process::Command::new("rsync");
        local.args(&args[..args.len()-1]).arg(&destination);
        let mut progress = false;
        crate::progress::run(&mut local,&mut |event| {if matches!(event,crate::progress::Event::Transferring{..}) {progress=true;}}).unwrap();
        assert!(progress);
        assert_eq!(fs::read_to_string(target.join("file")).unwrap(),"new source content");
        assert_eq!(fs::read_to_string(target.join("keep")).unwrap(),"destination only");
        assert!(!target.join(".env").exists());
        assert_eq!(fs::read_to_string(destination.join(second.strip_prefix("/").unwrap()).join("file")).unwrap(),"second source");
        drop(list);
        fs::remove_dir_all(root).unwrap();
    }

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

    }
}
