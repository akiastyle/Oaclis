# shared_settings.py v1.11.323
"""Replicate presentation settings over mutually approved SSH; no chat/auth files."""
import fcntl
import hashlib
import json
import os
import pathlib
import re
import shlex
import sqlite3
import subprocess
import sys

if 'unified_ensure' not in globals():
    from chat_records import *

LIMIT = 16 * 1024 * 1024
KINDS = {'device', 'chat', 'preference', 'link', 'position', 'linkposition', 'chatdata'}

def read(path, default):
    if path.is_symlink():
        raise ValueError('Configurazione tramite link non consentita')
    return json.loads(path.read_text()) if path.exists() else default

def atomic(path, value):
    from tempfile import NamedTemporaryFile
    if path.is_symlink():
        raise ValueError('Configurazione tramite link non consentita')
    with NamedTemporaryFile('w', dir=path.parent, delete=False) as file:
        json.dump(value, file, ensure_ascii=False)
        file.flush(); os.fsync(file.fileno())
        temporary = file.name
    os.replace(temporary, path)

def identifier(value):
    return isinstance(value, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,200}', value)

def validate(records):
    if not isinstance(records, dict) or len(records) > 10000:
        raise ValueError('Impostazioni condivise non valide')
    for key, record in records.items():
        parts = key.split(':')
        if parts[0] not in KINDS or any(not identifier(p) for p in parts[1:]) or len(parts) != (4 if parts[0] in ('link', 'linkposition') else 3 if parts[0] == 'chatdata' else 2):
            raise ValueError('Oggetto condiviso non valido')
        clock = record.get('clock') if isinstance(record, dict) else None
        if not isinstance(clock, list) or len(clock) != 2 or type(clock[0]) is not int or not 0 <= clock[0] < 2**53 or not identifier(clock[1]):
            raise ValueError('Revisione condivisa non valida')
        value = record.get('value')
        if value is None:
            continue
        if not isinstance(value, dict):
            raise ValueError('Valore condiviso non valido')
        if parts[0] == 'chatdata' and (set(value) - set(MODULES) or any(not isinstance(v, dict) for v in value.values())):
            raise ValueError('Dati sorgenti non validi')
        if parts[0] == 'device':
            name = value.get('name')
            if not isinstance(name, str) or not name.strip() or len(name) > 160 or any(ord(c) < 32 for c in name):
                raise ValueError('Nome dispositivo non valido')
        if parts[0] in ('device', 'chat'):
            appearance = value.get('appearance', {}) if parts[0] == 'device' else value
            if set(appearance) - {'label', 'icon', 'color'} or any(not isinstance(v, str) for v in appearance.values()):
                raise ValueError('Aspetto non valido')
            if len(appearance.get('label', '')) > 320 or len(appearance.get('icon', '')) > 2 * 1024 * 1024:
                raise ValueError('Aspetto troppo grande')
            icon = appearance.get('icon', '')
            if icon not in ('', 'chat', 'code', 'folder', 'desktop', 'laptop', 'server', 'terminal', 'globe', 'cloud', 'box', 'database', 'git', 'book', 'star', 'bolt', 'shield', 'bug', 'settings', 'cpu', 'rocket', 'link', 'image', 'wrench', 'layers') and not re.fullmatch(r'data:image/png;base64,[A-Za-z0-9+/=]+', icon):
                raise ValueError('Icona non valida')
            if any(ord(c) < 32 for c in appearance.get('label', '')):
                raise ValueError('Etichetta non valida')
            if appearance.get('color') and not re.fullmatch('#[a-fA-F0-9]{6}', appearance['color']):
                raise ValueError('Colore non valido')
        if parts[0] in ('preference', 'link'):
            scopes = value.get('scopes')
            if scopes is not None and (set(scopes) != {'chat', 'paths', 'apps'} or any(type(v) is not bool for v in scopes.values())):
                raise ValueError('Categorie non valide')
        if parts[0] == 'preference' and any(name in value and type(value[name]) is not bool for name in ('hidden', 'list_hidden')):
            raise ValueError('Visibilità chat non valida')
        if parts[0] == 'link':
            name = value.get('entry', {}).get('name')
            label = value.get('label', '')
            if not isinstance(name, str) or len(name) > 320 or any(ord(c) < 32 for c in name) or not isinstance(label, str) or len(label) > 160:
                raise ValueError('Titolo collegamento non valido')
        if parts[0] == 'link' and (value.get('source') not in parts[2:] or value.get('target') not in parts[2:] or value['source'] == value['target'] or value.get('operation') not in ('copy', 'send', 'sync') or value.get('entry', {}).get('id') != parts[1]):
            raise ValueError('Collegamento non valido')
        if parts[0] in ('position', 'linkposition') and (set(value) != {'x', 'y'} or any(type(v) not in (int, float) or not (-100000 if parts[0] == 'linkposition' else 0) <= v <= 100000 for v in value.values())):
            raise ValueError('Posizione non valida')

def merge(records, incoming):
    validate(incoming)
    for key, record in incoming.items():
        if key not in records or tuple(record['clock']) > tuple(records[key]['clock']):
            records[key] = record

class Store:
    def __init__(self, home):
        self.home = pathlib.Path(home)
        self.config = self.home / '.config/oaclis'
        self.config.mkdir(parents=True, exist_ok=True)
        public = ' '.join((self.config / 'pairing_ed25519.pub').read_text().split()[:2])
        self.id = hashlib.sha256(public.encode()).hexdigest()[:32]

    def mapping(self, saved):
        mapping = dict(saved.get('mapping', {}))
        mapping['master'] = self.id
        for peer, approval in read(self.config / 'pairing.json', {}).get('approved', {}).items():
            if approval.get('device_id'):
                mapping[approval['device_id']] = peer
        for device in read(self.config / 'devices.json', []):
            mapping.setdefault(device['id'], 'manual-' + self.id + '-' + device['id'])
        saved['mapping'] = mapping
        return mapping

    def snapshot(self, db, mapping, saved):
        appearances = {}
        for key, raw in db.execute('SELECT key,value FROM appearance'):
            value = json.loads(raw)
            if value: appearances[key] = value
        devices = [{'id': 'master', 'name': appearances.get('device:master', {}).get('label') or os.uname().nodename}, *read(self.config / 'devices.json', [])]
        snapshot = {}
        for device in devices:
            key = 'device:' + mapping[device['id']]
            if saved.get('records', {}).get(key, {}).get('value', True) is None:
                continue
            appearance = appearances.get('device:' + device['id'], {})
            name = appearance.get('label') or device['name'] if device['id'] == 'master' else device['name']
            shared_appearance = {k: v for k, v in appearance.items() if k != 'label'}
            shared_appearance.setdefault('icon', 'desktop')
            if not shared_appearance.get('color'):
                colors = ['#3d5a80', '#98c1d9', '#e0fbfc', '#ee6c4d', '#293241']
                shared_appearance['color'] = colors[int(hashlib.sha256(mapping[device['id']].encode()).hexdigest()[:8], 16) % len(colors)]
            snapshot[key] = {'name': name, 'appearance': shared_appearance}
        for key, value in appearances.items():
            if key.startswith('chat:'):
                snapshot[key] = value
        for key, hidden, raw in db.execute("SELECT key,hidden,sync FROM appearance WHERE key LIKE 'chat:%'"):
            chat = key[5:]; value = json.loads(raw)
            if value.get('removed'): snapshot.pop(key, None)
            snapshot['preference:' + chat] = {'hidden':bool(value.get('removed')), 'list_hidden':bool(hidden), 'scopes':{name:bool(value.get(name,{}).get('enabled',1)) for name in MODULES}}
            sources = {source for name in MODULES for source in value.get(name,{}).get('sources',{})}
            for source in sources:
                snapshot['chatdata:' + chat + ':' + source] = {name:value[name]['sources'][source] for name in MODULES if source in value.get(name,{}).get('sources',{})}
        for _, raw in db.execute('SELECT key,value FROM chat_links'):
            value = json.loads(raw)
            if value['source'] not in mapping or value['target'] not in mapping:
                continue
            value.pop('transfer_pending', None)
            value['source'], value['target'] = mapping[value['source']], mapping[value['target']]
            key = ':'.join(['link', value['entry']['id'], *sorted([value['source'], value['target']])])
            snapshot[key] = value
        for local, position in read(self.config / 'positions.json', {}).items():
            if local in mapping:
                snapshot['position:' + mapping[local]] = position
        for key, point in read(self.config / 'link-positions.json', {}).items():
            chat, a, b = key.split(':')
            if a in mapping and b in mapping:
                snapshot[':'.join(['linkposition', chat, *sorted([mapping[a], mapping[b]])])] = point
        return snapshot

    def exchange(self, packet=None):
        with open(self.config / 'shared-settings.lock', 'a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            saved = read(self.config / 'shared-settings.json', {'records': {}, 'baseline': {}})
            mapping = self.mapping(saved)
            database = self.config / 'appearance.sqlite'
            if database.is_symlink():
                raise ValueError('Database tramite link non consentito')
            with sqlite3.connect(database, timeout=10) as db:
                db.execute('BEGIN IMMEDIATE')
                unified_ensure(db, self.config)
                db.execute('CREATE TABLE IF NOT EXISTS chat_links (key TEXT PRIMARY KEY, value TEXT NOT NULL)')
                current = self.snapshot(db, mapping, saved)
                records, baseline = saved['records'], saved['baseline']
                clock = max([r['clock'][0] for r in records.values()] + [1000000 if not records and (self.config / 'role').read_text().strip() == 'master' else 0])
                for key in current.keys() | baseline.keys():
                    if current.get(key) != baseline.get(key):
                        clock += 1
                        records[key] = {'clock': [clock, self.id], 'value': current.get(key)}
                if packet is not None:
                    approvals = read(self.config / 'pairing.json', {}).get('approved', {})
                    if packet.get('schema') != 1 or packet.get('sender') not in approvals or not approvals[packet['sender']].get('device_id'):
                        raise ValueError('Dispositivo non autorizzato a sincronizzare le impostazioni')
                    merge(records, packet['records'])
                validate(records)
                self.apply(db, records, mapping)
                saved['baseline'] = self.snapshot(db, mapping, saved)
                atomic(self.config / 'shared-settings.json', saved)
            return {'schema': 1, 'sender': self.id, 'records': records}

    def apply(self, db, records, mapping):
        reverse = {global_id: local for local, global_id in mapping.items()}
        devices = {d['id']: d for d in read(self.config / 'devices.json', [])}
        positions, link_positions = {}, {}
        for key, record in records.items():
            kind, identifier_ = key.split(':', 1)
            value = record['value']
            if kind == 'device':
                local = reverse.get(identifier_)
                if local is None and value is not None:
                    local = 'shared-' + hashlib.sha256(identifier_.encode()).hexdigest()[:32]
                    mapping[local] = identifier_; reverse[identifier_] = local
                if local is None:
                    continue
                if value is None:
                    devices.pop(local, None)
                    db.execute('DELETE FROM appearance WHERE key=?', ('device:' + local,))
                else:
                    appearance = dict({'icon': 'desktop', 'color': ''}, **value.get('appearance', {}))
                    appearance['label'] = value['name']
                    db.execute('INSERT INTO appearance(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value', ('device:' + local, json.dumps(appearance)))
                    if local != 'master':
                        device = devices.get(local, {'id': local, 'ssh_alias': 'oaclis-unconfigured-' + local})
                        device['name'] = value['name']; devices[local] = device
            elif kind == 'chat':
                if value is None:
                    db.execute("UPDATE appearance SET value='{}' WHERE key=?", (key,))
                else:
                    db.execute('INSERT INTO appearance(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value', (key, json.dumps(dict({'label': '', 'icon': '', 'color': ''}, **value))))
            elif kind == 'preference':
                current = chat_object(db, identifier_)
                if value is None:
                    current.pop('removed', None); save_chat_object(db, identifier_, current)
                    set_switches(db, identifier_, dict.fromkeys(MODULES, True))
                    hidden = False
                else:
                    if value.get('hidden'): current['removed'] = 1
                    else: current.pop('removed', None)
                    save_chat_object(db, identifier_, current)
                    if 'scopes' in value: set_switches(db, identifier_, value['scopes'])
                    hidden = value.get('hidden', False) or value.get('list_hidden', False)
                db.execute('UPDATE appearance SET hidden=? WHERE key=?', (int(hidden), 'chat:' + identifier_))
            elif kind == 'chatdata':
                chat, source = identifier_.split(':', 1)
                current = chat_object(db, chat)
                for name in MODULES:
                    sources = current.setdefault(name, {}).setdefault('sources', {})
                    if value is None or name not in value: sources.pop(source, None)
                    else: sources[source] = value[name]
                save_chat_object(db, chat, current)
        for key, record in records.items():
            kind, identifier_ = key.split(':', 1)
            value = record['value']
            if kind == 'link':
                chat, a, b = identifier_.split(':')
                if a not in reverse or b not in reverse:
                    continue
                local_key = ':'.join([chat, *sorted([reverse[a], reverse[b]])])
                if value is None or any(records.get('device:' + peer, {}).get('value', True) is None for peer in (a, b)):
                    db.execute('DELETE FROM chat_links WHERE key=?', (local_key,))
                    continue
                pending = db.execute('SELECT value FROM chat_links WHERE key=?', (local_key,)).fetchone()
                adapted = dict(value, source=reverse[value['source']], target=reverse[value['target']], transfer_pending=json.loads(pending[0]).get('transfer_pending', False) if pending else False)
                preference = records.get('preference:' + chat, {}).get('value') or {}
                if 'scopes' in preference:
                    adapted['scopes'] = preference['scopes']
                db.execute('INSERT OR REPLACE INTO chat_links VALUES(?,?)', (local_key, json.dumps(adapted)))
            elif kind == 'linkposition' and value is not None:
                chat, a, b = identifier_.split(':')
                if a in reverse and b in reverse and records.get('link:' + identifier_, {}).get('value') is not None:
                    link_positions[':'.join([chat, *sorted([reverse[a], reverse[b]])])] = value
            elif kind == 'position' and value is not None and identifier_ in reverse:
                positions[reverse[identifier_]] = value
        atomic(self.config / 'devices.json', list(devices.values()))
        atomic(self.config / 'positions.json', positions)
        atomic(self.config / 'link-positions.json', link_positions)

    def sync(self, source):
        packet = self.exchange()
        devices = {d['id']: d for d in read(self.config / 'devices.json', [])}
        errors = {}
        for peer, approval in read(self.config / 'pairing.json', {}).get('approved', {}).items():
            device = devices.get(approval.get('device_id'))
            if not device:
                continue
            try:
                command = 'python3 -c ' + shlex.quote(source) + ' exchange'
                result = subprocess.run(['ssh', '-o', 'BatchMode=yes', '-o', 'ConnectTimeout=3', '-o', 'StrictHostKeyChecking=yes', device['ssh_alias'], command], input=json.dumps(packet), text=True, capture_output=True, timeout=10)
                if result.returncode:
                    raise ValueError(result.stderr.strip()[:300] or 'Scambio impostazioni non riuscito')
                if len(result.stdout) > LIMIT:
                    raise ValueError('Impostazioni troppo grandi')
                response = json.loads(result.stdout)
                if response.get('sender') != peer:
                    raise ValueError('Identità del dispositivo SSH diversa da quella autorizzata')
                packet = self.exchange(response)
            except (ValueError, OSError, subprocess.TimeoutExpired) as error:
                errors[approval.get('device_id', peer)] = str(error)
        atomic(self.config / 'shared-status.json', {'errors': errors})
        return {'errors': errors}

if __name__ == '__main__':
    try:
        os.umask(0o077)
        store = Store(pathlib.Path.home())
        operation = sys.argv[1]
        packet = None
        if operation in ('exchange', 'positions', 'link-position'):
            raw = sys.stdin.read(LIMIT + 1)
            if len(raw) > LIMIT:
                raise ValueError('Impostazioni troppo grandi')
            packet = json.loads(raw)
        if operation == 'link-position':
            if any(not identifier(packet.get(k)) for k in ('id', 'source', 'target')) or packet['source'] == packet['target']:
                raise ValueError('Collegamento non valido')
            point = {'x': packet.get('x'), 'y': packet.get('y')}
            key = ':'.join([packet['id'], *sorted([packet['source'], packet['target']])])
            validate({'linkposition:' + key: {'clock': [0, store.id], 'value': point}})
            with open(store.config / 'shared-settings.lock', 'a') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                points = read(store.config / 'link-positions.json', {})
                points[key] = point
                atomic(store.config / 'link-positions.json', points)
            print(json.dumps({'ok': True})); sys.exit(0)
        if operation == 'positions':
            validate({'position:' + key: {'clock': [0, store.id], 'value': value} for key, value in packet.items()})
            with open(store.config / 'shared-settings.lock', 'a') as lock:
                fcntl.flock(lock, fcntl.LOCK_EX)
                atomic(store.config / 'positions.json', packet)
            print(json.dumps({'ok': True})); sys.exit(0)
        source = os.environ.get('OACLIS_SHARED_SOURCE')
        if operation == 'sync' and not source:
            source = pathlib.Path(sys.argv[2]).read_text()
        print(json.dumps(store.sync(source) if operation == 'sync' else store.exchange(packet)))
    except (ValueError, OSError, sqlite3.Error) as error:
        print(json.dumps({'error': str(error)})); sys.exit(1)
