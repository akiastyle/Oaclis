# test_presentation_settings.py v1.2.50
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

STORE = Path(__file__).with_name('appearance_store.py')


class PresentationSettings(unittest.TestCase):
    def test_scopes_cascade_and_hide_removes_only_chat_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            db = Path(directory) / 'appearance.sqlite'
            original = Path(directory) / 'chat.jsonl'; original.write_text('unchanged')
            def call(operation, key='', value=None):
                result = subprocess.run(['python3', str(STORE), str(db), operation, key], input=json.dumps(value) if value is not None else None, text=True, capture_output=True, check=True)
                return json.loads(result.stdout) if result.stdout else None
            call('preferences-get')
            for chat, device in [('a', 'one'), ('a', 'two'), ('b', 'two')]:
                call('links-save', chat + device, dict(entry=dict(id=chat), source='master', target=device, operation='sync', scopes=dict(chat=True, paths=True, apps=True)))
            call('analysis-save', 'master:a', dict(status='ready'))
            call('analysis-save', 'one:a', dict(status='ready'))
            scopes = dict(chat=True, paths=False, apps=False)
            result = call('chat-scopes', 'a', scopes)
            self.assertEqual(sum(link.get('scopes') == scopes for link in result['links']), 2)
            self.assertEqual(call('analysis-get', 'one:a')['scopes'], scopes)
            link = dict(entry=dict(id='a'), source='master', target='three', operation='sync', scopes=dict(chat=True, paths=True, apps=True))
            self.assertEqual(call('links-save', 'athree', link)[2]['scopes'], scopes)
            call('save', value={'chat:a': dict(label='A', icon='', color=''), 'chat:b': dict(label='B', icon='', color='')})
            result = call('chat-visibility', 'a', {'hidden': True})
            self.assertTrue(result['preferences']['a']['hidden'])
            self.assertEqual(len(result['links']), 4)
            self.assertIn('chat:a', call('load'))
            self.assertEqual(call('analysis-get', 'master:a')['status'], 'ready')
            result = call('chat-visibility', 'a', {'hidden': False})
            self.assertFalse(result['preferences']['a']['hidden'])
            result = call('chat-hide', 'a')
            self.assertTrue(result['preferences']['a']['hidden'])
            self.assertEqual([link['entry']['id'] for link in result['links']], ['b'])
            self.assertEqual(list(call('load')), ['chat:b'])
            self.assertEqual(original.read_text(), 'unchanged')
            with self.assertRaises(subprocess.CalledProcessError):
                call('links-save', 'aone', link)
            result = call('device-forget', 'two')
            self.assertEqual(result['links'], [])
            self.assertEqual(original.read_text(), 'unchanged')


if __name__ == '__main__':
    unittest.main()
