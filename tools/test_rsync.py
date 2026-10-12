#!/usr/bin/env python3
"""Prove isolate del trasporto Rust/rsync nelle due direzioni fra le WSL."""
import hashlib
import json
from pathlib import Path
import shlex
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / 'target/debug/oaclis'
ALIAS = 'oaclis-wsl-2'


def run(args, success=True):
    result = subprocess.run([str(arg) for arg in args], capture_output=True, text=True, timeout=60)
    assert (result.returncode == 0) == success, result.stderr
    if 'send-file' in args:
        events = [json.loads(line) for line in result.stderr.splitlines() if line.startswith('{')]
        phases = [event['phase'] for event in events]
        assert phases and phases[0] == 'preparing', phases
        if success:
            assert phases[-2:] == ['verifying', 'completed'], phases
            assert any(event['phase'] == 'transferring' for event in events), phases
        else:
            assert 'completed' not in phases, phases
    return result.stdout


def remote(*args, success=True):
    return run(['ssh', '-o', 'BatchMode=yes', ALIAS, shlex.join(args)], success)


def main():
    results = []
    remote_dir = remote('mktemp', '-d', '/tmp/oaclis-rsync-test-XXXXXXXXXX').strip()
    assert remote_dir.startswith('/tmp/oaclis-rsync-test-') and len(remote_dir.rsplit('-', 1)[1]) == 10
    try:
        with tempfile.TemporaryDirectory(prefix='oaclis-rsync-test-') as local_dir:
            local = Path(local_dir)
            config = local / 'devices.json'
            run([BINARY, 'device-set', config, 'laptop', 'Portatile WSL', ALIAS])
            source = local / 'source'
            source.write_bytes(b'project data\n' * 100000)
            source.chmod(0o640)
            destination = remote_dir + "/copy ' $(touch INJECTION) file"
            receipt = json.loads(run([BINARY, 'send-file', config, 'laptop', source, destination]))
            assert receipt == {'bytes': source.stat().st_size, 'sha256': hashlib.sha256(source.read_bytes()).hexdigest()}
            stat = json.loads(remote('python3', '-c', 'import os,sys,json; s=os.stat(sys.argv[1]); print(json.dumps([s.st_size,s.st_mode & 511,s.st_mtime_ns]))', destination))
            assert stat == [source.stat().st_size, 0o640, source.stat().st_mtime_ns]
            assert remote('cat', destination).encode() == source.read_bytes()
            results.extend(['copia ROG → portatile verificata SHA-256', 'permessi e mtime preservati', 'spazi, apostrofi e metacaratteri gestiti'])
            source.write_bytes(b'replacement')
            run([BINARY, 'send-file', config, 'laptop', source, destination], False)
            assert remote('wc', '-c', destination).split()[0] == str(receipt['bytes'])
            results.append('destinazione esistente preservata')
            empty = local / 'empty'
            empty.touch()
            run([BINARY, 'send-file', config, 'laptop', empty, remote_dir + '/empty'])
            assert remote('wc', '-c', remote_dir + '/empty').split()[0] == '0'
            results.append('file vuoto')
            link = local / 'link'
            link.symlink_to(source)
            run([BINARY, 'send-file', config, 'laptop', link, remote_dir + '/unsafe'], False)
            remote('ln', '-s', remote_dir, remote_dir + '/link')
            run([BINARY, 'send-file', config, 'laptop', source, remote_dir + '/link/unsafe'], False)
            remote('ln', '-s', remote_dir + '/absent', remote_dir + '/dangling')
            run([BINARY, 'send-file', config, 'laptop', source, remote_dir + '/dangling'], False)
            run([BINARY, 'send-file', config, 'laptop', source, remote_dir + '/missing/unsafe'], False)
            results.append('link locali/remoti, link pendenti e cartelle assenti rifiutati')
            run(['scp', '-q', BINARY, f'{ALIAS}:{remote_dir}/oaclis'])
            remote(remote_dir + '/oaclis', 'device-set', remote_dir + '/devices.json', 'rog', 'ROG WSL', 'oaclis-rog')
            back = local / "back ' file"
            reverse = json.loads(remote(remote_dir + '/oaclis', 'send-file', remote_dir + '/devices.json', 'rog', destination, str(back)))
            assert reverse == receipt and hashlib.sha256(back.read_bytes()).hexdigest() == receipt['sha256']
            results.append('copia portatile → ROG verificata SHA-256')
            assert not list(local.glob('.oaclis-*'))
            assert remote('find', remote_dir, '-name', '.oaclis-*').strip() == ''
            results.append('cartelle temporanee ripulite e avanzamento strutturato verificato')
    finally:
        remote('rm', '-rf', '--', remote_dir)
    report = {'transport': 'rsync over SSH', 'passed': results, 'archives_modified': False}
    (ROOT / 'docs/rsync-test-results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
