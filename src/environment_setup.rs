// environment_setup.rs v1.20.272
//! Read-only environment identity, path policy and dependency fingerprints.
use crate::{environment::Fingerprint,invalid,paths::validate_absolute};
use serde_json::{Value,json};
use std::{env,fs,io,os::unix::fs::{MetadataExt,PermissionsExt},path::{Path,PathBuf}};

pub(crate) const MANIFESTS:&[&str]=&["Cargo.toml","package.json","requirements.txt","requirements.lock.txt","pyproject.toml","composer.json","Gemfile","go.mod","Dockerfile","docker-compose.yml","compose.yaml"];
pub(crate) const SKIP:&[&str]=&[".git",".ssh",".codex",".agents",".config",".cache",".env",".venv","venv","node_modules","target","__pycache__","backups","primary","specialist","_imports","_experiments"];
pub(crate) fn home()->io::Result<PathBuf> {let home=env::var_os("HOME").map(PathBuf::from).ok_or_else(||invalid("HOME non definita"))?;validate_absolute(&home)?;Ok(home)}
pub(crate) fn codex_root()->io::Result<PathBuf> {
    let root=env::var_os("CODEX_HOME").map(PathBuf::from).unwrap_or(home()?.join(".codex"));
    Ok(if root.is_absolute(){root}else{env::current_dir()?.join(root)})
}
pub(crate) fn identity()->io::Result<Value> {Ok(json!({"home":home()?,"codex_root":codex_root()?}))}
pub(crate) fn bins()->io::Result<Vec<PathBuf>> {
    let home=home()?;let mut paths=vec![home.join(".cargo/bin"),home.join(".local/bin")];
    let mut nodes=entries(&home.join(".nvm/versions/node"))?;
    nodes.sort_by_key(|p|std::cmp::Reverse(p.metadata().and_then(|m|m.modified()).ok()));
    paths.extend(nodes.into_iter().map(|p|p.join("bin")));
    paths.extend(env::split_paths(&env::var_os("PATH").unwrap_or_else(||"/usr/local/bin:/usr/bin:/bin".into())));Ok(paths)
}
pub(crate) fn find(name:&str)->io::Result<Option<PathBuf>> {Ok(bins()?.into_iter().map(|p|p.join(name)).find(|p|p.metadata().is_ok_and(|m|m.is_file() && m.permissions().mode()&0o111!=0)))}
pub(crate) fn entries(path:&Path)->io::Result<Vec<PathBuf>> {
    let mut result=match fs::read_dir(path) {Ok(entries)=>entries.map(|e|e.map(|e|e.path())).collect::<io::Result<Vec<_>>>()?,Err(e) if matches!(e.kind(),io::ErrorKind::NotFound|io::ErrorKind::PermissionDenied)=>return Ok(vec![]),Err(e)=>return Err(e)};result.sort();Ok(result)
}
pub(crate) fn usable(path:&Path)->bool {
    let parts:Vec<_>=path.components().collect();
    let windows=parts.get(1).is_some_and(|p|p.as_os_str()=="mnt") && parts.get(2).is_some_and(|p|{let b=p.as_os_str().as_encoded_bytes();b.len()==1 && b[0].is_ascii_alphabetic()});
    !windows && path.as_os_str().as_encoded_bytes().len()<4096 && parts.iter().all(|p|p.as_os_str().as_encoded_bytes().len()<=255)
}
pub(crate) fn unlinked(path:&Path)->bool {
    usable(path) && path.ancestors().all(|p|match p.symlink_metadata(){Ok(m)=>!m.file_type().is_symlink(),Err(e)=>e.kind()==io::ErrorKind::NotFound})
}
pub(crate) fn regular(path:&Path)->bool {unlinked(path) && path.is_file()}
pub(crate) fn project(path:&Path)->bool {
    if validate_absolute(path).is_err() || !unlinked(path) || path.components().count()<3 || ["/var/www","/var/www/html"].iter().any(|p|path==Path::new(p))
        || home().is_ok_and(|h|path==h) || path.parent()==Some(Path::new("/home")) {return false;}
    if ["/etc","/proc","/sys","/dev","/run","/usr","/bin","/lib","/boot"].iter().any(|p|path.starts_with(p)) {return false;}
    if path.components().any(|p|[".ssh",".codex",".agents",".config",".cache",".local",".git",".aws",".npm",".nvm",".cargo",".rustup",".venv","venv","node_modules","target","site-packages","__pycache__"].iter().any(|n|p.as_os_str()==*n)) {return false;}
    path.is_dir()
}
pub(crate) fn minimal(mut paths:Vec<PathBuf>)->Vec<PathBuf> {paths.sort();paths.dedup();let all=paths.clone();paths.retain(|p|!all.iter().any(|a|a!=p && p.starts_with(a)));paths}
pub(crate) fn walk(root:&Path,limit:usize,include:impl Fn(&Path)->bool)->io::Result<Vec<PathBuf>> {
    let mut pending=vec![(root.to_owned(),0)];let mut files=Vec::new();
    while let Some((directory,depth))=pending.pop(){
        for path in entries(&directory)? {
            let meta=path.symlink_metadata()?;if meta.file_type().is_symlink(){continue;}
            if meta.is_dir(){if depth<5 && !SKIP.iter().any(|n|path.file_name().is_some_and(|v|v==*n)){pending.push((path,depth+1));}}
            else if meta.is_file() && include(&path){files.push(path);if files.len()>=limit{return Ok(files);}}
        }
    }Ok(files)
}
pub(crate) fn fingerprint(path:&Path)->io::Result<Fingerprint> {
    validate_absolute(path)?;
    if !unlinked(path){return Err(invalid("Manifest dipendenze non regolare"));}
    let metadata=match path.metadata(){Ok(m)=>Some(m),Err(e) if e.kind()==io::ErrorKind::NotFound=>None,Err(e)=>return Err(e)};
    if metadata.as_ref().is_some_and(|m|!m.is_file()){return Err(invalid("Manifest dipendenze non regolare"));}
    Ok(Fingerprint{path:path.into(),bytes:metadata.as_ref().map(|m|m.len()),modified_ns:metadata.as_ref().map(|m|(m.mtime() as i128*1_000_000_000+m.mtime_nsec() as i128).to_string())})
}
pub(crate) fn fresh(items:&[Fingerprint])->io::Result<bool> {
    for item in items {let actual=match fingerprint(&item.path){Ok(v)=>v,Err(_)=>return Ok(false)};if actual.bytes!=item.bytes || actual.modified_ns!=item.modified_ns{return Ok(false);}}Ok(true)
}
