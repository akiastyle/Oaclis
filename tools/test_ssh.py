#!/usr/bin/env python3
# test_ssh.py v1.4.121
"""Prova SSH su dati dimostrativi, senza modificare archivi o progetti reali."""

import json
from pathlib import Path
import re
import shlex
import sqlite3
import subprocess
import sys
import tempfile
import uuid


def run(arguments):
    return subprocess.check_output(arguments, text=True, timeout=60).strip()


def main():
    if len(sys.argv) != 2 or not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_.-]*", sys.argv[1]):
        raise SystemExit("Uso: python3 tools/test_ssh.py ALIAS_SSH")
    host = sys.argv[1]
    binary = Path(__file__).resolve().parents[1] / "target/debug/oaclis"
    if not binary.is_file():
        raise SystemExit("Compilare prima con cargo build --locked")

    def ssh(command):
        return run(["ssh", "-o", "BatchMode=yes", host, command])

    remote = ssh("mktemp -d /tmp/oaclis-ssh-test.XXXXXXXXXX")
    if not re.fullmatch(r"/tmp/oaclis-ssh-test\.[a-zA-Z0-9]+", remote):
        raise RuntimeError("Directory remota di prova inattesa")
    results = {}
    try:
        with tempfile.TemporaryDirectory(prefix="oaclis-ssh-test-") as temporary:
            directory = Path(temporary)
            directory.chmod(0o700)
            source = directory / "source.jsonl"
            destination = directory / "destination.jsonl"
            meta = {"type": "session_meta", "payload": {"id": str(uuid.uuid4()),
                    "cwd": str(directory), "history_mode": "legacy"}}
            first = (json.dumps(meta) + "\n").encode()
            update = b'{"type":"event_msg","payload":{"type":"task_complete"}}\n'
            source.write_bytes(first)
            destination.write_bytes(first)
            project = directory / "note.txt"
            project.write_bytes(b"Progetto dimostrativo Oaclis\n")
            run(["scp", "-q", str(binary), str(source), str(destination), str(project), f"{host}:{remote}/"])
            executable = f"{remote}/oaclis"

            def compare(command, source_name, destination_name):
                return ssh(shlex.join([executable, command, f"{remote}/{source_name}", f"{remote}/{destination_name}"]))

            assert compare("compare-chat", "source.jsonl", "destination.jsonl") == "Identical"
            results["initial_copy"] = "Identical"
            source.write_bytes(first + update)
            run(["scp", "-q", str(source), f"{host}:{remote}/source.jsonl"])
            assert compare("compare-chat", "source.jsonl", "destination.jsonl") == "SourceAhead"
            results["increment_detected"] = "SourceAhead"
            ssh(shlex.join(["cp", f"{remote}/source.jsonl", f"{remote}/destination.jsonl"]))
            recovered = directory / "recovered.jsonl"
            run(["scp", "-q", f"{host}:{remote}/destination.jsonl", str(recovered)])
            assert recovered.read_bytes() == source.read_bytes()
            assert run([str(binary), "compare-chat", str(source), str(recovered)]) == "Identical"
            results["round_trip"] = "Identical"
            assert compare("compare-file", "note.txt", "note.txt") == "Identical"
            returned_note = directory / "returned-note.txt"
            run(["scp", "-q", f"{host}:{remote}/note.txt", str(returned_note)])
            assert returned_note.read_bytes() == project.read_bytes()
            results["project_file_round_trip"] = "Identical"
            destination.write_bytes(first + b'{"type":"event_msg","payload":{"type":"different"}}\n')
            run(["scp", "-q", str(destination), f"{host}:{remote}/destination.jsonl"])
            assert compare("compare-chat", "source.jsonl", "destination.jsonl") == "Conflict"
            results["divergence"] = "Conflict"
            assert compare("compare-chat", "source.jsonl", "missing.jsonl") == "DestinationMissing"
            results["missing_destination"] = "DestinationMissing"
            incomplete = directory / "incomplete.jsonl"
            incomplete.write_bytes(first + b'{"type":')
            run(["scp", "-q", str(incomplete), f"{host}:{remote}/incomplete.jsonl"])
            failed = subprocess.run(["ssh", "-o", "BatchMode=yes", host,
                shlex.join([executable, "inspect", f"{remote}/incomplete.jsonl"])],
                capture_output=True, text=True, timeout=60)
            assert failed.returncode == 1
            results["incomplete_chat"] = "rejected"
            mapped = ssh(shlex.join([executable, "map-path", str(directory), remote, str(project)]))
            assert mapped == f"{remote}/note.txt"
            results["path_mapping"] = "OK"
            probe = Path(__file__).with_name("probe_codex.py")
            run(["scp", "-q", str(probe), f"{host}:{remote}/probe_codex.py"])
            codex_home = Path.home() / ".codex"
            with sqlite3.connect(f"{(codex_home / 'state_5.sqlite').as_uri()}?mode=ro", uri=True) as database:
                rows = database.execute("SELECT id, rollout_path, history_mode FROM threads WHERE source IN ('cli', 'vscode') AND archived=0").fetchall()
            for mode in ("legacy", "paginated"):
                candidates = [(Path(path).stat().st_size, identifier, Path(path))
                    for identifier, path, stored_mode in rows
                    if stored_mode == mode and Path(path).is_file()]
                if not candidates:
                    raise RuntimeError(f"Nessun campione {mode} disponibile")
                _, identifier, sample = min(candidates)
                before = sample.stat()
                data = sample.read_bytes()
                after = sample.stat()
                assert (before.st_size, before.st_mtime_ns) == (after.st_size, after.st_mtime_ns)
                copy = directory / sample.name
                copy.write_bytes(data)
                copy.chmod(0o600)
                run(["scp", "-q", str(copy), f"{host}:{remote}/{sample.name}"])
                code = ("import json; from pathlib import Path; from probe_codex import probe; "
                    f"print(json.dumps(probe(Path({str(remote + '/' + sample.name)!r}), {identifier!r}, {mode!r})))")
                response = json.loads(ssh(f"cd {shlex.quote(remote)} && " + shlex.join(["python3", "-c", code])))
                assert response["thread/resume"]["id_matches"]
                assert response["thread/resume"]["cwd_remapped"]
                assert sample.read_bytes() == data
                results[f"remote_codex_{mode}"] = response
    finally:
        ssh(shlex.join(["rm", "-rf", "--", remote]))
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
