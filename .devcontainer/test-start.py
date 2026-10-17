"""Launcher argument tests; Docker and SSH are stubbed, no container needed."""
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
import unittest

START = Path(__file__).with_name("start.sh")


class StartTests(unittest.TestCase):
    def run_start(self, *args, with_agent=True):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            controls = root / "controls/.devcontainer"
            shutil.copytree(START.parent, controls, ignore=shutil.ignore_patterns("__pycache__"))
            (root / "work/.git").mkdir(parents=True)
            (root / "controls/session.json").write_text(json.dumps({"version": 1, "source": str(root / "work")}))
            docker = root / "docker"
            docker.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "with open(os.environ['COMMAND_LOG'], 'a') as log:\n"
                "    log.write(json.dumps([os.environ.get('HERDR_AGENT'), sys.argv[1:]]) + '\\n')\n"
            )
            docker.chmod(0o755)
            for tool in ("ssh-add", "ssh-agent"):
                stub = root / tool
                stub.write_text('#!/bin/sh\necho invoked >> "$SSH_COMMAND_LOG"\nexit 97\n')
                stub.chmod(0o755)
            log = root / "commands"
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                       SSH_AUTH_SOCK=str(root / "socket"), COMMAND_LOG=str(log),
                       SSH_COMMAND_LOG=str(root / "ssh-commands"))
            env.pop("HERDR_AGENT", None)
            if not with_agent:
                env.pop("SSH_AUTH_SOCK", None)
            result = subprocess.run(["bash", str(controls / "start.sh"), *args], env=env,
                                    capture_output=True, text=True)
            self.assertFalse((root / "ssh-commands").exists(), "launcher invoked an SSH tool")
            calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
            return result, calls

    def test_start_never_uses_host_agent(self):
        for with_agent in (False, True):
            with self.subTest(with_agent=with_agent):
                result, calls = self.run_start(with_agent=with_agent)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertNotIn("unexpected SSH invocation", result.stderr)
                self.assertFalse(any("compose.ssh.yml" in arg for call in calls for arg in call[1]))

    def test_stop_preserves_source_and_volumes(self):
        result, calls = self.run_start("--stop")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(calls), 1)
        self.assertEqual(calls[0][1][-1], "down")
        self.assertNotIn("--volumes", calls[0][1])

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
