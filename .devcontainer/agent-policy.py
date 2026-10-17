#!/usr/bin/env python3
"""Verify the outer OS boundary before executing any container entry command."""
import os
from pathlib import Path
import socket
import sys


def verify():
    if os.getuid() == 0:
        raise ValueError('non-root user required')
    status = dict(line.split(':', 1) for line in Path('/proc/self/status').read_text().splitlines()
                  if ':' in line)
    if status['NoNewPrivs'].strip() != '1' or any(
            int(status[key].strip(), 16) for key in ('CapInh', 'CapPrm', 'CapEff', 'CapBnd', 'CapAmb')):
        raise ValueError('no-new-privileges and dropped capabilities required')
    if status['Seccomp'].strip() != '2':
        raise ValueError('default seccomp filter required')
    for path in ('/', '/opt/family-ai-controls', '/run/egress'):
        if not os.statvfs(path).f_flag & os.ST_RDONLY:
            raise ValueError('read-only filesystem required: ' + path)
    if {name for _, name in socket.if_nameindex()} != {'lo'}:
        raise ValueError('network namespace must contain only loopback')
    for path in ('/workspace', '/home/vscode', '/tmp'):
        if not os.access(path, os.W_OK):
            raise ValueError('development directory must be writable: ' + path)
    for path in ('/etc/claude-code/managed-settings.json', '/etc/codex/config.toml',
                 '/etc/codex/requirements.toml'):
        if not Path(path).is_file() or os.access(path, os.W_OK):
            raise ValueError('immutable agent policy required: ' + path)


def main():
    try:
        verify()
        if sys.argv[1:2] == ['--'] and len(sys.argv) > 2:
            os.execvp(sys.argv[2], sys.argv[2:])
        elif sys.argv[1:]:
            raise ValueError('expected -- command [args...]')
    except (OSError, ValueError, KeyError) as error:
        sys.exit(f'Agent boundary refused: {error}')


if __name__ == '__main__':
    main()
