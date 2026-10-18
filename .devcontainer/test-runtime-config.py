"""Check effective entry-point configurations without starting Docker containers."""
import json
import os
from pathlib import Path
import subprocess
import unittest
import tempfile

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

    def test_setuid_probe_is_only_built_for_smoke_image(self):
        dockerfile = (ROOT / "Dockerfile").read_text()
        self.assertIn("FROM ubuntu:26.04 AS development", dockerfile)
        self.assertIn("FROM development AS smoke", dockerfile)
        self.assertTrue(dockerfile.rstrip().endswith("FROM development AS final"))
        self.assertNotIn("docker cp", (ROOT / "smoke-test.sh").read_text())

    def test_dynamic_smoke_profiles_keep_test_stage_and_selected_state(self):
        # Execute the same generator embedded in the smoke launcher, rather than
        # reconstructing an override that could drift from the actual entry point.
        generator = (ROOT / 'smoke-test.sh').read_text().split("<<'PROFILE'\n", 1)[1].split('\nPROFILE', 1)[0]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for agent in ('claude', 'codex', 'none'):
                subprocess.run(['python3', '-I', '-', str(root), agent],
                               input=generator, text=True, check=True)
                result = subprocess.run(['docker', 'compose', '--env-file', '/dev/null',
                                         '-f', str(ROOT / 'docker-compose.yml'),
                                         '-f', str(root / 'agent-state.compose.json'),
                                         'config', '--format', 'json'],
                                        env=dict(os.environ, WORKSPACE_SOURCE=str(root / 'work')),
                                        capture_output=True, text=True, check=True)
                app = json.loads(result.stdout)['services']['app']
                self.assertEqual(app['build']['target'], 'smoke')
                self.assertEqual(app['environment']['FAMILY_AI_AGENT'], agent)
                state = [mount for mount in app['volumes']
                         if mount['target'].startswith('/home/vscode/.')]
                self.assertEqual(len(state), 0 if agent == 'none' else 1)
                if state:
                    self.assertEqual(state[0]['source'], str(root / agent))
                    self.assertEqual(state[0]['target'], '/home/vscode/.' + agent)
                self.assertEqual(app['user'], 'vscode')
                self.assertEqual(app['cap_drop'], ['ALL'])
                self.assertEqual(app['network_mode'], 'none')

    def test_entry_points_have_no_docker_authority(self):
        vscode = json.loads((ROOT / "devcontainer.json").read_text())
        files = [file for file in vscode["dockerComposeFile"] if file != "agent-state.compose.json"]
        if isinstance(files, str):
            files = [files]
        profiles = {
            "vscode": files,
            "terminal": ["docker-compose.yml"],
        }
        env = {key: os.environ[key] for key in ("PATH", "HOME") if key in os.environ}
        env.update(WORKSPACE_SOURCE=str(ROOT.parent), GH_TOKEN="dummy-host-gh-token", GITHUB_TOKEN="dummy-host-github-token",
                   SSH_AUTH_SOCK="/tmp/test-ssh-agent.sock")
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
                self.assertNotIn("target", app["build"])
                self.assertTrue(gateway["read_only"])
                self.assertIn("ALL", gateway["cap_drop"])
                self.assertEqual(app["depends_on"]["egress"]["condition"], "service_healthy")
                self.assertTrue(next(mount for mount in app["volumes"]
                                     if mount["target"] == "/run/egress")["read_only"])
                self.assertEqual(len(gateway["volumes"]), 1)
                self.assertNotIn("ports", gateway)
                self.assertTrue(app["read_only"])
                self.assertEqual(len(app["tmpfs"]), 2)
                self.assertEqual({item.split(":", 1)[0] for item in app["tmpfs"]},
                                 {"/tmp", "/home/vscode"})
                for mount in app["tmpfs"]:
                    options = set(mount.split(":", 1)[1].split(","))
                    self.assertTrue({"exec", "nosuid", "nodev"} <= options, mount)
                    self.assertNotIn("noexec", options)
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
                allowed = {"/workspace", "/run/egress", "/opt/family-ai-controls"}
                for key in ("GH_TOKEN", "GITHUB_TOKEN", "SSH_AUTH_SOCK", "HOST_SSH_AUTH_SOCK"):
                    self.assertNotIn(key, environment)
                self.assertTrue(next(mount for mount in app["volumes"] if mount["target"] == "/opt/family-ai-controls")["read_only"])
                self.assertEqual({mount["target"] for mount in app.get("volumes", [])}, allowed)


if __name__ == "__main__":
    unittest.main()
