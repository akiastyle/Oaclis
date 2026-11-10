// ssh.rs v1.6.97
use crate::devices::Device;
use std::io;
use std::process::Command;

pub(crate) const OPTIONS: &str = "ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o ConnectTimeout=5 -o ServerAliveInterval=15 -o ServerAliveCountMax=2";

// The timer owns no child handle and exits as soon as the operation completes.
pub(crate) fn deadline(pid: u32, timeout: std::time::Duration) -> (std::sync::mpsc::Sender<()>, std::thread::JoinHandle<bool>) {
    let (done, wait) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        if !matches!(wait.recv_timeout(timeout),Err(std::sync::mpsc::RecvTimeoutError::Timeout)) { return false; }
        let _=Command::new("kill").args(["-KILL","--",&format!("-{pid}")]).status();
        true
    });
    (done, worker)
}

pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(crate) fn file(device: &Device, value: &serde_json::Value) -> io::Result<Vec<u8>> {
    use std::io::Write;
    use std::process::Stdio;
    device.validate()?;
    use std::os::unix::process::CommandExt;
    let mut child = Command::new("ssh").process_group(0).args(OPTIONS.split_whitespace().skip(1))
        .arg(&device.ssh_alias).arg("$HOME/.local/bin/oaclis file-request")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let (done, timer)=deadline(child.id(),std::time::Duration::from_secs(60));
    if let Err(error) = child.stdin.take().unwrap().write_all(&serde_json::to_vec(value)?) {
        let _ = child.kill(); let _ = child.wait(); return Err(error);
    }
    let output = child.wait_with_output();
    let _=done.send(());
    if timer.join().unwrap_or(true) { return Err(io::Error::new(io::ErrorKind::TimedOut,"Tempo massimo SSH superato")); }
    let output=output?;
    if !output.status.success() { return Err(io::Error::new(io::ErrorKind::ConnectionAborted, String::from_utf8_lossy(&output.stderr).into_owned())); }
    Ok(output.stdout)
}

pub(crate) fn run(device: &Device, command: &str) -> io::Result<Vec<u8>> {
    device.validate()?;
    crate::ssh_settings::command(Command::new("ssh").args(OPTIONS.split_whitespace().skip(1)).arg(&device.ssh_alias).arg(command),None,std::time::Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes_shell_metacharacters_without_execution() {
        assert_eq!(quote("a'b;$(touch x)"), "'a'\\''b;$(touch x)'");
    }
}

#[cfg(test)]
mod optimization_tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    #[test]
    fn deadline_kills_a_stalled_group_and_cancels_after_success() {
        let mut child=Command::new("sleep").arg("10").process_group(0).spawn().unwrap();
        let (done,timer)=deadline(child.id(),std::time::Duration::from_millis(50));
        assert!(!child.wait().unwrap().success());
        assert!(timer.join().unwrap());drop(done);
        let (done,timer)=deadline(u32::MAX,std::time::Duration::from_secs(30));
        done.send(()).unwrap();assert!(!timer.join().unwrap());
    }
}
