"""Controlled socket fixtures: never contact external or host-network services."""
import importlib.util
from pathlib import Path
import socket
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True

SPEC = importlib.util.spec_from_file_location('egress', Path(__file__).with_name('proxy.py'))
proxy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(proxy)


class ProxyTests(unittest.TestCase):
    def test_authorities(self):
        for authority in ('other.test:443', 'api.test:80', '127.0.0.1:443',
                          '[::1]:443', 'api.test.:443', 'user@api.test:443',
                          'api.test:443/anything', 'api.test:0443'):
            with self.subTest(authority=authority), self.assertRaises(ValueError):
                proxy.destination(authority, {'api.test'})
        self.assertEqual(proxy.destination('api.test:443', {'api.test'}), 'api.test')

    def test_dns_public_only_and_pinned(self):
        for ip in ('127.0.0.1', '10.0.0.1', '169.254.169.254', '192.168.1.1',
                   '100.64.0.1', '224.0.0.1', '255.255.255.255', '0.0.0.0', '::1', 'fe80::1', 'fc00::1', '::ffff:8.8.8.8',
                   '2002:0808:0808::1', '64:ff9b::808:808'):
            with self.subTest(ip=ip), patch.object(socket, 'getaddrinfo', return_value=[
                (socket.AF_INET6 if ':' in ip else socket.AF_INET, socket.SOCK_STREAM, 6, '', (ip, 443))
            ]), self.assertRaises(ValueError):
                proxy.public_addresses('api.test')
        records = [(socket.AF_INET, socket.SOCK_STREAM, 6, '', ('8.8.8.8', 443))]
        with patch.object(socket, 'getaddrinfo', return_value=records) as resolve:
            self.assertEqual(proxy.public_addresses('api.test'), records)
            resolve.assert_called_once_with('api.test', 443, type=socket.SOCK_STREAM)
        with patch.object(socket, 'getaddrinfo', return_value=records + [
            (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('127.0.0.1', 443))
        ]), self.assertRaises(ValueError):
            proxy.public_addresses('api.test')

    def test_connect_and_denials_over_unix_socket(self):
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / 'proxy.sock')
            server = proxy.Server(path, {'api.test'})
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                for request in (b'CONNECT other.test:443 HTTP/1.1\r\n\r\n',
                                b'GET https://api.test/ HTTP/1.1\r\n\r\n',
                                b'CONNECT api.test:443 HTTP/1.1\r\nContent-Length: 1\r\n\r\nx'):
                    with socket.socket(socket.AF_UNIX) as client:
                        client.connect(path)
                        client.sendall(request)
                        self.assertIn(b'403', client.recv(4096))
                client_side, upstream = socket.socketpair()
                def echo():
                    with upstream:
                        upstream.sendall(upstream.recv(4096))
                echo_thread = threading.Thread(target=echo, daemon=True)
                echo_thread.start()
                with patch.object(proxy, 'connect_public', return_value=client_side):
                    with socket.socket(socket.AF_UNIX) as client:
                        client.settimeout(2)
                        client.connect(path)
                        client.sendall(b'CONNECT api.test:443 HTTP/1.1\r\nHost: api.test:443\r\n\r\n')
                        self.assertIn(b'200', client.recv(4096))
                        client.sendall(b'opaque TLS bytes')
                        self.assertEqual(client.recv(4096), b'opaque TLS bytes')
            finally:
                server.shutdown()
                server.server_close()
                thread.join()

    def test_connection_uses_validated_sockaddr_without_second_resolution(self):
        record = (socket.AF_INET, socket.SOCK_STREAM, 6, '', ('8.8.8.8', 443))
        with patch.object(proxy, 'public_addresses', return_value=[record]), \
                patch.object(socket, 'socket') as factory:
            proxy.connect_public('api.test')
            factory.return_value.connect.assert_called_once_with(record[4])

    def test_relay_forwards_bytes_and_closes_when_gateway_disappears(self):
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / 'proxy.sock')
            listener = socket.socket(socket.AF_UNIX)
            listener.bind(path)
            listener.listen()
            relay = proxy.Relay(('127.0.0.1', 0), proxy.RelayHandler)
            relay.path = path
            thread = threading.Thread(target=relay.serve_forever, daemon=True)
            thread.start()
            try:
                with socket.create_connection(relay.server_address, timeout=2) as client:
                    client.sendall(b'CONNECT api.test:443 HTTP/1.1\r\n\r\n')
                    upstream, _ = listener.accept()
                    with upstream:
                        self.assertIn(b'CONNECT api.test:443', upstream.recv(4096))
                        upstream.sendall(b'fixture-response')
                        self.assertEqual(client.recv(4096), b'fixture-response')
                listener.close()
                Path(path).unlink()
                with socket.create_connection(relay.server_address, timeout=2) as client:
                    # A missing Unix gateway closes the client; no alternate route.
                    self.assertEqual(client.recv(4096), b'')
            finally:
                listener.close()
                relay.shutdown()
                relay.server_close()
                thread.join()

    def test_parser_header_size_and_header_deadline(self):
        with tempfile.TemporaryDirectory() as directory:
            path = str(Path(directory) / 'proxy.sock')
            gateway = proxy.Server(path, {'api.test'})
            thread = threading.Thread(target=gateway.serve_forever, daemon=True)
            thread.start()
            try:
                with patch.object(proxy, 'LIMIT', 64):
                    with socket.socket(socket.AF_UNIX) as client:
                        client.settimeout(2)
                        client.connect(path)
                        client.sendall(b'CONNECT api.test:443 HTTP/1.1\r\nX: ' + b'a' * 65)
                        self.assertIn(b'403', client.recv(4096))
                with patch.object(proxy, 'HEADER_TIMEOUT', .05):
                    with socket.socket(socket.AF_UNIX) as client:
                        client.settimeout(2)
                        client.connect(path)
                        client.sendall(b'CONNECT ')
                        self.assertIn(b'403', client.recv(4096))
            finally:
                gateway.shutdown()
                gateway.server_close()
                thread.join()

    def test_missing_socket_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as directory, self.assertRaises(OSError):
            proxy.check_socket(str(Path(directory) / 'missing'))


if __name__ == '__main__':
    unittest.main()
