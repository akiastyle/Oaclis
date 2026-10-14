# test_bootstrap.py v1.4.66
"""Bootstrap checks must not install or start anything."""
from pathlib import Path
import os
import subprocess
import tempfile
import shutil
import unittest

ROOT = Path(__file__).resolve().parent.parent

class BootstrapTests(unittest.TestCase):
    def check(self, system, missing=None, normal=False):
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
            (folder / 'bash').symlink_to('/bin/bash')
            (folder / 'dirname').symlink_to('/usr/bin/dirname')
            (folder / 'mkdir').symlink_to('/bin/mkdir')
            (folder / 'chmod').symlink_to('/bin/chmod')
            (folder / 'home').mkdir()
            repo = folder / 'repo'
            (repo / 'scripts').mkdir(parents=True)
            shutil.copy(ROOT / 'run.sh', repo / 'run.sh')
            shutil.copy(ROOT / 'scripts/prepare_codex.sh', repo / 'scripts/prepare_codex.sh')
            (repo / 'scripts/install.sh').write_text('''#!/bin/sh
echo "install $*" >> "$OACLIS_TEST_LOG"
mkdir -p "$HOME/.local/bin"
printf '#!/bin/sh\necho setup >> "$OACLIS_TEST_LOG"\n' > "$HOME/.local/bin/oaclis"
chmod +x "$HOME/.local/bin/oaclis"
''')
            env = dict(os.environ, HOME=str(folder/'home'), PATH=str(folder), OACLIS_TEST_LOG=str(folder/'mutations'))
            arguments = [] if normal else ['--check']
            result = subprocess.run(['/bin/bash', str(repo/'run.sh'), *arguments], env=env, capture_output=True, text=True)
            if normal:
                self.assertEqual((folder/'mutations').read_text(), 'install --unconfigured\nsetup\n')
            else:
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

    def test_normal_run_installs_without_assigning_role(self):
        result = self.check('Linux', normal=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('Oaclis compilato e installato', result.stdout)

if __name__ == '__main__': unittest.main()
