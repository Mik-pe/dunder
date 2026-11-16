#!/usr/bin/env python3
"""Exercise pinned Git dependencies with real local repositories and native programs."""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import tomllib

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('arm64-macos', 'arm64-linux', 'x86_64-linux')
COMMIT = re.compile(r'(?:[0-9a-f]{40}|[0-9a-f]{64})')


def host_target():
    system, machine = platform.system(), platform.machine()
    if system == 'Darwin' and machine == 'arm64':
        return TARGETS[0]
    if system == 'Linux' and machine in ('arm64', 'aarch64'):
        return TARGETS[1]
    if system == 'Linux' and machine == 'x86_64':
        return TARGETS[2]
    raise RuntimeError(f'unsupported native host {system}/{machine}')


def quote(value):
    return json.dumps(str(value), ensure_ascii=False)


def table(**fields):
    return '{ ' + ', '.join(f'{key} = {quote(value)}' for key, value in fields.items()) + ' }'


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)


class Repository:
    def __init__(self, probe, name, files, links=(), object_format=None):
        self.probe = probe
        self.author = probe.work / 'authors' / name
        self.origin = probe.work / 'bare origins' / (name + '.git')
        self.author.mkdir(parents=True)
        self.origin.parent.mkdir(parents=True, exist_ok=True)
        options = ['--object-format=' + object_format] if object_format else []
        probe.git('init', '--initial-branch=main', *options, self.author)
        for name, text in files.items():
            write(self.author / name, text)
            if name == 'build.sh':
                (self.author / name).chmod(0o755)
        for name, target in links:
            path = self.author / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.symlink_to(target)
        self.commit('initial')
        probe.git('clone', '--bare', self.author, self.origin)
        self.url = self.origin.as_uri()
        self.config = (self.author / '.git/config').read_bytes()

    def head(self):
        return self.probe.git('-C', self.author, 'rev-parse', 'HEAD').stdout.strip()

    def commit(self, message, files=None):
        for name, text in (files or {}).items():
            write(self.author / name, text)
        self.probe.git('-C', self.author, 'add', '--all')
        self.probe.git('-C', self.author, 'commit', '--quiet', '-m', message)
        return self.head()

    def push(self, reference='main'):
        self.probe.git('-C', self.author, 'push', self.origin, 'HEAD:refs/heads/' + reference)


class Probe:
    def __init__(self, compiler, work, runners):
        self.compiler = compiler
        self.work = work
        self.runners = runners
        self.commands = self.images = self.executions = self.rejections = 0
        self.git_config = work / 'isolated git config'
        write(self.git_config, '[user]\nname = Dunder fixture\nemail = fixture@example.invalid\n')
        self.git_config_bytes = self.git_config.read_bytes()
        self.env = dict(os.environ, GIT_CONFIG_NOSYSTEM='1',
                        GIT_CONFIG_GLOBAL=str(self.git_config), GIT_TERMINAL_PROMPT='0',
                        GIT_ALLOW_PROTOCOL='file')

    def command(self, argv, cwd=None, status=0, env=None):
        result = subprocess.run(list(map(str, argv)), cwd=cwd or self.work,
                                env=env or self.env, capture_output=True, text=True, timeout=90)
        if status is None:
            assert result.returncode in (1, 2), (argv, result.returncode, result.stdout, result.stderr)
        else:
            assert result.returncode == status, (argv, result.returncode,
                                                  result.stdout, result.stderr)
        return result

    def git(self, *args, status=0):
        return self.command(['git', '-c', 'protocol.file.allow=always', *args], status=status)

    def cli(self, project, *args, status=0, env=None):
        self.commands += 1
        return self.command([self.compiler, *args], project, status, env)

    def execute(self, image, target, expected):
        data = image.read_bytes()
        if target == TARGETS[0]:
            assert data[:4] == b'\xcf\xfa\xed\xfe', image
        else:
            assert data[:6] == b'\x7fELF\x02\x01', image
            assert int.from_bytes(data[18:20], 'little') == (183 if target == TARGETS[1] else 62)
        self.images += 1
        if target in self.runners:
            result = self.command([*self.runners[target], image], image.parent)
            assert (result.stdout, result.stderr) == (f'{expected}\n', ''), result
            self.executions += 1

    def reject(self, project, args, words=(), output=None):
        lock = project / 'dunder.lock'
        before = lock.read_bytes() if lock.exists() else None
        previous = output.read_bytes() if output and output.exists() else None
        result = self.cli(project, *args, status=None)
        assert result.stderr, result
        for word in words:
            assert word.lower() in result.stderr.lower(), (word, result.stderr)
        assert (lock.read_bytes() if lock.exists() else None) == before, result
        if output is not None:
            assert (output.read_bytes() if output.exists() else None) == previous, result
        self.rejections += 1
        return result


def project(probe, name, source, expected, duplicate=True):
    path = probe.work / 'projects' / name
    local = probe.work / 'local path with spaces'
    inline = probe.work / 'inline path with spaces'
    write(local / 'local.ddr', 'pub fn offset() -> i32 { 1 }\n')
    write(inline / 'dunder.toml', 'root = "src/api.ddr"\n')
    write(inline / 'src/api.ddr', 'pub fn offset() -> i32 { 2 }\n')
    dependencies = ['kit = ' + source,
                    'local = ' + quote(os.path.relpath(local, path)),
                    'inline = ' + table(path=os.path.relpath(inline, path))]
    if duplicate:
        dependencies.append('again = ' + source)
    manifest = ('name = "consumer"\nroot = "src/main.ddr"\ntest_root = "tests/suite.ddr"\n\n'
                '[dependencies]\n' + '\n'.join(dependencies) + '\n')
    write(path / 'dunder.toml', manifest)
    imports = 'import kit\nimport kit.detail as detail\nimport local\nimport inline\n'
    expression = 'kit.answer() + detail.offset() + local.offset() + inline.offset()'
    alias = 'import again\n' if duplicate else ''
    assertion = 'assert(kit.answer() == again.answer()); ' if duplicate else ''
    write(path / 'src/main.ddr', imports + alias + 'fn main() { ' + assertion +
          'print(' + expression + ') }\n')
    write(path / 'tests/suite.ddr', imports + '@test fn resolved() { assert(' +
          expression + ' == ' + str(expected) + ') }\n')
    return path


def pins(project, expected):
    data = (project / 'dunder.lock').read_bytes()
    lock = tomllib.loads(data.decode())
    assert type(lock['version']) is int and lock['version'] == 1, lock
    rows = lock.get('git', [])
    assert len(rows) == len(expected), lock
    actual = {}
    for row in rows:
        assert {'url', 'kind', 'reference', 'commit'} <= row.keys(), row
        assert COMMIT.fullmatch(row['commit']), row
        identity = row['url'], row['kind'], row['reference']
        assert identity not in actual, lock
        actual[identity] = row['commit']
    assert actual == expected, (actual, expected)
    return data


def checkouts(probe, project, commits):
    cache = project / 'build/.dunder/git'
    found = set()
    for metadata in cache.rglob('.git'):
        path = metadata.parent
        head = probe.git('-C', path, 'rev-parse', 'HEAD').stdout.strip()
        assert COMMIT.fullmatch(head) and head in commits, (path, head, commits)
        probe.git('-C', path, 'symbolic-ref', '--quiet', 'HEAD', status=1)
        found.add(head)
    assert found == commits, (cache, found, commits)


def checkout_for(probe, project, commit):
    for metadata in (project / 'build/.dunder/git').rglob('.git'):
        if probe.git('-C', metadata.parent, 'rev-parse', 'HEAD').stdout.strip() == commit:
            return metadata.parent
    raise AssertionError(('missing cached checkout', project, commit))


def matrix(probe, path, expected, flags=()):
    for target in TARGETS:
        image = path / ('image-' + target)
        probe.cli(path, 'build', '--target', target, '--jobs', '1', *flags, '-o', image)
        probe.execute(image, target, expected)


def basic_contracts(probe, path, expected, lock_bytes):
    checked = probe.cli(path, 'check', '--agent', '--offline', '--locked')
    assert json.loads(checked.stdout.splitlines()[-1])['kind'] == 'ok', checked
    ran = probe.cli(path, 'run', '--offline', '--locked', '-o', path / 'run-image')
    assert (ran.stdout, ran.stderr) == (f'{expected}\n', ''), ran
    tested = probe.cli(path, 'test', '--offline', '--locked', '-o', path / 'test-images')
    assert '::resolved' in tested.stdout and tested.stdout.endswith('1 passed, 0 failed\n'), tested
    assert tested.stderr == '', tested
    nested = probe.cli(path / 'src', 'check', 'main.ddr', '--agent', '--offline', '--locked')
    assert json.loads(nested.stdout.splitlines()[-1])['kind'] == 'ok', nested
    matrix(probe, path, expected, ('--offline', '--locked'))
    assert (path / 'dunder.lock').read_bytes() == lock_bytes


def offline_contracts(probe, path, repositories, expected, lock_bytes):
    moved = []
    try:
        for repo in repositories:
            hidden = repo.origin.with_name(repo.origin.name + '.unavailable')
            repo.origin.rename(hidden)
            moved.append((hidden, repo.origin))
        # A normal warm build must use its lock and cache without even trying
        # to refresh a moving branch. The actual local URLs are unavailable.
        probe.cli(path, 'build', '--jobs', '1', '-o', path / 'warm-image')
        basic_contracts(probe, path, expected, lock_bytes)
    finally:
        for hidden, origin in reversed(moved):
            hidden.rename(origin)
    cache = path / 'build/.dunder/git'
    saved = path / 'build/git-cache-saved'
    cache.rename(saved)
    image = path / 'offline-sentinel'
    image.write_bytes(b'previous valid image')
    try:
        probe.reject(path, ['build', '--offline', '--locked', '-o', image], ('offline',), image)
    finally:
        if cache.exists():
            shutil.rmtree(cache)
        saved.rename(cache)


def readonly_check(probe, path, marker):
    files = [file for file in (path / 'build/.dunder/git').rglob('*')
             if file.is_file() and (file.suffix == '.ddr' or file.name == 'dunder.toml')
             and '.git' not in file.relative_to(path).parts]
    assert files, path
    before = [(file, file.read_bytes(), file.stat().st_mode & 0o777) for file in files]
    try:
        for file, _, _ in before:
            file.chmod(0o444)
        probe.cli(path, 'check', '--offline', '--locked')
        for file, data, _ in before:
            assert file.read_bytes() == data and file.stat().st_mode & 0o777 == 0o444, file
    finally:
        for file, _, mode in before:
            if file.exists():
                file.chmod(mode)
    assert not marker.exists(), 'a dependency build script ran'


def malformed_contracts(probe, path, kit):
    manifest = path / 'dunder.toml'
    original = manifest.read_text()
    main = path / 'src/main.ddr'
    original_main = main.read_text()
    image = path / 'malformed-sentinel'
    image.write_bytes(b'previous valid image')
    local = os.path.relpath(probe.work / 'inline path with spaces', path)
    invalid = (
        table(git=''), table(git=kit.url, branch='main', tag='v1'),
        table(git=kit.url, rev=''), table(git=kit.url, path=local),
        table(branch='main'), table(git=kit.url, unexpected='value'),
        '{ git = 42 }', '{ git = ' + quote(kit.url),
        '{ git = ' + quote(kit.url) + ', git = ' + quote(kit.url) + ' }',
        table(path=''), table(path=local, branch='main'),
    )
    try:
        main.write_text('fn main() { print(42) }\n')
        for value in invalid:
            manifest.write_text('name = "consumer"\nroot = "src/main.ddr"\n[dependencies]\nkit = ' + value + '\n')
            probe.reject(path, ['build', '-o', image], ('dunder.toml',), image)
    finally:
        manifest.write_text(original)
        main.write_text(original_main)


def graph_rejection(probe, path, source, words):
    manifest = path / 'dunder.toml'
    original = manifest.read_text()
    main = path / 'src/main.ddr'
    original_main = main.read_text()
    image = path / 'graph-sentinel'
    image.write_bytes(b'previous valid image')
    try:
        manifest.write_text('name = "consumer"\nroot = "src/main.ddr"\n[dependencies]\n' + source + '\n')
        # The consumer would otherwise be valid even if the resolver wrongly
        # accepted the graph. Missing imported symbols must not fake rejection.
        main.write_text('fn main() { print(42) }\n')
        probe.reject(path, ['build', '-o', image], words, image)
    finally:
        manifest.write_text(original)
        main.write_text(original_main)


def graph_contracts(probe, path, leaf):
    origin_a = probe.work / 'bare origins/cycle a.git'
    origin_b = probe.work / 'bare origins/cycle b.git'
    a = Repository(probe, 'cycle a', {'dunder.toml': 'root = "api.ddr"\n[dependencies]\nb = ' +
                   table(git=origin_b.as_uri()) + '\n', 'api.ddr': 'pub fn value() -> i32 { 1 }\n'})
    b = Repository(probe, 'cycle b', {'dunder.toml': 'root = "api.ddr"\n[dependencies]\na = ' +
                   table(git=origin_a.as_uri()) + '\n', 'api.ddr': 'pub fn value() -> i32 { 2 }\n'})
    graph_rejection(probe, path, 'a = ' + table(git=a.url), ('dependency cycle',))

    different = Repository(probe, 'different shared', {'dunder.toml': 'root = "api.ddr"\n',
                           'api.ddr': 'pub fn value() -> i32 { 99 }\n'})
    left = Repository(probe, 'left', {'dunder.toml': 'root = "api.ddr"\n[dependencies]\nshared = ' +
                      table(git=leaf.url) + '\n', 'api.ddr': 'pub fn value() -> i32 { 1 }\n'})
    right = Repository(probe, 'right', {'dunder.toml': 'root = "api.ddr"\n[dependencies]\nshared = ' +
                       table(git=different.url) + '\n', 'api.ddr': 'pub fn value() -> i32 { 2 }\n'})
    graph_rejection(probe, path, 'left = ' + table(git=left.url) + '\nright = ' + table(git=right.url),
                    ('conflict',))
    missing_root = Repository(probe, 'missing root', {'dunder.toml': 'name = "broken"\n',
                              'api.ddr': 'pub fn value() -> i32 { 1 }\n'})
    graph_rejection(probe, path, 'kit = ' + table(git=missing_root.url), ('root',))

    outside = probe.work / 'outside checkout'
    write(outside / 'dunder.toml', 'root = "api.ddr"\n')
    write(outside / 'api.ddr', 'pub fn answer() -> i32 { 42 }\n')
    root_link = Repository(probe, 'escaping root', {'dunder.toml': 'root = "src/link.ddr"\n'},
                           [('src/link.ddr', outside / 'api.ddr')])
    graph_rejection(probe, path, 'kit = ' + table(git=root_link.url), ('root',))
    path_link = Repository(probe, 'escaping path', {'dunder.toml': 'root = "api.ddr"\n[dependencies]\nexternal = ' +
                           table(path='vendor/linked') + '\n',
                           'api.ddr': 'import external\npub fn answer() -> i32 { external.answer() }\n'},
                           [('vendor/linked', outside)])
    graph_rejection(probe, path, 'kit = ' + table(git=path_link.url), ('dependency',))
    # A failed Git graph leaves an immutable checkout available. An earlier
    # local alias may visit the same physical module with a wider boundary;
    # that visit must not suppress validation under the later Git boundary.
    checkout = checkout_for(probe, path, path_link.head())
    alias = probe.work / 'local boundary alias'
    write(alias / 'dunder.toml', 'root = "api.ddr"\n[dependencies]\nkit = ' + table(path=checkout) + '\n')
    write(alias / 'api.ddr', 'pub fn value() -> i32 { 1 }\n')
    graph_rejection(probe, path, 'a = ' + table(path=alias) + '\nkit = ' + table(git=path_link.url),
                    ('dependency',))


def local_only_contract(probe):
    path = probe.work / 'local only project'
    write(path / 'dunder.toml', 'name = "plain"\nroot = "main.ddr"\n')
    write(path / 'main.ddr', 'fn main() { print(42) }\n')
    empty_path = probe.work / 'no external programs'
    empty_path.mkdir()
    environment = dict(probe.env, PATH=str(empty_path))
    probe.cli(path, 'check', '--offline', '--locked', env=environment)
    image = path / 'plain'
    probe.cli(path, 'build', '--offline', '--locked', '-o', image, env=environment)
    probe.execute(image, host_target(), 42)
    assert not (path / 'build/.dunder/git').exists()
    assert probe.git_config.read_bytes() == probe.git_config_bytes


def deleted_ref_contracts(probe, kit, selectors):
    for name, reference in (('tag', 'refs/tags/v1'), ('feature branch', 'refs/heads/feature/next')):
        path, answer, before = next(row for row in selectors if row[0].name == name)
        probe.git('--git-dir', kit.origin, 'update-ref', '-d', reference)
        probe.reject(path, ['update'], output=path / ('image-' + TARGETS[0]))
        assert (path / 'dunder.lock').read_bytes() == before
        # Deleting a remote ref prevents an explicit update; it must not
        # invalidate the previous exact commit already available offline.
        result = probe.cli(path, 'run', '--offline', '--locked', '-o', path / 'deleted-ref-pinned')
        assert (result.stdout, result.stderr) == (f'{answer}\n', ''), result


def hidden_edit_contracts(probe, path, commit):
    checkout = checkout_for(probe, path, commit)
    relative = 'src/api.ddr'
    source = checkout / relative
    before, mode = source.read_bytes(), source.stat().st_mode & 0o777
    assert b'+ 21' in before, before
    image = path / 'hidden-edit-sentinel'
    image.write_bytes(b'previous valid image')
    for flag, undo in (('--assume-unchanged', '--no-assume-unchanged'),
                       ('--skip-worktree', '--no-skip-worktree')):
        try:
            probe.git('-C', checkout, 'update-index', flag, '--', relative)
            source.chmod(mode | 0o200)
            source.write_bytes(before.replace(b'+ 21', b'+ 91'))
            assert probe.git('-C', checkout, 'status', '--porcelain').stdout == '', flag
            probe.reject(path, ['build', '--offline', '--locked', '-o', image], output=image)
        finally:
            probe.git('-C', checkout, 'update-index', undo, '--', relative)
            source.write_bytes(before)
            source.chmod(mode)
        assert probe.git('-C', checkout, 'status', '--porcelain').stdout == '', flag


def concurrent_contract(probe, kit, expected, expected_pins):
    path = project(probe, 'concurrent cold project', table(git=kit.url, branch='main'), expected)
    children = []
    try:
        for number in range(2):
            image = path / f'concurrent-{number}'
            child = subprocess.Popen([str(probe.compiler), 'build', '--jobs', '1', '-o', str(image)],
                                     cwd=path, env=probe.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     text=True)
            children.append((child, image))
        for child, image in children:
            stdout, stderr = child.communicate(timeout=90)
            assert child.returncode == 0 and stderr == '', (child.returncode, stdout, stderr)
            probe.commands += 1
            probe.execute(image, host_target(), expected)
        assert children[0][1].read_bytes() == children[1][1].read_bytes()
        pins(path, expected_pins)
    finally:
        for child, _ in children:
            if child.poll() is None:
                child.kill()
                child.wait()


def prune_contract(probe, path):
    image = path / 'pruned'
    image.write_bytes(b'previous valid image')
    write(path / 'dunder.toml', 'name = "local"\nroot = "src/main.ddr"\n')
    write(path / 'src/main.ddr', 'fn main() { print(42) }\n')
    probe.reject(path, ['build', '--locked', '-o', image], ('locked',), image)
    probe.cli(path, 'build', '--offline', '-o', image)
    pins(path, {})
    probe.execute(image, host_target(), 42)


def embedded_contracts(probe):
    # The graph module imports both parser and Store, so one test executable
    # keeps their source-level parser/lock contracts in routine verification.
    result = probe.cli(ROOT, 'test', ROOT / 'selfhost/driver/dependencies.ddr', '--jobs', '1',
                       '-o', probe.work / 'dependency-unit-tests')
    summary = re.search(r'(\d+) passed, 0 failed\n$', result.stdout)
    assert summary and int(summary[1]) > 0 and result.stderr == '', result
    for group in ('dependency_paths_keep_relative_and_absolute_roots',
                  'malformed_dependency_tables_never_publish_a_partial_row',
                  'git_lock_round_trips_selectors_quoted_urls_and_exact_commits',
                  'git_lock_rejects_ambiguous_records_without_changing_pins'):
        assert '::' + group in result.stdout, result


def sha256_contract(probe):
    supported = subprocess.run(['git', 'init', '--bare', '--object-format=sha256',
                                str(probe.work / 'sha256 capability probe')],
                               env=probe.env, capture_output=True, text=True, timeout=30)
    if supported.returncode != 0:
        assert 'sha256' in supported.stderr or 'object-format' in supported.stderr, supported
        print('SKIP SHA256-origin contract: installed Git does not support SHA256 repositories')
        return
    repo = Repository(probe, 'sha256 origin', {'dunder.toml': 'root = "api.ddr"\n',
                      'api.ddr': 'pub fn answer() -> i32 { 42 }\n',
                      'detail.ddr': 'pub fn offset() -> i32 { 2 }\n'}, object_format='sha256')
    commit = repo.head()
    assert len(commit) == 64, commit
    path = project(probe, 'sha256 consumer', table(git=repo.url), 47)
    result = probe.cli(path, 'run', '-o', path / 'sha256-image')
    assert (result.stdout, result.stderr) == ('47\n', ''), result
    pins(path, {(repo.url, 'head', ''): commit})
    checkouts(probe, path, {commit})


def hidden_revision_contract(probe):
    repo = Repository(probe, 'hidden revision', {'dunder.toml': 'root = "api.ddr"\n',
                      'api.ddr': 'pub fn answer() -> i32 { 42 }\n',
                      'detail.ddr': 'pub fn offset() -> i32 { 2 }\n'})
    commit = repo.commit('PR revision', {'api.ddr': 'pub fn answer() -> i32 { 52 }\n'})
    probe.git('-C', repo.author, 'push', repo.origin, 'HEAD:refs/pull/7/head')
    path = project(probe, 'pull reference consumer', table(git=repo.url, rev='refs/pull/7/head'), 57)
    result = probe.cli(path, 'run', '-o', path / 'pull-image')
    assert (result.stdout, result.stderr) == ('57\n', ''), result
    pins(path, {(repo.url, 'rev', 'refs/pull/7/head'): commit})
    checkouts(probe, path, {commit})

    # The second consumer starts cold, with this commit absent from every
    # advertised ref. Configure our local server to permit its explicit fetch;
    # a client still has to request the full commit rather than just heads/tags.
    probe.git('--git-dir', repo.origin, 'update-ref', '-d', 'refs/pull/7/head')
    probe.git('--git-dir', repo.origin, 'config', 'uploadpack.allowAnySHA1InWant', 'true')
    assert probe.git('--git-dir', repo.origin, 'cat-file', '-t', commit).stdout.strip() == 'commit'
    path = project(probe, 'unadvertised revision consumer', table(git=repo.url, rev=commit), 57)
    result = probe.cli(path, 'run', '-o', path / 'revision-image')
    assert (result.stdout, result.stderr) == ('57\n', ''), result
    pins(path, {(repo.url, 'rev', commit): commit})
    checkouts(probe, path, {commit})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--runner', action='append', default=[], metavar='TARGET=COMMAND',
                        help='also execute a native target through an emulator or wrapper')
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    host = host_target()
    runners = {host: []}
    for value in args.runner:
        target, separator, command = value.partition('=')
        if not separator or target not in TARGETS or not shlex.split(command):
            parser.error('--runner requires a native TARGET=COMMAND')
        runners[target] = shlex.split(command)
    with tempfile.TemporaryDirectory(prefix='dunder-git dependencies-') as temporary:
        probe = Probe(compiler, Path(temporary).resolve(), runners)
        embedded_contracts(probe)
        leaf = Repository(probe, 'leaf library', {'dunder.toml': 'root = "api.ddr"\n',
                          'api.ddr': 'pub fn value() -> i32 { 10 }\n'})
        leaf_v1 = leaf.head()
        marker = probe.work / 'dependency build script ran'
        kit_files = {'dunder.toml': 'root = "src/api.ddr"\n[dependencies]\nleaf = ' +
                     table(git=leaf.url, branch='main') + '\ninside = ' + table(path='vendor/path helper') + '\n',
                     'src/api.ddr': 'import leaf\nimport inside\npub fn answer() -> i32 { leaf.value() + inside.offset() + 20 }\n',
                     'src/detail.ddr': 'pub fn offset() -> i32 { 2 }\n',
                     'vendor/path helper/dunder.toml': 'root = "api.ddr"\n',
                     'vendor/path helper/api.ddr': 'pub fn offset() -> i32 { 12 }\n',
                     'build.sh': '#!/bin/sh\nprintf unexpected > ' + shlex.quote(str(marker)) + '\n'}
        kit = Repository(probe, 'kit library', kit_files)
        kit_v1 = kit.head()
        probe.git('-C', kit.author, 'tag', 'v1')
        probe.git('-C', kit.author, 'push', kit.origin, 'refs/tags/v1')
        probe.git('-C', kit.author, 'checkout', '--quiet', '-b', 'feature/next')
        feature = kit.commit('feature branch', {'src/api.ddr': kit_files['src/api.ddr'].replace('+ 20', '+ 30')})
        kit.push('feature/next')
        probe.git('-C', kit.author, 'checkout', '--quiet', 'main')
        main_path = project(probe, 'consumer with spaces', table(git=kit.url, branch='main'), 47)
        image = main_path / 'sentinel'
        image.write_bytes(b'previous valid image')
        probe.reject(main_path, ['build', '--locked', '-o', image], ('locked',), image)
        probe.reject(main_path, ['build', '--offline', '-o', image], ('offline',), image)
        matrix(probe, main_path, 47)
        expected = {(kit.url, 'branch', 'main'): kit_v1, (leaf.url, 'branch', 'main'): leaf_v1}
        lock_bytes = pins(main_path, expected)
        checkouts(probe, main_path, {kit_v1, leaf_v1})
        readonly_check(probe, main_path, marker)
        offline_contracts(probe, main_path, (kit, leaf), 47, lock_bytes)

        selectors = []
        for name, spec, kind, reference, commit, answer in (
            ('default head', table(git=kit.url), 'head', '', kit_v1, 47),
            ('tag', table(git=kit.url, tag='v1'), 'tag', 'v1', kit_v1, 47),
            ('revision', table(git=kit.url, rev=feature), 'rev', feature, feature, 57),
            ('feature branch', table(git=kit.url, branch='feature/next'), 'branch', 'feature/next', feature, 57),
        ):
            path = project(probe, name, spec, answer)
            matrix(probe, path, answer)
            selector_pins = {(kit.url, kind, reference): commit, (leaf.url, 'branch', 'main'): leaf_v1}
            selectors.append((path, answer, pins(path, selector_pins)))
            checkouts(probe, path, {commit, leaf_v1})

        # Source identity is part of a lock even when another URL serves the
        # exact same commits. --locked must reject this manifest change.
        alternate = probe.work / 'bare origins/alternate kit.git'
        probe.git('clone', '--bare', kit.origin, alternate)
        manifest = main_path / 'dunder.toml'
        original = manifest.read_text()
        manifest.write_text(original.replace(kit.url, alternate.as_uri()))
        probe.reject(main_path, ['build', '--locked', '-o', image], ('locked',), image)
        manifest.write_text(original)

        kit_v2 = kit.commit('advance main', {'src/api.ddr': kit_files['src/api.ddr'].replace('+ 20', '+ 21')})
        kit.push()
        ran = probe.cli(main_path, 'run', '-o', main_path / 'pinned')
        assert ran.stdout == '47\n' and (main_path / 'dunder.lock').read_bytes() == lock_bytes, ran
        probe.reject(main_path, ['update', '--locked'], ('locked',))
        probe.cli(main_path, 'update')
        expected[(kit.url, 'branch', 'main')] = kit_v2
        pins(main_path, expected)
        assert probe.cli(main_path, 'run', '-o', main_path / 'updated').stdout == '48\n'
        leaf_v2 = leaf.commit('advance nested branch', {'api.ddr': 'pub fn value() -> i32 { 11 }\n'})
        leaf.push()
        assert probe.cli(main_path, 'run', '-o', main_path / 'nested-pinned').stdout == '48\n'
        probe.cli(main_path, 'update')
        expected[(leaf.url, 'branch', 'main')] = leaf_v2
        pins(main_path, expected)
        assert probe.cli(main_path, 'run', '-o', main_path / 'nested-updated').stdout == '49\n'
        for path, answer, previous in selectors:
            ran = probe.cli(path, 'run', '--offline', '--locked', '-o', path / 'still-pinned')
            assert ran.stdout == f'{answer}\n' and (path / 'dunder.lock').read_bytes() == previous, ran

        deleted_ref_contracts(probe, kit, selectors)
        hidden_edit_contracts(probe, main_path, kit_v2)
        malformed_contracts(probe, main_path, kit)
        graph_contracts(probe, main_path, leaf)
        local_only_contract(probe)
        concurrent_contract(probe, kit, 49, expected)
        prune_contract(probe, selectors[0][0])
        hidden_revision_contract(probe)
        sha256_contract(probe)
        assert not marker.exists(), 'a dependency build script ran'
        assert probe.git_config.read_bytes() == probe.git_config_bytes
        assert (kit.author / '.git/config').read_bytes() == kit.config
        assert (leaf.author / '.git/config').read_bytes() == leaf.config
        print(f'Git dependencies: {probe.commands} CLI commands, {probe.images} native images, '
              f'{probe.executions} target executions ({", ".join(runners)}), {probe.rejections} rejections; '
              'selectors, transitive pins, offline/locked use, update, graph failures, '
              'detached checkouts and concurrent cold builds passed')


if __name__ == '__main__':
    main()
