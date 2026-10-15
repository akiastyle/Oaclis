# chat_records.py v1.10.124
"""One chat record: presentation, visibility, switches and source-owned measurements."""
import hashlib
import json
import pathlib

MODULES = ('chat', 'paths', 'apps')

def source_mapping(config):
    config = pathlib.Path(config)
    def read(name, default):
        path = config / name
        if path.is_symlink():
            raise ValueError('Configurazione tramite link non consentita')
        return json.loads(path.read_text()) if path.exists() else default
    public = config / 'pairing_ed25519.pub'
    if public.is_symlink():
        raise ValueError('Identità tramite link non consentita')
    local = hashlib.sha256(' '.join(public.read_text().split()[:2]).encode()).hexdigest()[:32] if public.exists() else 'local'
    mapping = dict(read('shared-settings.json', {}).get('mapping', {}))
    mapping['master'] = local
    for peer, approval in read('pairing.json', {}).get('approved', {}).items():
        if approval.get('device_id'):
            mapping[approval['device_id']] = peer
    return mapping

def chat_object(db, chat):
    row = db.execute('SELECT sync FROM appearance WHERE key=?', ('chat:' + chat,)).fetchone()
    return json.loads(row[0]) if row and row[0] else {}

def save_chat_object(db, chat, value):
    db.execute("INSERT INTO appearance(key,value,sync) VALUES (?,'{}',?) ON CONFLICT(key) DO UPDATE SET sync=excluded.sync WHERE appearance.sync IS NOT excluded.sync", ('chat:' + chat, json.dumps(value)))

def set_switches(db, chat, scopes):
    value = chat_object(db, chat)
    for name in MODULES:
        value.setdefault(name, {})['enabled'] = int(scopes.get(name, True))
    save_chat_object(db, chat, value)

def save_inventory(db, key, inventory, config):
    source, chat = key.split(':', 1)
    source = source_mapping(config).get(source, source)
    value = chat_object(db, chat)
    value.setdefault('apps', {}).setdefault('sources', {})[source] = inventory
    save_chat_object(db, chat, value)

def save_preparation(db, key, data, config):
    source, chat = key.split(':', 1)
    source = source_mapping(config).get(source, source)
    value = chat_object(db, chat)
    value.setdefault('chat', {}).setdefault('sources', {})[source] = {'bytes':data['chat_bytes'], 'modified_ns':data['chat_modified_ns'], 'measured_at':data['measured_at']}
    value.setdefault('paths', {}).setdefault('sources', {})[source] = {'status':data['status'], 'items':data['paths'], 'bytes':data['paths_bytes'], 'files':data['path_files'], 'measured_at':data['measured_at']}
    save_chat_object(db, chat, value)

def unified_preferences(db, chat=None):
    fields = ','.join("CASE WHEN json_type(sync,'$." + name + ".enabled') IS NULL THEN 1 ELSE json_extract(sync,'$." + name + ".enabled') END" for name in MODULES)
    where, args = ("key=?", ('chat:' + chat,)) if chat is not None else ("key LIKE 'chat:%'", ())
    return {key[5:]: {'hidden':bool(hidden), 'scopes':dict(zip(MODULES, map(bool, flags)))}
            for key, hidden, *flags in db.execute('SELECT key,hidden,' + fields + ' FROM appearance WHERE ' + where, args)}

def unified_data(db, config):
    reverse = {global_id:local for local, global_id in source_mapping(config).items()}
    inventories, preparations = {}, {}
    for key, raw in db.execute("SELECT key,sync FROM appearance WHERE key LIKE 'chat:%'"):
        value = json.loads(raw or '{}'); chat = key[5:]
        for source, record in value.get('apps', {}).get('sources', {}).items():
            inventories[reverse.get(source, source) + ':' + chat] = record
        chats = value.get('chat', {}).get('sources', {})
        for source, record in value.get('paths', {}).get('sources', {}).items():
            if source not in chats:
                continue
            c = chats[source]
            preparations[reverse.get(source, source) + ':' + chat] = {'status':record['status'], 'chat_bytes':c['bytes'], 'chat_modified_ns':c['modified_ns'], 'measured_at':record['measured_at'], 'paths':record['items'], 'paths_bytes':record['bytes'], 'path_files':record['files']}
    return inventories, preparations

def unified_ensure(db, config):
    """Called inside a write transaction; malformed old JSON aborts the entire migration."""
    old_tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    old_columns = {row[1] for row in db.execute('PRAGMA table_info(appearance)')}
    legacy_scopes = 'scopes' in {row[1] for row in db.execute('PRAGMA table_info(analysis)')}
    if {'hidden', 'sync'} <= old_columns and not legacy_scopes and not old_tables.intersection({'chat_settings','chat_visibility','app_checks','preparations'}):
        return
    db.execute("CREATE TABLE IF NOT EXISTS appearance (key TEXT PRIMARY KEY, value TEXT NOT NULL, hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN (0,1)), sync TEXT NOT NULL DEFAULT '{}')")
    columns = {row[1] for row in db.execute('PRAGMA table_info(appearance)')}
    if 'hidden' not in columns:
        db.execute('ALTER TABLE appearance ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0 CHECK(hidden IN (0,1))')
    if 'sync' not in columns:
        db.execute("ALTER TABLE appearance ADD COLUMN sync TEXT NOT NULL DEFAULT '{}'")
    tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
    if 'chat_settings' in tables:
        for chat, hidden, scopes in db.execute('SELECT id,hidden,scopes FROM chat_settings').fetchall():
            set_switches(db, chat, json.loads(scopes) if scopes else {})
            if hidden:
                value = chat_object(db, chat); value.pop('removed', None)
                save_chat_object(db, chat, value)
                db.execute('UPDATE appearance SET hidden=1 WHERE key=?', ('chat:' + chat,))
    if 'chat_visibility' in tables:
        for chat, hidden in db.execute('SELECT id,hidden FROM chat_visibility').fetchall():
            value = chat_object(db, chat); save_chat_object(db, chat, value)
            db.execute('UPDATE appearance SET hidden=? WHERE key=?', (int(bool(hidden or value.get('removed'))), 'chat:' + chat))
    for table, save in [('app_checks', save_inventory), ('preparations', save_preparation)]:
        if table in tables:
            for key, raw in db.execute('SELECT key,value FROM ' + table).fetchall():
                save(db, key, json.loads(raw), config)
    # Legacy enablement is retained if the chat never had an explicit preference.
    if legacy_scopes:
        for key, raw in db.execute('SELECT key,scopes FROM analysis').fetchall():
            chat = key.split(':', 1)[-1]
            value = chat_object(db, chat)
            if not any('enabled' in value.get(name, {}) for name in MODULES):
                set_switches(db, chat, json.loads(raw) if raw else {})
        db.execute('ALTER TABLE analysis DROP COLUMN scopes')
    for table in ('chat_settings', 'chat_visibility', 'app_checks', 'preparations'):
        if table in tables:
            db.execute('DROP TABLE ' + table)
