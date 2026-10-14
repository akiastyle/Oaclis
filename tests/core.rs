// core.rs v1.12.197
use oaclis::devices::{self, Device};
use oaclis::{
    chat::Chat,
    compare::{self, ChatComparison, FileComparison},
    paths::ProjectPaths,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

fn device(id: &str, name: &str) -> Device {
    Device {
        id: id.into(),
        name: name.into(),
        ssh_alias: "test-host".into(),
    }
}

#[test]
fn adds_and_edits_devices_preserving_other_entries() {
    let fixture = Fixture::new();
    let config = fixture.0.join("devices.json");
    let mut entries = devices::load(&config).unwrap();
    devices::upsert(&mut entries, device("a", "First")).unwrap();
    devices::upsert(&mut entries, device("b", "Second")).unwrap();
    devices::save(&config, &entries).unwrap();
    let mut entries = devices::load(&config).unwrap();
    let mut edited = device("a", "Edited");
    edited.ssh_alias = "another-host".into();
    devices::upsert(&mut entries, edited).unwrap();
    devices::save(&config, &entries).unwrap();
    let entries = devices::load(&config).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "Edited");
    assert_eq!(entries[0].ssh_alias, "another-host");
    assert_eq!(entries[1].name, "Second");
    let before = fs::read(&config).unwrap();
    assert!(devices::save(&config, &[device("a", "One"), device("a", "Duplicate")]).is_err());
    assert_eq!(fs::read(&config).unwrap(), before);
}

#[test]
fn rejects_corrupt_configuration_and_injected_aliases() {
    let fixture = Fixture::new();
    let config = fixture.write("devices.json", b"not json");
    assert!(devices::load(&config).is_err());
    for alias in [
        "-oProxyCommand=evil",
        "host;command",
        "host\ncommand",
        "",
        "user@host",
    ] {
        let mut entry = device("a", "Test");
        entry.ssh_alias = alias.into();
        assert!(entry.validate().is_err());
    }
    assert!(devices::save(&config, &[device("", "Test")]).is_err());
    assert_eq!(fs::read(config).unwrap(), b"not json");
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "oaclis-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

const META: &str = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\",\"cwd\":\"/work/app\"}}\n";
const EVENT: &str = "{\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\"}}\n";

#[test]
fn compares_real_files_by_content_and_preserves_originals() {
    let fixture = Fixture::new();
    let source = fixture.write("source", b"same");
    let destination = fixture.write("destination", b"same");
    assert_eq!(
        compare::files(&source, &destination).unwrap(),
        FileComparison::Identical
    );
    fs::write(&destination, b"diff").unwrap();
    assert_eq!(
        compare::files(&source, &destination).unwrap(),
        FileComparison::Different
    );
    assert_eq!(
        compare::files(&source, fixture.0.join("missing")).unwrap(),
        FileComparison::DestinationMissing
    );
    assert!(compare::files(fixture.0.join("missing"), &destination).is_err());
    assert!(compare::files(&source, &fixture.0).is_err());
    assert_eq!(fs::read(source).unwrap(), b"same");
}

#[test]
fn chat_comparison_ignores_json_formatting_but_detects_history_changes() {
    let fixture = Fixture::new();
    let source_path = fixture.write("source.jsonl", format!("{META}{EVENT}").as_bytes());
    let destination_path = fixture.write(
        "destination.jsonl",
        b"{ \"payload\": {\"cwd\": \"/work/app\", \"id\": \"a\"}, \"type\": \"session_meta\" }\n",
    );
    let source = Chat::read(&source_path).unwrap();
    let destination = Chat::read(&destination_path).unwrap();
    assert_eq!(
        compare::chats(&source, Some(&destination)),
        ChatComparison::SourceAhead
    );
    fs::write(&destination_path, format!("{META}{EVENT}")).unwrap();
    assert_eq!(
        compare::chats(&source, Some(&Chat::read(&destination_path).unwrap())),
        ChatComparison::Identical
    );
    fs::write(&destination_path, META.replace("/work/app", "/other/app")).unwrap();
    assert_eq!(
        compare::chats(&source, Some(&Chat::read(&destination_path).unwrap())),
        ChatComparison::Conflict
    );
    assert_eq!(
        fs::read_to_string(&source_path).unwrap(),
        format!("{META}{EVENT}")
    );
}

#[test]
fn rejects_a_truncated_tail_and_duplicate_metadata() {
    let fixture = Fixture::new();
    let path = fixture.write("chat.jsonl", format!("{META}{{\"type\":").as_bytes());
    assert!(Chat::read(&path).is_err());
    fs::write(&path, format!("{META}{META}")).unwrap();
    assert!(Chat::read(path).is_err());
}

#[test]
fn preserves_large_json_numbers_when_comparing() {
    let fixture = Fixture::new();
    let source = fixture.write(
        "a.jsonl",
        format!("{META}{{\"type\":\"event_msg\",\"payload\":{{\"n\":184467440737095516160}}}}\n")
            .as_bytes(),
    );
    let destination = fixture.write(
        "b.jsonl",
        format!("{META}{{\"type\":\"event_msg\",\"payload\":{{\"n\":184467440737095516161}}}}\n")
            .as_bytes(),
    );
    assert_eq!(
        compare::chats(
            &Chat::read(source).unwrap(),
            Some(&Chat::read(destination).unwrap())
        ),
        ChatComparison::Conflict
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlinks_and_dangling_symlinks() {
    let fixture = Fixture::new();
    let source = fixture.write("source", b"content");
    let link = fixture.0.join("link");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    assert!(compare::files(&source, &link).is_err());
    assert!(Chat::read(&link).is_err());
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(fixture.0.join("absent"), &link).unwrap();
    assert!(compare::files(&source, &link).is_err());
}

#[test]
fn maps_paths_with_spaces() {
    let mapping = ProjectPaths::new("/work/app", "/home/user/progetto").unwrap();
    assert_eq!(
        mapping.map("/work/app/docs/note con spazi.md").unwrap(),
        PathBuf::from("/home/user/progetto/docs/note con spazi.md")
    );
}
