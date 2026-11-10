// devices.rs v1.6.66
use crate::{invalid, open_regular};
use serde::{Deserialize, Serialize};
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub ssh_alias: String,
}

pub(crate) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
}

impl Device {
    pub fn validate(&self) -> io::Result<()> {
        if !identifier(&self.id) || !identifier(&self.ssh_alias) || self.name.trim().is_empty() {
            return Err(invalid("ID, nome o alias SSH del dispositivo non valido"));
        }
        Ok(())
    }
}

pub fn load(path: impl AsRef<Path>) -> io::Result<Vec<Device>> {
    let (file, _) = match open_regular(path.as_ref()) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let devices: Vec<Device> = serde_json::from_reader(file)?;
    validate(&devices)?;
    Ok(devices)
}

fn validate(devices: &[Device]) -> io::Result<()> {
    let mut ids = std::collections::HashSet::new();
    for device in devices {
        device.validate()?;
        if !ids.insert(&device.id) {
            return Err(invalid("ID dispositivo duplicato"));
        }
    }
    Ok(())
}

pub fn upsert(devices: &mut Vec<Device>, device: Device) -> io::Result<()> {
    device.validate()?;
    if let Some(existing) = devices.iter_mut().find(|existing| existing.id == device.id) {
        *existing = device;
    } else {
        devices.push(device);
    }
    Ok(())
}

pub fn save(path: impl AsRef<Path>, devices: &[Device]) -> io::Result<()> {
    validate(devices)?;
    crate::staging::save_json(path.as_ref(), &devices)
}
