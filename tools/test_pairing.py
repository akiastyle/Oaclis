"""Pairing must never grant SSH access on a single approval or forged packet."""
import json
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch
import pairing
import ssh_settings

class PairingTests(unittest.TestCase):
    def test_signed_mutual_approval_and_preservation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            homes = [root / 'a', root / 'b']
            configs = [home / '.config/oaclis' for home in homes]
            for config in configs: config.mkdir(parents=True)
            keys = [pairing.identity(config)[1] for config in configs]
            cards = [dict(id=pairing.identifier(keys[i]), public_key=keys[i], name='Device '+str(i), role='master' if i == 0 else 'client', ssh=dict(address='127.0.0.1', port=2200+i, user='root', host_key=keys[i]), approved=[], time=int(time.time())) for i in range(2)]
            packets = [pairing.sign(configs[i], cards[i]) for i in range(2)]
            self.assertEqual(pairing.verify(packets[1])['id'], cards[1]['id'])
            changed = json.loads(packets[1]['hello']); changed['name'] = 'Fake one'
            tampered = dict(packets[1], hello=json.dumps(changed))
            with self.assertRaises(ValueError): pairing.verify(tampered)
            with self.assertRaises(ValueError): pairing.validate(dict(cards[1], time=0))
            pairing.approve(configs[0], cards[1], True, ssh_settings.atomic)
            with self.assertRaises(ValueError): pairing.commit(homes[0], configs[0], cards[1], ssh_settings.settings, ssh_settings.atomic)
            self.assertFalse((homes[0] / '.ssh/authorized_keys').exists())
            pairing.approve(configs[1], cards[0], True, ssh_settings.atomic)
            approved_cards = [pairing.verify(pairing.sign(configs[i], cards[i])) for i in range(2)]
            for i in range(2):
                (homes[i] / '.ssh').mkdir()
                (homes[i] / '.ssh/authorized_keys').write_text('existing-key\n')
                (homes[i] / '.ssh/config').write_text('Host existing\n  HostName localhost\n')
                with patch.object(ssh_settings, 'local_networks', return_value=[]):
                    pairing.commit(homes[i], configs[i], approved_cards[1-i], ssh_settings.settings, ssh_settings.atomic)
                    pairing.commit(homes[i], configs[i], approved_cards[1-i], ssh_settings.settings, ssh_settings.atomic)
                authorized = (homes[i] / '.ssh/authorized_keys').read_text()
                self.assertTrue(authorized.startswith('existing-key\n'))
                self.assertEqual(authorized.count(keys[1-i]), 1)
                self.assertIn('Host existing', (homes[i] / '.ssh/config').read_text())
                self.assertEqual(len(json.loads((configs[i] / 'devices.json').read_text())), 1)
            with self.assertRaises(ValueError): pairing.approve(configs[0], approved_cards[1], False, ssh_settings.atomic)
            self.assertNotEqual((configs[0] / 'pairing_ed25519').read_bytes(), (configs[1] / 'pairing_ed25519').read_bytes())
            device_id = pairing.state(configs[0])['approved'][cards[1]['id']]['device_id']
            pairing.forget(homes[0], configs[0], device_id, ssh_settings.atomic)
            self.assertEqual((homes[0] / '.ssh/authorized_keys').read_text(), 'existing-key\n')
            self.assertIn(cards[1]['id'], pairing.state(configs[0])['ignored'])

    def test_reject_and_invalid_input(self):
        with tempfile.TemporaryDirectory() as temporary:
            config = Path(temporary)
            key = pairing.identity(config)[1]
            card = dict(id=pairing.identifier(key), public_key=key, name='Device', role='client', ssh=dict(address='192.168.1.1', port=22, user='root', host_key=key), approved=[], time=int(time.time()))
            pairing.approve(config, card, False, ssh_settings.atomic)
            self.assertIn(card['id'], pairing.state(config)['ignored'])
            self.assertEqual(pairing.state(config)['approved'], {})
            with self.assertRaises(ValueError): pairing.validate(dict(card, ssh=dict(card['ssh'], address='8.8.8.8')))
            with self.assertRaises(ValueError): pairing.validate(dict(card, name='x\nHost evil'))

if __name__ == '__main__': unittest.main()
