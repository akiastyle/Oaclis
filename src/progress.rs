// progress.rs v1.5.150
use crate::invalid;
use serde::Serialize;
use std::io::{self, BufReader, Read};
use std::process::{Command, Stdio};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum Event {
    Preparing,
    Transferring {
        processed_bytes: u64,
        percent: u8,
        rate: String,
        remaining_seconds: Option<u64>,
        total_known: bool,
    },
    Verifying,
    Completed,
}

fn parse(line: &str) -> Option<Event> {
    let mut fields = line.split_whitespace();
    let bytes = fields.next()?.replace(',', "");
    if bytes.is_empty() || !bytes.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let processed_bytes = bytes.parse().ok()?;
    let percent: u8 = fields.next()?.strip_suffix('%')?.parse().ok()?;
    if percent > 100 {
        return None;
    }
    let rate = fields.next()?;
    if !rate.ends_with("B/s") {
        return None;
    }
    let time = fields.next()?;
    let total_known = !line.contains("ir-chk=");
    // Le righe con xfr# riportano tempo trascorso, non tempo rimanente.
    let remaining_seconds = if percent < 100 && total_known && !line.contains("xfr#") {
        let parts: Vec<u64> = time
            .split(':')
            .map(str::parse)
            .collect::<Result<_, _>>()
            .ok()?;
        if parts.len() != 3 || parts[1] >= 60 || parts[2] >= 60 {
            return None;
        }
        Some(
            parts[0]
                .checked_mul(3600)?
                .checked_add(parts[1] * 60 + parts[2])?,
        )
    } else {
        None
    };
    Some(Event::Transferring {
        processed_bytes,
        percent,
        rate: rate.into(),
        remaining_seconds,
        total_known,
    })
}

pub(crate) fn run(command: &mut Command, notify: &mut impl FnMut(Event)) -> io::Result<()> {
    let mut child = command
        .env("LC_ALL", "C")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let output = child.stdout.take().unwrap();
    let mut line = Vec::new();
    let mut oversized = false;
    for byte in BufReader::new(output).bytes() {
        let byte = match byte {
            Ok(byte) => byte,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        if byte == b'\r' || byte == b'\n' {
            if !oversized && let Some(event) = parse(&String::from_utf8_lossy(&line)) {
                notify(event);
            }
            line.clear();
            oversized = false;
        } else if line.len() < 4096 {
            line.push(byte);
        } else {
            oversized = true;
        }
    }
    if !oversized && let Some(event) = parse(&String::from_utf8_lossy(&line)) {
        notify(event);
    }
    let status = child.wait()?;
    if !status.success() {
        return Err(invalid(format!("rsync terminato con {status}")));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_progress_and_remaining_time() {
        assert_eq!(
            parse(" 1,024 25% 2.00kB/s 0:01:02"),
            Some(Event::Transferring {
                processed_bytes: 1024,
                percent: 25,
                rate: "2.00kB/s".into(),
                remaining_seconds: Some(62),
                total_known: true,
            })
        );
    }
    #[test]
    fn hides_elapsed_time_and_unknown_totals() {
        for line in [
            "1024 100% 2.00kB/s 0:00:08 (xfr#1, to-chk=0/1)",
            "1024 25% 2.00kB/s 0:00:08 (xfr#1, to-chk=3/4)",
            "1024 25% 2.00kB/s 0:00:08 (ir-chk=3/4)",
        ] {
            assert!(matches!(
                parse(line),
                Some(Event::Transferring {
                    remaining_seconds: None,
                    ..
                })
            ));
        }
    }
    #[test]
    fn ignores_unrelated_or_invalid_output() {
        for line in [
            "",
            "rsync error",
            "1 101% 2kB/s 0:00:01",
            "1 20% bad 0:00:01",
            "1 20% 2kB/s 0:60:01",
            "1 20% 2kB/s 999999999999999999:00:00",
        ] {
            assert_eq!(parse(line), None);
        }
    }
}
