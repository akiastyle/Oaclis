// app_update.rs v1.8.146
//! Explicit fast-forward update; never discard local changes.
use crate::{invalid, staging::StagedFile};
use std::{io::{self, Read, Seek, SeekFrom}, path::Path, process::{Command, Stdio}, thread, time::{Duration, Instant}};

pub(crate) fn run(command: &mut Command, timeout: Duration) -> io::Result<String> {
    let mut log = StagedFile::new(&std::env::temp_dir())?;
    command.stdin(Stdio::null()).stdout(log.file.try_clone()?).stderr(log.file.try_clone()?);
    #[cfg(unix)] {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? { break status; }
        if Instant::now() >= deadline {
            #[cfg(unix)]
            let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", child.id())]).status();
            let _ = child.kill(); let _ = child.wait();
            return Err(io::Error::new(io::ErrorKind::TimedOut, "Tempo massimo del comando superato"));
        }
        thread::sleep(Duration::from_millis(50));
    };
    let length = log.file.metadata()?.len();
    log.file.seek(SeekFrom::Start(length.saturating_sub(65536)))?;
    let mut output = String::new();
    let mut bytes = Vec::new(); log.file.read_to_end(&mut bytes)?;
    output.push_str(&String::from_utf8_lossy(&bytes));
    if !status.success() {
        let detail = output.chars().rev().take(2000).collect::<String>().chars().rev().collect::<String>();
        return Err(io::Error::other(if detail.trim().is_empty() { format!("Comando non riuscito: {command:?} ({status})") } else { detail.trim().into() }));
    }
    Ok(output.trim().into())
}

pub(crate) fn update(directory: &Path, revision: &str) -> io::Result<()> {
    if revision.len() != 40 || !revision.bytes().all(|b| b.is_ascii_hexdigit()) { return Err(invalid("Revisione master non valida")); }
    let revision = revision.to_ascii_lowercase();
    if !directory.join("scripts/install.sh").is_file() { return Err(invalid("Checkout Oaclis non disponibile: aggiornare dalla cartella della repo")); }
    let git = |args: &[&str]| run(Command::new("git").args(args).current_dir(directory).env("GIT_TERMINAL_PROMPT", "0"), Duration::from_secs(60));
    if !git(&["status", "--porcelain"] )?.is_empty() { return Err(invalid("Aggiornamento sospeso: la repo contiene modifiche locali")); }
    if git(&["branch", "--show-current"])? != "main" { return Err(invalid("Aggiornamento sospeso: la repo non e sul ramo main")); }
    git(&["fetch", "origin", "main"])?;
    git(&["merge-base", "--is-ancestor", &revision, "origin/main"])?;
    git(&["merge-base", "--is-ancestor", "HEAD", &revision])?;
    git(&["merge", "--ff-only", &revision])?;
    if git(&["rev-parse", "HEAD"])? != revision { return Err(invalid("La revisione locale non coincide con quella del master")); }
    run(Command::new("bash").args(["scripts/install.sh", "--unconfigured"]).current_dir(directory).env("GIT_TERMINAL_PROMPT", "0"), Duration::from_secs(1800))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    fn git(path: &Path, args: &[&str]) -> String {
        run(Command::new("git").args(args).current_dir(path).env("GIT_CONFIG_GLOBAL", "/dev/null").env("GIT_CONFIG_NOSYSTEM", "1"), Duration::from_secs(10)).unwrap()
    }
    #[test]
    fn step2_update_fast_forwards_and_preserves_dirty_divergent_checkouts() {
        let root = Fixture(std::env::temp_dir().join(format!("oaclis-update-test-{}", std::process::id())));
        fs::create_dir(&root.0).unwrap();
        let source = root.0.join("source"); fs::create_dir(&source).unwrap();
        git(&source, &["init", "-b", "main"]);
        git(&source, &["config", "user.email", "fixture@example.invalid"]);
        git(&source, &["config", "user.name", "Fixture"]);
        git(&source, &["config", "commit.gpgsign", "false"]);
        fs::create_dir(source.join("scripts")).unwrap();
        fs::write(source.join("scripts/install.sh"), "#!/bin/sh\n[ \"$1\" = --unconfigured ] || exit 9\necho installed > ../installed\n").unwrap();
        git(&source, &["add", "."]); git(&source, &["commit", "-m", "initial"]);
        let client = root.0.join("client");
        git(&root.0, &["clone", source.to_str().unwrap(), client.to_str().unwrap()]);
        git(&client, &["config", "user.email", "fixture@example.invalid"]);
        git(&client, &["config", "user.name", "Fixture"]);
        git(&client, &["config", "commit.gpgsign", "false"]);
        fs::write(source.join("version"), "new").unwrap(); git(&source, &["add", "."]); git(&source, &["commit", "-m", "next"]);
        let revision = git(&source, &["rev-parse", "HEAD"]);
        fs::write(client.join("local-file"), "preserve").unwrap();
        assert!(update(&client, &revision).unwrap_err().to_string().contains("modifiche locali"));
        assert!(!root.0.join("installed").exists());
        assert_eq!(fs::read_to_string(client.join("local-file")).unwrap(), "preserve");
        fs::remove_file(client.join("local-file")).unwrap();
        update(&client, &revision).unwrap();
        assert_eq!(git(&client, &["rev-parse", "HEAD"]), revision);
        assert_eq!(fs::read_to_string(root.0.join("installed")).unwrap().trim(), "installed");
        fs::remove_file(root.0.join("installed")).unwrap();
        fs::write(client.join("local-commit"), "preserve").unwrap(); git(&client, &["add", "."]); git(&client, &["commit", "-m", "local"]);
        let local = git(&client, &["rev-parse", "HEAD"]);
        fs::write(source.join("version"), "remote next").unwrap(); git(&source, &["add", "."]); git(&source, &["commit", "-m", "diverged"]);
        let revision = git(&source, &["rev-parse", "HEAD"]);
        assert!(update(&client, &revision).is_err());
        assert_eq!(git(&client, &["rev-parse", "HEAD"]), local);
        assert!(!root.0.join("installed").exists());
        assert!(update(&client, "HEAD; echo unsafe").is_err());
    }
    #[test]
    fn step2_update_command_timeout_is_bounded_and_reports_stderr() {
        assert_eq!(run(Command::new("sh").args(["-c", "echo native"]), Duration::from_secs(2)).unwrap(), "native");
        assert!(run(Command::new("sh").args(["-c", "echo detail >&2; exit 2"]), Duration::from_secs(2)).unwrap_err().to_string().contains("detail"));
        let start = Instant::now();
        assert_eq!(run(Command::new("sh").args(["-c", "sleep 10"]), Duration::from_millis(100)).unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}
