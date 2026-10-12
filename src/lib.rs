pub mod appearance;
pub mod chat;
pub mod compare;
pub mod devices;
pub mod environment;
pub mod paths;
pub mod progress;
pub mod rules;
mod ssh;
pub mod sync;
pub mod transfer;
pub mod terminal;
pub mod web;

mod staging;

use std::fs::{File, Metadata};
use std::io;
use std::path::Path;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn open_regular(path: &Path) -> io::Result<(File, Metadata)> {
    if !path.symlink_metadata()?.file_type().is_file() {
        return Err(invalid("Serve un file regolare, non un link simbolico"));
    }
    let file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(invalid("Il percorso non è un file regolare"));
    }
    Ok((file, metadata))
}

fn check_unchanged(file: &File, before: &Metadata) -> io::Result<()> {
    // shortcut: controllo metadati senza lock, coordinare uno snapshot prima di trasferire chat attive.
    let after = file.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? {
        return Err(invalid(
            "File modificato durante la lettura; riprovare a riposo",
        ));
    }
    Ok(())
}
