#!/usr/bin/env python3
"""Verify emitted native images and signed bundles. --graphics also renders."""
import argparse
import hashlib
import os
from pathlib import Path
import platform
import plistlib
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MAC = platform.system() == 'Darwin'
LINUX_TARGETS = {'arm64-linux': (183, '/lib/ld-linux-aarch64.so.1'),
                 'x86_64-linux': (62, '/lib64/ld-linux-x86-64.so.2')}


def command(args, *, cwd, status=0, env=None):
    result = subprocess.run(args, cwd=cwd, env={**os.environ, **(env or {})},
                            capture_output=True, text=True, timeout=60)
    accepted = result.returncode > 0 if status is None else result.returncode == status
    if not accepted:
        raise RuntimeError(f'{args}: exit {result.returncode}\n{result.stdout}{result.stderr}')
    if 'VUID-' in result.stderr or 'Validation Error' in result.stderr:
        raise RuntimeError(result.stderr)
    return result


def check_elf(image, machine, interpreter):
    assert image[:6] == b'\x7fELF\x02\x01' and struct.unpack_from('<H', image, 18)[0] == machine
    table, = struct.unpack_from('<Q', image, 32)
    stride, count = struct.unpack_from('<HH', image, 54)
    segments = [struct.unpack_from('<IIQQQQQQ', image, table + i * stride) for i in range(count)]
    assert all(kind != 1 or flags & 3 != 3 for kind, flags, *_ in segments)
    assert any(kind == 0x6474e551 and flags == 6 for kind, flags, *_ in segments)
    loaders = [image[offset:offset + size] for kind, _, offset, _, _, size, _, _ in segments if kind == 3]
    assert loaders == [interpreter.encode() + b'\0']
    assert any(kind == 2 for kind, *_ in segments)


def bundle(build, work):
    source = work / 'bundle.ddr'
    source.write_text('import sys\nfn main()->i32{print(sys.arg(1));return 17}\n')
    app = work / 'Atlas & Light.app'
    contents = app / 'Contents'
    binary = contents / 'MacOS' / app.stem
    resource = contents / 'Resources/art.bin'
    info = contents / 'Info.plist'
    seal = contents / '_CodeSignature/CodeResources'
    resource.parent.mkdir(parents=True)
    resource.write_bytes(bytes(range(256)))
    args = ['--target', 'arm64-macos', '--app']
    build(source, app, *args)
    plist = plistlib.loads(info.read_bytes())
    assert plist['CFBundleExecutable'] == app.stem and plist['CFBundlePackageType'] == 'APPL'
    assert binary.read_bytes()[:4] == b'\xcf\xfa\xed\xfe'
    assert plistlib.loads(seal.read_bytes())['files2']['Resources/art.bin']['hash2'] == hashlib.sha256(resource.read_bytes()).digest()
    if MAC:
        command(['/usr/bin/codesign', '--verify', '--strict', app], cwd=work)
        assert command([binary, 'argument'], cwd=work, status=17).stdout == 'argument\n'
        resource.write_bytes(b'changed')
        command(['/usr/bin/codesign', '--verify', '--strict', app], cwd=work, status=None)
        build(source, app, *args)
        command(['/usr/bin/codesign', '--verify', '--strict', app], cwd=work)
    before = [path.read_bytes() for path in (binary, info, seal)]
    source.write_text('fn main(){ missing_symbol() }\n')
    assert "missing_symbol" in build(source, app, *args, status=None).stderr
    assert before == [path.read_bytes() for path in (binary, info, seal)]
    source.write_text('fn main(){print(42)}\n')
    (resource.parent / 'linked').symlink_to(source)
    assert "symbolic link" in build(source, app, *args, status=None).stderr
    assert before == [path.read_bytes() for path in (binary, info, seal)]
    print('PASS app bundle: metadata, binary resource seal and failed rebuild preservation'
          + ('; macOS verified signature, tamper detection, reseal and executable' if MAC else ' (cross-build)'))


def graphics(build, work, live):
    sources = ROOT / 'dogfood'
    shutil.copytree(sources / 'graphics', work / 'graphics')
    standalone = work / 'external.ddr'
    standalone.write_text('import graphics.vulkan as vk\nimport graphics.macos as mac\n'
                          'fn main(){print(vk.rgb(1.0,0.0,0.0).red == mac.rgb(1.0,0.0,0.0).red)}\n')
    build(standalone, work / 'external')
    assert command([work / 'external'], cwd=work).stdout == 'true\n'
    for name in ('macos', 'vulkan'):
        standalone.write_text(f'import std.{name}\nfn main(){{}}\n')
        assert f'std/{name}.ddr' in build(standalone, work / 'unexpected', status=None).stderr
    triangle = sources / 'vulkan/triangle.ddr'
    metal = sources / 'metal/triangle.ddr'
    for source, target in [(triangle, 'arm64-macos'), (metal, 'arm64-linux')]:
        rejection = build(source, work / 'unexpected', '--target', target, status=None)
        assert 'requires the selected native platform' in rejection.stderr
        assert not (work / 'unexpected').exists()
    for target, (machine, interpreter) in LINUX_TARGETS.items():
        path = work / target
        build(triangle, path, '--target', target)
        image = path.read_bytes()
        check_elf(image, machine, interpreter)
        build(triangle, path, '--target', target)
        assert image == path.read_bytes()
    mac_triangle = work / 'Triangle.app'
    build(metal, mac_triangle, '--target', 'arm64-macos', '--app')
    print('PASS external graphics modules and cross-builds: Metal app, ARM64 and x86-64 Vulkan ELF')
    if not live:
        print('Graphics execution was not requested. Pass --graphics on a suitable host.')
        return
    if MAC:
        command(['/usr/bin/codesign', '--verify', '--strict', mac_triangle], cwd=work)
        programs = [(mac_triangle / 'Contents/MacOS/Triangle', ['--frames', '3'], 'triangle frames=3')]
        for name, args, expected in [('main', ['--frames', '3'], 'atlas frames=3'),
                                     ('lifecycle', [], 'lifecycle cycles=3'), ('events', [], 'native event sequence ok')]:
            path = work / name
            build(sources / f'metal/{name}.ddr', path)
            programs.append((path, args, expected))
        for binary, args, expected in programs:
            assert command([binary, *args], cwd=work, env={'MTL_DEBUG_LAYER': '1'}).stdout.strip() == expected
        print('PASS live AppKit/Metal: triangle and Atlas frames, window lifecycle and native events')
    else:
        for name, args, expected in [('triangle', ['--headless', '--frames', '3'], 'triangle frames=3'),
                                     ('main', ['--headless', '--frames', '3'], 'atlas frames=3'),
                                     ('lifecycle', [], 'lifecycle cycles=3'), ('pixels', [], 'pixels ok'),
                                     ('events', [], 'SDL surface and events ok')]:
            path = work / name
            build(sources / f'vulkan/{name}.ddr', path)
            env = {'SDL_VIDEODRIVER': 'dummy'} if name == 'events' else {}
            result = command([path, *args], cwd=work, env=env)
            assert result.stdout.splitlines()[-1] == expected, result.stdout
        print('PASS live Vulkan: triangle and Atlas frames, RGBA interpolation, alpha, lifecycle and SDL surface/events')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--graphics', action='store_true', help='execute native Metal or Vulkan/SDL graphics')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='dunder-native-') as tmp:
        work = Path(tmp)

        def build(source, output, *flags, status=0):
            return command([compiler, 'build', source, '-o', output, *flags], cwd=work,
                           env={'PATH': '/no-external-tools'}, status=status)

        bundle(build, work)
        graphics(build, work, args.graphics)


if __name__ == '__main__':
    main()
