# test_service.py v1.5.55
"""Isolated startup/browser tests; do not change host services or open browsers."""
import importlib.util
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


service = load('prepare_service')
ui = load('open_ui')


class StartupTests(unittest.TestCase):
    def test_identity_environment_and_restart(self):
        user = SimpleNamespace(pw_dir='/home/test user', pw_name='test', pw_uid=1001, pw_gid=1001)
        unit = service.service(user, {'PATH': '/home/test user/.nvm/v22/bin:/usr/bin', 'CODEX_HOME': '/home/test user/.codex'})
        for expected in ('User=1001', 'Group=1001', 'Restart=always', 'WantedBy=multi-user.target', 'ExecStart="/home/test user/.local/bin/oaclis" serve', '/.nvm/v22/bin', 'CODEX_HOME=', 'OACLIS_PAUSE_TRANSFERS=0'):
            self.assertIn(expected, unit)
        self.assertNotIn('sudo', unit)
        self.assertNotIn('setup', unit)

    def test_unit_escaping(self):
        self.assertEqual(service.quoted('/home/a%u'), '"/home/a%%u"')
        with self.assertRaises(ValueError):
            service.quoted('path\nExecStart=unwanted')

    @patch.object(ui.urllib.request, 'urlopen')
    @patch.object(ui.subprocess, 'run')
    @patch.object(ui.platform, 'release', return_value='microsoft-standard-WSL2')
    def test_wsl_browser_after_readiness(self, release, run, urlopen):
        urlopen.return_value.__enter__.return_value.status = 200
        ui.open_ui()
        self.assertIn('Start-Process', run.call_args.args[0][-1])
        urlopen.assert_called_once()

    @patch.dict(ui.os.environ, {}, clear=True)
    @patch.object(ui.urllib.request, 'urlopen')
    @patch.object(ui.webbrowser, 'open')
    @patch.object(ui.platform, 'release', return_value='linux')
    @patch.object(ui.platform, 'system', return_value='Linux')
    def test_headless_does_not_launch_browser(self, system, release, browser, urlopen):
        urlopen.return_value.__enter__.return_value.status = 200
        ui.open_ui()
        browser.assert_not_called()


if __name__ == '__main__':
    unittest.main()
