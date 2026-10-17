"""Selected state and reset tests using dummy provider markers only."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent


class StateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.session = Path(self.temp.name)
        self.controls = self.session / 'controls/.devcontainer'
        shutil.copytree(ROOT, self.controls, ignore=shutil.ignore_patterns('__pycache__'))
        (self.session / 'work/.git').mkdir(parents=True)
        docker = self.session / 'docker'
        docker.write_text('#!/bin/sh\nexit 0\n')
        docker.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.session) + ':' + os.environ['PATH'])
        (self.session / 'controls/session.json').write_text(json.dumps({
            'version': 1, 'source': str(self.session / 'work')}))

    def run_state(self, *args):
        return subprocess.run(['python3', '-I', str(self.controls / 'agent-state.py'), *args],
                              capture_output=True, text=True, env=self.env)

    def test_only_selected_state_is_mounted(self):
        for agent in ('claude', 'codex', 'none'):
            result = self.run_state('select', agent)
            self.assertEqual(result.returncode, 0, result.stderr)
            data = json.loads((self.controls / 'agent-state.compose.json').read_text())
            app = data['services']['app']
            self.assertEqual(app['environment']['FAMILY_AI_AGENT'], agent)
            mounts = app.get('volumes', [])
            self.assertEqual(len(mounts), 0 if agent == 'none' else 1)
            if mounts:
                mount = mounts[0]
                self.assertEqual(mount['source'], str(self.session / 'state' / agent))
                self.assertEqual(mount['target'], '/home/vscode/.' + agent)
                self.assertFalse(mount['bind']['create_host_path'])

    def test_independent_sessions_have_separate_state(self):
        self.assertEqual(self.run_state('select', 'claude').returncode, 0)
        first = self.session / 'state/claude'
        (first / 'dummy-marker').write_text('first-session')
        other = self.session / 'other-session'
        shutil.copytree(self.session / 'controls', other / 'controls')
        (other / 'work/.git').mkdir(parents=True)
        (other / 'controls/session.json').write_text(json.dumps({
            'version': 1, 'source': str(other / 'work')}))
        result = subprocess.run(['python3', '-I', str(other / 'controls/.devcontainer/agent-state.py'),
                                 'select', 'claude'], env=self.env, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        second = other / 'state/claude'
        self.assertFalse((second / 'dummy-marker').exists())
        self.assertNotEqual(first, second)

    def test_reset_keeps_only_auth_and_including_auth_removes_it(self):
        for agent, auth in (('claude', '.credentials.json'), ('codex', 'auth.json')):
            self.assertEqual(self.run_state('select', agent).returncode, 0)
            state = self.session / 'state' / agent
            (state / auth).write_text('dummy-login-marker')
            (state / 'settings.json').write_text('dummy-executable-settings')
            (state / 'plugins').mkdir()
            (state / 'plugins/hook').write_text('dummy-hook')
            result = self.run_state('reset', agent)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual({p.name for p in state.iterdir()}, {auth})
            self.assertEqual((state / auth).read_text(), 'dummy-login-marker')
            result = self.run_state('reset', agent, '--including-auth')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(list(state.iterdir()), [])

    def test_refuses_state_symlink_and_auth_symlink(self):
        outside = self.session / 'unrelated'
        outside.mkdir()
        (outside / 'marker').write_text('dummy')
        (self.session / 'state').symlink_to(outside, target_is_directory=True)
        self.assertNotEqual(self.run_state('select', 'claude').returncode, 0)
        (self.session / 'state').unlink()
        self.assertEqual(self.run_state('select', 'codex').returncode, 0)
        (self.session / 'state/codex/auth.json').symlink_to(outside / 'marker')
        self.assertNotEqual(self.run_state('reset', 'codex').returncode, 0)
        self.assertEqual((outside / 'marker').read_text(), 'dummy')

    def test_reset_stops_both_editor_and_terminal_and_fails_closed(self):
        self.assertEqual(self.run_state('select', 'codex').returncode, 0)
        marker = self.session / 'state/codex/settings.json'
        marker.write_text('keep-until-containers-stop')
        docker = self.session / 'docker'
        docker.write_text('#!/bin/sh\nif [ "$1" = ps ]; then echo "terminal editor"; fi\nexit 0\n')
        result = self.run_state('reset', 'codex')
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(marker.exists())
        log = self.session / 'docker-log'
        alive = self.session / 'alive'
        alive.touch()
        docker.write_text('#!/bin/sh\n'
                          'if [ "$1" = ps ] && [ -e "' + str(alive) + '" ]; then echo "terminal editor"; fi\n'
                          'if [ "$1" = rm ]; then echo "$*" > "' + str(log) + '"; rm "' + str(alive) + '"; fi\n')
        self.assertEqual(self.run_state('reset', 'codex').returncode, 0)
        self.assertEqual(log.read_text().strip(), 'rm --force terminal editor')
        self.assertFalse(marker.exists())

    def test_selection_requires_installed_session(self):
        (self.session / 'controls/session.json').unlink()
        self.assertNotEqual(self.run_state('select', 'codex').returncode, 0)


if __name__ == '__main__':
    unittest.main()
