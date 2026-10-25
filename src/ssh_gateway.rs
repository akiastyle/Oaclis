// ssh_gateway.rs v1.21.575
//! Forced SSH entry point: native Oaclis requests and rsync server mode, never a remote shell.
use crate::{invalid, paths::validate_absolute, transfer::{self, Receipt}};
use serde_json::Value;
use std::{env, fs, io::{self, Read, Write}, os::unix::fs::{DirBuilderExt, PermissionsExt}, path::{Path, PathBuf}, process::Command};

fn arguments(command: &str) -> io::Result<Vec<&str>> {
    let words: Vec<_> = command.split_whitespace().collect();
    let flags = words.last().copied().unwrap_or("");
    let server = words.len() == 3 && words[..2] == ["rsync", "--server"]
        || words.len() == 4 && words[..3] == ["rsync", "--server", "--sender"];
    if !server || !flags.starts_with("-s") || flags.len() > 64
        || !flags[2..].bytes().all(|b| b"stpcrlORe.iLsfxCIvu".contains(&b)) {
        return Err(invalid("Comando SSH non consentito"));
    }
    Ok(words)
}

pub fn run() -> io::Result<()> {
    // Use the configured SSH account; command restrictions also apply to root.
    let identity = Command::new("id").arg("-u").output()?;
    if identity.status.success() {
        match env::var("SSH_ORIGINAL_COMMAND").unwrap_or_default().as_str() {
            "$HOME/.local/bin/oaclis state-request" => return crate::state_transport::forward(&PathBuf::from(env::var_os("HOME").ok_or_else(||invalid("HOME non definita"))?)),
            "$HOME/.local/bin/oaclis environment-request" => return crate::environment::forward(),
            "$HOME/.local/bin/oaclis file-request" => return files(),
            "true" => return Ok(()),
            command => {
                let words = arguments(command)?;
                let status = Command::new("rsync").args(&words[1..]).status()?;
                return if status.success() { Ok(()) } else { Err(io::Error::other("Trasferimento rsync non completato")) };
            }
        }
    }
    Err(invalid("Impossibile identificare l'utente del canale SSH"))
}

fn path<'a>(value: &'a Value, key: &str) -> io::Result<&'a Path> {
    let path = Path::new(value[key].as_str().ok_or_else(|| invalid("Percorso richiesto"))?);
    validate_absolute(path)?;
    Ok(path)
}
fn stage(path: &Path) -> io::Result<()> {
    let suffix = path.file_name().and_then(|s|s.to_str()).and_then(|s|s.strip_prefix(".oaclis-"))
        .filter(|s|s.len()==10 && s.bytes().all(|b|b.is_ascii_alphanumeric()));
    if suffix.is_none() || !path.symlink_metadata()?.is_dir() { return Err(invalid("Cartella temporanea non valida")); }
    Ok(())
}
fn receipt(path: &Path, expected: &Receipt) -> io::Result<()> {
    let (mut file, metadata) = crate::open_regular(path)?;
    let actual = transfer::copy_hash(&mut file, &mut io::sink())?;
    crate::check_unchanged(&file, &metadata)?;
    if &actual != expected { return Err(invalid("File modificato dopo il confronto")); }
    Ok(())
}
fn execute(value: &Value) -> io::Result<Vec<u8>> {
    let previous = || serde_json::from_value::<Receipt>(value["previous"].clone()).map_err(io::Error::other);
    match value["operation"].as_str() {
        Some("cleanup") => {
            let directory = path(value, "stage")?; stage(directory)?; fs::remove_dir_all(directory)?; Ok(vec![])
        },
        Some("mkdir") => { crate::rules::ensure_local(path(value,"target")?)?; Ok(vec![]) },
        Some("list") => {
            let output = Command::new("find").arg("--").arg(path(value,"target")?).args(["-printf", "%y\\0%P\\0"]).output()?;
            if !output.status.success() { return Err(io::Error::other("Lettura cartella non riuscita")); } Ok(output.stdout)
        },
        Some("chmod") => {
            let target = path(value,"target")?;
            crate::open_regular(target)?;
            let mode = value["mode"].as_u64().filter(|n|*n<=0o777).ok_or_else(||invalid("Permessi non validi"))?;
            fs::set_permissions(target,fs::Permissions::from_mode(mode as u32))?; Ok(vec![])
        },
        Some("stage" | "inspect") => {
            let target = path(value,"target")?;
            for parent in target.parent().ok_or_else(||invalid("Cartella mancante"))?.ancestors() {
                if !parent.symlink_metadata()?.is_dir() { return Err(invalid("Cartella non regolare")); }
            }
            let exists = match target.symlink_metadata() { Ok(_) => true, Err(e) if e.kind()==io::ErrorKind::NotFound => false, Err(e)=>return Err(e) };
            if value["operation"] == "inspect" {
                if !exists { return Ok(b"missing\n".to_vec()); }
                let (mut file,metadata)=crate::open_regular(target)?;
                let proof=transfer::copy_hash(&mut file,&mut io::sink())?; crate::check_unchanged(&file,&metadata)?;
                return Ok(format!("{} -\n{}\n{:o}\n", proof.sha256,proof.bytes,metadata.permissions().mode() & 0o777).into_bytes());
            }
            if value["previous"].is_null() { if exists { return Err(invalid("Il file esiste gia'")); } }
            else { receipt(target,&previous()?)?; }
            let mut bytes=[0u8;10];fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
            let suffix:String=bytes.iter().map(|b|b"abcdefghijklmnopqrstuvwxyz0123456789"[(*b as usize)%36] as char).collect();
            let directory=target.parent().unwrap().join(format!(".oaclis-{suffix}"));
            fs::DirBuilder::new().mode(0o700).create(&directory)?;
            Ok(format!("{}\n",directory.display()).into_bytes())
        },
        Some("seed") => {
            let target=path(value,"target")?;let directory=path(value,"stage")?;stage(directory)?;
            let previous=previous()?;let saved=directory.join("previous");
            fs::copy(target,&saved)?;receipt(&saved,&previous)?;fs::copy(saved,directory.join("file"))?;Ok(vec![])
        },
        Some("publish") => {
            let target=path(value,"target")?;let staged=path(value,"staged")?;
            if staged.file_name().is_none_or(|name|name!="file") { return Err(invalid("File temporaneo non valido")); }
            stage(staged.parent().ok_or_else(||invalid("Cartella mancante"))?)?;
            let previous=if value["previous"].is_null(){None}else{Some(previous()?)};
            let incoming:Receipt=serde_json::from_value(value["incoming"].clone()).map_err(io::Error::other)?;
            let script=transfer::publish_command(target.to_str().ok_or_else(||invalid("Percorso non UTF-8"))?,staged.to_str().ok_or_else(||invalid("Percorso non UTF-8"))?,previous.as_ref(),&incoming)?;
            if !Command::new("sh").args(["-c",&script]).status()?.success(){return Err(io::Error::other("Pubblicazione file non riuscita"));} Ok(vec![])
        },
        _=>Err(invalid("Operazione file non consentita")),
    }
}
pub fn files() -> io::Result<()> {
    let mut bytes=Vec::new();io::stdin().take(65537).read_to_end(&mut bytes)?;
    if bytes.len()>65536{return Err(invalid("Richiesta file troppo grande"));}
    let value:Value=serde_json::from_slice(&bytes)?;
    io::stdout().write_all(&execute(&value)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn gateway_rejects_shells_subsystems_and_rsync_command_injection() {
        for command in ["", "sh", "bash -c id", "id", "sftp", "rsync --daemon", "rsync --server -s;id", "rsync --server -s$(id)", "rsync --server -sflags . /", "rsync --server --config=/tmp/evil -sflags", "rsync --server -e/bin/sh", "rsync --server -sflags && id"] {
            assert!(arguments(command).is_err(), "{command}");
        }
        for command in ["rsync --server -stpce.LsfxCIvu", "rsync --server --sender -stpce.LsfxCIvu", "rsync --server -slOtrRe.iLsfxCIvu"] {
            assert!(arguments(command).is_ok(), "{command}");
        }
    }
    #[test]
    fn native_file_operations_publish_verify_and_clean_only_a_stage() {
        let root=env::temp_dir().join(format!("oaclis-gateway-{}",std::process::id()));
        fs::create_dir(&root).unwrap();
        let target=root.join("name'$(command).txt");
        let directory=String::from_utf8(execute(&json!({"operation":"stage","target":target})).unwrap()).unwrap();
        let directory=PathBuf::from(directory.trim());
        let source=directory.join("file");fs::write(&source,"fixture").unwrap();
        let proof=transfer::copy_hash(&mut fs::File::open(&source).unwrap(),&mut io::sink()).unwrap();
        assert!(execute(&json!({"operation":"shell","command":"id"})).is_err());
        assert!(execute(&json!({"operation":"cleanup","stage":root})).is_err());
        execute(&json!({"operation":"publish","target":target,"staged":source,"incoming":proof})).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(),"fixture");
        assert!(execute(&json!({"operation":"stage","target":target})).is_err());
        assert!(String::from_utf8(execute(&json!({"operation":"inspect","target":target})).unwrap()).unwrap().contains(&proof.sha256));
        execute(&json!({"operation":"cleanup","stage":directory})).unwrap();
        assert!(!directory.exists());assert!(target.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
