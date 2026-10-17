"""Check effective entry-point configurations without starting Docker containers."""
import json
import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent


class RuntimeConfigTests(unittest.TestCase):
    def test_image_and_editor_start_without_root(self):
        dockerfile = (ROOT / "Dockerfile").read_text()
        self.assertIn("USER vscode", dockerfile)
        self.assertNotIn("NOPASSWD", dockerfile)
        self.assertNotIn("gosu", (ROOT / "entrypoint.sh").read_text())
        editor = json.loads((ROOT / "devcontainer.json").read_text())
        self.assertEqual(editor["containerUser"], "vscode")
        self.assertFalse(editor["updateRemoteUserUID"])

    def test_entry_points_have_no_docker_authority(self):
        vscode = json.loads((ROOT / "devcontainer.json").read_text())
        files = vscode["dockerComposeFile"]
        if isinstance(files, str):
            files = [files]
        profiles = {
            "vscode": files,
            "terminal": ["docker-compose.yml"],
            "smoke": ["docker-compose.yml", "smoke-test.compose.yml"],
        }
        env = {key: os.environ[key] for key in ("PATH", "HOME") if key in os.environ}
        env.update(GH_TOKEN="dummy-host-gh-token", GITHUB_TOKEN="dummy-host-github-token",
                   SSH_AUTH_SOCK="/tmp/test-ssh-agent.sock",
                   SMOKE_CLAUDE_VOLUME="test-claude", SMOKE_CODEX_VOLUME="test-codex")
        for name, compose_files in profiles.items():
            with self.subTest(entry_point=name):
                command = ["docker", "compose", "--env-file", "/dev/null"]
                for file in compose_files:
                    command.extend(["-f", str(ROOT / file)])
                result = subprocess.run([*command, "config", "--format", "json"],
                                        env=env, capture_output=True, text=True, check=True)
                services = json.loads(result.stdout)["services"]
                app = services["app"]
                gateway = services["egress"]
                self.assertTrue(gateway["read_only"])
                self.assertIn("ALL", gateway["cap_drop"])
                self.assertEqual(app["depends_on"]["egress"]["condition"], "service_healthy")
                self.assertTrue(next(mount for mount in app["volumes"]
                                     if mount["target"] == "/run/egress")["read_only"])
                self.assertEqual(len(gateway["volumes"]), 1)
                self.assertNotIn("ports", gateway)
                self.assertFalse(app.get("privileged", False))
                self.assertFalse(app.get("devices"))
                self.assertFalse(app.get("device_cgroup_rules"))
                self.assertFalse(app.get("cap_add"))
                self.assertEqual(app["user"], "vscode")
                self.assertEqual(app["cap_drop"], ["ALL"])
                self.assertEqual(app["security_opt"], ["no-new-privileges:true"])
                self.assertEqual(app["pids_limit"], 512)
                self.assertEqual(app["mem_limit"], "8589934592")
                self.assertEqual(float(app["cpus"]), 4.0)
                self.assertEqual(app.get("network_mode"), "none")
                self.assertNotEqual(app.get("pid"), "host")
                environment = app.get("environment", {})
                self.assertNotIn("DOCKER_HOST", environment)
                self.assertNotIn("ENABLE_DOCKER", environment)
                allowed = {"/workspace", "/home/vscode/.claude", "/home/vscode/.codex", "/run/egress"}
                for key in ("GH_TOKEN", "GITHUB_TOKEN", "SSH_AUTH_SOCK", "HOST_SSH_AUTH_SOCK"):
                    self.assertNotIn(key, environment)
                self.assertEqual({mount["target"] for mount in app.get("volumes", [])}, allowed)


if __name__ == "__main__":
    unittest.main()
