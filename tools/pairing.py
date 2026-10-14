# pairing.py v1.19.318
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
    if card['role'] not in ('master', 'client', 'unconfigured'):
        raise ValueError('Ruolo non valido')
    ssh = card['ssh']
    private_address(ssh['address'])
    if type(ssh['port']) is not int or not 1 <= ssh['port'] <= 65535:
        raise ValueError('Porta non valida')
    if not re.fullmatch(r'[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}', ssh['user']):
        raise ValueError('Utente non valido')
    if ssh['host_key'] is not None:
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


def network_interfaces():
    """Active IPv4 interfaces, excluding loopback even when its scope is global."""
    if platform.system() == 'Linux':
        interfaces = json.loads(run(['ip', '-j', '-4', 'addr', 'show', 'up']).decode())
        try:
            routes = json.loads(run(['ip', '-j', '-4', 'route', 'show', 'default']).decode())
            default = routes[0]['dev'] if routes else None
        except (ValueError, KeyError, OSError):
            default = None
        result = []
        for interface in interfaces:
            if 'LOOPBACK' in interface.get('flags', []) or interface.get('operstate') == 'DOWN':
                continue
            for address in interface.get('addr_info', []):
                if address.get('scope') != 'global':
                    continue
                ip = ipaddress.IPv4Address(address['local'])
                if ip.is_loopback or ip.is_unspecified:
                    continue
                result.append(dict(name=interface['ifname'], address=str(ip),
                                   prefix=address['prefixlen'], broadcast=address.get('broadcast')))
        return sorted(result, key=lambda nic: nic['name'] != default)
    if platform.system() == 'Darwin':
        text = run(['ifconfig', '-a']).decode()
        result = []
        for block in re.split(r'(?m)(?=^[A-Za-z0-9]+:)', text):
            name = block.partition(':')[0]
            if not name or 'LOOPBACK' in block or 'status: inactive' in block:
                continue
            for address, mask in re.findall(r'\binet (\d+\.\d+\.\d+\.\d+) netmask (0x[0-9a-fA-F]+|[0-9.]+)', block):
                dotted = str(ipaddress.IPv4Address(int(mask, 16))) if mask.startswith('0x') else mask
                network = ipaddress.IPv4Network(f'{address}/{dotted}', strict=False)
                result.append(dict(name=name, address=address, prefix=network.prefixlen, broadcast=str(network.broadcast_address)))
        return result
    raise ValueError('Rilevamento rete non disponibile su questo sistema: usare un runtime Linux o macOS')


def windows_network(virtual_ip):
    executable = shutil.which('powershell.exe') or '/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
    script = r"""$ErrorActionPreference='Stop'; $nic=Get-NetIPConfiguration | Where-Object { $_.IPv4DefaultGateway -and $_.InterfaceAlias -notmatch 'vEthernet' } | Select-Object -First 1; $address=$nic.IPv4Address.IPAddress; $listeners=@(Get-NetTCPConnection -LocalPort 23232 -State Listen -ErrorAction SilentlyContinue); $udp=Get-NetFirewallRule -Name 'Oaclis-LAN-Discovery' -ErrorAction SilentlyContinue; $proxy=netsh interface portproxy show v4tov4; $mapping=$proxy | Where-Object { $_ -match ('^\s*'+[regex]::Escape($address)+'\s+23232\s+__VIRTUAL_IP__\s+23232\s*$') }; @{address=$address; ready=($mapping -and $listeners.Count -gt 0 -and $udp -and $udp.Enabled -eq 'True')} | ConvertTo-Json -Compress"""
    script = script.replace('__VIRTUAL_IP__', re.escape(virtual_ip))
    value = json.loads(run([executable, '-NoProfile', '-NonInteractive', '-Command', script]).decode('utf-8-sig'))
    return value['address'], bool(value['ready'])


def prepare(home, config):
    _, key = identity(config)
    host_path = Path('/etc/ssh/ssh_host_ed25519_key.pub')
    host_key = ' '.join(host_path.read_text().split()[:2]) if host_path.exists() else None
    wsl = platform.system() == 'Linux' and (bool(os.environ.get('WSL_DISTRO_NAME')) or 'microsoft' in platform.release().lower())
    interfaces = network_interfaces()
    if not interfaces:
        raise ValueError('Nessuna interfaccia IPv4 attiva: collegare il dispositivo alla rete')
    primary = interfaces[0]['address']
    address = primary
    needs_setup = False
    if wsl:
        address, ready = windows_network(primary)
        needs_setup = address != primary and not ready
    private_address(address)
    return {'id': identifier(key), 'public_key': key, 'name': label(config), 'instance': str(uuid.uuid4()),
            'role': (config / 'role').read_text().strip() if (config / 'role').exists() else 'unconfigured',
            'ssh': {'address': address, 'port': 2222 if wsl and address != primary else 22,
                    'user': __import__('pwd').getpwuid(os.getuid()).pw_name, 'host_key': public_key(host_key) if host_key else None},
            'broadcasts': [nic['broadcast'] for nic in interfaces if nic['broadcast']],
            'setup_needed': needs_setup, 'setup_kind': 'wsl-nat' if wsl and address != primary else None,
            'wsl': wsl, 'wsl_ip': primary, 'distro': os.environ.get('WSL_DISTRO_NAME', '')}


def sign(config, card):
    key, _ = identity(config)
    card = dict(card, name=label(config), time=int(time.time()), approved=list(state(config)['approved']))
    validate(card)
    message = json.dumps(card, sort_keys=True, separators=(',', ':'))
    signature = run(['ssh-keygen', '-Y', 'sign', '-f', str(key), '-n', 'oaclis-lan'], input=message.encode()).decode()
    return {'hello': message, 'signature': signature}


def neighbors():
    if platform.system() == 'Darwin':
        addresses = re.findall(r'\(([0-9.]+)\)', run(['arp', '-an']).decode())
    elif platform.system() == 'Linux':
        values = json.loads(run(['ip', '-j', '-4', 'neigh', 'show']).decode())
        addresses = [v['dst'] for v in values if v.get('lladdr') and not any(s in ('FAILED', 'INCOMPLETE') for s in v.get('state', []))]
        if os.environ.get('WSL_DISTRO_NAME') or 'microsoft' in platform.release().lower():
            executable = shutil.which('powershell.exe') or '/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
            script = "Get-NetNeighbor -AddressFamily IPv4 | Where-Object { $_.State -in 'Reachable','Stale','Delay','Probe','Permanent' } | ForEach-Object { $_.IPAddress }"
            addresses.extend(run([executable, '-NoProfile', '-NonInteractive', '-Command', script]).decode('utf-8-sig').split())
    else:
        raise ValueError('Tabella dei vicini non disponibile su questo sistema')
    result = []
    for address in addresses:
        try:
            address = private_address(address)
            if not ipaddress.IPv4Address(address).is_loopback and address not in result:
                result.append(address)
        except ValueError:
            pass
    return result[:32]


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
    if not ssh['host_key']:
        raise ValueError('SSH non pronto su ' + card['name'] + ': attivare il server SSH')
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
        if operation == 'neighbors': return neighbors()
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
