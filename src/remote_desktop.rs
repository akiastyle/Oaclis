// remote_desktop.rs v1.3.46
//! Optional installed desktop client, discovered once at app startup.
use serde_json::{Value, json};
use std::{path::Path, process::Command, time::Duration};

fn client_id(text: &str) -> Option<String> {
    let id = text.trim();
    (id.len() >= 6 && id.len() <= 16 && id.bytes().all(|c| c.is_ascii_digit())).then(|| id.into())
}

pub(crate) fn discover() -> Value {
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    let wsl = std::env::var_os("WSL_DISTRO_NAME").is_some() || kernel.to_lowercase().contains("microsoft");
    let mut command;
    if wsl || cfg!(target_os = "windows") {
        command = Command::new(if wsl { "/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe" } else { "powershell.exe" });
        command.args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; $paths=@((Join-Path $env:ProgramFiles 'RustDesk\\rustdesk.exe'),(Join-Path $env:LOCALAPPDATA 'Programs\\RustDesk\\rustdesk.exe')); $client=$paths | Where-Object {Test-Path -LiteralPath $_ -PathType Leaf} | Select-Object -First 1; if(-not $client){exit 1}; & $client --get-id"]);
    } else if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME").unwrap_or_default();
        let local = Path::new(&home).join("Applications/RustDesk.app/Contents/MacOS/RustDesk");
        command = Command::new(if local.is_file() { local } else { "/Applications/RustDesk.app/Contents/MacOS/RustDesk".into() });
        command.arg("--get-id");
    } else {
        let Ok(sessions) = crate::app_update::run(Command::new("loginctl").args(["list-sessions", "--no-legend"]), Duration::from_secs(3)) else { return Value::Null; };
        let graphical = sessions.lines().take(32).filter_map(|line| line.split_whitespace().next()).any(|session| {
            crate::app_update::run(Command::new("loginctl").args(["show-session", session, "-p", "Type", "--value"]), Duration::from_secs(2))
                .is_ok_and(|kind| matches!(kind.trim(), "x11" | "wayland"))
        });
        if !graphical { return Value::Null; }
        command = Command::new("rustdesk");
        command.arg("--get-id");
    }
    crate::app_update::run(&mut command, Duration::from_secs(15)).ok()
        .and_then(|output| client_id(&output)).map_or(Value::Null, |id| json!({"id":id}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_only_plain_desktop_ids() {
        assert_eq!(client_id(" 96335341\r\n"), Some("96335341".into()));
        for invalid in ["", "12345", "123456?password=secret", "123456/", "a123456", "123456\nnoise", "12345678901234567"] {
            assert_eq!(client_id(invalid), None);
        }
    }
}
