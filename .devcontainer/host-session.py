#!/usr/bin/env python3
"""Trusted-host session installation and data-only transfer. Run with python3 -I."""
import argparse
import base64
import difflib
import io
import json
import os
from pathlib import Path, PurePosixPath
import secrets
import stat
import subprocess
import sys
import tarfile
import unicodedata


HERE = Path(__file__).resolve().parent
MAX_FILE_BYTES = 8 * 1024 * 1024
MAX_TOTAL_BYTES = 32 * 1024 * 1024
MAX_BUNDLE_BYTES = 48 * 1024 * 1024
MAX_FILES = 128


def checked_path(value):
    if not isinstance(value, str) or len(value) > 4096:
        raise ValueError('Invalid transfer path')
    path = PurePosixPath(value)
    if (not value or path.is_absolute() or any(part.casefold() in ('', '.', '..', '.git') for part in value.split('/'))
            or '\\' in value or any(unicodedata.category(char).startswith('C') for char in value)):
        raise ValueError('Use explicit relative file paths outside .git')
    return path.parts


def directory_fd(root, parts=()):
    """Walk every component with no-follow descriptors, including the root."""
    absolute = Path(root).absolute()
    fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        for part in (*absolute.parts[1:], *parts):
            nxt = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd)
            fd = nxt
        return fd
    except BaseException:
        os.close(fd)
        raise


def read_file(root, name):
    parts = checked_path(name)
    fd = directory_fd(root)
    try:
        for part in parts[:-1]:
            try:
                nxt = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            except FileNotFoundError:
                return None
            os.close(fd)
            fd = nxt
        try:
            file_fd = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
        except FileNotFoundError:
            return None
        with os.fdopen(file_fd, 'rb') as stream:
            if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
                raise ValueError('Transfer accepts regular files only')
            if os.fstat(stream.fileno()).st_size > MAX_FILE_BYTES:
                raise ValueError('Transfer file exceeds 8 MiB')
            content = stream.read(MAX_FILE_BYTES + 1)
            if len(content) > MAX_FILE_BYTES:
                raise ValueError('Transfer file exceeds 8 MiB')
            return content
    finally:
        os.close(fd)


def write_file(root, name, content):
    parts = checked_path(name)
    fd = directory_fd(root)
    try:
        for part in parts[:-1]:
            try:
                os.mkdir(part, dir_fd=fd)
            except FileExistsError:
                pass
            nxt = os.open(part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd)
            fd = nxt
        if content is None:
            os.unlink(parts[-1], dir_fd=fd)
        else:
            try:
                mode = os.stat(parts[-1], dir_fd=fd, follow_symlinks=False).st_mode
                if not stat.S_ISREG(mode):
                    raise ValueError('Target must be a regular file')
                permissions = mode & 0o777
            except FileNotFoundError:
                permissions = 0o644
            temp = '.transfer-' + secrets.token_hex(16)
            file_fd = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                              0o644, dir_fd=fd)
            try:
                os.fchmod(file_fd, permissions)
                with os.fdopen(file_fd, 'wb') as stream:
                    stream.write(content)
                os.replace(temp, parts[-1], src_dir_fd=fd, dst_dir_fd=fd)
            finally:
                try:
                    os.unlink(temp, dir_fd=fd)
                except FileNotFoundError:
                    pass
    finally:
        os.close(fd)


def trusted_git(source, *args):
    env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
    env.update(GIT_CONFIG_GLOBAL='/dev/null', GIT_CONFIG_NOSYSTEM='1')
    return subprocess.run(['git', '-c', 'core.hooksPath=/dev/null', '-C', str(source), *args],
                          env=env, check=True, capture_output=True).stdout


def create(source, session):
    source = source.resolve(strict=True)
    session = session.absolute()
    # Also reject symlink parents and an installation inside its own source tree.
    parent_fd = directory_fd(session.parent)
    os.close(parent_fd)
    if session == source or source in session.parents:
        raise ValueError('Session must be outside the trusted checkout')
    revision = trusted_git(source, 'rev-parse', '--verify', 'HEAD').decode().strip()
    archive = trusted_git(source, 'archive', revision, '.devcontainer')
    session.mkdir(mode=0o700)  # Never reuse, reset or delete an existing session.
    controls = session / 'controls'
    controls.mkdir()
    with tarfile.open(fileobj=io.BytesIO(archive)) as files:
        for member in files:
            checked_path(member.name.rstrip('/'))
            target = controls / member.name
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            elif member.isfile():
                target.parent.mkdir(parents=True, exist_ok=True)
                with files.extractfile(member) as stream:
                    target.write_bytes(stream.read())
                target.chmod(member.mode & 0o777)
            else:
                raise ValueError('Reviewed launch controls must contain only files/directories')
    # A transport clone copies object data; no shared objects, alternates or Git dir.
    trusted_git(source, 'clone', '--no-local', '--no-hardlinks', '--template=', '--',
                str(source), str(session / 'work'))
    trusted_git(session / 'work', 'remote', 'remove', 'origin')
    trusted_git(session / 'work', 'checkout', '--detach', revision)
    trusted_git(session / 'work', 'switch', '-c', 'agent-work')
    (controls / 'session.json').write_text(json.dumps({
        'version': 1, 'source': str(session / 'work'), 'revision': revision,
    }, indent=2) + '\n')
    print(f'Installed reviewed controls: {controls}\nDisposable source: {session / "work"}')


def workspace():
    manifest = json.loads((HERE.parent / 'session.json').read_text())
    work = HERE.parent.parent / 'work'
    if manifest.get('version') != 1 or manifest.get('source') != str(work):
        raise ValueError('Invalid installed session manifest')
    fd = directory_fd(work / '.git')
    os.close(fd)
    if (work / '.git/objects/info/alternates').exists() or (work / '.git/commondir').exists():
        raise ValueError('Session Git metadata must be independent')
    return work


def load_bundle(path):
    with path.open('rb') as stream:
        if os.fstat(stream.fileno()).st_size > MAX_BUNDLE_BYTES:
            raise ValueError('Bundle exceeds 48 MiB')
        raw = stream.read(MAX_BUNDLE_BYTES + 1)
    if len(raw) > MAX_BUNDLE_BYTES:
        raise ValueError('Bundle exceeds 48 MiB')
    data = json.loads(raw)
    if (not isinstance(data, dict) or data.get('version') != 1
            or not isinstance(data.get('files'), dict) or len(data['files']) > MAX_FILES):
        raise ValueError('Invalid source bundle or more than 128 files')
    result = {}
    total = 0
    for name, encoded in data['files'].items():
        checked_path(name)
        if encoded is not None and (not isinstance(encoded, str)
                                     or len(encoded) > 4 * ((MAX_FILE_BYTES + 2) // 3)):
            raise ValueError('Invalid encoded file or more than 8 MiB')
        content = None if encoded is None else base64.b64decode(encoded, validate=True)
        total += len(content or b'')
        if len(content or b'') > MAX_FILE_BYTES or total > MAX_TOTAL_BYTES:
            raise ValueError('Transfer exceeds file or 32 MiB total limit')
        result[name] = content
    return result


def review_lines(content):
    text = content.decode('utf-8', errors='surrogateescape')
    # Escape literal backslashes first so literal text, invalid UTF-8 bytes and
    # Unicode controls each have distinct representations in the displayed diff.
    rendered = []
    for char in text:
        codepoint = ord(char)
        if char == '\\':
            rendered.append('\\\\')
        elif 0xdc80 <= codepoint <= 0xdcff:
            rendered.append(f'\\x{codepoint - 0xdc00:02x}')
        elif char not in '\n\t' and unicodedata.category(char).startswith('C'):
            rendered.append(f'\\u{codepoint:04x}')
        else:
            rendered.append(char)
    return ''.join(rendered).splitlines(keepends=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    install = commands.add_parser('create')
    install.add_argument('--source', type=Path, required=True)
    install.add_argument('--session', type=Path, required=True)
    commands.add_parser('check')
    commands.add_parser('path')
    export = commands.add_parser('export')
    export.add_argument('--path', action='append', required=True)
    export.add_argument('--output', type=Path, required=True)
    for command in ('review', 'apply'):
        action = commands.add_parser(command)
        action.add_argument('--bundle', type=Path, required=True)
        action.add_argument('--target', type=Path, required=True)
        if command == 'apply':
            action.add_argument('--reviewed', action='store_true', required=True)
    args = parser.parse_args()
    if args.command == 'create':
        create(args.source, args.session)
    elif args.command in ('check', 'path'):
        print(workspace())
    elif args.command == 'export':
        work = workspace()
        destination = args.output.resolve()
        if destination == work or work in destination.parents:
            raise ValueError('Export bundles must be stored outside disposable source')
        if len(args.path) > MAX_FILES:
            raise ValueError('Transfer supports at most 128 selected files')
        files = {}
        total = 0
        for name in args.path:
            content = read_file(work, name)
            total += len(content or b'')
            if total > MAX_TOTAL_BYTES:
                raise ValueError('Transfer exceeds 32 MiB total')
            files[name] = content
        with args.output.open('x') as stream:
            json.dump({'version': 1, 'files': {
                name: None if content is None else base64.b64encode(content).decode()
                for name, content in files.items()}}, stream, indent=2)
            stream.write('\n')
    else:
        changes = load_bundle(args.bundle)
        # Validate all target paths before mutating anything. No Git commands run.
        previous = {}
        total = 0
        for name in changes:
            content = read_file(args.target, name)
            total += len(content or b'')
            if total > MAX_TOTAL_BYTES:
                raise ValueError('Trusted input exceeds 32 MiB total limit')
            previous[name] = content
        if args.command == 'review':
            for name, content in changes.items():
                if previous[name] == content:
                    action = 'unchanged'
                elif previous[name] is None:
                    action = 'create'
                elif content is None:
                    action = 'delete'
                else:
                    action = 'modify'
                print(f'{action} {json.dumps(name, ensure_ascii=True)}')
                before = previous[name] or b''
                after = content or b''
                print(''.join(difflib.unified_diff(
                    review_lines(before), review_lines(after),
                    fromfile='trusted/' + name, tofile='proposed/' + name)), end='')
        else:
            for name, content in changes.items():
                if content != previous[name]:
                    write_file(args.target, name, content)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RecursionError, subprocess.CalledProcessError) as error:
        sys.exit(f'Session operation refused: {error}')
