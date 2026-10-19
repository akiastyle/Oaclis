// mobile.rs v1.3.64
//! Explicit mobile invitations. Relay credentials never enter browser responses or logs.
use crate::invalid;
use qrcode::{QrCode, render::svg};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::{self, Write}, path::Path, process::{Command, Stdio}};

const ENDPOINT: &str = "https://www.krprojects.it/oaclis-auth-test/index.php";
const INVITE_PREFIX: &str = "https://www.krprojects.it/oaclis-auth-test/index.php#pair=";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Settings {
    admin_key: String,
}

fn invitation_qr(url: &str) -> io::Result<String> {
    let code = url.strip_prefix(INVITE_PREFIX).ok_or_else(|| invalid("Invito mobile non valido"))?;
    if code.len() != 64 || !code.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Invito mobile non valido"));
    }
    Ok(QrCode::new(url).map_err(io::Error::other)?
        .render::<svg::Color>().min_dimensions(288, 288).build())
}

pub(crate) fn invite(config: &Path) -> io::Result<Value> {
    let settings: Settings = serde_json::from_reader(crate::open_regular(&config.join("mobile-relay.json"))?.0)
        .map_err(|_| invalid("Collegamento mobile da configurare"))?;
    if settings.admin_key.len() != 64 || !settings.admin_key.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(invalid("Collegamento mobile da configurare"));
    }
    crate::state_transport::log("SEND mobile-relay invite".into());
    // Feed the credential over stdin: it must never appear in process arguments.
    let input = format!("url = \"{ENDPOINT}\"\nheader = \"Origin: https://www.krprojects.it\"\nheader = \"Content-Type: application/json\"\nheader = \"X-Admin-Key: {}\"\ndata = \"{{\\\"action\\\":\\\"invite\\\"}}\"\n", settings.admin_key);
    let mut child = Command::new("curl")
        .args(["--silent", "--fail", "--proto", "=https", "--connect-timeout", "3", "--max-time", "10", "--max-filesize", "16384", "--config", "-"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
    let sent = child.stdin.take().unwrap().write_all(input.as_bytes());
    if sent.is_err() { let _ = child.kill(); let _ = child.wait(); return Err(io::Error::other("Invito non disponibile. Riprova.")); }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        crate::state_transport::log(format!("RECV mobile-relay invite failed {}", output.status));
        return Err(io::Error::other("Invito non disponibile. Riprova."));
    }
    let reply: Value = serde_json::from_slice(&output.stdout).map_err(|_| io::Error::other("Invito non disponibile. Riprova."))?;
    let qr = invitation_qr(reply["url"].as_str().unwrap_or(""))?;
    crate::state_transport::log("RECV mobile-relay invite OK".into());
    Ok(json!({"qr_svg":qr,"expires_in":600}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qr_accepts_only_the_fixed_relay_and_random_code() {
        let url = format!("{INVITE_PREFIX}{}", "a".repeat(64));
        let qr = invitation_qr(&url).unwrap();
        assert!(qr.contains("<svg"));
        assert!(!qr.contains(&url));
        for invalid in ["https://example.com/#pair=abc".to_string(), format!("{INVITE_PREFIX}short"), format!("{INVITE_PREFIX}{}", "g".repeat(64)), format!("{url}&admin=anything")] {
            assert!(invitation_qr(&invalid).is_err());
        }
    }
}
