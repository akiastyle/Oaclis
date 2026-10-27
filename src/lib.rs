// lib.rs v1.3.97
pub mod state_store;
mod chat_deletion;
mod state_migration;
mod shared_records;
pub mod state_transport;
mod app_inventory;
mod automation;
mod link_flow;
mod catalog;
mod assistant_stream;
mod initial_inventory;
mod transfer_job;
pub mod appearance;
pub mod chat;
pub mod compare;
pub mod devices;
pub mod environment;
pub mod paths;
pub mod path_access;
pub mod progress;
pub mod preparation;
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

pub mod peers;

pub mod setup;

pub mod shared_settings;

mod device_runtime;

#[cfg(all(test, oaclis_local_tests))]
mod state_tests;

pub mod bootstrap_settings;
pub mod service_setup;
pub mod open_ui;

pub mod keep_awake;
mod app_update;

mod ssh_settings;
mod pairing;

mod environment_setup;
mod environment_analysis;
mod dependency_inventory;

mod app_install;

mod mobile;

mod remote_desktop;

pub mod ssh_gateway;

mod mobile_auth;
