# test_api_reads.py v1.3.45
"""Status reads must work alongside writers without changing the database."""
import json
import pathlib
import sqlite3
import subprocess
import tempfile
import unittest

SCRIPT = pathlib.Path(__file__).with_name("appearance_store.py")
OPERATIONS = ("analysis-list", "inventory-list", "inventory-get", "transfer-list", "links-get", "preferences-get", "status-read", "preparation-get")

def read(path, operation):
    result = subprocess.run(["python3", str(SCRIPT), str(path), operation, "master:fixture"], capture_output=True, text=True, timeout=3)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)

class ApiReads(unittest.TestCase):
    def test_missing_database_is_not_created(self):
        with tempfile.TemporaryDirectory() as folder:
            path = pathlib.Path(folder) / "appearance.sqlite"
            for operation in OPERATIONS:
                read(path, operation)
            self.assertFalse(path.exists())

    def test_reads_work_while_a_writer_holds_an_uncommitted_transaction(self):
        with tempfile.TemporaryDirectory() as folder:
            path = pathlib.Path(folder) / "appearance.sqlite"
            with sqlite3.connect(path) as writer:
                writer.execute("CREATE TABLE app_checks (key TEXT PRIMARY KEY, value TEXT)")
                writer.execute("INSERT INTO app_checks VALUES (?, ?)", ("master:fixture", '{"status":"ready"}'))
                writer.commit()
                writer.execute("BEGIN IMMEDIATE")
                writer.execute("UPDATE app_checks SET value=?", ('{"status":"pending"}',))
                for operation in OPERATIONS:
                    read(path, operation)
                self.assertEqual(read(path, "inventory-get"), {"status": "ready"})
                snapshot = read(path, "status-read")
                self.assertEqual(snapshot["app_checks"], {"master:fixture": {"status": "ready"}})
                self.assertEqual(snapshot["links"], [])
                self.assertEqual(snapshot["preferences"], {})
                writer.rollback()
                self.assertEqual(writer.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(), [("app_checks",)])

if __name__ == "__main__":
    unittest.main()
