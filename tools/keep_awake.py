# keep_awake.py v1.0.60
"""Keep the host awake until the owning Oaclis process closes stdin."""
import json
import platform
from pathlib import Path
import select
import shutil
import subprocess
import sys
import time

child = None
try:
    if Path("/.dockerenv").exists() or Path("/run/.containerenv").exists():
        raise RuntimeError("Da un container non è possibile impedire lo standby del computer host")
    system = platform.system()
    wsl = system == 'Linux' and 'microsoft' in platform.release().lower()
    if wsl or system == 'Windows':
        powershell = '/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe' if wsl else 'powershell.exe'
        script = 'Add-Type -TypeDefinition \'using System; using System.Runtime.InteropServices; public class OaclisAwake { [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags); }\'; if ([OaclisAwake]::SetThreadExecutionState(2147483649) -eq 0) { exit 1 }; Write-Output "ready"; [Console]::In.ReadToEnd() | Out-Null; [OaclisAwake]::SetThreadExecutionState(2147483648) | Out-Null'
        child = subprocess.Popen([powershell, '-NoProfile', '-NonInteractive', '-Command', script], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        if child.stdout.readline().strip() != b'ready':
            raise RuntimeError('Windows non ha attivato il blocco standby')
    elif system == 'Darwin':
        child = subprocess.Popen(['caffeinate', '-i'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    elif system == 'Linux':
        if not shutil.which('systemd-inhibit'):
            raise RuntimeError('Blocco standby non disponibile: manca systemd-inhibit')
        child = subprocess.Popen(['systemd-inhibit', '--what=sleep:idle', '--mode=block', '--who=Oaclis', '--why=Oaclis sempre attivo', 'sleep', 'infinity'], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    else:
        raise RuntimeError('Blocco standby non supportato in questo ambiente')
    time.sleep(0.25)
    if child.poll() is not None:
        detail = child.stderr.read().decode(errors='replace').strip()
        if system == 'Linux' and 'Access denied' in detail:
            import os, pwd
            user = pwd.getpwuid(os.geteuid()).pw_name
            detail = f"L'utente {user} non è autorizzato da Linux a bloccare lo standby. Il gruppo sudo non concede automaticamente questo permesso."
        raise RuntimeError(detail or 'Blocco standby terminato')
    print(json.dumps({'active': True}), flush=True)
    while not select.select([sys.stdin], [], [], 1)[0]:
        if child.poll() is not None:
            raise RuntimeError('Il sistema ha interrotto il blocco standby')
    sys.stdin.read()
except Exception as error:
    print(json.dumps({'active': False, 'error': str(error)}), flush=True)
    sys.exit(1)
finally:
    if child is not None and child.poll() is None:
        if child.stdin:
            child.stdin.close()
        child.terminate()
        try:
            child.wait(timeout=3)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait()
