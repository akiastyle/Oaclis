# test_shared_settings.py v1.11.169
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
            db.execute('INSERT INTO appearance(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value', ('device:master',json.dumps({'label':'ROG','color':'#3d5a80','icon':'desktop'})))
            db.execute('INSERT INTO appearance(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value', ('chat:chat-one',json.dumps({'label':'Oaclis','color':'#ee6c4d','icon':'chat'})))
            shared.set_switches(db,'chat-one',{'chat':True,'paths':False,'apps':True})
            link={'source':'master','target':'peer-1','operation':'sync','entry':{'id':'chat-one','name':'Originale'},'scopes':{'chat':True,'paths':False,'apps':True},'transfer_pending':True}
            db.execute('INSERT OR REPLACE INTO chat_links VALUES(?,?)', ('chat-one:master:peer-1',json.dumps(link)))
        shared.atomic(root.config / 'positions.json', {'master':{'x':200,'y':100},'peer-1':{'x':400,'y':500}})
        shared.atomic(root.config / 'link-positions.json', {'chat-one:master:peer-1':{'x':-60,'y':20}})

    def test_unchanged_metadata_avoids_rewriting_files_and_chat_payloads(self):
        from unittest.mock import patch
        store = self.stores[0]
        path = store.config / 'unchanged.json'
        with patch.object(shared.os,'fsync',wraps=shared.os.fsync) as flush:
            shared.atomic(path, {'value':1})
            shared.atomic(path, {'value':1})
            self.assertEqual(flush.call_count,1)
        path.write_text('{invalid')
        shared.atomic(path, {'value':2})
        self.assertEqual(shared.read(path, None), {'value':2})
        with sqlite3.connect(store.config / 'appearance.sqlite') as db:
            shared.save_chat_object(db,'fixture',{'chat':{'enabled':1}})
            changes = db.total_changes
            shared.save_chat_object(db,'fixture',{'chat':{'enabled':1}})
            self.assertEqual(db.total_changes,changes)
            shared.save_chat_object(db,'fixture',{'chat':{'enabled':0}})
            self.assertEqual(db.total_changes,changes+1)

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
        offsets=shared.read(client.config / 'link-positions.json',{})
        self.assertEqual(offsets['chat-one:master:peer-0'],{'x':-60,'y':20})
        before=client.exchange()['records']; self.assertEqual(before,client.exchange()['records'])

    def test_visibility_propagates_without_deleting_links_or_appearance(self):
        self.converge()
        root = self.stores[0]
        for hidden in (1, 0):
            with sqlite3.connect(root.config / 'appearance.sqlite') as db:
                db.execute("UPDATE appearance SET hidden=? WHERE key='chat:chat-one'", (hidden,))
            self.converge()
            for store in self.stores:
                with sqlite3.connect(store.config / 'appearance.sqlite') as db:
                    self.assertEqual(db.execute("SELECT hidden FROM appearance WHERE key='chat:chat-one'").fetchone()[0], hidden)
                    self.assertEqual(db.execute('SELECT count(*) FROM chat_links').fetchone()[0], 1)
                    self.assertIsNotNone(db.execute("SELECT value FROM appearance WHERE key='chat:chat-one'").fetchone())

    def test_offline_rename_and_deletion_converge_without_resurrection(self):
        self.converge()
        client=self.stores[1]
        with sqlite3.connect(client.config / 'appearance.sqlite') as db:
            db.execute('INSERT INTO appearance(key,value) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value=excluded.value',('device:master',json.dumps({'label':'KDell nuovo','icon':'laptop'})))
            db.execute('DELETE FROM chat_links')
        self.converge()
        for store in self.stores:
            with sqlite3.connect(store.config / 'appearance.sqlite') as db:
                self.assertEqual(db.execute('SELECT count(*) FROM chat_links').fetchone()[0],0)
            self.assertEqual(shared.read(store.config / 'link-positions.json',{}),{})
            names=[d['name'] for d in shared.read(store.config / 'devices.json',[])]
            if store is not client:self.assertIn('KDell nuovo',names)

    def test_unapproved_packet_and_invalid_positions_are_rejected(self):
        packet=self.stores[0].exchange();packet['sender']='unknown'
        with self.assertRaisesRegex(ValueError,'non autorizzato'):self.stores[1].exchange(packet)
        packet['sender']=self.stores[0].id
        packet['records']={'position:device':{'clock':[1,'peer'],'value':{'x':float('nan'),'y':1}}}
        with self.assertRaisesRegex(ValueError,'Posizione'):self.stores[1].exchange(packet)

    def test_source_data_do_not_replace_switches_graphics_or_another_source(self):
        self.converge()
        root,client=self.stores[:2]
        for store,label in [(root,'Python 3.12'),(client,'Python 3.14')]:
            with sqlite3.connect(store.config/'appearance.sqlite') as db:
                shared.save_inventory(db,'master:chat-one',{'status':'ready','requirements':[{'label':label}]},store.config)
        with sqlite3.connect(client.config/'appearance.sqlite') as db:
            shared.set_switches(db,'chat-one',{'chat':True,'paths':True,'apps':False})
        self.converge()
        for store in self.stores:
            with sqlite3.connect(store.config/'appearance.sqlite') as db:
                value=shared.chat_object(db,'chat-one')
                self.assertEqual(value['apps']['enabled'],0)
                self.assertEqual(value['paths']['enabled'],1)
                sources=value['apps']['sources']
                self.assertEqual(sources[root.id]['requirements'][0]['label'],'Python 3.12')
                self.assertEqual(sources[client.id]['requirements'][0]['label'],'Python 3.14')
                self.assertEqual(json.loads(db.execute("SELECT value FROM appearance WHERE key='chat:chat-one'").fetchone()[0])['label'],'Oaclis')

    def test_file_digest_cache_stays_local_and_survives_settings_exchange(self):
        owner=self.stores[0]
        cached={'path':'/home/project','bytes':3,'files':1,'sha256':'root-proof','entries':{'file.txt':{'sha256':'file-proof'}}}
        with sqlite3.connect(owner.config/'appearance.sqlite') as db:
            shared.save_preparation(db,'master:chat-one',{'status':'ready','chat_bytes':1,'chat_modified_ns':'1','measured_at':1,'paths':[cached],'paths_bytes':3,'path_files':1},owner.config)
        packet=owner.exchange()
        record=packet['records']['chatdata:chat-one:'+owner.id]['value']['paths']['items'][0]
        self.assertNotIn('entries',record)
        with sqlite3.connect(owner.config/'appearance.sqlite') as db:
            saved=shared.chat_object(db,'chat-one')['paths']['sources'][owner.id]['items'][0]
            self.assertEqual(saved['entries'],cached['entries'])

    def test_link_lifecycle_replicates_without_rearming_a_completed_transfer(self):
        self.converge()
        owner=self.stores[0]
        with sqlite3.connect(owner.config/'appearance.sqlite') as db:
            key,raw=db.execute('SELECT key,value FROM chat_links').fetchone();link=json.loads(raw)
            link['flow']={'executor':owner.id,'generation':'initial','initialized':True,'phase':'aligned','requested':False}
            link['transfer_pending']=False
            db.execute('UPDATE chat_links SET value=? WHERE key=?',(json.dumps(link),key))
        self.converge()
        for store in self.stores:
            with sqlite3.connect(store.config/'appearance.sqlite') as db:
                link=json.loads(db.execute('SELECT value FROM chat_links').fetchone()[0])
                self.assertEqual(link['flow']['executor'],owner.id)
                self.assertTrue(link['flow']['initialized']);self.assertFalse(link['transfer_pending'])

    def test_concurrent_updates_resolve_deterministically(self):
        a={'chat:x':{'clock':[3,'a'],'value':{'label':'A'}}}
        b={'chat:x':{'clock':[3,'b'],'value':{'label':'B'}}}
        shared.merge(a,b);shared.merge(b,a);self.assertEqual(a,b)

if __name__=='__main__':unittest.main()
