"""Fail-closed execution policy checks; no provider login or model calls."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent


class PolicyTests(unittest.TestCase):
    def test_missing_outer_boundary_does_not_run_command(self):
        with tempfile.TemporaryDirectory() as directory:
            marker = Path(directory) / 'must-not-run'
            result = subprocess.run(['python3', '-I', str(ROOT / 'agent-policy.py'),
                                     '--', 'touch', str(marker)], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('Agent boundary refused', result.stderr)
            self.assertFalse(marker.exists())

    def test_managed_agent_defaults(self):
        claude = json.loads((ROOT / 'policy/claude.json').read_text())
        self.assertEqual(claude['permissions']['defaultMode'], 'dontAsk')
        self.assertEqual(claude['permissions']['disableBypassPermissionsMode'], 'disable')
        self.assertIn('Bash', claude['permissions']['allow'])
        self.assertEqual(claude['allowedMcpServers'], [])
        self.assertFalse(claude['sandbox']['enabled'])
        import tomllib
        codex = tomllib.loads((ROOT / 'policy/codex.toml').read_text())
        self.assertEqual(codex['approval_policy'], 'never')
        self.assertEqual(codex['sandbox_mode'], 'danger-full-access')

    def test_entrypoint_validates_before_requested_command(self):
        entrypoint = (ROOT / 'entrypoint.sh').read_text()
        self.assertIn('exec python3 -I /usr/local/lib/agent-policy.py -- "$@"', entrypoint)
        self.assertNotIn('bypassPermissions', (ROOT / 'postStartCommand.sh').read_text())


if __name__ == '__main__':
    unittest.main()
