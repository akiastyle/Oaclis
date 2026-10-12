"""LAN identities, signed discovery and mutually approved OpenSSH access."""
import base64
import fcntl
import hashlib
import ipaddress
import json
import os
from pathlib import Path
import platform
import re
import socket
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
import uuid


def run(args, **kwargs):
    result = subprocess.run(args, capture_output=True, timeout=15, **kwargs)
    if result.returncode:
        raise ValueError(result.stderr.decode(errors='replace').strip() or 'Comando non riuscito')
    return result.stdout


def public_key(value):
    if not isinstance(value, str) or not re.fullmatch(r'ssh-ed25519 [A-Za-z0-9+/]{40,120}={0,2}', value):
        raise ValueError('Chiave pubblica non valida')
    blob = base64.b64decode(value.split()[1], validate=True)
    if len(blob) != 51 or blob[:19] != b'\x00\x00\x00\x0bssh-ed25519\x00\x00\x00 ':
        raise ValueError('Chiave Ed25519 non valida')
    return value


def identifier(key):
    return hashlib.sha256(public_key(key).encode()).hexdigest()[:32]


def private_address(address):
    ip = ipaddress.IPv4Address(address)
    if not (ip.is_private and not ip.is_unspecified and not ip.is_multicast):
        raise ValueError('Il rilevamento richiede un indirizzo IPv4 locale')
    return str(ip)


def validate(card):
    key = public_key(card['public_key'])
    if card['id'] != identifier(key):
        raise ValueError('Identità non valida')
    if not isinstance(card['name'], str) or not card['name'].strip() or len(card['name']) > 160 or any(ord(c) < 32 for c in card['name']):
        raise ValueError('Nome non valido')
    if card['role'] not in ('master', 'client'):
        raise ValueError('Ruolo non valido')
    ssh = card['ssh']
    private_address(ssh['address'])
    if type(ssh['port']) is not int or not 1 <= ssh['port'] <= 65535:
        raise ValueError('Porta non valida')
    if not re.fullmatch(r'[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}', ssh['user']):
        raise ValueError('Utente non valido')
    public_key(ssh['host_key'])
    approved = card['approved']
    if not isinstance(approved, list) or len(approved) > 64 or any(not isinstance(i, str) or not re.fullmatch('[a-f0-9]{32}', i) for i in approved):
        raise ValueError('Approvazioni non valide')
    if type(card['time']) is not int or abs(time.time() - card['time']) > 30:
        raise ValueError('Annuncio scaduto: verificare gli orologi')
    return card


def state(config):
    path = config / 'pairing.json'
    return json.loads(path.read_text()) if path.exists() else {'approved': {}, 'ignored': []}


def identity(config):
    key = config / 'pairing_ed25519'
    if key.is_symlink() or key.with_suffix('.pub').is_symlink():
        raise ValueError('Chiave non regolare')
    if not key.exists():
        run(['ssh-keygen', '-q', '-t', 'ed25519', '-N', '', '-f', str(key), '-C', 'oaclis-pairing'])
    os.chmod(key, 0o600)
    value = ' '.join(key.with_suffix('.pub').read_text().split()[:2])
    return key, public_key(value)


def label(config):
    path = config / 'appearance.sqlite'
    if path.exists():
        with sqlite3.connect(f'file:{path}?mode=ro', uri=True) as db:
            if db.execute("SELECT 1 FROM sqlite_master WHERE name='appearance'").fetchone():
                row = db.execute("SELECT value FROM appearance WHERE key='device:master'").fetchone()
                if row:
                    return json.loads(row[0]).get('label') or socket.gethostname()
    return socket.gethostname()


def prepare(home, config):
    _, key = identity(config)
    host_key = ' '.join(Path('/etc/ssh/ssh_host_ed25519_key.pub').read_text().split()[:2])
    wsl = bool(os.environ.get('WSL_DISTRO_NAME')) or 'microsoft' in platform.release().lower()
    addresses = json.loads(run(['ip', '-j', '-4', 'addr', 'show', 'up']).decode())
    linux = [a['local'] for nic in addresses for a in nic.get('addr_info', []) if a.get('scope') == 'global']
    if not linux:
        raise ValueError('Nessuna rete IPv4 disponibile')
    address = linux[0]
    if wsl:
        script = "Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -and $_.InterfaceAlias -notmatch 'vEthernet' } | ForEach-Object { $_.IPv4Address.IPAddress } | Select-Object -First 1"
        executable = shutil.which('powershell.exe') or '/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
        address = run([executable, '-NoProfile', '-NonInteractive', '-Command', script]).decode('utf-8-sig').strip()
    private_address(address)
    return {'id': identifier(key), 'public_key': key, 'name': label(config), 'instance': str(uuid.uuid4()),
            'role': (config / 'role').read_text().strip() if (config / 'role').exists() else 'master',
            'ssh': {'address': address, 'port': 2222 if wsl and address != linux[0] else 22,
                    'user': __import__('pwd').getpwuid(os.getuid()).pw_name, 'host_key': public_key(host_key)},
            'wsl': wsl, 'wsl_ip': linux[0], 'distro': os.environ.get('WSL_DISTRO_NAME', '')}


def sign(config, card):
    key, _ = identity(config)
    card = dict(card, name=label(config), time=int(time.time()), approved=list(state(config)['approved']))
    validate(card)
    message = json.dumps(card, sort_keys=True, separators=(',', ':'))
    signature = run(['ssh-keygen', '-Y', 'sign', '-f', str(key), '-n', 'oaclis-lan'], input=message.encode()).decode()
    return {'hello': message, 'signature': signature}


def verify(packet):
    if not isinstance(packet['hello'], str) or len(packet['hello']) > 6000 or not isinstance(packet['signature'], str) or len(packet['signature']) > 1500:
        raise ValueError('Annuncio troppo grande')
    card = validate(json.loads(packet['hello']))
    with tempfile.TemporaryDirectory(prefix='oaclis-verify-') as directory:
        signer = Path(directory) / 'signers'
        signature = Path(directory) / 'signature'
        signer.write_text('oaclis ' + card['public_key'] + '\n')
        signature.write_text(packet['signature'])
        run(['ssh-keygen', '-Y', 'verify', '-f', str(signer), '-I', 'oaclis', '-n', 'oaclis-lan', '-s', str(signature)], input=packet['hello'].encode())
    return card


def approve(config, card, accept, atomic):
    validate(card)
    saved = state(config)
    if accept:
        if card['id'] not in saved['approved'] and len(saved['approved']) >= 64:
            raise ValueError('Limite di 64 dispositivi raggiunto')
        saved['approved'][card['id']] = {'public_key': card['public_key']}
        saved['ignored'] = [i for i in saved['ignored'] if i != card['id']]
    else:
        # A rejection is only available before provisioning SSH access.
        if saved['approved'].get(card['id'], {}).get('device_id'):
            raise ValueError('Dispositivo già collegato: eliminarlo dalla mappa')
        saved['approved'].pop(card['id'], None)
        saved['ignored'] = (saved['ignored'] + [card['id']])[-64:]
    atomic(config / 'pairing.json', json.dumps(saved))
    return {'ok': True}


def append_unique(path, line, atomic):
    text = path.read_text() if path.exists() else ''
    if line not in text.splitlines():
        atomic(path, text.rstrip('\n') + ('\n' if text else '') + line + '\n')


def forget(home, config, device_id, atomic):
    saved = state(config)
    for identifier_, approval in list(saved['approved'].items()):
        if approval.get('device_id') != device_id:
            continue
        line = approval['public_key'] + ' oaclis-peer-' + identifier_
        path = home / '.ssh/authorized_keys'
        if path.exists():
            atomic(path, ''.join(item + '\n' for item in path.read_text().splitlines() if item != line))
        saved['approved'].pop(identifier_)
        saved['ignored'] = (saved['ignored'] + [identifier_])[-64:]
    atomic(config / 'pairing.json', json.dumps(saved))
    return {'ok': True}


def commit(home, config, card, settings, atomic):
    validate(card)
    saved = state(config)
    _, key = identity(config)
    approval = saved['approved'].get(card['id'])
    if not approval or approval['public_key'] != card['public_key'] or identifier(key) not in card['approved']:
        raise ValueError('Sono richieste entrambe le autorizzazioni')
    # A paired key is scoped to this installation; never copy a private key.
    directory = home / '.ssh'
    if directory.is_symlink():
        raise ValueError('Cartella SSH non regolare')
    directory.mkdir(mode=0o700, exist_ok=True)
    known = config / 'known_hosts'
    ssh = card['ssh']
    endpoint = ssh['address'] if ssh['port'] == 22 else f"[{ssh['address']}]:{ssh['port']}"
    if known.exists():
        found = subprocess.run(['ssh-keygen', '-F', endpoint, '-f', str(known)], capture_output=True, text=True, timeout=3)
        for line in found.stdout.splitlines():
            if not line.startswith('#') and ' '.join(line.split()[1:3]) != ssh['host_key']:
                raise ValueError('Chiave host cambiata: verificare il dispositivo prima di collegarlo')
    append_unique(known, endpoint + ' ' + ssh['host_key'], atomic)
    devices_path = config / 'devices.json'
    devices = json.loads(devices_path.read_text()) if devices_path.exists() else []
    existing = settings(home, config, 'list', devices)
    device = next((d for d in devices if all(existing.get(d['id'], {}).get(k) == ssh[k] for k in ('address', 'port', 'user'))), None)
    if not device:
        device = {'id': 'peer-' + card['id'], 'name': card['name'], 'ssh_alias': 'oaclis-peer-' + card['id']}
        devices.append(device)
    settings(home, config, 'save', dict(device, paired=True, settings=dict(address=ssh['address'], port=ssh['port'], user=ssh['user'], identity_file=str(config / 'pairing_ed25519'))))
    # Commit metadata before granting access, so retries repair an interrupted setup.
    atomic(devices_path, json.dumps(devices))
    approval['device_id'] = device['id']
    atomic(config / 'pairing.json', json.dumps(saved))
    append_unique(directory / 'authorized_keys', card['public_key'] + ' oaclis-peer-' + card['id'], atomic)
    return {'device_id': device['id']}


def request(home, config, operation, value, source):
    helpers = {'__name__': 'oaclis_ssh_settings'}
    exec(source, helpers)
    with open(config / 'pairing.lock', 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if operation == 'prepare': return prepare(home, config)
        if operation == 'sign': return sign(config, value)
        if operation == 'verify': return verify(value)
        if operation == 'state': return state(config)
        if operation == 'forget': return forget(home, config, value['id'], helpers['atomic'])
        if operation == 'restore':
            saved = state(config)
            saved['ignored'] = []
            helpers['atomic'](config / 'pairing.json', json.dumps(saved))
            return {'ok': True}
        if operation == 'approve': return approve(config, value['peer'], value['accept'], helpers['atomic'])
        if operation == 'commit': return commit(home, config, value, helpers['settings'], helpers['atomic'])
        raise ValueError('Operazione sconosciuta')


if __name__ == '__main__':
    try:
        value = json.load(sys.stdin)
        print(json.dumps(request(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], value['value'], value['ssh_source'])))
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(json.dumps({'error': str(error)}))
        sys.exit(1)
