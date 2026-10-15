# test_update_app.py v1.3.50
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('updater', Path(__file__).with_name('update_app.py'))
updater = importlib.util.module_from_spec(spec)
spec.loader.exec_module(updater)


class UpdateApp(unittest.TestCase):
    def test_safe_update_and_local_edits(self):
        revision = 'a' * 40
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'scripts').mkdir()
            (root / 'scripts/install.sh').write_text('fixture')
            for dirty in (False, True):
                commands = []
                def run(path, *args, **kwargs):
                    commands.append(args)
                    if args == ('git', 'status', '--porcelain'): return ' M ui/app.js' if dirty else ''
                    if args == ('git', 'branch', '--show-current'): return 'main'
                    if args == ('git', 'rev-parse', 'HEAD'): return revision
                    return ''
                with patch.object(updater, 'run', side_effect=run):
                    if dirty:
                        with self.assertRaisesRegex(ValueError, 'modifiche locali'): updater.update(root, revision)
                        self.assertEqual(len(commands), 1)
                    else:
                        updater.update(root, revision)
                        self.assertEqual(commands[-1], ('bash', 'scripts/install.sh', '--unconfigured'))
                        self.assertIn(('git', 'merge', '--ff-only', revision), commands)
                        self.assertFalse(any('reset' in command or 'checkout' in command for command in commands))

    def test_invalid_revision_and_divergence_stop_before_install(self):
        with patch.object(updater, 'run') as run:
            with self.assertRaises(ValueError): updater.update('/unused', 'HEAD; rm anything')
            run.assert_not_called()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); (root / 'scripts').mkdir(); (root / 'scripts/install.sh').touch()
            with patch.object(updater, 'run', side_effect=['', 'main', '', '', subprocess.CalledProcessError(1, ['git', 'merge-base'], output='diverged')]) as run:
                with self.assertRaises(subprocess.CalledProcessError): updater.update(root, 'a' * 40)
                self.assertEqual(run.call_count, 5)


if __name__ == '__main__':
    unittest.main()
