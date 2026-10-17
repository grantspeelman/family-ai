"""Exercise reviewed local commits and host publishing without a network or login."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class PublishingTests(unittest.TestCase):
    def test_local_commit_then_host_push(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            home = root / 'home'
            home.mkdir()
            env = {'PATH': os.environ['PATH'], 'HOME': str(home),
                   'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null'}

            def git(*args, cwd=root):
                return subprocess.run(['git', *args], cwd=cwd, env=env, check=True,
                                      capture_output=True, text=True).stdout.strip()

            remote, checkout = root / 'remote.git', root / 'checkout'
            git('init', '--bare', str(remote))
            git('init', '-b', 'work', str(checkout))
            git('config', 'user.name', 'Fixture', cwd=checkout)
            git('config', 'user.email', 'fixture@example.invalid', cwd=checkout)
            (checkout / 'example.txt').write_text('reviewed local change\n')
            git('add', 'example.txt', cwd=checkout)
            git('commit', '-m', 'Add fixture', cwd=checkout)
            git('remote', 'add', 'origin', str(remote), cwd=checkout)
            # Review and publish the same explicit branch as the host instructions.
            self.assertIn('reviewed local change', git('show', 'HEAD', cwd=checkout))
            git('push', '-u', 'origin', 'work', cwd=checkout)
            self.assertEqual(git('rev-parse', 'HEAD', cwd=checkout),
                             git('--git-dir', str(remote), 'rev-parse', 'refs/heads/work'))


if __name__ == '__main__':
    unittest.main()
