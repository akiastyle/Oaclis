"""Persistent Codex terminal on this host; browser disconnects only detach."""
import json
import os
import pathlib
import re
import shlex
import shutil
import subprocess
import sys


def request(value):
    chat = value.get('chat_id', '')
    if not re.fullmatch(r'[A-Za-z0-9_-]{1,160}', chat):
        raise ValueError('Chat non valida')
    session = 'oaclis-' + chat
    target = session + ':0.0'
    def tmux(*args, check=True):
        return subprocess.run(['tmux', *args], text=True, capture_output=True, check=check)
    action = value.get('action')
    if action == 'open':
        if not shutil.which('tmux'):
            raise ValueError('Serve tmux sul dispositivo per aprire Codex')
        if tmux('has-session', '-t', session, check=False).returncode:
            root = pathlib.Path(os.environ.get('CODEX_HOME', pathlib.Path.home() / '.codex'))
            files = list((root / 'sessions').rglob('*' + chat + '.jsonl'))
            if not files:
                raise ValueError('Chat non presente su questo dispositivo: trasferiscila prima')
            codex = shutil.which('codex')
            if not codex:
                versions = list((pathlib.Path.home() / '.nvm/versions/node').glob('*/bin/codex'))
                codex = str(max(versions, key=lambda p: p.stat().st_mtime)) if versions else None
            if not codex:
                raise ValueError('Codex non trovato sul dispositivo')
            cwd = str(pathlib.Path.home())
            with files[0].open() as stream:
                meta = json.loads(stream.readline()).get('payload', {})
                candidate = meta.get('cwd', cwd)
                if pathlib.Path(candidate).is_dir():
                    cwd = candidate
            command = shlex.join([codex, 'resume', chat, '--no-alt-screen'])
            tmux('new-session', '-d', '-s', session, '-c', cwd, '-x', '120', '-y', '36', command)
    elif action == 'input':
        text = value.get('text', '')
        key = value.get('key', '')
        if not isinstance(text, str) or len(text) > 8000 or '\x00' in text:
            raise ValueError('Messaggio non valido')
        if key not in ('', 'Enter', 'Escape', 'Tab', 'Up', 'Down', 'Left', 'Right', 'BSpace', 'C-c'):
            raise ValueError('Tasto non valido')
        if text:
            tmux('send-keys', '-t', target, '-l', '--', text)
        if key:
            tmux('send-keys', '-t', target, key)
    elif action != 'read':
        raise ValueError('Azione non valida')
    result = tmux('capture-pane', '-p', '-t', target, '-S', '-200', check=False)
    if result.returncode:
        raise ValueError('Sessione terminata: riapri la chat')
    return {'screen': result.stdout}


if __name__ == '__main__':
    try:
        print(json.dumps(request(json.load(sys.stdin))))
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(json.dumps({'error': str(error)}))
        sys.exit(1)
