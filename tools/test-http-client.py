#!/usr/bin/env python3
"""Verify native libcurl requests, framing, deadlines, bounds and TLS trust."""
import argparse
from collections import Counter
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import platform
import ssl
import socket
from socketserver import StreamRequestHandler, ThreadingTCPServer
import subprocess
import tempfile
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'

    def handle(self):
        try:
            super().handle()
        except (BrokenPipeError, ConnectionResetError):
            # Deliberately aborting oversized/expired responses closes a
            # keep-alive connection while the fixture may still be writing.
            pass

    def log_message(self, *args):
        pass

    def response(self, status, body=b'', headers=()):
        self.send_response(status)
        self.send_header('Content-Length', str(len(body)))
        for name, value in headers:
            self.send_header(name, value)
        self.end_headers()
        if self.command != 'HEAD':
            try:
                self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError):
                pass

    def do_GET(self):
        self.server.observed[(self.command, self.path)] += 1
        if self.path == '/ok':
            self.response(200, b'hello HTTP')
        elif self.path == '/status':
            self.response(418, bytes((42, 0, 128, 255)))
        elif self.path == '/redirect':
            assert self.headers.get('X-Probe') == 'dunder'
            self.response(302, b'redirect', [('Location', '/must-not-follow')])
        elif self.path == '/large':
            self.response(200, b'x' * 8192)
        elif self.path == '/chunked-large':
            self.send_response(200)
            self.send_header('Transfer-Encoding', 'chunked')
            self.end_headers()
            self.wfile.write(b'2000\r\n' + b'x' * 8192 + b'\r\n0\r\n\r\n')
        elif self.path == '/empty':
            self.response(204)
        elif self.path == '/slow':
            time.sleep(0.25)
            self.response(200, b'slow')
        elif self.path == '/stall':
            time.sleep(3)
            self.response(200, b'stalled')
        elif self.path == '/renew':
            self.response(200, b'renewed')
        else:
            self.response(500, b'unexpected request')

    def do_HEAD(self):
        self.server.observed[(self.command, self.path)] += 1
        self.response(200, b'content that a HEAD request must never read')

    def do_POST(self):
        self.server.observed[(self.command, self.path)] += 1
        assert self.path == '/echo'
        assert self.headers.get('X-Probe') == 'dunder'
        assert self.headers.get('Content-Type') == 'application/octet-stream'
        length = int(self.headers['Content-Length'])
        payload = self.rfile.read(length)
        assert payload == bytes((0, 1, 2, 127, 128, 255, 0)), payload
        self.response(201, payload)

    do_PUT = do_PATCH = do_DELETE = do_POST


class StalledTLS(StreamRequestHandler):
    def handle(self):
        time.sleep(3)


def host_target():
    host = platform.system(), platform.machine()
    if host == ('Darwin', 'arm64'):
        return TARGETS[0]
    if host[0] == 'Linux' and host[1] in ('arm64', 'aarch64'):
        return TARGETS[1]
    if host == ('Linux', 'x86_64'):
        return TARGETS[2]
    raise RuntimeError(f'unsupported native host {host}')


def run(command, *, timeout=90, local_network=False):
    environment = os.environ.copy()
    if local_network:
        environment['NO_PROXY'] = environment['no_proxy'] = 'localhost,127.0.0.1'
    result = subprocess.run(list(map(str, command)), cwd=ROOT, capture_output=True,
                            text=True, timeout=timeout, env=environment)
    assert result.returncode == 0, (command, result.returncode, result.stdout, result.stderr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path, nargs='?')
    parser.add_argument('--binary', type=Path,
                        help='run an already cross-built fixture on its native host')
    parser.add_argument('--receipt', type=Path)
    args = parser.parse_args()
    if (args.compiler is None) == (args.binary is None):
        parser.error('provide either a compiler or --binary')
    compiler = args.compiler.resolve(strict=True) if args.compiler else None
    host = host_target()
    images = executions = 0
    started = time.monotonic()
    receipts = []
    with tempfile.TemporaryDirectory(prefix='dunder-http-client-') as temporary:
        work = Path(temporary)
        key, cert = work / 'server.key', work / 'ca.pem'
        run(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
             '-subj', '/CN=localhost', '-addext', 'subjectAltName=DNS:localhost',
             '-keyout', key, '-out', cert])
        servers = [ThreadingHTTPServer(('127.0.0.1', 0), Handler) for _ in range(2)]
        servers.append(ThreadingTCPServer(('127.0.0.1', 0), StalledTLS))
        trust = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        trust.load_cert_chain(cert, key)
        servers[1].socket = trust.wrap_socket(servers[1].socket, server_side=True)
        for server in servers:
            server.observed = Counter()
            server.daemon_threads = True
            threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            origin = f'http://127.0.0.1:{servers[0].server_port}'
            tls_origin = f'https://localhost:{servers[1].server_port}'
            stalled_tls = f'https://127.0.0.1:{servers[2].server_address[1]}/'
            with socket.socket() as unused_port:
                unused_port.bind(('127.0.0.1', 0))
                refused = f'http://127.0.0.1:{unused_port.getsockname()[1]}/'
            runnable = []
            if args.binary:
                runnable.append(args.binary.resolve(strict=True))
            else:
                for target in TARGETS:
                    for mode, flags in enumerate(((), ('--scalar-opt=off', '--ownership-opt=off', '--mir=off'))):
                        image = work / f'client-{target}-{mode}'
                        run([compiler, 'build', ROOT / 'tests/http_client.ddr', '--target', target,
                             '--jobs', '1', *flags, '-o', image])
                        images += 1
                        if target == host:
                            runnable.append(image)
            for image in runnable:
                start = time.monotonic()
                observed = run([image, origin, tls_origin, cert, stalled_tls, refused], timeout=30, local_network=True)
                assert observed.stdout == 'http client ok\n', observed.stdout
                assert time.monotonic() - start < 20
                receipts.append({'sha256': hashlib.sha256(image.read_bytes()).hexdigest(),
                                 'seconds': round(time.monotonic() - start, 3)})
                executions += 1
            assert servers[0].observed == Counter({
                ('GET', '/ok'): 12 * executions,
                **{(method, '/echo'): executions for method in ('POST', 'PUT', 'PATCH', 'DELETE')},
                **{('GET', path): executions for path in ('/status', '/redirect', '/large', '/chunked-large', '/empty', '/slow')},
                ('GET', '/stall'): executions,
                ('GET', '/renew'): 2 * executions,
                ('HEAD', '/head'): executions,
            }), servers[0].observed
            assert servers[1].observed == Counter({('GET', '/ok'): executions}), servers[1].observed
        finally:
            for server in servers:
                server.shutdown()
                server.server_close()
    if args.receipt:
        args.receipt.parent.mkdir(parents=True, exist_ok=True)
        args.receipt.write_text(json.dumps({
            'schema': 'dunder-http-client-verification/v1', 'host_target': host,
            'builds': images, 'executions': executions, 'binaries': receipts,
            'http_requests': sum(servers[0].observed.values()),
            'https_requests': sum(servers[1].observed.values()),
            'seconds': round(time.monotonic() - started, 3), 'passed': True,
        }, indent=2) + '\n')
    print(f'HTTP client: {images} native builds, {executions} live HTTP/TLS executions passed')


if __name__ == '__main__':
    main()
