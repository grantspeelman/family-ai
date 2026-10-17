"""Exercise host IDE startup with a fake editor and dummy host credentials."""
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent


class VscodeTests(unittest.TestCase):
    def test_direct_reopen_is_rejected(self):
        result = subprocess.run(['bash', str(ROOT / 'check-vscode-host.sh')],
                                env={'PATH': os.environ['PATH'], 'HOME': '/tmp'},
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('start-vscode.sh', result.stderr)

    def test_profile_override_is_rejected(self):
        result = subprocess.run(['bash', str(ROOT / 'start-vscode.sh'),
                                 '--user-data-dir', '/tmp/existing-profile'],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 64)

    def test_clean_profile_and_argument_boundaries(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = root / 'controls/.devcontainer'
            shutil.copytree(ROOT, controls, ignore=shutil.ignore_patterns('__pycache__'))
            (root / 'work/.git').mkdir(parents=True)
            (root / 'controls/session.json').write_text(json.dumps({'version': 1, 'source': str(root / 'work')}))
            code = root / 'code'
            code.write_text('''#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys
assert os.environ.get('GH_TOKEN') is None
assert os.environ.get('GITHUB_TOKEN') is None
assert os.environ.get('SSH_AUTH_SOCK') is None
assert os.environ.get('HOST_SSH_AUTH_SOCK') is None
assert os.environ.get('GIT_ASKPASS') is None
assert os.environ.get('AWS_SECRET_ACCESS_KEY') is None
assert os.environ['GIT_CONFIG_GLOBAL'] == '/dev/null'
assert os.environ['GIT_CONFIG_NOSYSTEM'] == '1'
assert int(os.environ['USER_UID']) > 0
assert int(os.environ['USER_GID']) > 0
assert not (pathlib.Path.home() / '.gitconfig').exists()
assert not (pathlib.Path.home() / '.ssh').exists()
assert not (pathlib.Path.home() / '.config/gh').exists()
args = sys.argv[1:]
profile = pathlib.Path(args[args.index('--user-data-dir') + 1])
settings = json.loads((profile / 'User/settings.json').read_text())
assert settings['dev.containers.gitCredentialHelperConfigLocation'] == 'none'
assert settings['dev.containers.copyGitConfig'] is False
assert settings['dev.containers.dockerCredentialHelper'] is False
assert json.loads((pathlib.Path.cwd() / '.devcontainer/devcontainer.json').read_text())['customizations']['vscode']['extensions'] == ['openai.chatgpt']
subprocess.run(['bash', str(pathlib.Path.cwd() / '.devcontainer/check-vscode-host.sh')], check=True)
if '--install-extension' not in args:
    assert args[-2:] == ['--goto', 'two words:3']
    assert '--wait' in args
''')
            code.chmod(0o755)
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                       GH_TOKEN='dummy', GITHUB_TOKEN='dummy', SSH_AUTH_SOCK='/tmp/dummy.sock',
                       HOST_SSH_AUTH_SOCK='/tmp/dummy.sock', GIT_ASKPASS='/tmp/dummy',
                       AWS_SECRET_ACCESS_KEY='dummy')
            result = subprocess.run(['bash', str(controls / 'start-vscode.sh'), '--codex', '--goto', 'two words:3'],
                                    env=env, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
