// terminal.rs v1.1.24
//! Terminal transport; Codex and its archive stay on the selected device.
use crate::{devices::Device, invalid};
use serde_json::Value;
use std::{io::{self, Write}, process::{Command, Stdio}};
const SCRIPT: &str = include_str!("../tools/chat_terminal.py");
pub fn request(device: Option<&Device>, value: &Value) -> io::Result<Value> {
    let mut command = if let Some(device) = device {
        device.validate()?;
        let mut command = Command::new("ssh");
        command.args(crate::ssh::OPTIONS.split_whitespace().skip(1))
            .arg(&device.ssh_alias).arg(format!("python3 -c {}", crate::ssh::quote(SCRIPT)));
        command
    } else {
        let mut command = Command::new("python3");
        command.args(["-c", SCRIPT]);
        command
    };
    let mut process = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    process.stdin.take().unwrap().write_all(&serde_json::to_vec(value)?)?;
    let output = process.wait_with_output()?;
    let result: Value = serde_json::from_slice(&output.stdout).map_err(|_| invalid(String::from_utf8_lossy(&output.stderr)))?;
    if !output.status.success() { return Err(invalid(result["error"].as_str().unwrap_or("Terminale non disponibile"))); }
    Ok(result)
}
