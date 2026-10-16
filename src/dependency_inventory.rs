// dependency_inventory.rs v1.29.341
//! Inspect source files and installed metadata without importing project code.
use crate::{environment_setup as setup,invalid};
use regex::Regex;
use rustpython_parser::{ast,Parse};
use rustpython_ast::Visitor;
use serde_json::{Value,json};
use std::{collections::{BTreeMap,BTreeSet},env,fs,io,path::{Path,PathBuf},process::Command,sync::LazyLock,time::Duration};
const COMMANDS:&[(&str,&str)]=&[("python3","Python 3"),("node","Node.js"),("npm","npm"),("cargo","Cargo"),("rustc","Rust"),("php","PHP"),("composer","Composer"),("ruby","Ruby"),("bundle","Bundler"),("go","Go"),("docker","Docker"),("apache2ctl","Apache"),("nginx","Nginx"),("pdftoppm","Poppler"),("pdftotext","Poppler"),("ffmpeg","FFmpeg"),("mutool","MuPDF"),("potrace","Potrace"),("resvg","resvg"),("tesseract","Tesseract"),("gs","Ghostscript"),("sqlite3","SQLite CLI"),("chromium","Chromium")];
static VERSION:LazyLock<Regex>=LazyLock::new(||Regex::new(r"\d+(?:\.\d+){1,3}").unwrap());
static PORT:LazyLock<Regex>=LazyLock::new(||Regex::new(r"https?://(?:localhost|127\.0\.0\.1|\[::1\]):(\d+)").unwrap());
fn command_label(name:&str)->Option<&'static str>{COMMANDS.iter().find(|(n,_)|*n==name).map(|(_,label)|*label)}
fn identifier(name:&str)->bool{!name.is_empty() && name.bytes().enumerate().all(|(i,b)|b.is_ascii_alphabetic() || b==b'_' || (i>0 && b.is_ascii_digit()))}
fn library(name:&str,node:bool)->bool{
    if name=="." || name==".." || name.is_empty(){return false;}
    if node && name.starts_with('@'){let parts:Vec<_>=name[1..].split('/').collect();return parts.len()==2 && parts.iter().all(|p|library(p,false));}
    name.bytes().all(|b|b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
fn read(path:&Path)->io::Result<String>{if !setup::regular(path) || path.metadata()?.len()>131072{return Err(invalid("File di dipendenza non regolare o troppo grande"));}fs::read_to_string(path).map_err(io::Error::other)}
#[derive(Default)]
struct PythonSource{imports:BTreeSet<String>,commands:BTreeSet<String>,models:BTreeSet<String>,version:Option<String>}
impl Visitor for PythonSource{
    fn visit_stmt_import(&mut self,node:ast::StmtImport){for name in node.names{self.imports.insert(name.name.as_str().split('.').next().unwrap().into());}}
    fn visit_stmt_import_from(&mut self,node:ast::StmtImportFrom){if node.level.is_none_or(|n|n.to_u32()==0){if let Some(name)=node.module{self.imports.insert(name.as_str().split('.').next().unwrap().into());}}}
    fn visit_expr_call(&mut self,node:ast::ExprCall){
        if let ast::Expr::Attribute(function)=node.func.as_ref(){if let ast::Expr::Name(name)=function.value.as_ref(){
            if name.id.as_str()=="subprocess" && ["run","Popen","check_output","check_call"].contains(&function.attr.as_str()){
                let args=match node.args.first(){Some(ast::Expr::List(args))=>Some(&args.elts),Some(ast::Expr::Tuple(args))=>Some(&args.elts),_=>None};
                if let Some(ast::Expr::Constant(c))=args.and_then(|v|v.first()){if let ast::Constant::Str(name)=&c.value{if let Some(name)=Path::new(name).file_name().and_then(|s|s.to_str()){if command_label(name).is_some(){self.commands.insert(name.into());}}}}
            }
        }}self.generic_visit_expr_call(node);
    }
    fn visit_expr_constant(&mut self,node:ast::ExprConstant){if let ast::Constant::Str(text)=node.value{if {static MODEL:LazyLock<Regex>=LazyLock::new(||Regex::new(r"^(?:en|it)_core_(?:web|news)_[a-z]+$").unwrap());MODEL.is_match(&text)}{self.models.insert(text);}}}
    fn visit_stmt_assign(&mut self,node:ast::StmtAssign){
        if node.targets.iter().any(|t|matches!(t,ast::Expr::Name(n) if n.id.as_str()=="__version__")){if let ast::Expr::Constant(c)=node.value.as_ref(){if let ast::Constant::Str(v)=&c.value{self.version=Some(v.clone());}}}self.generic_visit_stmt_assign(node);
    }
}
fn python_source(text:&str)->Option<PythonSource>{let tree=ast::Suite::parse(text,"<inventory>").ok()?;let mut result=PythonSource::default();for node in tree{result.visit_stmt(node);}Some(result)}
#[derive(Default)]
struct PythonMetadata{versions:BTreeMap<String,(String,String)>,modules:BTreeMap<String,(String,String)>,paths:Vec<PathBuf>,stdlib:BTreeSet<String>}
fn metadata(roots:&[PathBuf])->io::Result<PythonMetadata>{
    let mut result=PythonMetadata::default();let home=setup::home()?;let mut stdlib=Vec::new();
    for base in [PathBuf::from("/usr/lib"),PathBuf::from("/usr/local/lib"),home.join(".local/lib")]{for path in setup::entries(&base)?{if path.file_name().is_some_and(|n|n.to_string_lossy().starts_with("python3")) && path.is_dir(){stdlib.push(path.clone());result.paths.extend([path.join("site-packages"),path.join("dist-packages")]);}}}
    result.paths.push("/usr/lib/python3/dist-packages".into());
    let framework=Path::new("/Library/Frameworks/Python.framework/Versions");for version in setup::entries(framework)?{for path in setup::entries(&version.join("lib"))?{if path.is_dir() && path.file_name().is_some_and(|n|n.to_string_lossy().starts_with("python3")){stdlib.push(path.clone());result.paths.push(path.join("site-packages"));}}}
    if let Some(paths)=env::var_os("PYTHONPATH"){result.paths.extend(env::split_paths(&paths).filter(|p|p.is_absolute() && setup::unlinked(p)));}
    for root in roots{for base in [root.join(".venv"),root.join("venv"),root.parent().unwrap().join(".venv"),root.parent().unwrap().join(format!(".venv-{}",root.file_name().unwrap().to_string_lossy()))]{if !setup::unlinked(&base){continue;}for python in setup::entries(&base.join("lib"))?{if python.file_name().is_some_and(|n|n.to_string_lossy().starts_with("python")){result.paths.push(python.join("site-packages"));}}}}
    for directory in stdlib{for path in setup::entries(&directory)?.into_iter().chain(setup::entries(&directory.join("lib-dynload"))?){if let Some(name)=path.file_name().and_then(|s|s.to_str()){let module=name.split('.').next().unwrap_or("");if identifier(module){result.stdlib.insert(module.into());}}}}
    let mut seen=BTreeSet::new();result.paths.retain(|p|seen.insert(p.clone()));
    for base in &result.paths{if !setup::unlinked(base){continue;}for dist in setup::entries(base)?{
        let suffix=dist.extension().and_then(|s|s.to_str()).unwrap_or("");if !["dist-info","egg-info"].contains(&suffix) || !dist.is_dir() || !setup::unlinked(&dist){continue;}
        let path=if dist.join("METADATA").is_file(){dist.join("METADATA")}else{dist.join("PKG-INFO")};if !setup::regular(&path){continue;}
        let data=fs::read_to_string(path)?;let name=data.lines().find_map(|l|l.strip_prefix("Name: ")).unwrap_or("");let version=data.lines().find_map(|l|l.strip_prefix("Version: ")).unwrap_or("");if name.is_empty(){continue;}
        let value=(name.to_owned(),version.to_owned());result.versions.insert(name.to_lowercase().replace('_',"-"),value.clone());
        let top=dist.join("top_level.txt");let mut modules=if setup::regular(&top){fs::read_to_string(top)?.lines().map(str::to_owned).collect::<Vec<_>>()}else{vec![]};
        let record=dist.join("RECORD");if modules.is_empty() && setup::regular(&record){modules.extend(fs::read_to_string(record)?.lines().filter_map(|l|l.split(',').next()).filter_map(|l|l.split('/').next()).map(|s|s.trim_end_matches(".py").to_owned()));}
        if modules.is_empty(){modules.push(name.replace('-',"_"));}
        for module in modules{if identifier(&module){result.modules.insert(module,value.clone());}}
    }}Ok(result)
}
fn builtin(name:&str,py:&PythonMetadata)->bool{
    ["sys","builtins","_thread","_io","_abc","_ast","_imp","_warnings","_weakref","marshal","posix","nt","time","gc","itertools","atexit","errno","pwd","grp","math"].contains(&name)
        || py.stdlib.contains(name)
}
fn python_version(name:&str,module:&str,py:&PythonMetadata)->io::Result<Option<String>>{
    if !library(name,false) || !identifier(module){return Err(invalid("Libreria Python non valida"));}
    if let Some((_,version))=py.versions.get(&name.to_lowercase().replace('_',"-")){return Ok(Some(version.clone()));}
    for base in &py.paths{
        for path in [base.join(format!("{module}.py")),base.join(module).join("__init__.py")]{if setup::regular(&path){let source=python_source(&read(&path)?);return Ok(Some(source.and_then(|v|v.version).unwrap_or_default()));}}
        if setup::entries(base)?.iter().any(|p|p.file_name().is_some_and(|n|n.to_string_lossy().starts_with(&format!("{module}.")) && n.to_string_lossy().ends_with(".so"))){return Ok(Some(String::new()));}
    }Ok(None)
}
fn version(name:&str)->io::Result<Option<String>>{
    let Some(command)=setup::find(name)? else{return Ok(None);};
    let option=match name{"ffmpeg"|"sqlite3"=>"-version","pdftoppm"|"pdftotext"|"apache2ctl"|"nginx"=>"-v",_=>"--version"};
    let text=crate::app_update::run(Command::new(command).arg(option).env("PATH",env::join_paths(setup::bins()?).map_err(io::Error::other)?),Duration::from_secs(3)).unwrap_or_default();
    Ok(Some(VERSION.find(text.lines().next().unwrap_or("")).map(|m|m.as_str().to_owned()).unwrap_or_default()))
}
fn service_info(port:u16)->io::Result<Option<Value>>{
    if !Path::new("/proc").is_dir(){return Ok(None);}let Some(ss)=setup::find("ss")? else{return Ok(None);};
    let text=crate::ssh_settings::text(Command::new(ss).args(["-ltnp",&format!("sport = :{port}")]),3).unwrap_or_default();
    static PID:LazyLock<Regex>=LazyLock::new(||Regex::new(r"pid=(\d+)").unwrap());let Some(pid)=PID.captures(&text)else{return Ok(None);};
    let process=Path::new("/proc").join(&pid[1]);let cwd=match process.join("cwd").canonicalize(){Ok(p)=>p,Err(_)=>return Ok(None)};let args=fs::read(process.join("cmdline")).unwrap_or_default();
    for base in cwd.ancestors(){let path=base.join("package.json");if setup::regular(&path){let value:Value=serde_json::from_str(&read(&path)?)?;return Ok(Some(json!({"name":value["name"].as_str().unwrap_or(base.file_name().and_then(|s|s.to_str()).unwrap_or("")),"version":value["version"].as_str().unwrap_or(""),"source_root":base,"runtime":if args.split(|b|*b==0).next().is_some_and(|a|a.windows(4).any(|s|s==b"node")){"node"}else{""}})));}}
    Ok(None)
}
fn files(root:&Path)->io::Result<Vec<PathBuf>>{
    Ok(setup::walk(root,2000,|p|{let n=p.file_name().unwrap().to_string_lossy();!["test_","test-","check_","check-"].iter().any(|s|n.starts_with(s)) && (p.extension().is_some_and(|e|e=="py") || ["package.json","Cargo.toml","composer.json","Gemfile","go.mod","requirements.txt","pyproject.toml"].contains(&n.as_ref()))})?.into_iter().filter(|p|p.metadata().is_ok_and(|m|m.len()<=131072)).collect())
}
fn add_command(items:&mut BTreeMap<(String,String),Value>,name:&str,evidence:&Path)->io::Result<()>{
    let key=("command".into(),name.into());if !items.contains_key(&key){if let Some(version)=version(name)?{items.insert(key,json!({"kind":"command","name":name,"label":command_label(name).ok_or_else(||invalid("Comando non supportato"))?,"evidence":evidence,"version":version}));}else{items.insert(key,Value::Null);}}Ok(())
}
fn add_python(items:&mut BTreeMap<(String,String),Value>,module:&str,evidence:&Path,py:&PythonMetadata)->io::Result<()>{
    let package=py.modules.get(module).map(|(n,_)|n.as_str()).unwrap_or(module);let key=("python".into(),package.into());if items.contains_key(&key){return Ok(());}if let Some(version)=python_version(package,module,py)?{items.insert(key,json!({"kind":"python","name":package,"label":match module{"spacy"=>"spaCy","PIL"=>"Pillow",_=>package},"module":module,"evidence":evidence,"version":version}));}else{items.insert(key,Value::Null);}Ok(())
}
pub(crate) fn source(value:&Value)->io::Result<Value>{
    let roots=value["paths"].as_array().ok_or_else(||invalid("Percorsi assenti"))?.iter().map(|p|{let p=PathBuf::from(p.as_str().ok_or_else(||invalid("Percorso non valido"))?);if !setup::project(&p){return Err(invalid("Radice progetto non valida"));}Ok(p)}).collect::<io::Result<Vec<_>>>()?;
    let mut items=BTreeMap::new();let mut ports=BTreeMap::new();let mut cached=BTreeMap::new();for root in &roots{cached.insert(root.clone(),files(root)?);}
    let py=if cached.values().flatten().any(|p|p.extension().is_some_and(|e|e=="py")){metadata(&roots)?}else{PythonMetadata::default()};
    for root in &roots{
        let local=setup::entries(root)?.into_iter().filter_map(|p|if p.is_dir(){p.file_name().map(|s|s.to_string_lossy().into_owned())}else if p.extension().is_some_and(|e|e=="py"){p.file_stem().map(|s|s.to_string_lossy().into_owned())}else{None}).collect::<BTreeSet<_>>();
        for path in &cached[root]{
            let name=path.file_name().unwrap().to_string_lossy();
            if path.extension().is_some_and(|e|e=="py"){
                add_command(&mut items,"python3",path)?;let text=read(path)?;for capture in PORT.captures_iter(&text){if let Ok(port)=capture[1].parse::<u16>(){if port!=0{ports.insert(port,path.clone());}}}
                let Some(parsed)=python_source(&text)else{continue;};let locals=setup::entries(path.parent().unwrap())?.into_iter().filter_map(|p|p.file_stem().map(|s|s.to_string_lossy().into_owned())).collect::<BTreeSet<_>>();
                for module in parsed.imports{if !builtin(&module,&py) && !local.contains(&module) && !locals.contains(&module){add_python(&mut items,&module,path,&py)?;}}
                for command in parsed.commands{add_command(&mut items,&command,path)?;}for model in parsed.models{add_python(&mut items,&model,path,&py)?;}
            }else if name=="package.json"{
                add_command(&mut items,"node",path)?;add_command(&mut items,"npm",path)?;let package:Value=serde_json::from_str(&read(path)?)?;
                for deps in ["dependencies","devDependencies"]{for dependency in package[deps].as_object().into_iter().flat_map(|v|v.keys()){
                    if !library(dependency,true){return Err(invalid("Libreria Node non valida"));}let key=("node".into(),dependency.clone());if items.contains_key(&key){continue;}
                    for root in &roots{for base in std::iter::once(root.as_path()).chain(cached[root].iter().filter(|p|p.file_name().is_some_and(|n|n=="package.json")).filter_map(|p|p.parent())){
                        let installed=base.join("node_modules").join(dependency).join("package.json");if installed.is_file() && !installed.symlink_metadata()?.file_type().is_symlink(){if installed.metadata()?.len()>131072{return Err(invalid("Manifest Node troppo grande"));}let data:Value=serde_json::from_str(&fs::read_to_string(&installed)?)?;items.insert(key.clone(),json!({"kind":"node","name":dependency,"label":dependency,"evidence":path,"version":data["version"].as_str().unwrap_or("")}));break;}
                    }if items.contains_key(&key){break;}}
                }}
            }else{let commands:&[&str]=match name.as_ref(){"Cargo.toml"=>&["cargo","rustc"],"composer.json"=>&["php","composer"],"Gemfile"=>&["ruby","bundle"],"go.mod"=>&["go"],"requirements.txt"|"pyproject.toml"=>&["python3"],_=>&[]};for command in commands{add_command(&mut items,command,path)?;}}
        }
    }
    for (port,evidence) in ports{if let Some(info)=service_info(port)?{let path=Path::new(info["source_root"].as_str().unwrap());let name=info["name"].as_str().unwrap();if !roots.iter().any(|root|path.starts_with(root)){
        let mut item=info.clone();item["kind"]=json!("service");item["label"]=json!(if name=="openwa"{"OpenWA"}else{name});item["port"]=json!(port);item["evidence"]=json!(evidence);items.insert(("service".into(),name.into()),item);
    }if info["runtime"]=="node"{add_command(&mut items,"node",&evidence)?;add_command(&mut items,"npm",&evidence)?;}}}
    for config in value["configs"].as_array().into_iter().flatten(){let command=match config["service"].as_str(){Some("apache2")=>"apache2ctl",Some("nginx")=>"nginx",_=>return Err(invalid("Servizio non supportato"))};add_command(&mut items,command,Path::new(config["path"].as_str().ok_or_else(||invalid("Configurazione senza percorso"))?))?;}
    for plugin in value["plugins"].as_array().into_iter().flatten(){let path=Path::new(plugin["path"].as_str().ok_or_else(||invalid("Plugin senza percorso"))?);if path.is_dir() && setup::unlinked(path){let name=plugin["name"].as_str().ok_or_else(||invalid("Nome plugin assente"))?;let kind=plugin["kind"].as_str().ok_or_else(||invalid("Tipo plugin assente"))?;items.insert((kind.into(),name.into()),json!({"kind":kind,"name":name,"label":name,"version":"","evidence":path,"source_path":path}));}}
    let mut paths=roots;paths.extend(items.values().filter(|p|p["kind"]=="service").filter_map(|p|p["source_root"].as_str()).map(PathBuf::from));Ok(json!({"paths":setup::minimal(paths),"requirements":items.into_values().filter(|v|!v.is_null()).collect::<Vec<_>>()}))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn step5_inventory_reads_real_metadata_without_importing_or_installing(){
        let root=env::temp_dir().join(format!("oaclis-inventory-step5-{}",std::process::id()));fs::create_dir(&root).unwrap();let app=root.join("app");fs::create_dir(&app).unwrap();let marker=root.join("executed");
        fs::write(app.join("server.py"),format!("import json, sqlite3\nimport fixture_dependency\nfrom pathlib import Path\nPath({:?}).touch()\n# ChatGPT Windows Android Browser Codex\n",marker.to_str().unwrap())).unwrap();
        fs::write(app.join("check-browser.py"),"import playwright\n").unwrap();
        let dist=app.join(".venv/lib/python3.12/site-packages/fixture_dependency-3.8.16.dist-info");fs::create_dir_all(&dist).unwrap();fs::write(dist.join("METADATA"),"Name: fixture-dependency\nVersion: 3.8.16\n").unwrap();fs::write(dist.join("top_level.txt"),"fixture_dependency\n").unwrap();
        fs::write(app.join("package.json"),r#"{"dependencies":{"library-one":"^2","library-two":"^1"},"scripts":{"postinstall":"must not execute"}}"#).unwrap();let installed=app.join("node_modules/library-one");fs::create_dir_all(&installed).unwrap();fs::write(installed.join("package.json"),r#"{"version":"2.4.1"}"#).unwrap();
        let store=root.join("package-store");fs::create_dir(&store).unwrap();fs::write(store.join("package.json"),r#"{"version":"1.2.3"}"#).unwrap();std::os::unix::fs::symlink(&store,app.join("node_modules/library-two")).unwrap();
        let result=source(&json!({"paths":[app],"configs":[],"plugins":[]})).unwrap();let items=result["requirements"].as_array().unwrap();
        assert!(items.iter().any(|v|v["name"]=="fixture-dependency" && v["version"]=="3.8.16"));assert!(items.iter().any(|v|v["name"]=="library-one" && v["version"]=="2.4.1"));
        assert!(items.iter().any(|v|v["name"]=="library-two" && v["version"]=="1.2.3"));
        assert!(!items.iter().any(|v|["json","sqlite3","pathlib","playwright","ChatGPT","Browser"].iter().any(|name|v["name"]==*name)));assert!(!marker.exists());
        assert_eq!(result["paths"],json!([app]));assert!(!library("../private",true));
        let parsed=python_source("def work():\n import fixture_dependency\n subprocess.run(['ffmpeg','-version'])\n model='it_core_news_sm'\n").unwrap();assert!(parsed.imports.contains("fixture_dependency"));assert!(parsed.commands.contains("ffmpeg"));assert!(parsed.models.contains("it_core_news_sm"));
        fs::remove_dir_all(root).unwrap();
    }
}
