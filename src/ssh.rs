// ssh.rs v1.4.49
use crate::devices::Device;
use std::io;
use std::process::Command;

pub(crate) const OPTIONS: &str = "ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=5 -o ServerAliveInterval=15 -o ServerAliveCountMax=2";

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn file(device: &Device, value: &serde_json::Value) -> io::Result<Vec<u8>> {
    use std::io::Write;
    use std::process::Stdio;
    device.validate()?;
    let mut child = Command::new("ssh").args(OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias).arg("$HOME/.local/bin/oaclis file-request")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    if let Err(error) = child.stdin.take().unwrap().write_all(&serde_json::to_vec(value)?) {
        let _ = child.kill(); let _ = child.wait(); return Err(error);
    }
    let output = child.wait_with_output()?;
    if !output.status.success() { return Err(io::Error::new(io::ErrorKind::ConnectionAborted, String::from_utf8_lossy(&output.stderr).into_owned())); }
    Ok(output.stdout)
}

pub(crate) fn run(device: &Device, command: &str) -> io::Result<Vec<u8>> {
    device.validate()?;
    let output = Command::new("ssh")
        .args(OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias)
        .arg(command)
        .output()?;
    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::ConnectionAborted, format!(
            "SSH: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes_shell_metacharacters_without_execution() {
        assert_eq!(quote("a'b;$(touch x)"), "'a'\\''b;$(touch x)'");
    }
}
