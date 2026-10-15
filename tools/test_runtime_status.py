# test_runtime_status.py v1.4.50
import importlib.util
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

spec = importlib.util.spec_from_file_location('shared', Path(__file__).with_name('shared_settings.py'))
shared = importlib.util.module_from_spec(spec); spec.loader.exec_module(shared)


class RuntimeStatus(unittest.TestCase):
    def test_live_status_and_sleep_state_use_local_app(self):
        page = MagicMock(); page.read.return_value = b'<meta name="oaclis-token" content="fixture">'
        response = MagicMock(); response.__enter__.return_value.read.return_value = json.dumps({'active':True,'build':{'source_id':'fixture','version':'v1.0.1'}})
        with patch.object(shared.urllib.request, 'urlopen', side_effect=[page, response]) as request:
            status = shared.runtime_status()
        self.assertTrue(status['available']); self.assertTrue(status['active'])
        self.assertEqual(status['build']['source_id'], 'fixture')
        self.assertEqual(request.call_count, 2)
        self.assertEqual(json.loads(request.call_args.args[0].data)['action'], 'status')

    def test_stopped_app_is_unavailable_not_awake(self):
        with patch.object(shared.urllib.request, 'urlopen', side_effect=OSError('app stopped')):
            status = shared.runtime_status()
        self.assertFalse(status['available']); self.assertNotIn('active', status)
        self.assertIn('app stopped', status['error'])

    def test_existing_ssh_exchange_carries_status_and_checks_identity(self):
        for sender in ['peer', 'unexpected']:
            store = object.__new__(shared.Store); store.config = Path('/fixture')
            store.exchange = MagicMock(return_value={'schema':1,'sender':'local','records':{}})
            runtime = {'available':True,'active':True,'build':{'source_id':'fixture'}}
            def read(path, default):
                return [{'id':'client','ssh_alias':'fixture-alias'}] if path.name == 'devices.json' else {'approved':{'peer':{'device_id':'client'}}}
            output = SimpleNamespace(returncode=0, stderr='', stdout=json.dumps({'sender':sender,'runtime':runtime}))
            with patch.object(shared,'read',side_effect=read), patch.object(shared,'atomic') as save, patch.object(shared.subprocess,'run',return_value=output) as ssh:
                result = store.sync('fixture-source')
            self.assertEqual(ssh.call_count, 1, 'status must not create a second SSH probe')
            if sender == 'peer':
                self.assertEqual(result['runtime']['client'], runtime)
                self.assertEqual(result['errors'], {})
            else:
                self.assertFalse(result['runtime']['client']['available'])
                self.assertIn('client', result['errors'])
            self.assertEqual(save.call_args.args[1], result)


if __name__ == '__main__':
    unittest.main()
