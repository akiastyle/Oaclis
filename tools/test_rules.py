#!/usr/bin/env python3
# test_rules.py v1.3.94
"""Regole persistenti e cartelle su dati temporanei nelle due WSL."""
import json
from pathlib import Path
import tempfile
from test_rsync import ROOT, BINARY, run, remote
from test_sync import write_remote


def main():
    folder = remote('mktemp', '-d', '/tmp/oaclis-rules-test-XXXXXXXXXX').strip()
    assert folder.startswith('/tmp/oaclis-rules-test-') and len(folder.rsplit('-', 1)[1]) == 10
    checks = []
    try:
        with tempfile.TemporaryDirectory(prefix='oaclis-rules-test-') as temporary:
            root = Path(temporary); project = root / 'project'; project.mkdir()
            for name in ['src', 'assets', 'docs/private', '.git', 'target']:
                (project / name).mkdir(parents=True)
            unusual = "src/file ' \n$(literal).rs"
            files = {unusual: b'code', 'assets/image.txt': b'asset', 'docs/readme.md': b'doc',
                     'src/.env': b'private', 'src/private.key': b'private', '.git/config': b'git',
                     'target/cache': b'cache', 'docs/private/secret.txt': b'excluded'}
            for name, data in files.items(): (project / name).write_bytes(data)
            config = root / 'devices.json'; registry = root / 'rules.json'; definition = root / 'rule.json'
            state = root / 'state'; destination = folder + '/project'
            run([BINARY, 'device-set', config, 'laptop', 'Portatile WSL', 'oaclis-wsl-2'])
            rule = {'id': 'project', 'name': 'Project test', 'device_id': 'laptop', 'local_root': str(project),
                    'remote_root': destination, 'operation': 'sync', 'direction': 'both',
                    'include': ['src', 'assets', 'docs'], 'exclude': ['docs/private']}

            def save():
                definition.write_text(json.dumps(rule)); run([BINARY, 'rule-set', registry, definition])

            def execute(identifier='project'):
                result = json.loads(run([BINARY, 'rule-run', config, registry, identifier, state]))
                return {item['path']: item['outcome'] for item in result}

            save()
            assert json.loads(run([BINARY, 'rule-list', registry]))[0]['name'] == 'Project test'
            remote('test', '!', '-e', destination)
            result = execute()
            expected = {unusual, 'assets/image.txt', 'docs/readme.md'}
            assert set(result) == expected and all(value == 'sent' for value in result.values())
            copied = json.loads(remote('python3', '-c', 'import json,pathlib,sys; p=pathlib.Path(sys.argv[1]); print(json.dumps([str(x.relative_to(p)) for x in p.rglob("*") if x.is_file()]))', destination))
            assert set(copied) == expected
            assert all(value == 'identical' for value in execute().values())
            checks.extend(['regola salvata senza esecuzione automatica', 'cartelle annidate e percorsi con spazi, apostrofi e newline', 'inclusioni, esclusioni, credenziali e cache', 'seconda esecuzione senza aggiornamenti inutili'])
            (project / unusual).write_bytes(b'local updated')
            write_remote(destination + '/assets/image.txt', b'remote asset updated')
            remote('mkdir', '-p', destination + '/src/new folder')
            write_remote(destination + '/src/new folder/return.txt', b'new remote file')
            result = execute()
            assert result[unusual] == 'sent' and result['assets/image.txt'] == 'received' and result['src/new folder/return.txt'] == 'received'
            assert (project / 'assets/image.txt').read_bytes() == b'remote asset updated'
            assert (project / 'src/new folder/return.txt').read_bytes() == b'new remote file'
            checks.append('modifiche nelle due direzioni e nuovi file dal portatile')
            (project / 'docs/readme.md').write_bytes(b'local branch')
            write_remote(destination + '/docs/readme.md', b'remote branch')
            (project / 'src/new.rs').write_bytes(b'new local file')
            result = execute()
            assert result['docs/readme.md'] == 'conflict' and result['src/new.rs'] == 'sent'
            assert (project / 'docs/readme.md').read_bytes() == b'local branch' and remote('cat', destination + '/docs/readme.md') == 'remote branch'
            checks.append('conflitti conservati e file indipendenti aggiornati')
            rule['direction'] = 'send'; rule['name'] = 'One way'; save()
            write_remote(destination + '/assets/image.txt', b'remote change in one-way rule')
            write_remote(destination + '/src/remote-only.txt', b'leave remote')
            result = execute()
            assert result['assets/image.txt'] == 'conflict' and not (project / 'src/remote-only.txt').exists()
            assert (project / 'assets/image.txt').read_bytes() == b'remote asset updated'
            checks.append('regola monodirezionale: nessun ritorno o sovrascrittura di modifiche remote')
            rule['id'] = 'copy'; rule['operation'] = 'copy'; rule['name'] = 'Copy only'; save()
            (project / unusual).write_bytes(b'new copy source')
            (project / 'src/copy-new.rs').write_bytes(b'copy new')
            result = execute('copy')
            assert result[unusual] == 'conflict' and result['src/copy-new.rs'] == 'sent'
            assert remote('cat', destination + '/' + unusual) == 'local updated'
            checks.append('copia distinta dalla sincronizzazione: destinazioni esistenti preservate')
            entries = json.loads(run([BINARY, 'rule-list', registry])); assert len(entries) == 2 and entries[0]['name'] == 'One way'
            before = registry.read_bytes(); rule['include'] = ['../outside']; definition.write_text(json.dumps(rule))
            run([BINARY, 'rule-set', registry, definition], False); assert registry.read_bytes() == before
            checks.append('modifica delle regole preservando le altre e rifiuto di percorsi esterni')
            rule = entries[0]; rule['direction'] = 'both'; save()
            (project / 'src/new folder/return.txt').unlink()
            result = execute(); assert result['src/new folder/return.txt'] == 'conflict' and not (project / 'src/new folder/return.txt').exists()
            checks.append('cancellazione locale non propagata e nessuna ricreazione automatica')
    finally:
        remote('python3', '-c', 'import shutil,sys; shutil.rmtree(sys.argv[1])', folder)
    report = {'passed': checks, 'real_archives_modified': False, 'qr3d_modified': False}
    (ROOT / 'docs/rules-test-results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
