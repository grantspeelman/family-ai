"""Launcher argument tests; Docker and SSH are stubbed, no container needed."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

START = Path(__file__).with_name("start.sh")


class StartTests(unittest.TestCase):
    def run_start(self, *args):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            docker = root / "docker"
            docker.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "with open(os.environ['COMMAND_LOG'], 'a') as log:\n"
                "    log.write(json.dumps([os.environ.get('HERDR_AGENT'), sys.argv[1:]]) + '\\n')\n"
            )
            docker.chmod(0o755)
            ssh_add = root / "ssh-add"
            ssh_add.write_text("#!/bin/sh\nexit 0\n")
            ssh_add.chmod(0o755)
            log = root / "commands"
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                       SSH_AUTH_SOCK=str(root / "socket"), COMMAND_LOG=str(log))
            env.pop("HERDR_AGENT", None)
            result = subprocess.run(["bash", str(START), *args], env=env,
                                    capture_output=True, text=True)
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            return result, calls

    def test_default_shell(self):
        result, calls = self.run_start()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls[-1][1][-5:], ["exec", "--user", "vscode", "app", "bash"])

    def test_rebuild_and_agent_arguments(self):
        result, calls = self.run_start("--rebuild", "--", "claude", "--resume")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls[0][1][-3:], ["build", "--pull", "--no-cache"])
        self.assertEqual(calls[-1][0], "claude")
        self.assertEqual(calls[-1][1][-3:], ["app", "claude", "--resume"])

    def test_arbitrary_arguments_are_preserved(self):
        args = ["printf", "%s\\n", "two words", "", "$(touch nope)", "--docker"]
        result, calls = self.run_start("--", *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(calls[-1][1][-len(args):], args)
        self.assertIsNone(calls[-1][0])

    def test_codex_and_existing_shortcuts(self):
        for args in [("--", "codex", "resume"), ("--codex",), ("--claude",)]:
            with self.subTest(args=args):
                result, calls = self.run_start(*args)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(calls[-1][0], args[-1].removeprefix("--") if len(args) == 1 else "codex")

    def test_removed_docker_mode_fails_before_docker(self):
        for args in [("--docker",), ("--docker", "--codex"),
                     ("--claude", "--docker"), ("--docker", "--", "bash")]:
            with self.subTest(args=args):
                result, calls = self.run_start(*args)
                self.assertEqual(result.returncode, 64)
                self.assertEqual(calls, [])
                self.assertIn("CI", result.stderr)
                self.assertIn(".devcontainer/README.md", result.stderr)

    def test_invalid_options_fail_before_docker(self):
        for args in [("--unknown",), ("--claude", "--codex"), ("--",),
                     ("--claude", "--", "echo", "hi")]:
            with self.subTest(args=args):
                result, calls = self.run_start(*args)
                self.assertEqual(result.returncode, 64)
                self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
