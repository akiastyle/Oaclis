#!/usr/bin/env python3
"""Diagnostica temporanea: prova copie JSONL in un Codex isolato e senza rete."""

import hashlib
import json
import os
from pathlib import Path
import selectors
import shutil
import sqlite3
import subprocess
import tempfile
import time


def request(process, selector, identifier, method, params):
    process.stdin.write((json.dumps({"id": identifier, "method": method, "params": params}) + "\n").encode())
    process.stdin.flush()
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        if not selector.select(timeout=1):
            if process.poll() is not None:
                raise RuntimeError("App-server terminato prima della risposta")
            continue
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("App-server ha chiuso stdout")
        response = json.loads(line)
        if response.get("id") == identifier:
            return response
    raise TimeoutError(method)


def probe(source, identifier, mode):
    before = source.stat()
    data = source.read_bytes()
    after = source.stat()
    if (before.st_size, before.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
        raise RuntimeError("Conversazione modificata durante la lettura; riprovare a riposo")
    records = [json.loads(line) for line in data.splitlines() if line.strip()]
    assert records[0]["type"] == "session_meta"
    assert records[0]["payload"]["id"] == identifier
    result = {"mode": mode, "bytes": len(data), "records": len(records)}
    with tempfile.TemporaryDirectory(prefix="oaclis-probe-") as temporary:
        root = Path(temporary)
        root.chmod(0o700)
        home = root / "codex"
        destination = home / "sessions" / "2026" / "01" / "01" / source.name
        destination.parent.mkdir(parents=True)
        destination.write_bytes(data)
        destination.chmod(0o600)
        assert hashlib.sha256(destination.read_bytes()).digest() == hashlib.sha256(data).digest()
        work = root / "project"
        work.mkdir()
        # shortcut: solo importazione JSONL, estendere la prova ai dati paginati prima di sincronizzarli.
        env = {key: value for key, value in os.environ.items()
               if key in {"PATH", "LANG", "TERM", "LD_LIBRARY_PATH"}}
        env.update(HOME=str(root), CODEX_HOME=str(home), XDG_CONFIG_HOME=str(root / "config"))
        with (root / "stderr.log").open("w") as errors:
            process = subprocess.Popen(
                ["unshare", "-n", "codex", "app-server", "--stdio"],
                cwd=work, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                stderr=errors, bufsize=0,
            )
            selector = selectors.DefaultSelector()
            selector.register(process.stdout, selectors.EVENT_READ)
            try:
                initialized = request(process, selector, 1, "initialize", {
                    "clientInfo": {"name": "oaclis_probe", "version": "0.1.0"},
                    "capabilities": {"experimentalApi": True},
                })
                if "error" in initialized:
                    raise RuntimeError(initialized["error"]["message"])
                process.stdin.write(b'{"method":"initialized"}\n')
                process.stdin.flush()
                for number, method, params in [
                    (2, "thread/list", {"limit": 100, "sourceKinds": ["cli", "vscode", "exec", "appServer", "subAgent", "subAgentReview", "subAgentCompact", "subAgentThreadSpawn", "subAgentOther", "unknown"], "modelProviders": []}),
                    (3, "thread/read", {"threadId": identifier, "includeTurns": True}),
                    (4, "thread/resume", {"threadId": identifier, "cwd": str(work),
                                          "approvalPolicy": "never", "sandbox": "read-only"}),
                    (5, "thread/read", {"threadId": identifier, "includeTurns": True}),
                ]:
                    response = request(process, selector, number, method, params)
                    key = "thread/read_after_resume" if number == 5 else method
                    if "error" in response:
                        result[key] = {"error": response["error"]}
                    elif method == "thread/list":
                        result[method] = {"found": any(t["id"] == identifier
                                                      for t in response["result"]["data"])}
                    else:
                        thread = response["result"]["thread"]
                        result[key] = {"id_matches": thread["id"] == identifier,
                                          "turns": len(thread.get("turns", [])),
                                          "items": sum(len(turn.get("items", [])) for turn in thread.get("turns", [])),
                                          "cwd_remapped": thread.get("cwd") == str(work)}
            finally:
                selector.close()
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
                process.stdin.close()
                process.stdout.close()
        assert result["thread/list"]["found"], "Chat non visibile nell'elenco"
        assert result["thread/resume"].get("id_matches"), "Ripresa fallita"
        assert result["thread/resume"]["cwd_remapped"], "Percorso non rimappato"
        assert result["thread/read_after_resume"].get("turns", 0) > 0, "Cronologia assente"
        if mode == "legacy":
            assert result["thread/read"]["turns"] == result["thread/read_after_resume"]["turns"]
            assert result["thread/read"]["items"] == result["thread/read_after_resume"]["items"]
        assert source.read_bytes() == data, "L'origine è cambiata durante la prova"
        result["source_unchanged"] = True
        result["network"] = "disabled by unshare -n"
    return result


def main():
    if not shutil.which("codex") or not shutil.which("unshare"):
        raise RuntimeError("Servono Codex e unshare")
    home = Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))
    with sqlite3.connect(f"{(home / 'state_5.sqlite').as_uri()}?mode=ro", uri=True) as database:
        rows = database.execute("SELECT id, rollout_path, history_mode FROM threads WHERE source IN ('cli', 'vscode') AND archived=0").fetchall()
    results = []
    for mode in ("legacy", "paginated"):
        candidates = [(Path(path).stat().st_size, identifier, Path(path))
                      for identifier, path, stored_mode in rows
                      if stored_mode == mode and Path(path).is_file()]
        if not candidates:
            results.append({"mode": mode, "skipped": "Nessuna chat disponibile"})
            continue
        _, identifier, source = min(candidates)
        result = probe(source, identifier, mode)
        with sqlite3.connect(f"{(home / 'thread_history_1.sqlite').as_uri()}?mode=ro", uri=True) as history:
            result["source_database_turns"] = history.execute(
                "SELECT COUNT(*) FROM thread_turns WHERE thread_id=?", (identifier,)
            ).fetchone()[0]
            result["source_database_items"] = history.execute(
                "SELECT COUNT(*) FROM thread_items WHERE thread_id=?", (identifier,)
            ).fetchone()[0]
        if mode == "paginated":
            assert result["thread/read_after_resume"]["turns"] == result["source_database_turns"]
            assert result["thread/read_after_resume"]["items"] == result["source_database_items"]
        results.append(result)
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
