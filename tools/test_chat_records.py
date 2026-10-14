# test_chat_records.py v1.5.69
import json
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import unittest

SCRIPT=Path(__file__).with_name('appearance_store.py')

class ChatRecords(unittest.TestCase):
    def setUp(self):
        self.folder=tempfile.TemporaryDirectory();self.addCleanup(self.folder.cleanup)
        self.config=Path(self.folder.name);self.db=self.config/'appearance.sqlite'
        (self.config/'pairing_ed25519.pub').write_text('ssh-ed25519 local-fixture')
        (self.config/'pairing.json').write_text(json.dumps({'approved':{'remote-peer':{'device_id':'laptop'}}}))
        self.inventory={'status':'ready','requirements':[{'label':'Python 3','version':'3.12'}],'checked_at':1}
        self.other={'status':'ready','requirements':[{'label':'Python 3','version':'3.14'}],'checked_at':2}
        self.preparation={'status':'ready','chat_bytes':123,'chat_modified_ns':'456','measured_at':10,'paths':[{'path':'/home/project','bytes':789,'files':2}],'paths_bytes':789,'path_files':2}
        with sqlite3.connect(self.db) as db:
            db.executescript('CREATE TABLE appearance(key TEXT PRIMARY KEY,value TEXT); CREATE TABLE chat_settings(id TEXT PRIMARY KEY,hidden INTEGER,scopes TEXT); CREATE TABLE chat_visibility(id TEXT PRIMARY KEY,hidden INTEGER); CREATE TABLE app_checks(key TEXT PRIMARY KEY,value TEXT); CREATE TABLE preparations(key TEXT PRIMARY KEY,value TEXT); CREATE TABLE analysis(key TEXT PRIMARY KEY,value TEXT,scopes TEXT);')
            db.execute('INSERT INTO appearance VALUES(?,?)',('chat:fixture',json.dumps({'label':'Keep','icon':'chat','color':'#ee6c4d'})))
            db.execute('INSERT INTO chat_settings VALUES(?,?,?)',('fixture',0,json.dumps({'chat':True,'paths':False,'apps':False})))
            db.execute('INSERT INTO chat_visibility VALUES(?,?)',('fixture',1))
            db.executemany('INSERT INTO app_checks VALUES(?,?)',[('master:fixture',json.dumps(self.inventory)),('laptop:fixture',json.dumps(self.other))])
            db.execute('INSERT INTO preparations VALUES(?,?)',('master:fixture',json.dumps(self.preparation)))
            db.execute('INSERT INTO analysis VALUES(?,?,?)',('master:fixture','{"status":"ready"}',json.dumps(dict.fromkeys(('chat','paths','apps'),True))))

    def call(self,op,key='',value=None,success=True):
        p=subprocess.run(['python3',str(SCRIPT),str(self.db),op,key],input=json.dumps(value) if value is not None else None,capture_output=True,text=True)
        if success:self.assertEqual(p.returncode,0,p.stderr)
        else:self.assertNotEqual(p.returncode,0)
        return json.loads(p.stdout) if p.stdout.strip() else None

    def test_migration_preserves_all_fields_is_idempotent_and_keeps_sources_separate(self):
        self.call('migrate')
        self.assertEqual(self.call('inventory-get','master:fixture'),self.inventory)
        self.assertEqual(self.call('inventory-get','laptop:fixture'),self.other)
        self.assertEqual(self.call('preparation-get','master:fixture'),self.preparation)
        prefs=self.call('preferences-get')['preferences']['fixture']
        self.assertTrue(prefs['hidden']);self.assertEqual(prefs['scopes'],{'chat':True,'paths':False,'apps':False})
        with sqlite3.connect(self.db) as db:
            tables={row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            self.assertFalse(tables & {'chat_settings','chat_visibility','app_checks','preparations'})
            self.assertEqual([row[1] for row in db.execute('PRAGMA table_info(appearance)')],['key','value','hidden','sync'])
            self.assertEqual([row[1] for row in db.execute('PRAGMA table_info(analysis)')],['key','value'])
            before=db.execute('SELECT * FROM appearance').fetchall()
        self.call('migrate')
        with sqlite3.connect(self.db) as db:self.assertEqual(db.execute('SELECT * FROM appearance').fetchall(),before)

    def test_partial_updates_do_not_overwrite_other_settings_or_other_sources(self):
        self.call('migrate')
        self.call('inventory-save','master:fixture',dict(self.inventory,checked_at=20))
        self.call('save',value={'chat:fixture':{'label':'Renamed','color':'#98c1d9','icon':'code'}})
        self.call('chat-scopes','fixture',{'chat':False,'paths':True,'apps':True})
        self.assertEqual(self.call('inventory-get','laptop:fixture'),self.other)
        self.assertEqual(self.call('preparation-get','master:fixture'),self.preparation)
        self.assertTrue(self.call('preferences-get')['preferences']['fixture']['hidden'])
        self.assertEqual(self.call('load')['chat:fixture']['label'],'Renamed')
        self.assertEqual(self.call('inventory-get','master:fixture')['checked_at'],20)

    def test_bad_legacy_json_rolls_back_the_schema_and_data(self):
        with sqlite3.connect(self.db) as db:db.execute("UPDATE app_checks SET value='broken' WHERE key='master:fixture'")
        self.call('migrate',success=False)
        with sqlite3.connect(self.db) as db:
            self.assertEqual([row[1] for row in db.execute('PRAGMA table_info(appearance)')],['key','value'])
            self.assertIsNotNone(db.execute("SELECT 1 FROM sqlite_master WHERE name='app_checks'").fetchone())
            self.assertEqual(db.execute("SELECT value FROM app_checks WHERE key='master:fixture'").fetchone()[0],'broken')

if __name__=='__main__':unittest.main()
