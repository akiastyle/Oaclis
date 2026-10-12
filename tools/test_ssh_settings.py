import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from ssh_settings import settings, validate, same_lan
import ipaddress


class ConnectionSettings(unittest.TestCase):
    def test_lan_is_an_estimate_from_subnets(self):
        networks = [ipaddress.ip_network('192.168.1.138/24', strict=False)]
        self.assertTrue(same_lan('192.168.1.167', networks))
        self.assertFalse(same_lan('192.168.2.167', networks))
        self.assertFalse(same_lan('203.0.113.10', networks))
        self.assertIsNone(same_lan('example.org', networks))
        self.assertIsNone(same_lan('192.168.1.167', []))

    def test_rejects_config_injection(self):
        value = dict(address='example.com', user='root', port=22, identity_file='')
        for key, invalid in [('address', 'example.com\nProxyCommand bad'), ('user', '-bad'), ('port', 0), ('port', True), ('identity_file', '/tmp/key"\nHost bad')]:
            with self.assertRaises(ValueError):
                validate({**value, key: invalid})

    def test_real_ssh_config_and_update_preserve_existing_hosts(self):
        with tempfile.TemporaryDirectory() as temporary:
            home = Path(temporary); config = home / 'oaclis'; config.mkdir(); (home / '.ssh').mkdir()
            original = 'Host legacy\n HostName 192.168.1.2\n User root\n UserKnownHostsFile /tmp/original-known-hosts\n\nHost unrelated\n HostName example.org\n'
            (home / '.ssh/config').write_text(original)
            value = dict(id='client', ssh_alias='legacy', settings=dict(address='203.0.113.10', user='kim', port=2222, identity_file='/tmp/key with space'))
            settings(home, config, 'save', value)
            effective = subprocess.check_output(['ssh', '-G', '-F', str(home / '.ssh/config'), 'legacy'], text=True, stderr=subprocess.DEVNULL)
            self.assertIn('hostname 203.0.113.10\n', effective)
            self.assertIn('port 2222\n', effective)
            self.assertIn('user kim\n', effective)
            self.assertIn('identityfile /tmp/key with space\n', effective)
            self.assertIn('userknownhostsfile /tmp/original-known-hosts\n', effective)
            self.assertTrue((home / '.ssh/config').read_text().endswith(original))
            self.assertEqual(settings(home, config, 'list', [dict(id='client', ssh_alias='legacy')])['client']['address'], '203.0.113.10')
            value['settings']['port'] = 2223
            settings(home, config, 'save', value)
            self.assertEqual((home / '.ssh/config').read_text().count('Include '), 1)
            self.assertEqual(json.loads((config / 'ssh_settings.json').read_text())['client']['settings']['port'], 2223)
            settings(home, config, 'delete', {'id': 'client', 'ssh_alias': 'legacy'})
            self.assertEqual(json.loads((config / 'ssh_settings.json').read_text()), {})
            self.assertEqual((config / 'ssh_config').read_text(), '')
            self.assertTrue((home / '.ssh/config').read_text().endswith(original))


if __name__ == '__main__':
    unittest.main()
