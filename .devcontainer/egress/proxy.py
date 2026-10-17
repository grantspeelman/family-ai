"""Small CONNECT-only egress gateway and in-container Unix-to-TCP relay.

The gateway is a separate container. Client-side proxy settings are convenience,
not enforcement: the application container has no external network interface.
No request headers, authorities, bodies, or authentication data are logged.
"""
import argparse
import ipaddress
import os
from pathlib import Path
import re
import select
import socket
import socketserver
import time

SOCKET = '/run/egress/proxy.sock'
LIMIT = 16384
TIMEOUT = 300
HEADER_TIMEOUT = 10
CONNECT_TIMEOUT = 10


def destination(authority, allowed):
    if not re.fullmatch(r'[a-z0-9]+(?:[.-][a-z0-9]+)*:443', authority):
        raise ValueError('invalid authority')
    host = authority[:-4]
    if host not in allowed:
        raise ValueError('destination denied')
    try:
        ipaddress.ip_address(host)
    except ValueError:
        return host
    raise ValueError('IP literals denied')


def public_addresses(host):
    records = socket.getaddrinfo(host, 443, type=socket.SOCK_STREAM)
    if not records:
        raise ValueError('no addresses')
    for family, _, _, _, address in records:
        ip = ipaddress.ip_address(address[0])
        # IPv6 transition mechanisms can encode private IPv4 destinations. Restrict
        # IPv6 to native global unicast; reject all mixed public/private answers.
        if family not in (socket.AF_INET, socket.AF_INET6) or not ip.is_global \
                or ip.is_multicast or ip.is_reserved or ip.is_unspecified \
                or ip.is_link_local or ip.is_loopback:
            raise ValueError('non-public address')
        if ip.version == 6 and (ip not in ipaddress.ip_network('2000::/3')
                                or ip.sixtofour or ip.teredo or ip.ipv4_mapped):
            raise ValueError('non-native IPv6 address')
    return records


def connect_public(host):
    for family, kind, protocol, _, address in public_addresses(host):
        connection = socket.socket(family, kind, protocol)
        try:
            connection.settimeout(CONNECT_TIMEOUT)
            # Use the already validated numeric sockaddr, never resolve twice.
            connection.connect(address)
            return connection
        except OSError:
            connection.close()
    raise OSError('no reachable public address')


def tunnel(left, right):
    # Finite idle timeout also bounds abandoned tunnels. No buffering of request
    # bodies; each bounded read is forwarded before reading more.
    for connection in (left, right):
        connection.settimeout(TIMEOUT)
    while True:
        readable, _, _ = select.select([left, right], [], [], TIMEOUT)
        if not readable:
            return
        for source in readable:
            data = source.recv(65536)
            if not data:
                return
            (right if source is left else left).sendall(data)


class Handler(socketserver.BaseRequestHandler):
    def handle(self):
        client = self.request
        client.settimeout(TIMEOUT)
        established = False
        try:
            header = bytearray()
            deadline = time.monotonic() + HEADER_TIMEOUT
            # Do not consume tunnel bytes beyond the HTTP header boundary.
            while not header.endswith(b'\r\n\r\n'):
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise ValueError('header timeout')
                client.settimeout(remaining)
                byte = client.recv(1)
                if not byte or len(header) >= LIMIT:
                    raise ValueError('invalid header')
                header.extend(byte)
            lines = header.decode('ascii').split('\r\n')
            method, authority, version = lines[0].split(' ')
            if method != 'CONNECT' or version != 'HTTP/1.1':
                raise ValueError('CONNECT required')
            host = destination(authority, self.server.allowed)
            for line in lines[1:-2]:
                key, value = line.split(':', 1)
                if not re.fullmatch(r'[A-Za-z0-9-]+', key):
                    raise ValueError('invalid header')
                if key.lower() in ('content-length', 'transfer-encoding'):
                    raise ValueError('request body denied')
                if key.lower() == 'host' and value.strip() != authority:
                    raise ValueError('authority mismatch')
            with connect_public(host) as upstream:
                client.sendall(b'HTTP/1.1 200 Connection Established\r\n\r\n')
                established = True
                tunnel(client, upstream)
        except (OSError, ValueError, UnicodeError):
            if not established:
                try:
                    client.sendall(b'HTTP/1.1 403 Forbidden\r\nConnection: close\r\n\r\n')
                except OSError:
                    pass


class BoundedThreads(socketserver.ThreadingMixIn):
    daemon_threads = True
    # Bound concurrency rather than allowing unlimited threads from a client.
    def __init__(self, *args, **kwargs):
        import threading
        self.slots = threading.BoundedSemaphore(64)
        super().__init__(*args, **kwargs)

    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False):
            self.close_request(request)
            return
        try:
            super().process_request(request, client_address)
        except Exception:
            self.slots.release()
            raise

    def process_request_thread(self, request, client_address):
        try:
            super().process_request_thread(request, client_address)
        finally:
            self.slots.release()

    def handle_error(self, request, client_address):
        pass  # Do not log request-derived data on failure.


class Server(BoundedThreads, socketserver.UnixStreamServer):
    def __init__(self, path, allowed):
        self.allowed = allowed
        super().__init__(path, Handler)


def check_socket(path):
    with socket.socket(socket.AF_UNIX) as connection:
        connection.settimeout(2)
        connection.connect(path)


class RelayHandler(socketserver.BaseRequestHandler):
    def handle(self):
        try:
            with socket.socket(socket.AF_UNIX) as connection:
                connection.settimeout(2)
                connection.connect(self.server.path)
                tunnel(self.request, connection)
        except OSError:
            pass  # No direct-network fallback.


class Relay(BoundedThreads, socketserver.TCPServer):
    daemon_threads = True
    allow_reuse_address = True

    def handle_error(self, request, client_address):
        pass


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['gateway', 'relay', 'check'])
    parser.add_argument('--socket', default=SOCKET)
    args = parser.parse_args()
    if args.mode == 'check':
        check_socket(args.socket)
    elif args.mode == 'relay':
        check_socket(args.socket)
        with Relay(('127.0.0.1', 3128), RelayHandler) as server:
            server.path = args.socket
            server.serve_forever()
    else:
        allowed = set(Path('/etc/egress/allowed-hosts.txt').read_text().splitlines())
        allowed = {host for host in allowed if host and not host.startswith('#')}
        os.umask(0o077)
        Path(args.socket).unlink(missing_ok=True)
        with Server(args.socket, allowed) as server:
            # The volume is read-only in the app, socket connections still work.
            os.chmod(args.socket, 0o666)
            server.serve_forever()


if __name__ == '__main__':
    main()
