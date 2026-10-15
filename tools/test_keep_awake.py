# test_keep_awake.py v1.3.65
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

    def test_windows_preparation_and_hold_share_the_installed_helper(self):
        child=Mock();child.poll.return_value=None;child.stdout.readline.return_value=b'ready\r\n'
        ready=Mock(returncode=0,stdout=b'ready\r\n',stderr=b'')
        output=io.StringIO()
        with patch('pathlib.Path.exists',return_value=False), patch('platform.system',return_value='Windows'), patch('subprocess.run',return_value=ready) as prepare, patch('subprocess.Popen',return_value=child) as hold, patch('time.sleep'), patch('select.select',return_value=([True],[],[])), patch('sys.stdin',io.StringIO('')), contextlib.redirect_stdout(output):
            runpy.run_path(str(Path(__file__).with_name('keep_awake.py')))
        self.assertIn('"active": true',output.getvalue())
        self.assertIn('-EncodedCommand',prepare.call_args.args[0])
        self.assertIn('-EncodedCommand',hold.call_args.args[0])
        child.stdin.close.assert_called_once()
        child.terminate.assert_called_once()

    def test_linux_preparation_grants_only_the_two_inhibitor_actions(self):
        import types
        result=Mock(returncode=0)
        user=types.SimpleNamespace(pw_name='kimrusso',pw_uid=1000)
        output=io.StringIO()
        with patch('sys.argv',['keep_awake.py','--prepare']), patch('pathlib.Path.exists',return_value=False), patch('platform.system',return_value='Linux'), patch('platform.release',return_value='native'), patch('shutil.which',return_value='/usr/bin/systemd-inhibit'), patch.dict('os.environ',{'SUDO_USER':'kimrusso'}), patch('os.getuid',return_value=1000), patch('os.geteuid',return_value=1000), patch('pwd.getpwnam',return_value=user), patch('subprocess.run',return_value=result) as install, patch('sys.stdin',io.StringIO('')), contextlib.redirect_stdout(output):
            with self.assertRaises(SystemExit) as stopped:
                runpy.run_path(str(Path(__file__).with_name('keep_awake.py')))
        self.assertEqual(stopped.exception.code,0)
        command=install.call_args.args[0]
        self.assertEqual(command[:2],['sudo','-n'])
        self.assertEqual(command[-1],'/etc/polkit-1/rules.d/49-oaclis-awake-1000.rules')
        rule=install.call_args.kwargs['input'].decode()
        self.assertIn('subject.user === "kimrusso"',rule)
        self.assertEqual(rule.count('action.id ==='),2)
        self.assertNotIn('subject.isInGroup',rule)


if __name__ == '__main__':
    unittest.main()
