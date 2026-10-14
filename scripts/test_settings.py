# test_settings.py v1.2.36
from pathlib import Path
import json
import tempfile
import unittest
from save_settings import save

class SettingsTests(unittest.TestCase):
    def test_preserves_existing_settings_and_reuses_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            path = folder / 'settings.json'
            path.write_text(json.dumps({'devices': [{'id': 'existing'}], 'theme': 'dark'}))
            environment = dict(os='linux', host_os='windows', container=False, wsl_kernel=True, runtime='wsl', wsl_distribution='Ubuntu')
            self.assertEqual(save(folder, environment), path)
            data = json.loads(path.read_text())
            self.assertEqual(data['devices'], [{'id': 'existing'}])
            self.assertEqual(data['control_port'], 23232)
            self.assertEqual(data['environment'], environment)
            modified = path.stat().st_mtime_ns
            save(folder, environment)
            self.assertEqual(path.stat().st_mtime_ns, modified)
            save(folder, environment, {'codex': {'detected': False, 'path': None}})
            save(folder, environment, {'codex': {'detected': True, 'path': '/example/codex'}})
            self.assertTrue(json.loads(path.read_text())['tools']['codex']['detected'])

    def test_unknown_container_host_and_bad_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            environment = dict(os='linux', host_os=None, container=True, wsl_kernel=False, runtime='container', wsl_distribution=None)
            path = save(folder, environment)
            self.assertIsNone(json.loads(path.read_text())['environment']['host_os'])
            path.write_text('invalid json')
            with self.assertRaises(ValueError): save(folder, environment)
            self.assertEqual(path.read_text(), 'invalid json')

if __name__ == '__main__': unittest.main()
