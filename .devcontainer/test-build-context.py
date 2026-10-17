"""Docker-host fixture: prove dummy secrets never enter an image build context."""
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
POLICY = ROOT / 'Dockerfile.dockerignore'
assert POLICY.read_bytes() == (ROOT / 'egress/Dockerfile.dockerignore').read_bytes()
# The installed controls build context intentionally lacks a root .dockerignore;
# Dockerfile-specific policies must protect both that path and ordinary CI builds.
for installed in (False, True):
    with tempfile.TemporaryDirectory() as directory:
        context = Path(directory)
        shutil.copytree(ROOT, context / '.devcontainer', ignore=shutil.ignore_patterns('__pycache__'))
        if not installed:
            shutil.copyfile(ROOT.parent / '.dockerignore', context / '.dockerignore')
        for name in ('.env', '.env.local', '.git/config', 'secrets/key.pem',
                     '.devcontainer/.env', '.devcontainer/secret.key',
                     '.devcontainer/egress/.env', '.devcontainer/egress/secret.key',
                     '.devcontainer/egress/nested/private.key', 'unrelated.txt'):
            path = context / name
            path.parent.mkdir(exist_ok=True, parents=True)
            path.write_text('dummy-marker-never-copy')
        fixture = context / '.devcontainer/context-fixture.Dockerfile'
        fixture.write_text('FROM scratch\nCOPY . /context/\n')
        shutil.copyfile(POLICY, fixture.with_suffix('.Dockerfile.dockerignore'))
        # BuildKit's local exporter returns the actual COPY result without a daemon image.
        output = context / 'result'
        subprocess.run(['docker', 'build', '--file', str(fixture), '--output',
                        'type=local,dest=' + str(output), str(context)], check=True)
        copied = {str(path.relative_to(output / 'context')) for path in (output / 'context').rglob('*')
                  if path.is_file()}
        assert copied == {'.devcontainer/Dockerfile', '.devcontainer/entrypoint.sh',
                          '.devcontainer/runtime-setuid-probe.c',
                          '.devcontainer/ssh_known_hosts', '.devcontainer/egress/Dockerfile',
                          '.devcontainer/egress/proxy.py', '.devcontainer/egress/allowed-hosts.txt'}, copied
print('ordinary and installed build contexts include only required image inputs')
