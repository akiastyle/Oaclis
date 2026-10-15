# update_app.py v1.2.40
"""Update an existing checkout to the master's revision without discarding edits."""
import os
from pathlib import Path
import re
import subprocess
import sys


def run(directory, *args, timeout=60):
    return subprocess.run(args, cwd=directory, env={**os.environ, 'GIT_TERMINAL_PROMPT': '0'}, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=timeout, check=True).stdout.strip()


def update(directory, revision):
    if not re.fullmatch(r'[a-fA-F0-9]{40}', revision):
        raise ValueError('Revisione master non valida')
    directory = Path(directory)
    if not (directory / 'scripts/install.sh').is_file():
        raise ValueError('Checkout Oaclis non disponibile: aggiornare dalla cartella della repo')
    if run(directory, 'git', 'status', '--porcelain'):
        raise ValueError('Aggiornamento sospeso: la repo contiene modifiche locali')
    if run(directory, 'git', 'branch', '--show-current') != 'main':
        raise ValueError('Aggiornamento sospeso: la repo non è sul ramo main')
    run(directory, 'git', 'fetch', 'origin', 'main')
    run(directory, 'git', 'merge-base', '--is-ancestor', revision, 'origin/main')
    run(directory, 'git', 'merge-base', '--is-ancestor', 'HEAD', revision)
    run(directory, 'git', 'merge', '--ff-only', revision)
    if run(directory, 'git', 'rev-parse', 'HEAD') != revision:
        raise ValueError('La revisione locale non coincide con quella del master')
    run(directory, 'bash', 'scripts/install.sh', '--unconfigured', timeout=1800)


if __name__ == '__main__':
    try:
        update(*sys.argv[1:])
    except subprocess.CalledProcessError as error:
        print(error.stdout[-2000:], file=sys.stderr)
        sys.exit(1)
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
