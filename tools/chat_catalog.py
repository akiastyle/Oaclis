"""Read Codex chat names and rollout locations; never modify the archive."""
import json
import os
import pathlib
import sqlite3
import sys

root = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 and sys.argv[1] else os.environ.get("CODEX_HOME", pathlib.Path.home() / ".codex")).absolute()
names, hidden = {}, set()
database = root / "state_5.sqlite"
if not database.exists():
    database = root / "sqlite/state_5.sqlite"
if database.is_file():
    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True, timeout=2) as db:
        for chat_id, name, title, source in db.execute("SELECT id,name,title,source FROM threads"):
            if source and source.startswith("{") and "subagent" in json.loads(source):
                hidden.add(chat_id)
            if (name or title or "").strip():
                names[chat_id] = (name or title).strip()
index = root / "session_index.jsonl"
if index.is_file():
    with index.open() as stream:
        for line in stream:
            try:
                entry = json.loads(line)
                if entry.get("thread_name", "").strip():
                    names.setdefault(entry["id"], entry["thread_name"])
            except (ValueError, KeyError):
                pass
chats = []
for directory, _, files in os.walk(root / "sessions", followlinks=False):
    for filename in files:
        path = pathlib.Path(directory) / filename
        if path.is_symlink() or not path.is_file() or not filename.startswith("rollout-") or not filename.endswith(".jsonl") or len(filename) < 42:
            continue
        chat_id = filename[-42:-6]
        if chat_id not in hidden:
            chats.append({"id": chat_id, "name": names.get(chat_id, "Senza titolo"), "path": str(path), "relative_path": str(path.relative_to(root)), "bytes": path.stat().st_size, "modified_ns": str(path.stat().st_mtime_ns)})
settings = pathlib.Path(os.environ.get("OACLIS_SETTINGS_DIR", pathlib.Path.home() / ".config/oaclis")) / "appearance.sqlite"
if settings.is_file():
    with sqlite3.connect(settings.as_uri() + "?mode=ro", uri=True, timeout=2) as db:
        if db.execute("SELECT 1 FROM sqlite_master WHERE name='analysis'").fetchone():
            records = {key: json.loads(value) for key, value in db.execute("SELECT key,value FROM analysis WHERE value IS NOT NULL")}
            for chat in chats:
                record = records.get("master:" + chat["id"], {"status": "missing"})
                revision = "environment-v3:master:{}:{}:{}".format(json.dumps(chat["path"], ensure_ascii=False), chat["bytes"], json.dumps(chat["modified_ns"]))
                if record.get("status") in ("ready", "failed") and record.get("revision") != revision:
                    record = {"status": "pending"}
                chat["analysis"] = record
chats.sort(key=lambda c: c["path"], reverse=True)
print(json.dumps({"root": str(root), "chats": chats}))
