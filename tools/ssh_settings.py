"""Explicit connection fields, translated to managed OpenSSH hosts."""
import ipaddress
import json
import os
import platform
from pathlib import Path
import re
import subprocess
import sys
import tempfile


def validate(value):
    host, user, port, key = (value.get(k) for k in ('address', 'user', 'port', 'identity_file'))
    if not isinstance(host, str) or not host or len(host) > 253:
        raise ValueError('Indirizzo non valido')
    try:
        ipaddress.ip_address(host)
    except ValueError:
        if not re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?', host):
            raise ValueError('Indirizzo non valido')
    if not isinstance(user, str) or not re.fullmatch(r'[A-Za-z0-9_][A-Za-z0-9_.-]{0,63}', user):
        raise ValueError('Utente SSH non valido')
    if type(port) is not int or not 1 <= port <= 65535:
        raise ValueError('Porta SSH non valida')
    if not isinstance(key, str) or (key and not key.startswith(('/', '~/'))) or any(c in key for c in '\r\n\x00"\\%'):
        raise ValueError('Percorso della chiave non valido')
    return dict(address=host, user=user, port=port, identity_file=key)


def atomic(path, text):
    if path.is_symlink() or (path.exists() and not path.is_file()):
        raise ValueError('Configurazione SSH non regolare')
    descriptor, temp = tempfile.mkstemp(prefix='.oaclis-', dir=path.parent)
    try:
        with os.fdopen(descriptor, 'w') as stream:
            stream.write(text)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temp, path)
    finally:
        if os.path.exists(temp):
            os.unlink(temp)


def local_networks():
    addresses = []
    try:
        output = subprocess.run(['ip', '-j', '-4', 'addr', 'show', 'up'], capture_output=True, text=True, timeout=2)
        for interface in json.loads(output.stdout):
            addresses.extend((a['local'], a['prefixlen']) for a in interface.get('addr_info', []) if a.get('scope') == 'global')
    except (OSError, ValueError, subprocess.SubprocessError):
        pass
    # WSL's virtual subnet does not describe the Windows LAN.
    if os.environ.get('WSL_DISTRO_NAME') or 'microsoft' in platform.release().lower():
        script = "Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.InterfaceAlias -notmatch 'vEthernet|Loopback' -and $_.AddressState -eq 'Preferred' -and $_.IPAddress -notlike '169.254.*' } | Select-Object IPAddress,PrefixLength | ConvertTo-Json -Compress"
        try:
            output = subprocess.run(['powershell.exe', '-NoProfile', '-NonInteractive', '-Command', script], capture_output=True, text=True, timeout=5)
            values = json.loads(output.stdout.lstrip('\ufeff'))
            if isinstance(values, dict):
                values = [values]
            addresses.extend((v['IPAddress'], v['PrefixLength']) for v in values or [])
        except (OSError, ValueError, subprocess.SubprocessError):
            pass
    return [ipaddress.ip_network(f'{address}/{prefix}', strict=False) for address, prefix in addresses]


def same_lan(address, networks):
    try:
        ip = ipaddress.ip_address(address)
    except ValueError:
        return None
    return any(ip in network for network in networks) if networks else None


def settings(home, config, operation, value):
    path = config / 'ssh_settings.json'
    saved = json.loads(path.read_text()) if path.exists() else {}
    if operation == 'list':
        result = {}
        for device in value:
            if device['id'] in saved:
                result[device['id']] = validate(saved[device['id']]['settings'])
                continue
            output = subprocess.run(['ssh', '-G', device['ssh_alias']], capture_output=True, text=True, timeout=3)
            fields = {}
            for line in output.stdout.splitlines():
                key, _, content = line.partition(' ')
                fields.setdefault(key, content)
            if output.returncode == 0:
                result[device['id']] = dict(address=fields.get('hostname', ''), user=fields.get('user', ''), port=int(fields.get('port', 22)), identity_file=fields.get('identityfile', ''))
        networks = local_networks()
        for connection in result.values():
            connection['in_lan'] = same_lan(connection['address'], networks)
        return result
    identifier, alias = value['id'], value['ssh_alias']
    if not all(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,159}', s) for s in (identifier, alias)):
        raise ValueError('Dispositivo non valido')
    if operation == 'delete':
        if identifier not in saved:
            return {'ok': True}
        saved.pop(identifier)
    else:
        saved[identifier] = {'ssh_alias': alias, 'settings': validate(value['settings'])}
    directory = home / '.ssh'
    if directory.is_symlink():
        raise ValueError('Cartella SSH non regolare')
    directory.mkdir(mode=0o700, exist_ok=True)
    ssh_config = directory / 'config'
    if ssh_config.is_symlink():
        raise ValueError('Configurazione SSH non regolare')
    previous = ssh_config.read_text() if ssh_config.exists() else ''
    managed = config / 'ssh_config'
    include = 'Include "' + str(managed) + '"\n'
    hosts = []
    for entry in saved.values():
        data = validate(entry['settings'])
        text = f"Host {entry['ssh_alias']}\n  HostName {data['address']}\n  Port {data['port']}\n  User {data['user']}\n  StrictHostKeyChecking yes\n"
        if data['identity_file']:
            text += f"  IdentityFile \"{data['identity_file']}\"\n  IdentitiesOnly yes\n"
        hosts.append(text)
    atomic(managed, '\n'.join(hosts))
    if not previous.startswith(include):
        atomic(ssh_config, include + previous)
    atomic(path, json.dumps(saved))
    return {'ok': True}


if __name__ == '__main__':
    try:
        print(json.dumps(settings(Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], json.load(sys.stdin))))
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(json.dumps({'error': str(error)}))
        sys.exit(1)
