//! Oaclis presentation overrides, separate from Codex metadata and transfer rules.
use crate::{devices, invalid, open_regular};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io::{self, Write},
    path::Path,
    process::{Command, Stdio},
};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub label: String,
    pub icon: String,
    pub color: String,
}
impl Appearance {
    pub fn validate(&self) -> io::Result<()> {
        if self.label.chars().count() > 160 || self.label.chars().any(char::is_control) {
            return Err(invalid("Label fino a 160 caratteri, su una sola riga"));
        }
        let uploaded = self
            .icon
            .strip_prefix("data:image/png;base64,")
            .is_some_and(|payload| {
                payload.len() <= 48000
                    && payload.starts_with("iVBORw0KGgo")
                    && payload
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'='))
            });
        if !uploaded
            && !matches!(
                self.icon.as_str(),
                "" | "chat" | "code" | "folder" | "desktop" | "laptop" | "server" | "terminal" | "globe" | "cloud" | "box" | "database" | "git" | "book" | "star" | "bolt" | "shield" | "bug" | "settings" | "cpu" | "rocket" | "link" | "image" | "wrench" | "layers"
            )
        {
            return Err(invalid("Icona non valida"));
        }
        if !self.color.is_empty()
            && !(self.color.len() == 7
                && self.color.starts_with('#')
                && self.color[1..].bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(invalid("Colore esadecimale non valido"));
        }
        Ok(())
    }
}
pub type Appearances = BTreeMap<String, Appearance>;
fn validate(entries: &Appearances) -> io::Result<()> {
    for (key, value) in entries {
        let (kind, id) = key
            .split_once(':')
            .ok_or_else(|| invalid("Oggetto non valido"))?;
        if !matches!(kind, "chat" | "device") || !devices::identifier(id) {
            return Err(invalid("Oggetto non valido"));
        }
        value.validate()?;
    }
    Ok(())
}
pub fn load(path: impl AsRef<Path>) -> io::Result<Appearances> {
    let db = path.as_ref().with_extension("sqlite");
    match open_regular(&db) {
        Ok(_) => {
            let output = Command::new("python3")
                .args(["-c", include_str!("../tools/appearance_store.py")])
                .arg(&db)
                .arg("load")
                .output()?;
            if !output.status.success() {
                return Err(invalid("Lettura database personalizzazioni non riuscita"));
            }
            let entries = serde_json::from_slice(&output.stdout)?;
            validate(&entries)?;
            return Ok(entries);
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let file = match open_regular(path.as_ref()) {
        Ok((file, _)) => file,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e),
    };
    let entries = serde_json::from_reader(file)?;
    validate(&entries)?;
    Ok(entries)
}
pub fn save(path: impl AsRef<Path>, entries: &Appearances) -> io::Result<()> {
    validate(entries)?;
    let db = path.as_ref().with_extension("sqlite");
    crate::rules::ensure_local(
        db.parent()
            .ok_or_else(|| invalid("Cartella database mancante"))?,
    )?;
    match open_regular(&db) {
        Ok(_) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let mut child = Command::new("python3")
        .args(["-c", include_str!("../tools/appearance_store.py")])
        .arg(&db)
        .arg("save")
        .stdin(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(entries)?)?;
    if !child.wait()?.success() {
        return Err(invalid(
            "Salvataggio database personalizzazioni non riuscito",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stores_and_replaces_uploaded_icon() {
        let root = std::env::temp_dir().join(format!("oaclis-appearance-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("appearance.json");
        let mut entries = BTreeMap::from([(
            "chat:abc".into(),
            Appearance {
                label: "Test".into(),
                icon: "data:image/png;base64,iVBORw0KGgo=".into(),
                color: String::new(),
            },
        )]);
        save(&path, &entries).unwrap();
        assert_eq!(
            load(&path).unwrap()["chat:abc"].icon,
            entries["chat:abc"].icon
        );
        entries.get_mut("chat:abc").unwrap().icon = "folder".into();
        save(&path, &entries).unwrap();
        assert_eq!(load(&path).unwrap()["chat:abc"].icon, "folder");
        assert!(root.join("appearance.sqlite").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn validates_labels_icons_colors_and_keys() {
        let good = Appearance {
            label: "Il mio lavoro".into(),
            icon: "laptop".into(),
            color: "#4488aa".into(),
        };
        assert!(good.validate().is_ok());
        for icon in ["chat", "code", "folder", "desktop", "laptop", "server", "terminal", "globe", "cloud", "box", "database", "git", "book", "star", "bolt", "shield", "bug", "settings", "cpu", "rocket", "link", "image", "wrench", "layers"] {
            assert!(Appearance { icon: icon.into(), ..good.clone() }.validate().is_ok());
        }
        let mut bad = good.clone();
        bad.color = "red;display:none".into();
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.icon = "<svg>".into();
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.label = "a\nb".into();
        assert!(bad.validate().is_err());
        bad = good.clone();
        bad.icon = "data:image/png;base64,iVBORw0KGgo=".into();
        assert!(bad.validate().is_ok());
        bad.icon = "data:image/svg+xml;base64,PHN2Zz4=".into();
        assert!(bad.validate().is_err());
        bad.icon = format!("data:image/png;base64,iVBORw0KGgo{}", "A".repeat(48000));
        assert!(bad.validate().is_err());
        let entries = BTreeMap::from([("chat:abc".into(), good.clone())]);
        assert!(validate(&entries).is_ok());
        assert!(validate(&BTreeMap::from([("../../file".into(), good)])).is_err());
    }
}
