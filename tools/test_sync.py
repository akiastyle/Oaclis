#!/usr/bin/env python3
"""Prove di aggiornamenti, checkpoint e conflitti su dati temporanei in LAN."""
import hashlib
import json
from pathlib import Path
import tempfile
import subprocess
import os
from test_rsync import ROOT, BINARY, run, remote


def write_remote(path, data):
    remote('python3', '-c', 'import pathlib,sys; pathlib.Path(sys.argv[1]).write_bytes(bytes.fromhex(sys.argv[2]))', path, data.hex())


def main():
    folder = remote('mktemp', '-d', '/tmp/oaclis-sync-test-XXXXXXXXXX').strip()
    assert folder.startswith('/tmp/oaclis-sync-test-') and len(folder.rsplit('-', 1)[1]) == 10
    checks = []
    try:
        with tempfile.TemporaryDirectory(prefix='oaclis-sync-test-') as directory:
            root = Path(directory)
            config = root / 'devices.json'
            run([BINARY, 'device-set', config, 'laptop', 'Portatile WSL', 'oaclis-wsl-2'])
            source = root / "file ' $(literal)"
            target = folder + "/file ' $(literal)"
            checkpoint = root / 'file-checkpoint.json'
            source.write_bytes(b'base'); source.chmod(0o751)

            def sync(kind='file', local=source, dest=target, state=checkpoint):
                return json.loads(run([BINARY, 'sync-' + kind, config, 'laptop', local, dest, state]))

            assert sync() == 'sent'
            assert sync() == 'identical'
            initial_time = source.stat().st_mtime_ns
            source.write_bytes(b'next')
            os.utime(source, ns=(initial_time, initial_time))
            assert sync() == 'sent'
            assert remote('cat', target) == 'next'
            assert int(remote('stat', '-c', '%a', target).strip(), 8) == 0o751
            checks.append('invio iniziale e aggiornamento anche con dimensione e mtime identici')
            write_remote(target, b'remote updated')
            assert sync() == 'received'
            assert source.read_bytes() == b'remote updated'
            assert source.stat().st_mode & 0o777 == 0o751
            checks.append('ritorno portatile → master e permessi preservati')
            fault = root / 'fault-bin'; fault.mkdir()
            wrapper = fault / 'rsync'
            wrapper.write_text('#!/bin/sh\necho "simulated rsync interruption" >&2\nexit 23\n')
            wrapper.chmod(0o700)
            before_failure = checkpoint.read_bytes()
            source.write_bytes(b'pending update')
            interrupted = subprocess.run([str(BINARY), 'sync-file', str(config), 'laptop', str(source), target, str(checkpoint)],
                env={**os.environ, 'PATH': str(fault) + ':' + os.environ['PATH']}, capture_output=True, text=True, timeout=30)
            assert interrupted.returncode != 0
            assert checkpoint.read_bytes() == before_failure and source.read_bytes() == b'pending update'
            assert remote('cat', target) == 'remote updated'
            assert sync() == 'sent'
            assert remote('cat', target) == 'pending update'
            checks.append('fallimento rsync simulato: nessuna modifica e nuovo tentativo riuscito')
            before = checkpoint.read_bytes()
            source.write_bytes(b'local conflict'); write_remote(target, b'remote conflict')
            assert sync() == 'conflict'
            assert source.read_bytes() == b'local conflict' and remote('cat', target) == 'remote conflict'
            assert checkpoint.read_bytes() == before
            checks.append('conflitto: copie e checkpoint preservati')
            other = root / 'other'; other.write_bytes(b'other')
            run([BINARY, 'sync-file', config, 'laptop', other, target, checkpoint], False)
            checks.append('checkpoint vincolato a dispositivo, tipo e percorsi')
            fresh = root / 'fresh'; fresh.write_bytes(b'local')
            different = folder + '/different'; write_remote(different, b'remote')
            assert sync(local=fresh, dest=different, state=root / 'fresh.json') == 'conflict'
            checks.append('nessuna scelta automatica fra file diversi senza checkpoint')
            backups = json.loads(remote('python3', '-c', 'import json,pathlib,sys; print(json.dumps([p.read_bytes().hex() for p in pathlib.Path(sys.argv[1]).rglob("*") if p.is_file() and p.name != "lock"]))', folder + '/.oaclis-backups'))
            assert b'base'.hex() in backups
            local_backups = [p.read_bytes() for p in (root / '.oaclis-backups').rglob('*') if p.is_file() and p.name != 'lock']
            assert b'next' in local_backups
            checks.append('versioni precedenti conservate su entrambe le macchine')
            meta = b'{"type":"session_meta","payload":{"id":"test-chat","cwd":"/work/test"}}\n'
            def event(value):
                return json.dumps({'type': 'event_msg', 'payload': {'type': 'user_message', 'message': value}}).encode() + b'\n'
            chat = root / 'chat.jsonl'; chat.write_bytes(meta)
            chat_target = folder + '/chat.jsonl'; chat_state = root / 'chat-checkpoint.json'
            assert sync('chat', chat, chat_target, chat_state) == 'sent'
            chat.write_bytes(meta + event('local message'))
            assert sync('chat', chat, chat_target, chat_state) == 'sent'
            write_remote(chat_target, chat.read_bytes() + event('remote message'))
            assert sync('chat', chat, chat_target, chat_state) == 'received'
            history = chat.read_bytes()
            assert history.endswith(event('remote message'))
            chat.write_bytes(history + event('local branch'))
            write_remote(chat_target, history + event('remote branch'))
            assert sync('chat', chat, chat_target, chat_state) == 'conflict'
            checks.append('chat: nuovi messaggi nelle due direzioni e rami divergenti rifiutati')
            unchanged = chat_state.read_bytes()
            write_remote(chat_target, b'{"type":')
            run([BINARY, 'sync-chat', config, 'laptop', chat, chat_target, chat_state], False)
            assert chat_state.read_bytes() == unchanged and chat.read_bytes().endswith(event('local branch'))
            checks.append('chat troncata rifiutata senza pubblicazione')
            assert not list(root.glob('.oaclis-*.part'))
    finally:
        remote('python3', '-c', 'import shutil,sys; shutil.rmtree(sys.argv[1])', folder)
    report = {'passed': checks, 'real_archives_modified': False, 'project_modified': False}
    (ROOT / 'docs/sync-test-results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
