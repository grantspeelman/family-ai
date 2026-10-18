"""Container-only harmless OS probes: same boundary as native files and local tools."""
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile

subprocess.run(['python3', '-I', '/usr/local/lib/agent-policy.py'], check=True)
for destination in ('/etc/family-ai-denied', '/usr/local/bin/family-ai-denied',
                    '/opt/family-ai-controls/family-ai-denied'):
    try:
        Path(destination).write_text('dummy-marker')
    except (PermissionError, OSError):
        pass
    else:
        raise AssertionError('protected write succeeded: ' + destination)
try:
    Path('/root/family-ai-denied').read_text()
except PermissionError:
    pass
else:
    raise AssertionError('root directory was readable')
for directory in ('/workspace', '/tmp', '/home/vscode'):
    with tempfile.TemporaryDirectory(dir=directory) as temp:
        marker = Path(temp) / 'routine-edit'
        marker.write_text('dummy-edit')
        assert marker.read_text() == 'dummy-edit'
        # Temporary builds and the remote editor install executable binaries in
        # these writable directories; opening a script through bash would hide
        # a noexec mount, so execute a copied harmless ELF binary directly.
        executable = Path(temp) / 'routine-executable'
        shutil.copyfile('/bin/true', executable)
        executable.chmod(0o700)
        subprocess.run([str(executable)], check=True)
# No user login, hooks, provider environment or model request is used.
with tempfile.TemporaryDirectory() as temp:
    env = {'PATH': os.environ['PATH'], 'HOME': temp, 'CODEX_HOME': temp,
           'CLAUDE_CONFIG_DIR': temp, 'DISABLE_AUTOUPDATER': '1'}
    result = subprocess.run(['codex', 'doctor', '--json'], env=env,
                            capture_output=True, text=True, timeout=30)
    report = json.loads(result.stdout)
    config = report['checks']['config.load']
    assert config['status'] == 'ok', config
    flags = config['details']['enabled feature flags'].split(', ')
    assert not {'apps', 'plugins', 'remote_plugin'} & set(flags), flags
    assert config['details']['mcp servers'] == '0'
    # User-writable config must not enable an external server against policy.
    Path(temp, 'config.toml').write_text(
        '[mcp_servers.dummy]\ncommand="/bin/true"\n')
    listed = subprocess.run(['codex', 'mcp', 'list', '--json'], env=env,
                            capture_output=True, text=True, check=True, timeout=30)
    servers = json.loads(listed.stdout)
    assert all(not server['enabled'] for server in servers), servers
    result = subprocess.run(['claude', 'doctor'], env=env,
                            capture_output=True, text=True, check=True, timeout=30)
    assert 'Settings Error' not in result.stdout + result.stderr
subprocess.run(['python3', '-I', '/workspace/.devcontainer/egress/test-network.py'], check=True)
print('agent boundary probes passed; authenticated native tool sessions remain manual')
