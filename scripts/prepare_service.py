# prepare_service.py v1.3.89
"""Enable Oaclis at Linux/WSL boot using the existing installation identity."""
import os
from pathlib import Path
import pwd
import subprocess
import sys
import tempfile


def quoted(value):
    if any(char in value for char in '\n\r\x00'):
        raise ValueError('Percorso o ambiente non valido per systemd')
    return '"' + value.replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%') + '"'


def service(user, environment):
    home = Path(user.pw_dir)
    paths = [str(home / '.local/bin'), str(home / '.cargo/bin')]
    paths.extend(environment.get('PATH', '/usr/local/bin:/usr/bin:/bin').split(':'))
    variables = {'HOME': str(home), 'PATH': ':'.join(dict.fromkeys(paths)),
                 'OACLIS_PAUSE_SETTINGS': '0', 'OACLIS_PAUSE_CHECKS': '0', 'OACLIS_PAUSE_TRANSFERS': '0'}
    for key in ('CODEX_HOME', 'NVM_DIR', 'WSL_DISTRO_NAME'):
        if environment.get(key):
            variables[key] = environment[key]
    settings = '\n'.join('Environment=' + quoted(key + '=' + value) for key, value in variables.items())
    return f'''[Unit]
Description=Oaclis ({user.pw_name})
Wants=network-online.target
After=network-online.target
StartLimitIntervalSec=0

[Service]
Type=simple
User={user.pw_uid}
Group={user.pw_gid}
WorkingDirectory=~
{settings}
ExecStart={quoted(str(home / '.local/bin/oaclis'))} serve
Restart=always
RestartSec=5
TimeoutStopSec=30
KillMode=control-group
UMask=0077

[Install]
WantedBy=multi-user.target
'''


def prepare():
    if sys.platform != 'linux' or Path('/proc/1/comm').read_text().strip() != 'systemd':
        raise RuntimeError('Avvio automatico richiede Linux/WSL con systemd attivo. In WSL impostare [boot] systemd=true in /etc/wsl.conf e riavviare la distribuzione. Nei container usare la politica di riavvio del container.')
    user = pwd.getpwuid(os.getuid())
    if Path.home().resolve() != Path(user.pw_dir).resolve():
        raise RuntimeError('HOME non coincide con l\'utente corrente: eseguire come utente dell\'installazione Oaclis, senza sudo davanti allo script.')
    if not (Path(user.pw_dir) / '.local/bin/oaclis').is_file():
        raise RuntimeError('Installare Oaclis prima di abilitare il servizio.')
    unit = f'oaclis-{user.pw_uid}.service'
    prefix = [] if os.geteuid() == 0 else ['sudo']
    if prefix:
        subprocess.run(prefix + ['-v'], check=True)
    with tempfile.TemporaryDirectory(prefix='oaclis-service-') as folder:
        source = Path(folder) / unit
        source.write_text(service(user, os.environ))
        subprocess.run(['systemd-analyze', 'verify', str(source)], check=True)
        target = Path('/etc/systemd/system') / unit
        if target.exists() and target.read_text() != source.read_text() and not target.read_text().startswith('[Unit]\nDescription=Oaclis ('):
            raise RuntimeError(f'{target} contiene un servizio diverso: file conservato.')
        subprocess.run(prefix + ['install', '-m', '644', str(source), str(target)], check=True)
    subprocess.run(prefix + ['systemctl', 'daemon-reload'], check=True)
    subprocess.run(prefix + ['systemctl', 'enable', unit], check=True)
    # Never terminate an independently running instance or a transfer to take its port.
    import socket
    with socket.socket() as probe:
        occupied = probe.connect_ex(('127.0.0.1', 23232)) == 0
    if occupied and subprocess.run(['systemctl', 'is-active', '--quiet', unit]).returncode != 0:
        print(f'Servizio {unit} abilitato. Oaclis e gia aperto fuori dal servizio: non lo interrompo; il servizio partira al prossimo avvio del sistema.')
        return
    subprocess.run(prefix + ['systemctl', 'start', unit], check=True)
    subprocess.run(['systemctl', 'is-active', '--quiet', unit], check=True)
    print(f'Oaclis attivo; avvio automatico abilitato: {unit}. Log: journalctl -u {unit}')


if __name__ == '__main__':
    try:
        prepare()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f'Avvio automatico non configurato: {error}', file=sys.stderr)
        sys.exit(1)
