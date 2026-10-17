"""Fixture-only tests for trusted controls and data-only source transfer."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
import sys
sys.dont_write_bytecode = True
from unittest import mock

MODULE = Path(__file__).with_name('host-session.py')


def git(root, *args):
    return subprocess.run(['git', '-C', str(root), *args], check=True,
                          capture_output=True, text=True).stdout.strip()


class IsolationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.trusted = self.root / 'trusted'
        self.trusted.mkdir()
        git(self.trusted, 'init')
        git(self.trusted, 'config', 'user.name', 'Fixture')
        git(self.trusted, 'config', 'user.email', 'fixture@example.invalid')
        controls = self.trusted / '.devcontainer'
        controls.mkdir()
        for file in MODULE.parent.rglob('*'):
            if file.is_file() and '__pycache__' not in file.parts:
                dest = controls / file.relative_to(MODULE.parent)
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(file.read_bytes())
        (self.trusted / 'source.txt').write_text('before\n')
        git(self.trusted, 'add', '.')
        git(self.trusted, 'commit', '-m', 'Fixture')
        self.session = self.root / 'session'

    def run_manager(self, *args, success=True, installed=False):
        script = self.session / 'controls/.devcontainer/host-session.py' if installed else MODULE
        result = subprocess.run(['python3', '-I', str(script), *map(str, args)],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode == 0, success, result.stderr)
        return result

    def install(self):
        self.run_manager('create', '--source', self.trusted, '--session', self.session)
        return self.session / 'work'

    def test_independent_git_and_restart_policy(self):
        work = self.install()
        git(work, 'config', 'user.name', 'Agent')
        git(work, 'config', 'user.email', 'agent@example.invalid')
        original = (self.session / 'controls/.devcontainer/docker-compose.yml').read_bytes()
        (work / '.devcontainer/docker-compose.yml').write_text('proposed policy\n')
        (work / '.git/hooks').mkdir(exist_ok=True)
        (work / '.git/hooks/pre-commit').write_text('agent hook')
        (work / 'source.txt').write_text('after\n')
        git(work, 'add', 'source.txt')
        git(work, 'commit', '-m', 'Edit')
        self.assertNotEqual(git(work, 'rev-parse', 'HEAD'), git(self.trusted, 'rev-parse', 'HEAD'))
        self.assertFalse((self.trusted / '.git/hooks/pre-commit').exists())
        self.assertEqual((self.session / 'controls/.devcontainer/docker-compose.yml').read_bytes(), original)
        self.assertEqual((self.trusted / 'source.txt').read_text(), 'before\n')
        self.assertNotEqual(os.stat(work / '.git/config').st_ino, os.stat(self.trusted / '.git/config').st_ino)
        self.run_manager('check', installed=True)
        self.assertIn(str(work), (self.session / 'controls/session.json').read_text())

    def test_data_transfer_never_runs_agent_git(self):
        work = self.install()
        marker = self.root / 'executed'
        (work / '.git/config').write_text(f'[core]\n fsmonitor = touch {marker}\n hooksPath = /bad\n')
        (work / 'source.txt').write_text('reviewed\n')
        (work / 'new.txt').write_text('new\n')
        bundle = self.root / 'changes.json'
        self.run_manager('export', '--path', 'source.txt', '--path', 'new.txt', '--output', bundle, installed=True)
        result = self.run_manager('review', '--bundle', bundle, '--target', self.trusted, installed=True)
        self.assertIn('+reviewed', result.stdout)
        self.run_manager('apply', '--bundle', bundle, '--target', self.trusted, '--reviewed', installed=True)
        self.assertEqual((self.trusted / 'source.txt').read_text(), 'reviewed\n')
        self.assertEqual((self.trusted / 'new.txt').read_text(), 'new\n')
        self.assertFalse(marker.exists())
        self.assertNotIn('hooksPath', (self.trusted / '.git/config').read_text())

    def test_rejects_paths_links_and_shared_metadata(self):
        work = self.install()
        (work / 'outside').symlink_to(self.trusted, target_is_directory=True)
        for path in ('../trusted/source.txt', '.git/config', 'outside/source.txt', '/etc/passwd'):
            with self.subTest(path=path):
                self.run_manager('export', '--path', path, '--output', self.root / 'bad.json',
                                 installed=True, success=False)
        import shutil
        shutil.rmtree(work / '.git')
        (work / '.git').write_text(f'gitdir: {self.trusted}/.git\n')
        self.run_manager('check', installed=True, success=False)

    def test_directory_swap_does_not_follow_replacement_link(self):
        work = self.install()
        (work / 'dir').mkdir()
        (work / 'dir/file').write_text('inside')
        (self.trusted / 'file').write_text('outside')
        spec = importlib.util.spec_from_file_location('host_session', MODULE)
        manager = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(manager)
        real_open = os.open
        swapped = False

        def swapping_open(name, flags, *args, **kwargs):
            nonlocal swapped
            fd = real_open(name, flags, *args, **kwargs)
            if name == 'dir' and not swapped:
                swapped = True
                (work / 'dir').rename(work / 'retained')
                (work / 'dir').symlink_to(self.trusted, target_is_directory=True)
            return fd

        with mock.patch.object(manager.os, 'open', side_effect=swapping_open):
            self.assertEqual(manager.read_file(work, 'dir/file'), b'inside')
        self.assertTrue(swapped)

    def test_trusted_worktree_creates_independent_git_and_export_stays_outside(self):
        linked = self.root / 'linked'
        git(self.trusted, 'worktree', 'add', str(linked), '-b', 'linked')
        self.run_manager('create', '--source', linked, '--session', self.session)
        work = self.session / 'work'
        self.assertTrue((work / '.git').is_dir())
        self.assertFalse((work / '.git/commondir').exists())
        self.run_manager('export', '--path', 'source.txt', '--output', work / 'bundle.json',
                         installed=True, success=False)

    def test_oversized_file_and_bundle_rejected_and_review_escapes_controls(self):
        work = self.install()
        with (work / 'large').open('wb') as stream:
            stream.truncate(9 * 1024 * 1024)
        self.run_manager('export', '--path', 'large', '--output', self.root / 'large.json',
                         installed=True, success=False)
        bundle = self.root / 'huge.json'
        with bundle.open('wb') as stream:
            stream.truncate(49 * 1024 * 1024)
        self.run_manager('review', '--bundle', bundle, '--target', self.trusted,
                         installed=True, success=False)
        (work / 'source.txt').write_text('hello\x1b[2J\u202eworld\n')
        bundle = self.root / 'escaped.json'
        self.run_manager('export', '--path', 'source.txt', '--output', bundle, installed=True)
        result = self.run_manager('review', '--bundle', bundle, '--target', self.trusted, installed=True)
        self.assertNotIn('\x1b', result.stdout)
        self.assertNotIn('\u202e', result.stdout)
        self.assertIn('\\u001b', result.stdout)
        for files in ({'source.txt': 42}, {str(i): None for i in range(129)}):
            bundle.write_text(json.dumps({'version': 1, 'files': files}))
            self.run_manager('review', '--bundle', bundle, '--target', self.trusted,
                             installed=True, success=False)

    def test_review_distinguishes_literal_escapes_from_controls_and_invalid_utf8(self):
        work = self.install()
        pairs = [(b'literal \\u202e\n', 'literal \u202e\n'.encode()),
                 (b'literal \\xff\n', b'literal \xff\n')]
        for index, (before, after) in enumerate(pairs):
            (self.trusted / 'source.txt').write_bytes(before)
            (work / 'source.txt').write_bytes(after)
            bundle = self.root / f'escape-{index}.json'
            self.run_manager('export', '--path', 'source.txt', '--output', bundle, installed=True)
            result = self.run_manager('review', '--bundle', bundle, '--target', self.trusted,
                                      installed=True)
            self.assertIn('@@', result.stdout)
            removed = [line[1:] for line in result.stdout.splitlines()
                       if line.startswith('-') and not line.startswith('---')]
            added = [line[1:] for line in result.stdout.splitlines()
                     if line.startswith('+') and not line.startswith('+++')]
            self.assertEqual(len(removed), 1)
            self.assertEqual(len(added), 1)
            self.assertNotEqual(removed, added)

    def test_review_reports_empty_file_creation_and_deletion(self):
        work = self.install()
        (work / 'create empty.txt').write_bytes(b'')
        (self.trusted / 'delete empty.txt').write_bytes(b'')
        bundle = self.root / 'empty-changes.json'
        self.run_manager('export', '--path', 'create empty.txt', '--path', 'delete empty.txt',
                         '--output', bundle, installed=True)
        result = self.run_manager('review', '--bundle', bundle, '--target', self.trusted,
                                  installed=True)
        self.assertIn('create "create empty.txt"', result.stdout)
        self.assertIn('delete "delete empty.txt"', result.stdout)
        self.run_manager('apply', '--bundle', bundle, '--target', self.trusted, '--reviewed',
                         installed=True)
        self.assertTrue((self.trusted / 'create empty.txt').is_file())
        self.assertEqual((self.trusted / 'create empty.txt').read_bytes(), b'')
        self.assertFalse((self.trusted / 'delete empty.txt').exists())

    def test_no_overwrite_or_implicit_cleanup(self):
        work = self.install()
        (work / 'source.txt').write_text('unreviewed\n')
        self.run_manager('create', '--source', self.trusted, '--session', self.session, success=False)
        self.assertEqual((work / 'source.txt').read_text(), 'unreviewed\n')
        self.run_manager('create', '--source', self.trusted,
                         '--session', self.trusted / 'nested', success=False)
        self.assertFalse((self.trusted / 'nested').exists())

    def test_apply_rejects_symlink_target_and_metadata_bundle(self):
        self.install()
        bundle = self.root / 'bad.json'
        bundle.write_text(json.dumps({'version': 1, 'files': {'.git/config': None}}))
        self.run_manager('apply', '--bundle', bundle, '--target', self.trusted, '--reviewed',
                         installed=True, success=False)
        (self.trusted / 'link').symlink_to(self.root / 'elsewhere')
        bundle.write_text(json.dumps({'version': 1, 'files': {'link/file': None}}))
        self.run_manager('apply', '--bundle', bundle, '--target', self.trusted, '--reviewed',
                         installed=True, success=False)


if __name__ == '__main__':
    unittest.main()
