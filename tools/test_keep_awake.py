# test_keep_awake.py v1.1.36
import contextlib
import io
from pathlib import Path
import runpy
import unittest
from unittest.mock import Mock, patch


class KeepAwake(unittest.TestCase):
    def test_lifetime_and_failure_are_reported(self):
        for failure in (False, True):
            with self.subTest(failure=failure):
                child = Mock()
                child.poll.return_value = 1 if failure else None
                child.stderr.read.return_value = b'No system sleep inhibitor'
                child.stdin = None
                output = io.StringIO()
                with patch('pathlib.Path.exists', return_value=False), patch('platform.system', return_value='Linux'), patch('platform.release', return_value='native'), patch('shutil.which', return_value='/usr/bin/systemd-inhibit'), patch('subprocess.Popen', return_value=child), patch('time.sleep'), patch('select.select', return_value=([True], [], [])), patch('sys.stdin', io.StringIO('')), contextlib.redirect_stdout(output):
                    if failure:
                        with self.assertRaises(SystemExit):
                            runpy.run_path(str(Path(__file__).with_name('keep_awake.py')))
                    else:
                        runpy.run_path(str(Path(__file__).with_name('keep_awake.py')))
                if failure:
                    self.assertIn('"active": false', output.getvalue())
                    self.assertIn('No system sleep inhibitor', output.getvalue())
                else:
                    self.assertIn('"active": true', output.getvalue())
                    child.terminate.assert_called_once()
                    child.wait.assert_called_once()


if __name__ == '__main__':
    unittest.main()
