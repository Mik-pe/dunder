#!/usr/bin/env python3
"""Exercise the native HTTP boundary with real fragmented/malformed sockets."""
import argparse
import errno
import pathlib
import select
import socket
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]


def run_case(binary, wire, status, *, limit=16, timeout=2000, fragments=None,
             expected=None, shutdown=True, interim=False, head_only=False,
             head_wait=False, split=False, streaming=False):
    args = [str(binary), str(limit), str(timeout)]
    if head_only:
        args.append("head")
    elif split:
        args.append("split")
    elif streaming:
        args.append("stream")
    process = subprocess.Popen(args,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    connection = None
    try:
        ready, _, _ = select.select([process.stdout], [], [], 5)
        assert ready, "fixture did not announce a port"
        port = int(process.stdout.readline())
        connection = socket.create_connection(("127.0.0.1", port), timeout=5)
        connection.settimeout(5)
        if head_wait:
            head, body = wire.split(b"\r\n\r\n", 1)
            connection.sendall(head + b"\r\n\r\n")
            ready, _, _ = select.select([connection], [], [], 0.1)
            assert not ready, "read_head unexpectedly sent a 100 Continue response"
            connection.sendall(body)
        elif interim:
            head, body = wire.split(b"\r\n\r\n", 1)
            connection.sendall(head + b"\r\n\r\n")
            response = b""
            while b"\r\n\r\n" not in response:
                response += connection.recv(4096)
            assert response == b"HTTP/1.1 100 Continue\r\n\r\n", response
            connection.sendall(body)
        elif fragments:
            position = 0
            for amount in fragments:
                connection.sendall(wire[position:position + amount])
                position += amount
                time.sleep(0.005)
            connection.sendall(wire[position:])
        elif wire:
            connection.sendall(wire)
        if shutdown:
            try:
                connection.shutdown(socket.SHUT_WR)
            except OSError as error:
                # An early framing rejection may already have closed the peer.
                # Still require its complete response and expected status below.
                if error.errno != errno.ENOTCONN:
                    raise
        response = b""
        while True:
            try:
                chunk = connection.recv(65536)
            except ConnectionResetError:
                # Rejecting an oversized request can close with unread bytes.
                # Its complete response still has to have arrived first.
                break
            if not chunk:
                break
            response += chunk
        head, body = response.split(b"\r\n\r\n", 1)
        actual = int(head.split(b" ", 2)[1])
        assert actual == status, (status, actual, response)
        headers = dict(line.split(b": ", 1) for line in head.split(b"\r\n")[1:])
        assert headers[b"Connection"] == b"close", response
        assert headers[b"X-Test"] == b"dunder", response
        if actual < 200 or actual in (204, 304):
            assert b"Content-Length" not in headers and body == b"", response
        elif not wire.startswith(b"HEAD "):
            assert int(headers[b"Content-Length"]) == len(body), response
        else:
            assert body == b"", response
        if expected is not None:
            assert body == expected, (expected, body)
        out, error = process.communicate(timeout=5)
        assert process.returncode == 0, (process.returncode, out, error)
    finally:
        if connection:
            connection.close()
        if process.poll() is None:
            process.kill()
        process.communicate()


def exercise(binary):
    run_case(binary, b"PATCH /api/tasks/1?view=all HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer test\r\nContent-Length: 5\r\n\r\na\x00\xffbc", 200,
             expected=b"PATCH /api/tasks/1?view=all\nBearer test\na\x00\xffbc", fragments=[1, 2, 3, 7, 20, 15, 15])
    run_case(binary, b"DELETE /api/tasks/1 HTTP/1.0\r\n\r\n", 200,
             expected=b"DELETE /api/tasks/1\n\n")
    run_case(binary, b"HEAD /healthz HTTP/1.1\r\nHost: localhost\r\n\r\n", 200)
    run_case(binary, b"POST /api HTTP/1.1\r\nHost: localhost\r\nContent-Length: 16\r\n\r\n0123456789abcdef", 200,
             expected=b"POST /api\n\n0123456789abcdef")
    run_case(binary, b"POST /api HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: ChUnKeD\r\n\r\n3;ext=value\r\na\x00b\r\n2\r\ncd\r\n0\r\nDigest: test\r\n\r\n", 200,
             expected=b"POST /api\n\na\x00bcd", fragments=[30, 20, 8, 3, 8, 5])
    run_case(binary, b"POST /api HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\nExpect: 100-continue\r\n\r\nabc", 200,
             expected=b"POST /api\n\nabc", interim=True)
    invalid = [
        (b"GET / HTTP/1.1\r\n\r\n", 400),
        (b"GET / HTTP/1.1\nHost: localhost\n\n", 400),
        (b"GET https://example.test/ HTTP/1.1\r\nHost: localhost\r\n\r\n", 400),
        (b"GET /#fragment HTTP/1.1\r\nHost: localhost\r\n\r\n", 400),
        (b"GET / HTTP/2.0\r\nHost: localhost\r\n\r\n", 400),
        (b"GET / HTTP/1.1\r\nHost: one\r\nhOsT: two\r\n\r\n", 400),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\nAuthorization: first\r\nauthorization: second\r\n\r\n", 400),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\nX-Snus-Lease-Token: first\r\nx-snus-lease-token: second\r\n\r\n", 400),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\nFolded: first\r\n second\r\n\r\n", 400),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\nBad : value\r\n\r\n", 400),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\nBad: value\x00\r\n\r\n", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length:\r\n\r\n", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: +3\r\n\r\nabc", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\ncontent-length: 3\r\n\r\nabc", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 17\r\n\r\n", 413),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 18446744073709551615\r\n\r\n", 413),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\nab", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nContent-Length: 3\r\n\r\nabc", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: gzip, chunked\r\n\r\n", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding:\r\n\r\n", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n11\r\n", 413),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\ng\r\n", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n1\r\naXX0\r\n\r\n", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nAuthorization: attacker\r\n\r\n", 400),
        (b"POST / HTTP/1.1\r\nHost: localhost\r\nExpect: unsupported\r\n\r\n", 417),
        (b"GET /" + b"a" * 8192 + b" HTTP/1.1\r\nHost: localhost\r\n\r\n", 431),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\nBig: " + b"a" * 8192 + b"\r\n\r\n", 431),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\n" + b"Accept: x\r\n" * 100 + b"\r\n", 431),
        (b"GET / HTTP/1.1\r\nHost: localhost\r\n" + b"Accept: " + b"a" * 7990 + b"\r\n" + (b"Accept: " + b"a" * 7990 + b"\r\n") * 4 + b"\r\n", 431),
    ]
    for wire, status in invalid:
        run_case(binary, wire, status)
    started = time.monotonic()
    run_case(binary, b"GET / HTTP/1.1\r\nHost:", 408, timeout=100, shutdown=False)
    assert time.monotonic() - started < 2, "slow client exceeded total deadline"
    run_case(binary, b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n", 200, limit=0,
             expected=b"POST /\n\n")
    run_case(binary, b"POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\n", 413, limit=0)
    run_case(binary, b"PUT /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1073741824\r\n\r\na\x00\xffbc", 200, head_only=True,
             expected=b"PUT /upload\n\na\x00\xffbc", fragments=[20, 42, 1, 1, 1])
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n3\r\na\x00b\r\n0\r\n\r\n", 200, head_only=True,
             expected=b"POST /upload\n\n3\r\na\x00b\r\n0\r\n\r\n")
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\nExpect: 100-continue\r\n\r\nabc", 200, head_only=True, head_wait=True,
             expected=b"POST /upload\n\nabc")
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 9223372036854775807\r\n\r\nabc", 200, head_only=True,
             expected=b"POST /upload\n\nabc")
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 9223372036854775808\r\n\r\n", 413, head_only=True)
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\nContent-Length: 3\r\n\r\nabc", 400, head_only=True)
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nAuthorization: a\r\nAUTHORIZATION: b\r\n\r\n", 400, head_only=True)
    run_case(binary, b"GET /empty HTTP/1.1\r\nHost: localhost\r\n\r\n", 204, expected=b"")
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nLong: " + b"a" * 8193, 431, head_only=True, shutdown=False)
    run_case(binary, b"POST /upload HTTP/1.1\nHost: localhost\n", 400, head_only=True, shutdown=False)
    run_case(binary, b"PUT /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\na\x00\xffbc", 200, split=True,
             expected=b"PUT /upload\n\na\x00\xffbc")
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n3\r\na\x00b\r\n0\r\n\r\n", 200, split=True,
             expected=b"POST /upload\n\na\x00b")
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\nExpect: 100-continue\r\n\r\nabc", 200, split=True, interim=True,
             expected=b"POST /upload\n\nabc")
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 17\r\nExpect: 100-continue\r\n\r\n", 413, split=True,
             shutdown=False)
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\na", 408,
             timeout=100, shutdown=False)
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: 3\r\n\r\na", 408,
             timeout=100, shutdown=False, split=True)
    run_case(binary, b"POST /upload HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n9\r\n123456789\r\n8\r\n", 413, split=True)
    binary_body = bytes(range(256)) * 100
    run_case(binary, b"PUT /large HTTP/1.1\r\nHost: localhost\r\nContent-Length: 25600\r\n\r\n" + binary_body,
             200, limit=25600, split=True, expected=b"PUT /large\n\n" + binary_body)
    chunks = b"32c8\r\n" + binary_body[:13000] + b"\r\n3138\r\n" + binary_body[13000:] + b"\r\n0\r\n\r\n"
    run_case(binary, b"PUT /large HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n" + chunks,
             200, limit=25600, split=True, expected=b"PUT /large\n\n" + binary_body)
    for length, status in [(b"9999999999999999999", 413),
                           (b"18446744073709551615", 413),
                           (b"18446744073709551616", 400),
                           (b"9999999999999999999x", 400),
                           (b"17x", 400)]:
        wire = b"POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: " + length + b"\r\n\r\n"
        run_case(binary, wire, status)
        run_case(binary, wire, status, head_only=True)
        run_case(binary, wire, status, split=True)
    run_case(binary, b"PUT /stream HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\n\r\na\x00\xffbc", 200,
             streaming=True, expected=b"PUT /stream\n\na\x00\xffbc")
    run_case(binary, b"PUT /stream HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n2\r\na\x00\r\n5\r\nb\xffcde\r\n0\r\nDigest: ok\r\n\r\n", 200,
             streaming=True, expected=b"PUT /stream\n\na\x00b\xffcde")
    run_case(binary, b"POST /stream HTTP/1.1\r\nHost: localhost\r\nContent-Length: 5\r\nExpect: 100-continue\r\n\r\nabcde", 200,
             streaming=True, interim=True, expected=b"POST /stream\n\nabcde")
    run_case(binary, b"POST /stream HTTP/1.1\r\nHost: localhost\r\nContent-Length: 17\r\nExpect: 100-continue\r\n\r\n", 413,
             streaming=True, shutdown=False)
    run_case(binary, b"POST /stream HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1073741824\r\n\r\nshort", 400,
             streaming=True, limit=1073741824)
    run_case(binary, b"PUT /tiny HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n" + b"1\r\nx\r\n" * 4096 + b"0\r\n\r\n", 200,
             streaming=True, limit=4096, expected=b"PUT /tiny\n\n" + b"x" * 4096)
    run_case(binary, b"PUT /stream HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n11g\r\n", 400, streaming=True)
    run_case(binary, b"GET /empty HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n", 204,
             streaming=True, limit=0, expected=b"")
    run_case(binary, b"PUT /stream HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n0\r\nX-Test: done\r\nDigest: harmless\r\n\r\n", 200,
             streaming=True, expected=b"PUT /stream\n\nabc")
    for trailer in [b"X-Snus-Lease-Token: forged", b"Connection: keep-alive", b"Transfer-Encoding: identity"]:
        run_case(binary, b"PUT /stream HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n" + trailer + b"\r\n\r\n", 400, streaming=True)
    print("HTTP protocol: 84 native socket cases passed")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--compiler", default=str(ROOT / "build/selfhost/arm64-macos/dunder"))
    parser.add_argument("--binary", help="run an already-built native fixture")
    args = parser.parse_args()
    if args.binary:
        exercise(pathlib.Path(args.binary).resolve())
    else:
        with tempfile.TemporaryDirectory(prefix="dunder-http-") as directory:
            binary = pathlib.Path(directory) / "http-fixture"
            subprocess.run([args.compiler, "build", str(ROOT / "tests/http.ddr"), "-o", str(binary)], cwd=ROOT, check=True)
            exercise(binary)


if __name__ == "__main__":
    main()
