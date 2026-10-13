"""Local presentation database; uses the same Python runtime as the chat catalog."""
import json
import os
import sqlite3
import sys
from pathlib import Path

path, operation = Path(sys.argv[1]), sys.argv[2]
os.umask(0o077)
if operation in ("preferences-get", "chat-hide", "chat-scopes", "device-forget"):
    with sqlite3.connect(path) as db:
        db.execute("CREATE TABLE IF NOT EXISTS chat_settings (id TEXT PRIMARY KEY, hidden INTEGER NOT NULL DEFAULT 0, scopes TEXT)")
        db.execute("CREATE TABLE IF NOT EXISTS chat_links (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        key = sys.argv[3] if len(sys.argv) > 3 else ""
        # shortcut: ripristino chat nascoste dal database; aggiungere un comando UI quando richiesto.
        if operation == "chat-hide":
            db.execute("INSERT INTO chat_settings(id,hidden) VALUES(?,1) ON CONFLICT(id) DO UPDATE SET hidden=1", (key,))
        if operation == "chat-scopes":
            scopes = json.dumps(json.load(sys.stdin))
            db.execute("INSERT INTO chat_settings(id,scopes) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET scopes=excluded.scopes", (key, scopes))
            if db.execute("SELECT 1 FROM sqlite_master WHERE name='analysis'").fetchone():
                for (analysis_key,) in db.execute("SELECT key FROM analysis").fetchall():
                    if analysis_key.split(':', 1)[-1] == key:
                        db.execute("UPDATE analysis SET scopes=? WHERE key=?", (scopes, analysis_key))
        for link_key, raw in db.execute("SELECT key,value FROM chat_links").fetchall():
            link = json.loads(raw)
            matches = link['entry']['id'] == key if operation.startswith('chat-') else key in (link['source'], link['target'])
            if not matches:
                continue
            if operation in ('chat-hide', 'device-forget'):
                db.execute("DELETE FROM chat_links WHERE key=?", (link_key,))
            elif operation == 'chat-scopes':
                link['scopes'] = json.loads(scopes)
                db.execute("UPDATE chat_links SET value=? WHERE key=?", (json.dumps(link), link_key))
        if operation in ('chat-hide', 'device-forget'):
            if db.execute("SELECT 1 FROM sqlite_master WHERE name='appearance'").fetchone():
                db.execute("DELETE FROM appearance WHERE key=?", (('chat:' if operation == 'chat-hide' else 'device:') + key,))
            if db.execute("SELECT 1 FROM sqlite_master WHERE name='analysis'").fetchone():
                for (analysis_key,) in db.execute("SELECT key FROM analysis").fetchall():
                    source, _, chat = analysis_key.partition(':')
                    if (chat == key if operation == 'chat-hide' else source == key):
                        db.execute("DELETE FROM analysis WHERE key=?", (analysis_key,))
        preferences = {identifier: {'hidden': bool(hidden), **({'scopes': json.loads(scopes)} if scopes else {})} for identifier, hidden, scopes in db.execute("SELECT id,hidden,scopes FROM chat_settings")}
        print(json.dumps({'preferences': preferences, 'links': [json.loads(value) for (value,) in db.execute("SELECT value FROM chat_links ORDER BY key")]}))
elif operation.startswith("links-"):
    with sqlite3.connect(path) as db:
        db.execute("CREATE TABLE IF NOT EXISTS chat_links (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        if operation == "links-save":
            value = json.load(sys.stdin)
            if db.execute("SELECT 1 FROM sqlite_master WHERE name='chat_settings'").fetchone():
                preference = db.execute("SELECT hidden,scopes FROM chat_settings WHERE id=?", (value['entry']['id'],)).fetchone()
                if preference and preference[0]:
                    raise ValueError("Chat rimossa da Oaclis")
                if preference and preference[1]:
                    value['scopes'] = json.loads(preference[1])
            db.execute("INSERT INTO chat_links(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (sys.argv[3], json.dumps(value)))
        elif operation == "links-delete":
            db.execute("DELETE FROM chat_links WHERE key=?", (sys.argv[3],))
        print(json.dumps([json.loads(value) for (value,) in db.execute("SELECT value FROM chat_links ORDER BY key")]))
elif operation in ("transfer-save", "transfer-list"):
    with sqlite3.connect(path) as db:
        db.execute("CREATE TABLE IF NOT EXISTS transfers (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        if operation == "transfer-save":
            db.execute("INSERT INTO transfers VALUES (?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (sys.argv[3], json.dumps(json.load(sys.stdin))))
        print(json.dumps({key: json.loads(value) for key, value in db.execute("SELECT key,value FROM transfers")}))
elif operation == "analysis-list":
    with sqlite3.connect(path) as db:
        exists = db.execute("SELECT 1 FROM sqlite_master WHERE name='analysis'").fetchone()
        print(json.dumps({key: json.loads(value) for key, value in db.execute("SELECT key,value FROM analysis WHERE value IS NOT NULL")} if exists else {}))
elif operation.startswith("analysis-"):
    key = sys.argv[3]
    with sqlite3.connect(path) as db:
        db.execute("CREATE TABLE IF NOT EXISTS analysis (key TEXT PRIMARY KEY, value TEXT, scopes TEXT NOT NULL DEFAULT '{\"chat\":true,\"paths\":true,\"apps\":true}')")
        if operation == "analysis-save":
            db.execute("INSERT INTO analysis(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", (key, json.dumps(json.load(sys.stdin))))
        elif operation == "analysis-scopes":
            db.execute("INSERT INTO analysis(key,scopes) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET scopes=excluded.scopes", (key, json.dumps(json.load(sys.stdin))))
        row = db.execute("SELECT value,scopes FROM analysis WHERE key=?", (key,)).fetchone()
        value = json.loads(row[0]) if row and row[0] else {"status":"missing"}
        value["scopes"] = json.loads(row[1]) if row else {"chat":True,"paths":True,"apps":True}
        if db.execute("SELECT 1 FROM sqlite_master WHERE name='chat_settings'").fetchone():
            preference = db.execute("SELECT scopes FROM chat_settings WHERE id=?", (key.split(':', 1)[-1],)).fetchone()
            if preference and preference[0]:
                value['scopes'] = json.loads(preference[0])
        print(json.dumps(value))
elif operation == "load":
    with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True) as db:
        exists = db.execute("SELECT 1 FROM sqlite_master WHERE name='appearance'").fetchone()
        print(json.dumps(dict((key, json.loads(value)) for key, value in db.execute("SELECT key, value FROM appearance")) if exists else {}))
else:
    entries = json.load(sys.stdin)
    with sqlite3.connect(path) as db:
        db.execute("CREATE TABLE IF NOT EXISTS appearance (key TEXT PRIMARY KEY, value TEXT NOT NULL)")
        db.executemany("INSERT INTO appearance VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value=excluded.value", [(key, json.dumps(value)) for key, value in entries.items()])
