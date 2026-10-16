// app_install.rs v1.33.891
//! Install the dependencies described by an explicitly transferred environment definition.
use crate::{environment_setup as setup, invalid};
use serde_json::{Value,json};
use std::{collections::BTreeSet,fs,io,path::{Path,PathBuf},process::Command,time::{Duration,Instant}};

fn emit(notify: &mut impl FnMut(Value), done: usize, total: usize, message: String) {
    notify(json!({"event":{"phase":"transferring","percent":if total==0 {0} else {(done*100/total).min(99)}},"message":message}));
}
fn execute(command: &mut Command, done: usize, total: usize, notify: &mut impl FnMut(Value)) -> io::Result<String> {
    emit(notify,done,total,format!("EXEC {command:?}"));
    let result=crate::app_update::run(command,Duration::from_secs(1800));
    match &result {
        Ok(output)=>emit(notify,done,total,format!("EXIT OK {}",crate::environment_analysis::redact(output))),
        Err(error)=>emit(notify,done,total,format!("EXIT ERROR {}",crate::environment_analysis::redact(&error.to_string()))),
    }
    result
}
fn numeric(version: &str) -> bool {
    let parts:Vec<_>=version.split('.').collect();
    parts.len()==3 && parts.iter().all(|p|!p.is_empty() && p.bytes().all(|b|b.is_ascii_digit()))
}
fn package_name(name: &str) -> bool {
    let part=|s:&str|!s.is_empty() && s!="." && s!=".." && s.bytes().all(|b|b.is_ascii_alphanumeric() || b"_.-".contains(&b));
    if let Some(scoped)=name.strip_prefix('@') {scoped.split_once('/').is_some_and(|(scope,name)|part(scope)&&part(name))} else {part(name)}
}
fn validate(definition: &Value) -> io::Result<Vec<Value>> {
    let roots=definition["paths"].as_array().ok_or_else(||invalid("Percorsi del progetto mancanti"))?;
    let manifest:crate::environment::Manifest=serde_json::from_value(json!({"home":setup::home()?,"codex_root":setup::codex_root()?,"paths":roots,"packages":[],"configs":[],"plugins":[],"uncertainties":[]}))?;
    manifest.validate()?;
    if manifest.paths.iter().any(|p|!setup::unlinked(p)) {return Err(invalid("Percorso progetto non regolare"));}
    let mut requirements=definition["requirements"].as_array().ok_or_else(||invalid("Dipendenze mancanti"))?.clone();
    for item in &requirements {
        let name=item["name"].as_str().ok_or_else(||invalid("Nome dipendenza mancante"))?;
        match item["kind"].as_str() {
            Some("command") if matches!(name,"node"|"npm") && item["version"].as_str().is_some_and(numeric)=>{},
            Some("node") if package_name(name) && item["version"].as_str().is_some_and(numeric)=> {
                let evidence=Path::new(item["evidence"].as_str().ok_or_else(||invalid("Manifest Node mancante"))?);
                crate::paths::validate_absolute(evidence)?;
                if evidence.file_name()!=Some(std::ffi::OsStr::new("package.json")) || !roots.iter().filter_map(Value::as_str).any(|root|evidence.starts_with(root)) {return Err(invalid("Manifest Node esterno al progetto o non disponibile"));}
            },
            Some("skill"|"plugin") if crate::devices::identifier(name)=>{},
            _=>return Err(invalid(format!("Installazione non ancora supportata: {name}"))),
        }
    }
    if requirements.iter().any(|r|r["kind"]=="node" || r["name"]=="npm") && !requirements.iter().any(|r|r["kind"]=="command" && r["name"]=="node") {return Err(invalid("Versione Node.js sorgente mancante"));}
    requirements.sort_by_key(|item|match (item["kind"].as_str(),item["name"].as_str()) {(Some("command"),Some("node"))=>0,(Some("command"),_)=>1,(Some("node"),_)=>2,_=>3});
    Ok(requirements)
}
fn command_version(path: &Path, node_bin: Option<&Path>) -> io::Result<String> {
    let mut command=Command::new(path);command.arg("--version");
    if let Some(bin)=node_bin {command.env("PATH",format!("{}:{}",bin.display(),std::env::var("PATH").unwrap_or_default()));}
    Ok(crate::app_update::run(&mut command,Duration::from_secs(15))?.trim().trim_start_matches('v').to_owned())
}
fn manifest_ready(definition: &Value, item: &Value, done: usize, total: usize, notify: &mut impl FnMut(Value)) -> io::Result<()> {
    let manifest=Path::new(item["evidence"].as_str().unwrap());
    let deadline=Instant::now()+Duration::from_secs(1800);let mut waiting=false;
    loop {
        if setup::regular(manifest) && setup::regular(&manifest.with_file_name("package-lock.json")) {
            let lock:Value=serde_json::from_slice(&fs::read(manifest.with_file_name("package-lock.json"))?)?;
            if lock["packages"][format!("node_modules/{}",item["name"].as_str().unwrap())]["version"]==item["version"] {return Ok(());}
        }
        if definition["wait_for_paths"]!=true || Instant::now()>=deadline {return Err(invalid("Manifest o lock del progetto non disponibili nella versione richiesta"));}
        if !waiting {emit(notify,done,total,format!("APP WAIT MANIFEST {}",manifest.display()));waiting=true;}
        std::thread::sleep(Duration::from_millis(250));
    }
}
fn installed(item: &Value) -> bool {
    let Some(path)=item["evidence"].as_str().map(Path::new).and_then(Path::parent) else {return false;};
    let path=path.join("node_modules").join(item["name"].as_str().unwrap()).join("package.json");
    path.is_file() && fs::read(path).ok().and_then(|data|serde_json::from_slice::<Value>(&data).ok()).is_some_and(|p|p["version"]==item["version"])
}
fn install(definition: &Value, home: &Path, notify: &mut impl FnMut(Value)) -> io::Result<Value> {
    emit(notify,0,definition["requirements"].as_array().map_or(0,Vec::len),"APP INSTALL START".into());
    let requirements=validate(definition)?;
    let total=requirements.len();let mut done=0;let mut bin=None::<PathBuf>;let mut projects=BTreeSet::new();let mut results=Vec::new();
    for item in &requirements {
        let name=item["name"].as_str().unwrap();let version=item["version"].as_str().unwrap_or("");
        emit(notify,done,total,format!("APP CHECK {name} {version}"));
        let mut changed=false;
        match item["kind"].as_str().unwrap() {
            "command" if name=="node"=> {
                let nvm=std::env::var_os("NVM_DIR").map(PathBuf::from).filter(|p|p.starts_with(home)).unwrap_or_else(||home.join(".nvm"));
                let expected=nvm.join(format!("versions/node/v{version}/bin/node"));
                let existing=if expected.is_file() {Some(expected.clone())} else {setup::find("node")?};
                let selected=if existing.as_ref().is_some_and(|path|command_version(path,None).is_ok_and(|v|v==version)) {existing.unwrap()} else {
                    if !setup::regular(&nvm.join("nvm.sh")) {return Err(invalid("NVM non disponibile: preparare Node.js con run.sh"));}
                    let mut command=Command::new("bash");command.args(["-c",". \"$NVM_DIR/nvm.sh\" --no-use && nvm install -b \"$1\"","oaclis-node",version]).env("NVM_DIR",&nvm);
                    execute(&mut command,done,total,notify)?;changed=true;expected
                };
                if command_version(&selected,None)?!=version {return Err(invalid("Verifica Node.js non riuscita"));}
                bin=selected.parent().map(Path::to_owned);
            },
            "command"=> {
                let node_bin=bin.as_ref().ok_or_else(||invalid("Node.js non pronto"))?;let npm=node_bin.join("npm");
                if command_version(&npm,Some(node_bin)).ok().as_deref()!=Some(version) {
                    if !node_bin.starts_with(home.join(".nvm")) {return Err(invalid("npm richiede una installazione Node.js dell'utente"));}
                    let mut command=Command::new(&npm);command.args(["install","--global","--prefix"]).arg(node_bin.parent().unwrap()).arg(format!("npm@{version}"));
                    command.env("PATH",format!("{}:{}",node_bin.display(),std::env::var("PATH").unwrap_or_default()));
                    execute(&mut command,done,total,notify)?;changed=true;
                }
                if command_version(&npm,Some(node_bin))?!=version {return Err(invalid("Verifica npm non riuscita"));}
            },
            "node"=> {
                manifest_ready(definition,item,done,total,notify)?;
                let project=Path::new(item["evidence"].as_str().unwrap()).parent().unwrap();
                if !installed(item) && projects.insert(project.to_owned()) {
                    let node_bin=bin.as_ref().ok_or_else(||invalid("Node.js non pronto"))?;
                    let mut command=Command::new(node_bin.join("npm"));command.args(["ci","--include=dev","--no-audit","--no-fund"]).current_dir(project);
                    command.env("PATH",format!("{}:{}",node_bin.display(),std::env::var("PATH").unwrap_or_default()));
                    execute(&mut command,done,total,notify)?;changed=true;
                }
                if !installed(item) {return Err(invalid(format!("Verifica {name} non riuscita")));}
            },
            "skill"|"plugin"=> {
                let kind=item["kind"].as_str().unwrap();
                let paths=if kind=="skill" {vec![home.join(".agents/skills").join(name),setup::codex_root()?.join("skills").join(name)]} else {vec![home.join(".config/oaclis/plugins").join(name)]};
                if !paths.iter().any(|p|p.is_dir() && setup::unlinked(p)) {return Err(invalid(format!("File di {name} non disponibili")));}
            },
            _=>unreachable!(),
        }
        done+=1;emit(notify,done,total,format!("APP {} {name} {version} {done}/{total}",if changed {"INSTALLED"} else {"PRESENT"}));
        results.push(json!({"kind":item["kind"],"name":name,"version":version,"status":"installed"}));
    }
    let result=json!({"status":"completed","requirements":results,"node_bin":bin,"completed_at":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()});
    Ok(result)
}
pub(crate) fn saved(path: &Path, notify: &mut impl FnMut(Value)) -> io::Result<Value> {
    let home=setup::home()?;
    let directory=home.join(".config/oaclis/environments");
    if path.parent()!=Some(directory.as_path()) || !setup::regular(path) || path.metadata()?.len()>2*1024*1024 {return Err(invalid("Definizione installazione non valida"));}
    let mut definition:Value=serde_json::from_slice(&fs::read(path)?)?;
    let lock=directory.join("install.lock");
    if lock.symlink_metadata().is_ok_and(|m|!m.is_file()) {return Err(invalid("Lock installazione non valido"));}
    let lock=fs::OpenOptions::new().create(true).append(true).open(lock)?;
    lock.try_lock().map_err(|_|invalid("Un'installazione e' gia' in corso sul dispositivo"))?;
    let result=(|| -> io::Result<Value> {
        let installed=install(&definition,&home,notify)?;
        definition["installation"]=installed.clone();crate::staging::save_json(path,&definition)?;
        let total=definition["requirements"].as_array().map_or(0,Vec::len);
        emit(notify,total,total,"APP INSTALL COMPLETE".into());Ok(installed)
    })();
    if let Err(error)=&result {emit(notify,0,1,format!("APP INSTALL FAILED {}",crate::environment_analysis::redact(&error.to_string())));}
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn node_install_verifies_versions_reports_steps_and_skips_installed_packages() {
        let home=std::env::temp_dir().join(format!("oaclis-app-install-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let bin=home.join(".nvm/versions/node/v24.13.1/bin");let project=home.join("project");
        fs::create_dir_all(&bin).unwrap();fs::create_dir(&project).unwrap();
        fs::write(bin.join("node"),"#!/bin/sh\necho v24.13.1\n").unwrap();
        fs::write(bin.join("npm"),r#"#!/bin/sh
if [ "$1" = --version ]; then echo 11.12.1; exit 0; fi
if [ "$1" != ci ]; then exit 92; fi
printf 'ci\n' >> install-events
mkdir -p node_modules/test-library
printf '{"version":"1.2.3"}' > node_modules/test-library/package.json
"#).unwrap();
        for name in ["node","npm"] {fs::set_permissions(bin.join(name),fs::Permissions::from_mode(0o755)).unwrap();}
        fs::write(project.join("package.json"),r#"{"dependencies":{"test-library":"1.2.3"}}"#).unwrap();
        fs::write(project.join("package-lock.json"),r#"{"packages":{"node_modules/test-library":{"version":"1.2.3"}}}"#).unwrap();
        let definition=json!({"paths":[project],"requirements":[
            {"kind":"node","name":"test-library","version":"1.2.3","evidence":project.join("package.json")},
            {"kind":"command","name":"npm","version":"11.12.1"},
            {"kind":"command","name":"node","version":"24.13.1"}]});
        let mut events=Vec::new();let result=install(&definition,&home,&mut |event|events.push(event)).unwrap();
        assert_eq!(result["status"],"completed");
        assert_eq!(result["requirements"].as_array().unwrap().len(),3);
        assert!(events.iter().any(|e|e["message"].as_str().unwrap().contains("APP INSTALL START")));
        assert!(events.iter().any(|e|e["event"]["percent"].as_u64()==Some(33)));
        assert!(events.iter().any(|e|e["event"]["percent"].as_u64()==Some(66)));
        assert!(events.iter().any(|e|e["message"].as_str().unwrap().contains("APP INSTALLED test-library")));
        install(&definition,&home,&mut |_|{}).unwrap();
        assert_eq!(fs::read_to_string(project.join("install-events")).unwrap(),"ci\n");
        fs::remove_file(project.join("node_modules/test-library/package.json")).unwrap();
        fs::write(bin.join("npm"),"#!/bin/sh\nif [ \"$1\" = --version ]; then echo 11.12.1; else exit 42; fi\n").unwrap();
        assert!(install(&definition,&home,&mut |_|{}).is_err());
        let mut invalid=definition.clone();invalid["requirements"][0]["name"]=json!("../escape");
        assert!(validate(&invalid).is_err());
        invalid=definition.clone();invalid["requirements"][0]["evidence"]=json!(project.join("../elsewhere/package.json"));assert!(validate(&invalid).is_err());
        invalid=definition.clone();invalid["requirements"][0]["kind"]=json!("python");assert!(validate(&invalid).is_err());
        fs::remove_dir_all(home).unwrap();
    }
}
