# test_codex_setup.py v1.8.74
"""Exercise optional prompts and npm invocation without installing packages."""
import os
from pathlib import Path
import pty
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent

class CodexSetupTests(unittest.TestCase):
    def run_case(self, answers, node=True, npm_failure=False, existing=False, authenticated=False):
        with tempfile.TemporaryDirectory() as directory:
            folder=Path(directory); log=folder/'actions'
            if node:
                for name, output in [('node','22'),('npm','10.9')]:
                    path=folder/name
                    if name=='node': text=f'#!/bin/sh\necho {output}\n'
                    else:
                        text='#!/bin/sh\necho "$*" >> "$OACLIS_TEST_LOG"\nif [ "$1" = install ]; then\n'
                        if npm_failure: text+='exit 1\n'
                        else: text+='printf "#!/bin/sh\\necho codex-cli-0.162.1\\n" > "$OACLIS_TEST_BIN/codex"\n/bin/chmod +x "$OACLIS_TEST_BIN/codex"\n'
                        text+='fi\n'
                    path.write_text(text);path.chmod(0o755)
            if existing:
                codex=folder/'codex'
                codex.write_text('#!/bin/sh\necho "$*" >> "$OACLIS_TEST_LOG"\ncase "$*" in\n"--version") echo codex-cli-0.162.1;;\n"login status") [ -f "$OACLIS_TEST_BIN/logged-in" ];;\n"login --device-auth") echo ok > "$OACLIS_TEST_BIN/logged-in";;\nesac\n')
                codex.chmod(0o755)
                if authenticated: (folder/'logged-in').touch()
            script='source "$1"; confirm() { read -r answer; [[ "$answer" = s ]]; }; prepare_codex'
            env=dict(os.environ,PATH=str(folder),OACLIS_TEST_LOG=str(log),OACLIS_TEST_BIN=str(folder),NVM_DIR=str(folder/'nvm'))
            master,slave=pty.openpty()
            try:
                proc=subprocess.Popen(['/bin/bash','-c',script,'test',str(ROOT/'scripts/prepare_codex.sh')],stdin=slave,stdout=subprocess.PIPE,stderr=subprocess.PIPE,env=env,text=True)
                os.write(master,answers.encode())
                output,error=proc.communicate(timeout=10)
                return proc.returncode, output+error, log.read_text() if log.exists() else ''
            finally: os.close(master);os.close(slave)

    def test_declining_codex_does_not_install_anything(self):
        code,output,actions=self.run_case('n\n',node=False)
        self.assertEqual(code,0);self.assertEqual(actions,'');self.assertIn('continua',output)

    def test_missing_node_requires_separate_confirmation(self):
        code,output,actions=self.run_case('s\nn\n',node=False)
        self.assertEqual(code,0);self.assertEqual(actions,'');self.assertIn('Node.js',output)

    def test_npm_install_and_cli_verification(self):
        code,output,actions=self.run_case('s\ns\n')
        self.assertEqual(code,0,output);self.assertIn('@openai/codex@0.162.1',actions);self.assertIn('Codex verificato',output)

    def test_existing_codex_offers_chatgpt_login(self):
        code,output,actions=self.run_case('s\n',existing=True)
        self.assertEqual(code,0,output)
        self.assertIn('login --device-auth',actions)
        self.assertNotIn('install --global',actions)

    def test_login_can_be_deferred_with_visible_warning(self):
        code,output,actions=self.run_case('n\n',existing=True)
        self.assertEqual(code,0,output)
        self.assertIn('Accesso rimandato',output)
        self.assertNotIn('login --device-auth',actions)

    def test_existing_login_is_preserved(self):
        code,output,actions=self.run_case('',existing=True,authenticated=True)
        self.assertEqual(code,0,output)
        self.assertIn('Codex autenticato',output)
        self.assertNotIn('login --device-auth',actions)

    def test_npm_failure_is_reported(self):
        code,output,actions=self.run_case('s\ns\n',npm_failure=True)
        self.assertEqual(code,1);self.assertNotIn('Codex verificato',output)

if __name__=='__main__': unittest.main()
