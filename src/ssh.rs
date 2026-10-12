use crate::{devices::Device, invalid};
use std::io;
use std::process::Command;

pub(crate) const OPTIONS: &str = "ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=5 -o ServerAliveInterval=15 -o ServerAliveCountMax=2";

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn run(device: &Device, command: &str) -> io::Result<Vec<u8>> {
    device.validate()?;
    let output = Command::new("ssh")
        .args(OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias)
        .arg(command)
        .output()?;
    if !output.status.success() {
        return Err(invalid(format!(
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
