//! Presentation replication uses the existing mutually approved SSH transport.
use std::{io::Write, path::{Path, PathBuf}, process::{Command, Stdio}, thread, time::Duration};
use serde_json::Value;
const SCRIPT: &str = include_str!("../tools/shared_settings.py");
pub fn start(home: PathBuf) {
    thread::spawn(move || loop {
        let output = Command::new("python3").args(["-c", SCRIPT, "sync"])
            .env("HOME", &home).env("OACLIS_SHARED_SOURCE", SCRIPT).output();
        match output {
            Ok(result) if result.status.success() => {}
            Ok(result) => eprintln!("Oaclis: impostazioni condivise: {}", String::from_utf8_lossy(&result.stdout).trim()),
            Err(error) => eprintln!("Oaclis: impostazioni condivise: {error}"),
        }
        thread::sleep(Duration::from_secs(5));
    });
}
pub fn positions(config: &Path) -> std::io::Result<Value> {
    match crate::open_regular(&config.join("positions.json")) {
        Ok((file, _)) => serde_json::from_reader(file).map_err(std::io::Error::other),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(serde_json::json!({})),
        Err(error) => Err(error),
    }
}
pub fn save_positions(config: &Path, value: &Value) -> std::io::Result<()> {
    let entries = value.as_object().ok_or_else(|| crate::invalid("Posizioni non valide"))?;
    if entries.len() > 1000 || entries.iter().any(|(id, point)| !crate::devices::identifier(id)
        || ["x", "y"].iter().any(|key| !point[*key].as_f64().is_some_and(|n| n.is_finite() && (0.0..=100000.0).contains(&n)))) {
        return Err(crate::invalid("Posizioni non valide"));
    }
    let mut child = Command::new("python3").args(["-c", SCRIPT, "positions"])
        .env("HOME", config.parent().and_then(Path::parent).ok_or_else(|| crate::invalid("Home non valida"))?)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(value)?)?;
    let output = child.wait_with_output()?;
    if !output.status.success() { return Err(crate::invalid("Salvataggio posizioni non riuscito")); }
    Ok(())
}
