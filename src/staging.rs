use crate::{invalid, paths::validate_absolute};
use std::fs::{self, File, OpenOptions};
use std::io;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) struct StagedFile {
    pub path: PathBuf,
    pub file: File,
}

impl StagedFile {
    pub fn new(directory: &Path) -> io::Result<Self> {
        validate_absolute(directory)?;
        for ancestor in directory.ancestors() {
            if !ancestor.symlink_metadata()?.is_dir() {
                return Err(invalid(
                    "La cartella di destinazione deve esistere e non contenere link",
                ));
            }
        }
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let path = directory.join(format!(".oaclis-{}-{nonce}.part", std::process::id()));
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(&path)?;
        Ok(Self { path, file })
    }
}

impl Drop for StagedFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub(crate) fn save_json(path: &Path, value: &impl serde::Serialize) -> io::Result<()> {
    use std::io::Write;
    validate_absolute(path)?;
    match path.symlink_metadata() {
        Ok(metadata) if !metadata.is_file() => return Err(invalid("Configurazione non regolare")),
        Err(error) if error.kind() != io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let parent = path.parent().ok_or_else(|| invalid("Cartella mancante"))?;
    let mut staged = StagedFile::new(parent)?;
    serde_json::to_writer_pretty(&mut staged.file, value)?;
    staged.file.write_all(b"\n")?;
    staged.file.sync_all()?;
    fs::rename(&staged.path, path)?;
    File::open(parent)?.sync_all()
}
