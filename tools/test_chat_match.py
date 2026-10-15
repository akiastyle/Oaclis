# test_chat_match.py v1.5.41
"""Proof checks use isolated archives; no SSH, model calls or user data."""
import json
from pathlib import Path
import tempfile
import unittest
from tools.chat_match import summary

class ChatMatchTests(unittest.TestCase):
    def setUp(self):
        self.folder=tempfile.TemporaryDirectory();self.addCleanup(self.folder.cleanup)
        self.root=Path(self.folder.name)
        self.meta=json.dumps({'type':'session_meta','payload':{'id':'chat','cwd':'/work'}}).encode()+b'\n'
        self.event=lambda text:json.dumps({'type':'event_msg','payload':{'text':text}}).encode()+b'\n'

    def test_append_proves_direction_without_trusting_clocks(self):
        a=self.root/'a';b=self.root/'b'
        a.write_bytes(self.meta+self.event('first'));b.write_bytes(a.read_bytes()+self.event('second'))
        old=summary(str(a),'chat',a.stat().st_size);new=summary(str(b),'chat',a.stat().st_size)
        self.assertEqual(old['sha256'],new['prefix']);self.assertNotEqual(old['sha256'],new['sha256'])
        self.assertEqual(new['records'],3)

    def test_activity_uses_records_not_the_archive_copy_time(self):
        path=self.root/'chat'
        path.write_bytes(self.meta+json.dumps({'timestamp':'2026-10-15T10:00:00.25Z','type':'event_msg','payload':{'text':'new'}}).encode()+b'\n')
        self.assertEqual(summary(str(path),'chat',0)['activity'],'2026-10-15T10:00:00.250000000Z')

    def test_equal_sizes_do_not_hide_a_divergence(self):
        a=self.root/'a';b=self.root/'b'
        a.write_bytes(self.meta+self.event('AAA'));b.write_bytes(self.meta+self.event('BBB'))
        self.assertEqual(a.stat().st_size,b.stat().st_size)
        self.assertNotEqual(summary(str(a),'chat',a.stat().st_size)['prefix'],summary(str(b),'chat',b.stat().st_size)['prefix'])

    def test_truncation_wrong_identity_and_symlinks_are_rejected(self):
        a=self.root/'a';a.write_bytes(self.meta[:-1])
        with self.assertRaises(ValueError):summary(str(a),'chat',0)
        a.write_bytes(self.meta)
        with self.assertRaises(ValueError):summary(str(a),'other',0)
        link=self.root/'link';link.symlink_to(a)
        with self.assertRaises(OSError):summary(str(link),'chat',0)

if __name__=='__main__':unittest.main()
