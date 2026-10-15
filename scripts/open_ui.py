# open_ui.py v1.1.42
"""Open the local UI after setup, never from the background service."""
import os
from pathlib import Path
import platform
import subprocess
import time
import urllib.request
import webbrowser


def open_ui():
    url = 'http://localhost:23232'
    for attempt in range(30):
        try:
            with urllib.request.urlopen(url, timeout=1) as response:
                if response.status == 200:
                    break
        except OSError:
            time.sleep(1)
    else:
        print(f'Interfaccia non ancora disponibile: {url}. Verificare il servizio Oaclis.')
        return
    if 'microsoft' in platform.release().lower() or os.environ.get('WSL_DISTRO_NAME'):
        if not Path(os.environ.get('WSL_INTEROP', '/nonexistent')).is_socket():
            for candidate in Path('/run/WSL').glob('*_interop'):
                if candidate.is_socket():
                    os.environ['WSL_INTEROP'] = str(candidate)
                    break
        powershell = '/mnt/c/Windows/System32/WindowsPowerShell/v1.0/powershell.exe'
        try:
            subprocess.run([powershell, '-NoProfile', '-NonInteractive', '-Command', 'Start-Process "http://localhost:23232"'], check=True, timeout=15)
            return
        except (OSError, subprocess.SubprocessError):
            pass
    elif platform.system() == 'Darwin' or os.environ.get('DISPLAY') or os.environ.get('WAYLAND_DISPLAY'):
        if webbrowser.open(url):
            return
    print(f'Webapp: {url}. Nessun browser disponibile in questa sessione; aprirla dal computer collegato.')


if __name__ == '__main__':
    open_ui()
