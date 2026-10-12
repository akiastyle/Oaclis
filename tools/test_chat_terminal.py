import unittest
from unittest.mock import patch
import chat_terminal


class TerminalValidation(unittest.TestCase):
    def test_untrusted_id_never_runs_a_command(self):
        with patch('chat_terminal.subprocess.run') as run:
            with self.assertRaises(ValueError):
                chat_terminal.request({'action': 'open', 'chat_id': 'x;touch bad'})
            run.assert_not_called()

    def test_input_is_literal_and_key_is_allowlisted(self):
        with patch('chat_terminal.subprocess.run') as run:
            run.return_value.returncode = 0
            run.return_value.stdout = 'screen'
            chat_terminal.request({'action': 'input', 'chat_id': 'chat-1', 'text': '$(touch bad)', 'key': 'Enter'})
            self.assertEqual(run.call_args_list[0].args[0], ['tmux', 'send-keys', '-t', 'oaclis-chat-1:0.0', '-l', '--', '$(touch bad)'])
            run.reset_mock()
            with self.assertRaises(ValueError):
                chat_terminal.request({'action': 'input', 'chat_id': 'chat-1', 'key': 'run-shell'})
            run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
