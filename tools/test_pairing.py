# test_pairing.py v1.7.101
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
    def test_neighbor_cache_only_provides_addresses_not_device_roles(self):
        neighbors = [{'dst':'192.168.1.138','lladdr':'aa:bb:cc:dd:ee:ff','state':['STALE']},
                     {'dst':'192.168.1.199','state':['FAILED']},
                     {'dst':'224.0.0.1','lladdr':'aa:bb:cc:dd:ee:ff','state':['PERMANENT']}]
        with patch.object(pairing.platform, 'system', return_value='Linux'), patch.object(pairing.platform, 'release', return_value='native'), patch.dict(pairing.os.environ, {}, clear=True), patch.object(pairing, 'run', return_value=json.dumps(neighbors).encode()) as run:
            self.assertEqual(pairing.neighbors(), ['192.168.1.138'])
            run.assert_called_once_with(['ip', '-j', '-4', 'neigh', 'show'])

    def test_linux_ignores_global_loopback_and_prefers_default_route(self):
        interfaces = [
            dict(ifname='lo', flags=['LOOPBACK', 'UP'], operstate='UNKNOWN', addr_info=[dict(local='10.255.255.254', scope='global', prefixlen=32)]),
            dict(ifname='docker0', flags=['UP'], operstate='DOWN', addr_info=[dict(local='172.17.0.1', scope='global', prefixlen=16)]),
            dict(ifname='eth1', flags=['UP'], operstate='UP', addr_info=[dict(local='192.168.2.10', scope='global', prefixlen=24, broadcast='192.168.2.255')]),
            dict(ifname='eth0', flags=['UP'], operstate='UP', addr_info=[dict(local='172.29.128.195', scope='global', prefixlen=20, broadcast='172.29.143.255')]),
        ]
        def run(arguments):
            return json.dumps([{'dev': 'eth0'}] if 'route' in arguments else interfaces).encode()
        with patch.object(pairing.platform, 'system', return_value='Linux'), patch.object(pairing, 'run', side_effect=run):
            result = pairing.network_interfaces()
        self.assertEqual([nic['address'] for nic in result], ['172.29.128.195', '192.168.2.10'])

    def test_native_macos_uses_ifconfig_without_windows_or_ip(self):
        text = 'lo0: flags=8049<UP,LOOPBACK,RUNNING>\n\tinet 127.0.0.1 netmask 0xff000000\nen0: flags=8863<UP,BROADCAST,RUNNING>\n\tinet 192.168.1.20 netmask 0xffffff00 broadcast 192.168.1.255\n\tstatus: active\nen1: flags=8863<UP,BROADCAST>\n\tinet 192.168.2.20 netmask 0xffffff00\n\tstatus: inactive\n'
        with patch.object(pairing.platform, 'system', return_value='Darwin'), patch.object(pairing, 'run', return_value=text.encode()) as run:
            self.assertEqual(pairing.network_interfaces(), [dict(name='en0', address='192.168.1.20', prefix=24, broadcast='192.168.1.255')])
            run.assert_called_once_with(['ifconfig', '-a'])

    def test_discovery_does_not_require_an_ssh_server(self):
        with tempfile.TemporaryDirectory() as temporary:
            config = Path(temporary)
            _, key = pairing.identity(config)
            card = dict(id=pairing.identifier(key), public_key=key, name='Linux', role='client', ssh=dict(address='192.168.1.20', port=22, user='root', host_key=None), approved=[], time=int(time.time()))
            packet = pairing.sign(config, card)
            self.assertIsNone(pairing.verify(packet)['ssh']['host_key'])
            pairing.approve(config, card, True, ssh_settings.atomic)
            card['approved'] = [card['id']]
            with self.assertRaisesRegex(ValueError, 'SSH non pronto'):
                pairing.commit(config, config, card, ssh_settings.settings, ssh_settings.atomic)
            self.assertFalse((config / '.ssh/authorized_keys').exists())

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
