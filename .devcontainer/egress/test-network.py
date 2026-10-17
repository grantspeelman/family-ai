"""Run only inside the application container; no real host/LAN probing."""
from pathlib import Path
import socket
import subprocess

# Docker's network_mode:none must expose only loopback, with no default routes.
assert {entry.name for entry in Path('/sys/class/net').iterdir()} == {'lo'}
assert len(Path('/proc/net/route').read_text().splitlines()) == 1
for line in Path('/proc/net/ipv6_route').read_text().splitlines():
    assert line.split()[-1] == 'lo'
for family, address in ((socket.AF_INET, ('192.0.2.1', 443)),
                        (socket.AF_INET6, ('2001:db8::1', 443))):
    with socket.socket(family) as connection:
        connection.settimeout(.5)
        try:
            connection.connect(address)
        except OSError:
            pass
        else:
            raise AssertionError('direct egress available')
# Check resolver failure with proxy settings irrelevant to system DNS. .invalid
# is reserved; this must never reach an external DNS server without an interface.
try:
    socket.getaddrinfo('egress-test.invalid', 443)
except socket.gaierror:
    pass
else:
    raise AssertionError('unexpected DNS resolution')
# A local server remains usable for tests and app development.
with socket.socket() as listener:
    listener.bind(('127.0.0.1', 0))
    listener.listen()
    with socket.create_connection(listener.getsockname(), timeout=1):
        incoming, _ = listener.accept()
        incoming.close()
assert not Path('/etc/egress/allowed-hosts.txt').exists()
assert subprocess.run(['touch', '/run/egress/should-not-exist'],
                      capture_output=True).returncode != 0
print('network isolation checked')
