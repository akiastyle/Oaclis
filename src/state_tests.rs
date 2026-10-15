// state_tests.rs v1.32.662
//! Isolated fixtures cover JSON persistence, former SQLite import and peer convergence.
use crate::{shared_settings as shared, state_store as store};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "oaclis-state-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn call(&self, op: &str, key: &str, value: Option<Value>) -> Value {
        store::database(&self.0, key, op, value.as_ref()).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn link(chat: &str, target: &str) -> Value {
    json!({"entry":{"id":chat,"name":chat},"source":"master","target":target,"operation":"sync","scopes":{"chat":true,"paths":true,"apps":true},"flow":{"generation":"one","initialized":false,"phase":"ready","requested":false}})
}

#[test]
fn memory_reads_do_not_create_json_and_noop_writes_do_not_rewrite_files() {
    let f = Fixture::new();
    assert_eq!(f.call("links-get", "", None), json!([]));
    assert!(!f.0.join("appearance.json").exists());
    f.call("migrate", "", None);
    f.call("links-save", "a", Some(link("a", "peer")));
    let path = f.0.join("chat_links.json");
    let metadata = fs::metadata(&path).unwrap();
    let before = metadata.modified().unwrap();
    for _ in 0..1000 {
        assert_eq!(f.call("links-get", "", None).as_array().unwrap().len(), 1);
    }
    f.call("links-save", "a", Some(link("a", "peer")));
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), before);
    for file in [
        "appearance.json",
        "analysis.json",
        "chat_links.json",
        "transfers.json",
    ] {
        assert!(f.0.join(file).exists());
    }
    assert!(!f.0.join(".state-commit.json").exists());
    assert!(!f.0.join("appearance.sqlite").exists());
}
#[test]
fn scopes_hide_and_link_deletion_preserve_appearance_and_user_data() {
    let f = Fixture::new();
    let original = f.0.join("chat.jsonl");
    fs::write(&original, "unchanged").unwrap();
    f.call("save","",Some(json!({"chat:a":{"label":"Custom","icon":"data:image/png;base64,iVBORw0KGgo=","color":"#ee6c4d"}})));
    for (key, id, target) in [("a1", "a", "one"), ("a2", "a", "two"), ("b", "b", "two")] {
        f.call("links-save", key, Some(link(id, target)));
    }
    f.call(
        "analysis-save",
        "master:a",
        Some(json!({"status":"ready","manifest":{"paths":["/root/app"]}})),
    );
    let scopes = json!({"chat":true,"paths":false,"apps":true});
    let result = f.call("chat-scopes", "a", Some(scopes.clone()));
    assert!(
        result["links"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|l| l["entry"]["id"] == "a")
            .all(|l| l["scopes"] == scopes)
    );
    f.call("links-delete", "a1", None);
    assert!(
        !f.call("preferences-get", "", None)["preferences"]["a"]["hidden"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(f.call("load", "", None)["chat:a"]["label"], "Custom");
    f.call("chat-visibility", "a", Some(json!({"hidden":true})));
    assert_eq!(f.call("links-get", "", None).as_array().unwrap().len(), 2);
    f.call("chat-hide", "a", None);
    assert_eq!(f.call("links-get", "", None).as_array().unwrap().len(), 1);
    assert_eq!(f.call("analysis-get", "master:a", None)["status"], "ready");
    assert_eq!(f.call("load", "", None)["chat:a"]["color"], "#ee6c4d");
    assert_eq!(fs::read_to_string(original).unwrap(), "unchanged");
}
#[test]
fn failed_changes_rollback_and_concurrent_writes_are_serialized() {
    let f = Fixture::new();
    f.call("migrate", "", None);
    std::thread::scope(|scope| {
        for i in 0..12 {
            let path = &f.0;
            scope.spawn(move || {
                store::database(
                    path,
                    "",
                    "save",
                    Some(&json!({format!("chat:{i}"):{"label":i.to_string()}})),
                )
                .unwrap();
            });
        }
    });
    assert_eq!(f.call("load", "", None).as_object().unwrap().len(), 12);
    let before = fs::read(f.0.join("appearance.json")).unwrap();
    assert!(
        store::database(
            &f.0,
            "0",
            "chat-visibility",
            Some(&json!({"hidden":"invalid"}))
        )
        .is_err()
    );
    assert_eq!(fs::read(f.0.join("appearance.json")).unwrap(), before);
    f.call("links-save", "one", Some(link("a", "peer")));
    let before = f.call("links-get", "", None);
    let mut changed = link("a", "other");
    changed["operation"] = json!("send");
    assert!(store::database(&f.0, "one", "links-save", Some(&changed)).is_err());
    assert_eq!(f.call("links-get", "", None), before);
}
#[test]
fn inventories_sizes_and_switches_remain_owned_by_source() {
    let f = Fixture::new();
    fs::write(f.0.join("pairing_ed25519.pub"), "ssh-ed25519 fixture").unwrap();
    let data = json!({"status":"ready","requirements":[{"name":"node"}]});
    f.call("inventory-save", "master:a", Some(data.clone()));
    let prep = json!({"status":"ready","chat_bytes":42,"chat_modified_ns":"123","measured_at":9,"paths":[{"path":"/root/app","sha256":"abc","entries":["one"]}],"paths_bytes":120,"path_files":1});
    f.call("preparation-save", "master:a", Some(prep.clone()));
    assert_eq!(f.call("inventory-get", "master:a", None), data);
    assert_eq!(f.call("preparation-get", "master:a", None), prep);
    assert!(
        f.call("status-read", "", None)["preparations"]["master:a"]["paths"][0]
            .get("entries")
            .is_none()
    );
    assert!(
        f.call("preparation-get", "master:a", None)["paths"][0]
            .get("entries")
            .is_some()
    );
    f.call(
        "chat-observed",
        "master:a",
        Some(json!({"bytes":80,"modified_ns":"456","sha256":"hash"})),
    );
    assert_eq!(
        f.call("preparation-get", "master:a", None)["chat_bytes"],
        80
    );
    assert_eq!(f.call("inventory-get", "master:a", None), data);
}
fn old_database(path: &Path) {
    let db = rusqlite::Connection::open(path).unwrap();
    db.execute_batch("CREATE TABLE appearance(key TEXT PRIMARY KEY,value TEXT NOT NULL,hidden INTEGER NOT NULL DEFAULT 0,sync TEXT NOT NULL DEFAULT '{}');CREATE TABLE analysis(key TEXT PRIMARY KEY,value TEXT);CREATE TABLE chat_links(key TEXT PRIMARY KEY,value TEXT NOT NULL);CREATE TABLE transfers(key TEXT PRIMARY KEY,value TEXT NOT NULL);").unwrap();
    db.execute(
        "INSERT INTO appearance VALUES(?,?,?,?)",
        rusqlite::params![
            "chat:a",
            r##"{"label":"Custom","icon":"chat","color":"#ee6c4d"}"##,
            1,
            r#"{"chat":{"enabled":1},"paths":{"enabled":0}}"#
        ],
    )
    .unwrap();
    db.execute(
        "INSERT INTO analysis VALUES(?,?)",
        [
            "master:a",
            r#"{"status":"ready","manifest":{"paths":["/root/app"]}}"#,
        ],
    )
    .unwrap();
    db.execute(
        "INSERT INTO chat_links VALUES(?,?)",
        ["a", &link("a", "peer").to_string()],
    )
    .unwrap();
    db.execute(
        "INSERT INTO transfers VALUES(?,?)",
        ["master:a", r#"{"status":"completed"}"#],
    )
    .unwrap();
}
#[test]
fn import_preserves_all_four_tables_and_never_modifies_the_old_database() {
    let f = Fixture::new();
    let old = f.0.join("appearance.sqlite");
    old_database(&old);
    let before = fs::read(&old).unwrap();
    f.call("migrate", "", None);
    assert_eq!(fs::read(&old).unwrap(), before);
    assert_eq!(f.call("load", "", None)["chat:a"]["label"], "Custom");
    assert_eq!(
        f.call("preferences-get", "", None)["preferences"]["a"],
        json!({"hidden":true,"scopes":{"chat":true,"paths":false,"apps":true}})
    );
    assert_eq!(f.call("analysis-get", "master:a", None)["status"], "ready");
    assert_eq!(f.call("links-get", "", None).as_array().unwrap().len(), 1);
    assert_eq!(
        f.call("transfer-list", "", None)["master:a"]["status"],
        "completed"
    );
    let reloaded = Fixture::new();
    for name in [
        "appearance.json",
        "analysis.json",
        "chat_links.json",
        "transfers.json",
    ] {
        fs::copy(f.0.join(name), reloaded.0.join(name)).unwrap();
    }
    assert_eq!(reloaded.call("load", "", None), f.call("load", "", None));
    assert_eq!(
        reloaded.call("links-get", "", None),
        f.call("links-get", "", None)
    );
}
#[test]
fn malformed_json_is_reported_without_overwriting_it() {
    let f = Fixture::new();
    for name in [
        "appearance.json",
        "analysis.json",
        "chat_links.json",
        "transfers.json",
    ] {
        fs::write(
            f.0.join(name),
            if name == "analysis.json" {
                "{broken"
            } else {
                "{}"
            },
        )
        .unwrap();
    }
    assert!(store::database(&f.0, "", "migrate", None).is_err());
    assert_eq!(
        fs::read_to_string(f.0.join("analysis.json")).unwrap(),
        "{broken"
    );
}
#[test]
fn interrupted_multi_table_save_is_completed_on_startup() {
    let f = Fixture::new();
    let target = json!({"appearance.json":{"chat:a":{"value":{"label":"Recovered"},"hidden":false,"sync":{}}},"analysis.json":{},"chat_links.json":{},"transfers.json":{}});
    fs::write(f.0.join(".state-commit.json"), target.to_string()).unwrap();
    assert_eq!(f.call("load", "", None)["chat:a"]["label"], "Recovered");
    assert!(!f.0.join(".state-commit.json").exists());
}
#[test]
fn flow_generation_prevents_resurrecting_a_deleted_link() {
    let f = Fixture::new();
    f.call("links-save", "a", Some(link("a", "peer")));
    f.call("links-delete", "a", None);
    f.call(
        "links-flow",
        "a",
        Some(json!({"generation":"one","phase":"transferred"})),
    );
    assert_eq!(f.call("links-get", "", None), json!([]));
}

fn peers() -> Vec<Fixture> {
    let all: Vec<_> = (0..3).map(|_| Fixture::new()).collect();
    for (i, f) in all.iter().enumerate() {
        fs::write(
            f.0.join("pairing_ed25519.pub"),
            format!("ssh-ed25519 fixture-{i}"),
        )
        .unwrap();
        fs::write(f.0.join("role"), if i == 0 { "master" } else { "client" }).unwrap();
    }
    let ids: Vec<_> = all.iter().map(|f| store::identity(&f.0).unwrap()).collect();
    for (i, f) in all.iter().enumerate() {
        let mut approvals = json!({});
        let mut devices = Vec::new();
        for (j, id) in ids.iter().enumerate() {
            if j != i {
                approvals[id] = json!({"device_id":format!("peer-{j}")});
                devices.push(json!({"id":format!("peer-{j}"),"name":(["ROG","KDell","KJetson"][j]),"ssh_alias":format!("alias-{j}")}));
            }
        }
        fs::write(
            f.0.join("pairing.json"),
            json!({"approved":approvals}).to_string(),
        )
        .unwrap();
        fs::write(f.0.join("devices.json"), json!(devices).to_string()).unwrap();
        f.call("save","",Some(json!({"device:master":{"label":(["ROG","KDell","KJetson"][i]),"icon":"desktop","color":"#3d5a80"}})));
        shared::exchange(&f.0, None).unwrap();
    }
    all
}
fn converge(all: &[Fixture]) {
    for _ in 0..2 {
        for a in all {
            let mut packet = shared::exchange(&a.0, None).unwrap();
            for b in all {
                if a.0 != b.0 {
                    let reply = shared::exchange(&b.0, Some(&packet)).unwrap();
                    packet = shared::exchange(&a.0, Some(&reply)).unwrap();
                }
            }
        }
    }
}
#[test]
fn native_peer_exchange_preserves_names_icons_links_positions_and_scopes() {
    let all = peers();
    let root = &all[0];
    root.call(
        "save",
        "",
        Some(json!({"chat:a":{"label":"Oaclis","icon":"chat","color":"#ee6c4d"}})),
    );
    root.call("links-save", "a:master:peer-1", Some(link("a", "peer-1")));
    root.call(
        "chat-scopes",
        "a",
        Some(json!({"chat":true,"paths":false,"apps":true})),
    );
    shared::save_positions(
        &root.0,
        &json!({"master":{"x":200,"y":100},"peer-1":{"x":400,"y":500}}),
    )
    .unwrap();
    shared::write_positions(
        &root.0,
        "link-position",
        &json!({"id":"a","source":"master","target":"peer-1","x":-60,"y":20}),
    )
    .unwrap();
    converge(&all);
    let client = &all[1];
    assert_eq!(
        client.call("load", "", None)["device:master"]["label"],
        "KDell"
    );
    assert_eq!(client.call("load", "", None)["chat:a"]["label"], "Oaclis");
    let links = client.call("links-get", "", None);
    assert_eq!(links[0]["source"], "peer-0");
    assert_eq!(links[0]["target"], "master");
    assert_eq!(links[0]["scopes"]["paths"], false);
    assert_eq!(shared::positions(&client.0).unwrap()["master"]["x"], 400);
    assert_eq!(
        shared::read_positions(&client.0, "link-positions.json").unwrap()["a:master:peer-0"]["x"],
        -60
    );
    root.call("links-delete", "a:master:peer-1", None);
    converge(&all);
    assert_eq!(client.call("links-get", "", None), json!([]));
    assert_eq!(client.call("load", "", None)["chat:a"]["icon"], "chat");
    assert_eq!(
        client.call("preferences-get", "", None)["preferences"]["a"]["hidden"],
        false
    );
}
#[test]
fn stable_exchange_does_not_rewrite_json_and_unapproved_peers_are_rejected() {
    let all = peers();
    converge(&all);
    let root = &all[0];
    let path = root.0.join("appearance.json");
    shared::exchange(&root.0, None).unwrap();
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    shared::exchange(&root.0, None).unwrap();
    assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), before);
    assert!(
        shared::exchange(
            &root.0,
            Some(&json!({"schema":1,"sender":"intruder","records":{}}))
        )
        .is_err()
    );
}
#[test]
fn source_owned_inventory_and_cached_path_entries_survive_round_trip() {
    let all = peers();
    let root = &all[0];
    let peer = &all[1];
    root.call("preparation-save","master:a",Some(json!({"status":"ready","chat_bytes":42,"chat_modified_ns":"123","measured_at":9,"paths":[{"path":"/root/app","sha256":"abc","entries":["one"]}],"paths_bytes":120,"path_files":1})));
    root.call(
        "inventory-save",
        "master:a",
        Some(json!({"status":"ready","requirements":[{"name":"node"}]})),
    );
    peer.call(
        "inventory-save",
        "master:a",
        Some(json!({"status":"ready","requirements":[{"name":"python"}]})),
    );
    converge(&all);
    assert!(
        root.call("preparation-get", "master:a", None)["paths"][0]
            .get("entries")
            .is_some()
    );
    assert!(
        peer.call("preparation-get", "peer-0:a", None)["paths"][0]
            .get("entries")
            .is_none()
    );
    assert_eq!(
        root.call("inventory-get", "peer-1:a", None)["requirements"][0]["name"],
        "python"
    );
    assert_eq!(
        peer.call("inventory-get", "peer-0:a", None)["requirements"][0]["name"],
        "node"
    );
}
#[test]
fn chat_proof_matches_prefix_and_uses_activity_not_copy_time() {
    let f = Fixture::new();
    let a = f.0.join("a");
    let b = f.0.join("b");
    let meta = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"chat\",\"cwd\":\"/work\"},\"timestamp\":\"2026-01-01T00:00:00Z\"}\n";
    fs::write(&a, meta).unwrap();
    fs::write(&b,format!("{meta}{{\"type\":\"event_msg\",\"payload\":{{}},\"timestamp\":\"2026-01-01T00:00:01.1Z\"}}\n")).unwrap();
    let short = crate::chat::proof(&a, "chat", meta.len() as u64).unwrap();
    let long = crate::chat::proof(&b, "chat", meta.len() as u64).unwrap();
    assert_eq!(short["sha256"], long["prefix"]);
    assert_eq!(long["records"], 2);
    assert_eq!(long["activity"], "2026-01-01T00:00:01.100000000Z");
    assert!(crate::chat::proof(&a, "other", 0).is_err());
    fs::write(&a, "{incomplete").unwrap();
    assert!(crate::chat::proof(&a, "chat", 0).is_err());
}
#[test]
fn native_socket_carries_settings_and_runtime_without_http_or_python() {
    let all = peers();
    let root = &all[0];
    let config = root.0.clone();
    let codex = root.0.join("codex");
    fs::create_dir(&codex).unwrap();
    let runtime = std::sync::Arc::new(std::sync::Mutex::new(crate::device_runtime::Runtime::new(
        &config,
    )));
    crate::state_transport::start(config.clone(), codex.clone(), runtime).unwrap();
    let packet = shared::exchange(&all[1].0, None).unwrap();
    let response =
        crate::state_transport::local(&config, &json!({"kind":"shared-exchange","value":packet}))
            .unwrap();
    assert_eq!(response["runtime"]["available"], true);
    assert!(response["runtime"]["active"].is_boolean());
    assert!(response["runtime"]["build"]["source_id"].is_string());
    assert_eq!(
        crate::state_transport::local(&config, &json!({"kind":"catalog"})).unwrap()["chats"],
        json!([])
    );
    assert!(crate::state_transport::local(&config, &json!({"kind":"invalid"})).is_err());
    assert!(
        crate::state_transport::local(&config, &json!({"kind":"database","operation":"links-get"}))
            .unwrap()
            .is_array()
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(config.join("state.sock"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}
#[test]
fn shared_validation_rejects_invalid_records_before_any_write() {
    for record in [
        json!({"position:a":{"clock":[0,"a"],"value":{"x":-1,"y":0}}}),
        json!({"device:a":{"clock":[-1,"a"],"value":null}}),
        json!({"chat:a":{"clock":[0,"a"],"value":{"icon":"<script>"}}}),
        json!({"chatdata:a:b":{"clock":[0,"a"],"value":{"unexpected":{}}}}),
    ] {
        assert!(crate::shared_records::validate(&record).is_err());
    }
}
