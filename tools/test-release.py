#!/usr/bin/env python3
"""Exercise local release packaging, draft recovery and published retries offline."""
import gzip
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
NAMES = {'release.txt', *(host + suffix for host in TARGETS for suffix in ('.gz', '.sha256'))}


def git(repo, *args):
    return subprocess.check_output(['git', '-C', repo, *args], text=True).strip()


def github_fixture(args):
    """Only the gh operations used by release.sh. Tags live in a real local bare repo."""
    work = Path(os.environ['DUNDER_TEST_RELEASE'])
    remote, assets, state = work / 'remote', work / 'assets', work / 'state'
    if args[0] == 'api':
        path = next(a for a in args if a.startswith('repos/Mik-pe/dunder/'))
        if '/matching-refs/tags/' in path:
            tag = path.split('/matching-refs/tags/')[1]
            ref = 'refs/tags/' + tag
            if subprocess.run(['git', '-C', remote, 'show-ref', '--verify', '--quiet', ref]).returncode == 0:
                print(git(remote, 'cat-file', '-t', ref), git(remote, 'rev-parse', ref))
        elif '/git/tags/' in path:
            tag = git(remote, 'cat-file', '-p', path.rsplit('/', 1)[1]).splitlines()
            print(tag[1].split()[1], tag[0].split()[1])
        elif path.endswith('/releases?per_page=100'):
            if state.exists():
                print(state.read_text())
        else:
            raise AssertionError(path)
        return
    assert args[0] == 'release' and args[args.index('--repo') + 1] == 'Mik-pe/dunder'
    action, tag = args[1:3]
    if action in ('create', 'upload', 'edit'):
        with (work / 'mutations').open('a') as log:
            log.write(' '.join(args) + '\n')
    if action == 'create':
        assert '--draft' in args and '--verify-tag' in args and not state.exists()
        assert git(remote, 'rev-parse', 'refs/tags/' + tag + '^{commit}') == args[args.index('--target') + 1]
        state.write_text('draft')
    elif action == 'view':
        if args[args.index('--json') + 1] == 'isDraft':
            print(str(state.read_text() == 'draft').lower())
        else:
            for file in assets.iterdir():
                print(file.name, 'sha256:' + hashlib.sha256(file.read_bytes()).hexdigest())
    elif action == 'upload':
        name = args[3]
        assert state.read_text() == 'draft' and name in NAMES
        dest = assets / name
        assert '--clobber' in args or not dest.exists()
        if name == 'arm64-macos.gz' and (work / 'interrupt').exists():
            dest.touch()
            (work / 'interrupt').unlink()
            sys.exit(1)
        shutil.copyfile(name, dest)
    elif action == 'download':
        dest = Path(args[args.index('--dir') + 1])
        for file in assets.iterdir():
            shutil.copyfile(file, dest / file.name)
    elif action == 'edit':
        assert state.read_text() == 'draft' and '--draft=false' in args
        assert {f.name for f in assets.iterdir()} == NAMES
        state.write_text('published')
    else:
        raise AssertionError(args)


def main():
    with tempfile.TemporaryDirectory(prefix='dunder-release-test-') as temp:
        work = Path(temp)
        repo, remote, assets = work / 'repo', work / 'remote', work / 'assets'
        assets.mkdir()
        # No git operation in the subprocess may contact a network host.
        env = dict(os.environ, DUNDER_TEST_RELEASE=str(work), GIT_ALLOW_PROTOCOL='file')
        env['PATH'] = str(work) + os.pathsep + env['PATH']
        (work / 'gh').symlink_to(Path(__file__).resolve())
        subprocess.run(['git', 'init', '--bare', '-q', remote], check=True)
        subprocess.run(['git', 'clone', '--shared', '--no-checkout', '--no-tags', '-q', ROOT, repo], check=True)
        commit = git(ROOT, 'rev-parse', 'HEAD')
        git(repo, 'checkout', '--detach', '-q', commit)
        git(repo, 'config', 'user.name', 'Release Fixture')
        git(repo, 'config', 'user.email', 'fixture@invalid.example')
        git(repo, 'config', f'url.{remote.as_uri()}.insteadOf', 'https://github.com/Mik-pe/dunder.git')
        script = repo / 'tools/release.sh'
        # Exercise the candidate script before it is committed. The release
        # itself still builds the pinned committed tree, ignoring dirty files.
        shutil.copyfile(ROOT / 'tools/release.sh', script)
        version = re.search(r'fn\s+compiler_version\s*\(\s*\)\s*->\s*string\s*\{\s*"([^"]+)"',
                            (repo / 'selfhost/driver/cli.ddr').read_text()).group(1)
        tag = 'v' + version
        package = work / 'local package'
        def run(publish=False, expected=0, version_tag=tag):
            args = [script, version_tag, '--outdir', package] + (['--publish'] if publish else [])
            result = subprocess.run(args, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
            if result.returncode != expected:
                raise RuntimeError(result.stdout.decode(errors='replace'))

        run(expected=1, version_tag=tag + '-wrong')
        assert not (package / 'release.txt').exists()
        run()
        assert not (work / 'mutations').exists()
        manifest = dict(line.split('=', 1) for line in (package / 'release.txt').read_text().splitlines())
        assert manifest['version'] == version and manifest['commit'] == commit
        assert {f.name for f in package.iterdir()} == NAMES
        for host in TARGETS:
            archive = (package / (host + '.gz')).read_bytes()
            checksum = hashlib.sha256(archive).hexdigest()
            assert manifest[host + '.gz.sha256'] == checksum
            assert manifest[host + '.sha256'] == hashlib.sha256(gzip.decompress(archive)).hexdigest()
            assert (package / (host + '.sha256')).read_text() == checksum + '  ' + host + '.gz\n'

        # Pin the tested source before moving local HEAD and dirtying its compiler.
        git(repo, 'tag', '-a', tag, '-m', 'Test release', commit)
        git(repo, 'commit', '--allow-empty', '-qm', 'Later local commit')
        (repo / 'selfhost/driver/cli.ddr').write_text('this uncommitted file must never be packaged')
        (work / 'interrupt').touch()
        run(publish=True, expected=1)
        assert (work / 'state').read_text() == 'draft'
        assert git(remote, 'rev-parse', 'refs/tags/' + tag) == git(repo, 'rev-parse', 'refs/tags/' + tag)
        run(publish=True)
        assert (work / 'state').read_text() == 'published'
        for file in package.iterdir():
            assert file.read_bytes() == (assets / file.name).read_bytes()
        previous = (work / 'mutations').read_bytes()
        run(publish=True)
        (assets / 'release.txt').write_text('changed remotely')
        run(publish=True, expected=1)
        assert (work / 'mutations').read_bytes() == previous
    print('PASS release packaging, draft recovery, published retry and remote mismatch')


if __name__ == '__main__':
    if Path(sys.argv[0]).name == 'gh':
        github_fixture(sys.argv[1:])
    else:
        main()
