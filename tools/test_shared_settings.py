"""Settings replication with isolated homes; never use real SSH or Codex."""
import importlib.util
import json
import pathlib
import sqlite3
import tempfile
import unittest
spec = importlib.util.spec_from_file_location('shared', pathlib.Path(__file__).with_name('shared_settings.py'))
shared = importlib.util.module_from_spec(spec); spec.loader.exec_module(shared)

class SharedSettingsTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.stores = []
        for index in range(3):
            home = pathlib.Path(self.temp.name) / str(index)
            config = home / '.config/oaclis'; config.mkdir(parents=True)
            (config / 'pairing_ed25519.pub').write_text('ssh-ed25519 key' + str(index))
            (config / 'role').write_text('master' if index == 0 else 'client')
            self.stores.append(shared.Store(home))
        for index, store in enumerate(self.stores):
            approved = {peer.id: {'device_id': 'peer-' + str(i)} for i,peer in enumerate(self.stores) if i != index}
            shared.atomic(store.config / 'pairing.json', {'approved': approved})
            shared.atomic(store.config / 'devices.json', [{'id': v['device_id'], 'name': ['ROG','KDell','Jetson'][i], 'ssh_alias': 'alias-'+str(i)} for i,peer in enumerate(self.stores) if i != index for v in [approved[peer.id]]])
            store.exchange()
        root = self.stores[0]
        with sqlite3.connect(root.config / 'appearance.sqlite') as db:
            db.execute('INSERT OR REPLACE INTO appearance VALUES(?,?)', ('device:master',json.dumps({'label':'ROG','color':'#3d5a80','icon':'desktop'})))
            db.execute('INSERT OR REPLACE INTO appearance VALUES(?,?)', ('chat:chat-one',json.dumps({'label':'Oaclis','color':'#ee6c4d','icon':'chat'})))
            db.execute('INSERT OR REPLACE INTO chat_settings VALUES(?,?,?)', ('chat-one',0,json.dumps({'chat':True,'paths':False,'apps':True})))
            link={'source':'master','target':'peer-1','operation':'sync','entry':{'id':'chat-one','name':'Originale'},'scopes':{'chat':True,'paths':False,'apps':True},'transfer_pending':True}
            db.execute('INSERT OR REPLACE INTO chat_links VALUES(?,?)', ('chat-one:master:peer-1',json.dumps(link)))
        shared.atomic(root.config / 'positions.json', {'master':{'x':200,'y':100},'peer-1':{'x':400,'y':500}})

    def converge(self):
        for _ in range(2):
            for store in self.stores:
                packet=store.exchange()
                for peer in self.stores:
                    if peer is not store: store.exchange(peer.exchange(packet))

    def test_names_icons_arcs_positions_and_scopes_adapt_to_each_host(self):
        self.converge()
        client=self.stores[1]
        with sqlite3.connect(client.config / 'appearance.sqlite') as db:
            local=json.loads(db.execute("SELECT value FROM appearance WHERE key='device:master'").fetchone()[0])
            self.assertEqual(local['label'],'KDell')
            self.assertEqual(set(local), {'label', 'icon', 'color'})
            for (raw,) in db.execute('SELECT value FROM appearance'):
                self.assertEqual(set(json.loads(raw)), {'label', 'icon', 'color'})
            chat=json.loads(db.execute("SELECT value FROM appearance WHERE key='chat:chat-one'").fetchone()[0])
            self.assertEqual(chat['label'],'Oaclis'); self.assertEqual(chat['icon'],'chat')
            link=json.loads(db.execute('SELECT value FROM chat_links').fetchone()[0])
            self.assertEqual(link['source'],'peer-0'); self.assertEqual(link['target'],'master')
            self.assertFalse(link['transfer_pending']); self.assertFalse(link['scopes']['paths'])
        positions=shared.read(client.config / 'positions.json',{})
        self.assertEqual(positions['master'],{'x':400,'y':500})
        self.assertEqual(positions['peer-0'],{'x':200,'y':100})
        before=client.exchange()['records']; self.assertEqual(before,client.exchange()['records'])

    def test_offline_rename_and_deletion_converge_without_resurrection(self):
        self.converge()
        client=self.stores[1]
        with sqlite3.connect(client.config / 'appearance.sqlite') as db:
            db.execute('INSERT OR REPLACE INTO appearance VALUES(?,?)',('device:master',json.dumps({'label':'KDell nuovo','icon':'laptop'})))
            db.execute('DELETE FROM chat_links')
        self.converge()
        for store in self.stores:
            with sqlite3.connect(store.config / 'appearance.sqlite') as db:
                self.assertEqual(db.execute('SELECT count(*) FROM chat_links').fetchone()[0],0)
            names=[d['name'] for d in shared.read(store.config / 'devices.json',[])]
            if store is not client:self.assertIn('KDell nuovo',names)

    def test_unapproved_packet_and_invalid_positions_are_rejected(self):
        packet=self.stores[0].exchange();packet['sender']='unknown'
        with self.assertRaisesRegex(ValueError,'non autorizzato'):self.stores[1].exchange(packet)
        packet['sender']=self.stores[0].id
        packet['records']={'position:device':{'clock':[1,'peer'],'value':{'x':float('nan'),'y':1}}}
        with self.assertRaisesRegex(ValueError,'Posizione'):self.stores[1].exchange(packet)

    def test_concurrent_updates_resolve_deterministically(self):
        a={'chat:x':{'clock':[3,'a'],'value':{'label':'A'}}}
        b={'chat:x':{'clock':[3,'b'],'value':{'label':'B'}}}
        shared.merge(a,b);shared.merge(b,a);self.assertEqual(a,b)

if __name__=='__main__':unittest.main()
