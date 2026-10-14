# chat_match.py v1.1.42
"""Read-only chat proof. Only small digests cross SSH; archives stay on their device."""
import hashlib
import json
import os
import stat
import sys


def summary(path, chat_id, prefix_bytes):
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(descriptor, 'rb') as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError('Archivio non regolare')
        digest, prefix, seen, records = hashlib.sha256(), hashlib.sha256(), 0, 0
        while True:
            line = stream.readline(64 * 1024 * 1024 + 1)
            if not line:
                break
            if len(line) > 64 * 1024 * 1024 or not line.endswith(b'\n'):
                raise ValueError('Archivio incompleto: attendere la fine del messaggio')
            record = json.loads(line)
            if not isinstance(record.get('type'), str) or not isinstance(record.get('payload'), dict):
                raise ValueError('Record chat non valido')
            if not records:
                if record['type'] != 'session_meta' or record['payload'].get('id') != chat_id:
                    raise ValueError('Identità chat diversa da quella richiesta')
            elif record['type'] == 'session_meta':
                raise ValueError('Identità chat duplicata')
            digest.update(line)
            prefix.update(line[:max(0, min(len(line), prefix_bytes - seen))])
            seen += len(line)
            records += 1
        after = os.fstat(stream.fileno())
        if not records or (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
            raise ValueError('Chat cambiata durante il confronto')
        return {'sha256': digest.hexdigest(), 'prefix': prefix.hexdigest(), 'bytes': seen,
                'modified_ns': str(after.st_mtime_ns), 'records': records}


if __name__ == '__main__':
    print(json.dumps(summary(sys.argv[1], sys.argv[2], int(sys.argv[3]))))
