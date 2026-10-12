"""Bootstrap checks must not install or start anything."""
from pathlib import Path
import os
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent

class BootstrapTests(unittest.TestCase):
    def check(self, system, missing=None):
        with tempfile.TemporaryDirectory() as directory:
            folder = Path(directory)
            for command in ('git', 'curl', 'python3', 'ssh', 'ssh-keygen', 'rsync', 'tmux', 'cc', 'ip', 'rustc', 'cargo', 'codex', 'uname', 'apt-get', 'brew', 'xcode-select'):
                if command == missing: continue
                text = '#!/bin/sh\nexit 0\n'
                if command == 'uname': text = '#!/bin/sh\nif [ "$1" = -r ]; then echo native; else echo '+system+'; fi\n'
                if command == 'rustc': text = '#!/bin/sh\necho "rustc 1.95.0"\n'
                if command in ('apt-get', 'brew', 'curl'):
                    text = '#!/bin/sh\necho invoked >> "$OACLIS_TEST_LOG"\nexit 98\n'
                path = folder / command
                path.write_text(text); path.chmod(0o755)
            (folder / 'sed').symlink_to('/usr/bin/sed')
            env = dict(os.environ, PATH=str(folder), OACLIS_TEST_LOG=str(folder/'mutations'))
            result = subprocess.run(['/bin/bash', str(ROOT/'run.sh'), '--check'], env=env, capture_output=True, text=True)
            self.assertFalse((folder/'mutations').exists())
            return result

    def test_linux_and_macos_checks(self):
        for system in ('Linux', 'Darwin'):
            result = self.check(system)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('Sistema: '+system, result.stdout)
            self.assertIn('non è stato compilato o avviato', result.stdout)

    def test_missing_tool_is_reported_without_installing(self):
        result = self.check('Linux', 'rsync')
        self.assertEqual(result.returncode, 1)
        self.assertIn('[Manca] rsync', result.stdout)

if __name__ == '__main__': unittest.main()
