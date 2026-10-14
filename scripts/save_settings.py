# save_settings.py v1.1.63
"""Persist bootstrap environment without replacing application settings."""
import json
import os
from pathlib import Path
import sys
import shutil
import tempfile


def save(directory, environment, tools=None):
    if environment['os'] not in ('linux', 'macos', 'windows') or environment['host_os'] not in (None, 'linux', 'macos', 'windows'):
        raise ValueError('Sistema non valido')
    if directory.is_symlink():
        raise ValueError('La cartella settings non deve essere un link simbolico')
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    path = directory / 'settings.json'
    if path.is_symlink():
        raise ValueError('settings.json non deve essere un link simbolico')
    saved = json.loads(path.read_text(encoding='utf-8-sig')) if path.exists() else {}
    if not isinstance(saved, dict) or saved.get('schema_version', 1) != 1:
        raise ValueError('Formato settings non supportato: file conservato')
    previous = dict(saved)
    old_environment = saved.get('environment', {})
    if not isinstance(old_environment, dict):
        raise ValueError('Scenario salvato non valido: file conservato')
    if environment['container'] and environment['host_os'] is None and all(old_environment.get(k) == environment[k] for k in ('os', 'container', 'wsl_kernel')):
        environment['host_os'] = old_environment.get('host_os')
    saved.setdefault('schema_version', 1)
    saved.setdefault('control_port', 23232)
    saved['environment'] = environment
    if tools is not None:
        existing_tools = saved.get('tools', {})
        if not isinstance(existing_tools, dict):
            raise ValueError('Impostazioni tools non valide: file conservato')
        saved['tools'] = dict(existing_tools, **tools)
    if saved == previous:
        return path
    fd, temporary = tempfile.mkstemp(prefix='.settings-', dir=directory)
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as stream:
            json.dump(saved, stream, ensure_ascii=False, indent=2)
            stream.write('\n'); stream.flush(); os.fsync(stream.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary): os.unlink(temporary)
    return path


if __name__ == '__main__':
    try:
        system, container, kernel = sys.argv[1:]
        container, kernel = container == 'yes', kernel == 'yes'
        system = 'macos' if system == 'Darwin' else 'linux'
        host = os.environ.get('OACLIS_HOST_OS') or ('windows' if kernel else system if not container else None)
        environment = dict(os=system, host_os=host, container=container, wsl_kernel=kernel,
                           runtime='container' if container else 'wsl' if kernel else 'native',
                           wsl_distribution=os.environ.get('OACLIS_WSL_DISTRO') or os.environ.get('WSL_DISTRO_NAME') or None)
        directory = Path(os.environ.get('OACLIS_SETTINGS_DIR') or str(Path.home() / '.config/oaclis'))
        tools = {name: {'detected': bool(shutil.which(name)), 'path': shutil.which(name)} for name in ('node', 'npm', 'codex')}
        print('Settings:', save(directory, environment, tools))
    except (OSError, ValueError, TypeError, KeyError) as error:
        print('Settings non salvati: ' + str(error), file=sys.stderr)
        sys.exit(1)
