"""Shared Bun-first WebAssembly host discovery and persistent worker runner."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
from types import SimpleNamespace

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
CONTRACT_SCRIPT = ROOT / 'tools/runtime-contract-wasm.cjs'


def js_runtime(preferred=None):
    if preferred and preferred not in ('bun', 'node'):
        found = shutil.which(str(preferred))
        if found:
            return str(Path(found).resolve(strict=True))
    env_rt = os.environ.get('DUNDER_JS')
    if env_rt:
        found = shutil.which(env_rt)
        if found:
            return str(Path(found).resolve(strict=True))
    candidates = []
    if preferred == 'node':
        candidates.append('node')
    else:
        candidates.append('bun')
        home = Path.home()
        candidates.extend([
            str(home / '.local/share/mise/shims/bun'),
            str(home / '.local/bin/bun'),
            str(home / '.bun/bin/bun'),
            'node',
        ])
    for cand in candidates:
        found = shutil.which(cand) if '/' not in cand else (cand if os.access(cand, os.X_OK) else None)
        if found:
            return str(Path(found).resolve(strict=True))
    raise SystemExit('WebAssembly tests require bun (preferred via mise.toml) or node')


class WasmWorker:
    """Persistent Bun/Node process executing many Wasm modules without cold-starts."""

    def __init__(self, runtime=None):
        self._runtime = js_runtime(runtime)
        self._lock = threading.Lock()
        self._proc = subprocess.Popen(
            [self._runtime, str(CONTRACT_SCRIPT), '--worker'],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
        )

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.close()

    def close(self):
        proc = self._proc
        if proc is None:
            return
        self._proc = None
        try:
            if proc.stdin:
                proc.stdin.close()
            proc.wait(timeout=5)
        except Exception:
            proc.kill()
            proc.wait()

    def run(self, path, *, repeat=False, ignore_result=False, binary_output=False):
        req = json.dumps({
            'path': str(path),
            'repeat': bool(repeat),
            'ignoreResult': bool(ignore_result),
        }) + '\n'
        with self._lock:
            assert self._proc is not None and self._proc.stdin is not None and self._proc.stdout is not None
            self._proc.stdin.write(req)
            self._proc.stdin.flush()
            line = self._proc.stdout.readline()
            if not line:
                err = self._proc.stderr.read() if self._proc.stderr else ''
                raise RuntimeError(f'WasmWorker exited unexpectedly: {err}')
        reply = json.loads(line)
        stdout = reply['stdout'].encode('utf-8') if binary_output else reply['stdout']
        stderr = reply['stderr'].encode('utf-8') if binary_output else reply['stderr']
        return SimpleNamespace(
            args=[self._runtime, str(CONTRACT_SCRIPT), str(path)],
            returncode=int(reply['returncode']),
            stdout=stdout,
            stderr=stderr,
        )
