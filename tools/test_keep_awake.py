# test_keep_awake.py v1.4.85
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
                with patch('pathlib.Path.exists', return_value=False), patch('platform.system', return_value='Linux'), patch('platform.release', return_value='native'), patch('shutil.which', return_value='/usr/bin/systemd-inhibit'), patch('subprocess.Popen', return_value=child) as launch, patch('time.sleep'), patch('select.select', return_value=([True], [], [])), patch('sys.stdin', io.StringIO('')), contextlib.redirect_stdout(output):
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
                    self.assertEqual(child.terminate.call_count,2)
                    self.assertEqual(child.wait.call_count,2)
                    arguments=[call.args[0] for call in launch.call_args_list]
                    self.assertTrue(any('--what=sleep' in args for args in arguments))
                    self.assertTrue(any('--what=idle' in args for args in arguments))
                    self.assertFalse(any('--what=sleep:idle' in args for args in arguments))

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

    def test_explicit_user_is_preserved_when_preparing_from_root_shell(self):
        import types
        user=types.SimpleNamespace(pw_name='kimrusso',pw_uid=1000)
        with patch('sys.argv',['keep_awake.py','--prepare','--user','kimrusso']), patch('pathlib.Path.exists',return_value=False), patch('pathlib.Path.is_symlink',return_value=False), patch('platform.system',return_value='Linux'), patch('platform.release',return_value='native'), patch('shutil.which',return_value='/usr/bin/systemd-inhibit'), patch.dict('os.environ',{'SUDO_USER':'root'}), patch('os.geteuid',return_value=0), patch('pwd.getpwnam',return_value=user) as lookup, patch('pathlib.Path.write_text') as write, patch('pathlib.Path.chmod'), patch('os.chown'), contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(SystemExit) as stopped:
                runpy.run_path(str(Path(__file__).with_name('keep_awake.py')))
        self.assertEqual(stopped.exception.code,0)
        lookup.assert_called_once_with('kimrusso')
        self.assertIn('subject.user === "kimrusso"',write.call_args.args[0])


if __name__ == '__main__':
    unittest.main()
