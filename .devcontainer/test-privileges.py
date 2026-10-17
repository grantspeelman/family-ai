"""Harmless runtime assertions; invoked only inside the disposable CI container."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

assert os.getuid() != 0 and os.geteuid() == os.getuid()
assert os.getgroups() in ([], [os.getgid()]), "unexpected supplementary groups"
status = dict(line.split(":", 1) for line in Path("/proc/self/status").read_text().splitlines()
              if ":" in line)
assert status["NoNewPrivs"].strip() == "1"
assert status["Seccomp"].strip() == "2", "default seccomp filter absent"
for field in ("CapInh", "CapPrm", "CapEff", "CapBnd", "CapAmb"):
    assert int(status[field], 16) == 0, field
assert subprocess.run(["sudo", "-n", "true"], capture_output=True).returncode != 0
# The CI host copies this root-owned setuid probe into the disposable container.
# It prints its effective UID; no-new-privileges must prevent switching to root.
probe = Path("/tmp/runtime-setuid-probe")
assert probe.stat().st_uid == 0 and probe.stat().st_mode & 0o4000
assert int(subprocess.check_output([str(probe)], text=True)) == os.getuid()
for tool in ("node", "python3", "cargo", "gitleaks", "claude", "codex"):
    target = Path(shutil.which(tool)).resolve()
    assert target.stat().st_uid == 0, tool
    try:
        descriptor = os.open(target, os.O_WRONLY)  # no truncation or writes
    except PermissionError:
        pass
    else:
        os.close(descriptor)
        raise AssertionError(f"system tool writable: {tool}")
for directory in (os.environ["CLAUDE_CONFIG_DIR"], os.environ["CODEX_HOME"], "/workspace"):
    with tempfile.NamedTemporaryFile(dir=directory) as output:
        assert os.stat(output.name).st_uid == os.getuid()
print("non-root identity, capabilities, seccomp, no-new-privileges, tools and fresh volumes verified")
