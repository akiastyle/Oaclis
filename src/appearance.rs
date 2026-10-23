// appearance.rs v1.6.261
//! Oaclis presentation overrides, separate from Codex metadata and transfer rules.
use crate::{devices, invalid};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    io,
    path::Path,
};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub label: String,
    pub icon: String,
    pub color: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
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
        if !self.url.is_empty() {
            let uri=self.url.split('#').next().unwrap_or("").parse::<ureq::http::Uri>()
                .map_err(|_|invalid("URL web non valido: usa http:// o https://"))?;
            if self.url.len()>2048 || self.url.chars().any(char::is_whitespace) || self.url.chars().any(char::is_control)
                || !matches!(uri.scheme_str(),Some("http"|"https")) || uri.host().is_none_or(str::is_empty)
                || uri.authority().is_some_and(|a|a.as_str().contains('@')) {
                return Err(invalid("URL web non valido: usa http:// o https://"));
            }
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
    let entries=serde_json::from_value(crate::environment::database(path.as_ref().parent().ok_or_else(||invalid("Cartella stato mancante"))?,"","load",None)?)?;
    validate(&entries)?;
    Ok(entries)
}
pub fn save(path: impl AsRef<Path>, entries: &Appearances) -> io::Result<()> {
    validate(entries)?;
    crate::environment::database(path.as_ref().parent().ok_or_else(||invalid("Cartella stato mancante"))?,"","save",Some(&serde_json::to_value(entries)?))?;
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
                url: "https://example.org/project/#preview".into(),
            },
        )]);
        save(&path, &entries).unwrap();
        assert_eq!(
            load(&path).unwrap()["chat:abc"].icon,
            entries["chat:abc"].icon
        );
        assert_eq!(load(&path).unwrap()["chat:abc"].url,"https://example.org/project/#preview");
        entries.get_mut("chat:abc").unwrap().url.clear();
        entries.get_mut("chat:abc").unwrap().icon = "folder".into();
        save(&path, &entries).unwrap();
        assert_eq!(load(&path).unwrap()["chat:abc"].icon, "folder");
        assert!(load(&path).unwrap()["chat:abc"].url.is_empty());
        assert!(root.join("appearance.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn validates_labels_icons_colors_and_keys() {
        let good = Appearance {
            label: "Il mio lavoro".into(),
            icon: "laptop".into(),
            color: "#4488aa".into(),
            url: String::new(),
        };
        assert!(good.validate().is_ok());
        for icon in ["chat", "code", "folder", "desktop", "laptop", "server", "terminal", "globe", "cloud", "box", "database", "git", "book", "star", "bolt", "shield", "bug", "settings", "cpu", "rocket", "link", "image", "wrench", "layers"] {
            assert!(Appearance { icon: icon.into(), ..good.clone() }.validate().is_ok());
        }
        for url in ["https://example.org/path?q=1#part", "http://localhost:8080/", "http://192.168.1.154/"] {
            assert!(Appearance {url:url.into(),..good.clone()}.validate().is_ok());
        }
        for url in ["javascript:alert(1)", "data:text/html,hello", "file:///tmp/a", "https://", "https://user:password@example.org", "https://example.org/ bad", "https://example.org/\n"] {
            assert!(Appearance {url:url.into(),..good.clone()}.validate().is_err());
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
