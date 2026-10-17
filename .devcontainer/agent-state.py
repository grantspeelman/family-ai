#!/usr/bin/env python3
"""Host-only selected provider state. Run from installed controls with python3 -I."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('host_session', HERE / 'host-session.py')
session = importlib.util.module_from_spec(spec)
spec.loader.exec_module(session)
AUTH = {'claude': '.credentials.json', 'codex': 'auth.json'}


def stop_users(work):
    """Includes terminal and editor projects; never delete any state volume."""
    query = ['docker', 'ps', '-aq', '--filter', 'label=org.family-ai.session=' + str(work)]
    ids = subprocess.run(query, check=True, capture_output=True, text=True).stdout.split()
    if ids:
        subprocess.run(['docker', 'rm', '--force', *ids], check=True, stdout=subprocess.DEVNULL)
    if subprocess.run(query, check=True, capture_output=True, text=True).stdout.strip():
        raise ValueError('Session containers remain; close the editor and stop the session first')


def state_dir(agent):
    root = HERE.parent.parent / 'state'
    root.mkdir(mode=0o700, exist_ok=True)
    fd = session.directory_fd(root)
    try:
        try:
            os.mkdir(agent, mode=0o700, dir_fd=fd)
        except FileExistsError:
            pass
        child = os.open(agent, os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
        os.close(child)
    finally:
        os.close(fd)
    return root / agent


def select(agent, work):
    target = HERE / 'agent-state.compose.json'
    if target.exists():
        previous = json.loads(target.read_text())['services']['app']['environment']['FAMILY_AI_AGENT']
        if previous != agent:
            stop_users(work)
    app = {'environment': {'FAMILY_AI_AGENT': agent}, 'volumes': []}
    if agent != 'none':
        app['volumes'] = [{'type': 'bind', 'source': str(state_dir(agent)),
                           'target': '/home/vscode/.' + agent,
                           'bind': {'create_host_path': False}}]
    temporary = target.with_suffix('.tmp')
    temporary.write_text(json.dumps({'services': {'app': app}}, indent=2) + '\n')
    temporary.replace(target)
    editor_path = HERE / 'devcontainer.json'
    editor = json.loads(editor_path.read_text())
    editor['customizations']['vscode']['extensions'] = {
        'none': [], 'claude': ['anthropic.claude-code'], 'codex': ['openai.chatgpt'],
    }[agent]
    editor_path.write_text(json.dumps(editor, indent=2) + '\n')


def reset(agent, including_auth, work):
    stop_users(work)
    path = state_dir(agent)
    # No-follow open of the one retained login file. Every other item is removed;
    # shutil.rmtree uses fd-based traversal and does not follow directory symlinks.
    fd = session.directory_fd(path)
    try:
        auth = None
        if not including_auth:
            try:
                auth_fd = os.open(AUTH[agent], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
            except FileNotFoundError:
                pass
            else:
                with os.fdopen(auth_fd, 'rb') as stream:
                    info = os.fstat(stream.fileno())
                    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1 or info.st_size > 1024 * 1024:
                        raise ValueError('Login file must be a single-link regular file of at most 1 MiB')
                    auth = stream.read(1024 * 1024 + 1)
        for name in os.listdir(fd):
            info = os.stat(name, dir_fd=fd, follow_symlinks=False)
            if stat.S_ISDIR(info.st_mode):
                shutil.rmtree(name, dir_fd=fd)
            else:
                os.unlink(name, dir_fd=fd)
        if auth is not None:
            auth_fd = os.open(AUTH[agent], os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                              0o600, dir_fd=fd)
            with os.fdopen(auth_fd, 'wb') as stream:
                stream.write(auth)
    finally:
        os.close(fd)
    print('Reset selected state; ' + ('login removed locally.' if including_auth else 'only login file retained if present.'))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    choose = commands.add_parser('select')
    choose.add_argument('agent', choices=['none', *AUTH])
    clear = commands.add_parser('reset')
    clear.add_argument('agent', choices=AUTH)
    clear.add_argument('--including-auth', action='store_true')
    args = parser.parse_args()
    work = session.workspace()
    if args.command == 'select':
        select(args.agent, work)
    else:
        reset(args.agent, args.including_auth, work)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        sys.exit(f'State operation refused: {error}')
