# test_api_reads.py v1.5.79
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

    def test_link_save_does_not_scan_unrelated_inventories(self):
        with tempfile.TemporaryDirectory() as folder:
            path = pathlib.Path(folder) / "appearance.sqlite"
            with sqlite3.connect(path) as db:
                db.executescript("CREATE TABLE appearance(key TEXT PRIMARY KEY,value TEXT NOT NULL,hidden INTEGER DEFAULT 0,sync TEXT DEFAULT '{}'); CREATE TABLE chat_links(key TEXT PRIMARY KEY,value TEXT NOT NULL);")
                db.execute("INSERT INTO appearance VALUES(?,?,?,?)", ('chat:fixture','{}',0,json.dumps({'chat':{'enabled':0},'paths':{'enabled':None},'apps':{}})))
                db.execute("INSERT INTO appearance VALUES(?,?,?,?)", ('chat:unrelated','{}',0,'unrelated inventory must not be decoded'))
            self.assertEqual(read(path, 'links-get'), [])
            link = {'entry':{'id':'fixture'},'source':'master','target':'peer','operation':'sync','scopes':dict.fromkeys(('chat','paths','apps'), True)}
            result = subprocess.run(['python3',str(SCRIPT),str(path),'links-save','fixture'],input=json.dumps(link),text=True,capture_output=True,timeout=3)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertEqual(json.loads(result.stdout)[0]['scopes'],{'chat':False,'paths':False,'apps':True})
            self.assertEqual(read(path,'links-get'),json.loads(result.stdout))
            with sqlite3.connect(path) as db:
                self.assertEqual(db.execute("SELECT sync FROM appearance WHERE key='chat:unrelated'").fetchone()[0], 'unrelated inventory must not be decoded')

    def test_delete_commits_while_an_existing_reader_keeps_its_snapshot(self):
        with tempfile.TemporaryDirectory() as folder:
            path = pathlib.Path(folder) / "appearance.sqlite"
            read(path, 'migrate')
            link = {'entry':{'id':'fixture'},'source':'master','target':'peer','operation':'sync'}
            result = subprocess.run(['python3',str(SCRIPT),str(path),'links-save','fixture'],input=json.dumps(link),text=True,capture_output=True,timeout=3)
            self.assertEqual(result.returncode,0,result.stderr)
            with sqlite3.connect(path) as reader:
                self.assertEqual(reader.execute('PRAGMA journal_mode').fetchone()[0],'wal')
                reader.execute('BEGIN')
                self.assertEqual(reader.execute('SELECT COUNT(*) FROM chat_links').fetchone()[0],1)
                result = subprocess.run(['python3',str(SCRIPT),str(path),'links-delete','fixture'],text=True,capture_output=True,timeout=3)
                self.assertEqual(result.returncode,0,result.stderr)
                self.assertEqual(json.loads(result.stdout),[])
                self.assertEqual(reader.execute('SELECT COUNT(*) FROM chat_links').fetchone()[0],1)
                reader.rollback()
                self.assertEqual(reader.execute('SELECT COUNT(*) FROM chat_links').fetchone()[0],0)

if __name__ == "__main__":
    unittest.main()
