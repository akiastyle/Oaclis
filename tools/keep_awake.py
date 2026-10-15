# keep_awake.py v1.3.201
"""Keep the host awake until the owning Oaclis process closes stdin."""
import json
import base64
import os
import re
import platform
from pathlib import Path
import select
import shutil
import subprocess
import sys
import time

def windows_command(script):
    powershell = '/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe' if platform.system() == 'Linux' else 'powershell.exe'
    if platform.system() == 'Linux' and not Path(os.environ.get('WSL_INTEROP', '/nonexistent')).is_socket():
        for socket in [Path('/run/WSL/1_interop'), *sorted(Path('/run/WSL').glob('*_interop'))]:
            if socket.is_socket():
                os.environ['WSL_INTEROP'] = str(socket)
                break
    return [powershell, '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', base64.b64encode(script.encode('utf-16le')).decode()]


def windows_prepare():
    script = "$ErrorActionPreference='Stop'; $path=Join-Path $env:LOCALAPPDATA 'Oaclis\\keep-awake.ps1'; if (-not (Test-Path $path)) { throw 'Preparare il supporto Windows con run.ps1' }; & $path -Check"
    result = subprocess.run(windows_command(script), capture_output=True, timeout=15)
    if result.returncode or result.stdout.strip() != b'ready':
        raise RuntimeError('Preparazione standby Windows: ' + (result.stderr.decode(errors='replace').strip() or 'PowerShell non raggiungibile da WSL'))


def prepare(user=None):
    if Path('/.dockerenv').exists() or Path('/run/.containerenv').exists():
        raise RuntimeError('Il blocco standby deve essere preparato sul sistema host, non nel container')
    system = platform.system()
    if system == 'Windows' or system == 'Linux' and 'microsoft' in platform.release().lower():
        windows_prepare()
        return
    if system == 'Darwin':
        if not shutil.which('caffeinate'):
            raise RuntimeError('Manca caffeinate')
        return
    if system != 'Linux' or not shutil.which('systemd-inhibit'):
        raise RuntimeError('Serve Linux con systemd-inhibit oppure macOS/Windows supportato')
    import pwd
    user = user or os.environ.get('SUDO_USER') or pwd.getpwuid(os.getuid()).pw_name
    uid = pwd.getpwnam(user).pw_uid
    if not re.fullmatch(r'[a-zA-Z_][a-zA-Z0-9_-]*[$]?', user):
        raise RuntimeError('Nome utente non supportato per la regola standby')
    rule = 'polkit.addRule(function(action, subject) { if (subject.user === ' + json.dumps(user) + ' && (action.id === "org.freedesktop.login1.inhibit-block-sleep" || action.id === "org.freedesktop.login1.inhibit-block-idle")) { return polkit.Result.YES; } });\n'
    path = '/etc/polkit-1/rules.d/49-oaclis-awake-' + str(uid) + '.rules'
    if os.geteuid() != 0:
        command = ['sudo'] + ([] if sys.stdin.isatty() else ['-n']) + ['install', '-m', '0644', '-o', 'root', '-g', 'root', '/dev/stdin', path]
        result = subprocess.run(command, input=rule.encode())
        if result.returncode:
            raise RuntimeError('Serve una prima autorizzazione amministrativa: eseguire bash run.sh --prepare-awake dal proprio utente sul dispositivo')
    else:
        if Path(path).is_symlink():
            raise RuntimeError('File del permesso standby non regolare')
        Path(path).write_text(rule)
        Path(path).chmod(0o644)
        os.chown(path, 0, 0)
    return user

if '--prepare' in sys.argv:
    try:
        target = sys.argv[sys.argv.index('--user') + 1] if '--user' in sys.argv else None
        user = prepare(target)
        print('Supporto Sempre attivo preparato' + (' per ' + user if user else '') + '. Il blocco si attiva dal pulsante Oaclis.')
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
    sys.exit(0)

child = None
idle_child = None
try:
    if Path("/.dockerenv").exists() or Path("/run/.containerenv").exists():
        raise RuntimeError("Da un container non è possibile impedire lo standby del computer host")
    system = platform.system()
    wsl = system == 'Linux' and 'microsoft' in platform.release().lower()
    if wsl or system == 'Windows':
        windows_prepare()
        script = "$ErrorActionPreference='Stop'; & (Join-Path $env:LOCALAPPDATA 'Oaclis\\keep-awake.ps1')"
        child = subprocess.Popen(windows_command(script), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if child.stdout.readline().strip() != b'ready':
            detail = child.stderr.read().decode(errors='replace').strip()
            raise RuntimeError('Windows non ha attivato il blocco standby: ' + detail)
    elif system == 'Darwin':
        child = subprocess.Popen(['caffeinate', '-i'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    elif system == 'Linux':
        if not shutil.which('systemd-inhibit'):
            raise RuntimeError('Blocco standby non disponibile: manca systemd-inhibit')
        # Separate requests avoid the unrelated lid-switch authorization used for combinations by systemd 255.
        child = subprocess.Popen(['systemd-inhibit', '--what=sleep', '--mode=block', '--who=Oaclis', '--why=Oaclis sempre attivo', 'sleep', 'infinity'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        idle_child = subprocess.Popen(['systemd-inhibit', '--what=idle', '--mode=block', '--who=Oaclis', '--why=Oaclis sempre attivo', 'sleep', 'infinity'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    else:
        raise RuntimeError('Blocco standby non supportato in questo ambiente')
    time.sleep(0.25)
    failed = next((process for process in (child, idle_child) if process is not None and process.poll() is not None), None)
    if failed is not None:
        detail = failed.stderr.read().decode(errors='replace').strip()
        if system == 'Linux' and 'Access denied' in detail:
            import pwd
            user = pwd.getpwuid(os.geteuid()).pw_name
            detail = f"L'utente {user} non è autorizzato da Linux a bloccare lo standby. Preparare il supporto con bash run.sh --prepare-awake sul dispositivo."
        raise RuntimeError(detail or 'Blocco standby terminato')
    print(json.dumps({'active': True}), flush=True)
    while not select.select([sys.stdin], [], [], 1)[0]:
        if any(process is not None and process.poll() is not None for process in (child, idle_child)):
            raise RuntimeError('Il sistema ha interrotto il blocco standby')
    sys.stdin.read()
except Exception as error:
    print(json.dumps({'active': False, 'error': str(error)}), flush=True)
    sys.exit(1)
finally:
    for process in (child, idle_child):
        if process is not None and process.poll() is None:
            if process.stdin:
                process.stdin.close()
            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
