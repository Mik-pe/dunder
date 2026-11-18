import csv
import hashlib
import io
import itertools
from pathlib import Path
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
ENV = {'PATH': '/no-external-tools'}
TEXT = ('Dunder läser UTF-8: åäö, Ελληνικά och 🌍.\n'
        'Säkra värden överlever sina anrop; loopar räknar byte och runor.\t'
        'Native kod utan externa byggverktyg. ') * 4


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def command(args, cwd=ROOT, env=ENV):
    start = time.perf_counter_ns()
    result = subprocess.run([str(a) for a in args], cwd=cwd, env=env,
                            capture_output=True, timeout=120)
    elapsed = (time.perf_counter_ns() - start) / 1_000_000
    if result.returncode:
        raise RuntimeError(f'{args}: exit {result.returncode}\n'
                           + (result.stdout + result.stderr).decode(errors='replace'))
    return elapsed, result.stdout


def checked_run(binary, args, expected):
    elapsed, output = command([binary, *args])
    if output != expected:
        raise RuntimeError(f'{binary}: expected {expected!r}, got {output!r}')
    return elapsed


def runtime_cases(scale=1):
    seed = 1729
    counts = {'numeric': 1_000_000 * scale, 'scan': 3000 * scale,
              'text': 2000 * scale, 'sort': 120 * scale}
    value, numeric = seed, 0
    for _ in range(counts['numeric']):
        value = (value * 1664525 + 1013904223) % 2147483647
        numeric += value
    values = [(i * 17 + seed) % 1009 for i in range(1024)]
    scan = 0
    for i in range(counts['scan']):
        values[i % 1024] = i
        scan += sum(values)
    words, inside = 0, False
    for scalar in TEXT:
        if scalar in ' \n\t':
            inside = False
        elif not inside:
            words, inside = words + 1, True
    text = (words * 1_000_000 + sum(map(ord, TEXT))) * counts['text']
    sort = 0
    for repeat in range(counts['sort']):
        values = sorted(((255-i) * 17 + seed + repeat) % 1009 for i in range(256))
        sort += sum(value * (i + 1) for i, value in enumerate(values))
    return [('numeric', [str(counts['numeric']), str(seed)], numeric),
            ('scan', [str(counts['scan']), str(seed)], scan),
            ('text', [str(counts['text']), TEXT], text),
            ('sort', [str(counts['sort']), str(seed)], sort)]


def tabell_case(work, project):
    path = work / 'expenses.csv'
    totals = {}
    labels = ['Stockholm', 'Örebro', 'Malmö', 'Team, A', 'Team "B"', 'Umeå']
    with path.open('w', encoding='utf-8', newline='') as output:
        records = csv.writer(output, lineterminator='\r\n')
        records.writerow(['team', 'cents', 'memo'])
        for row in range(32768):
            group = f'{labels[row % len(labels)]} {row % 67}'
            amount = (row % 101 - 50) * 1234567
            records.writerow([group, amount, f'order {row}'])
            totals[group] = totals.get(group, 0) + amount
    verified = io.StringIO(newline='')
    result = csv.writer(verified, lineterminator='\n')
    result.writerow(['group', 'total'])
    for group, total in sorted(totals.items()):
        result.writerow([group, total])
    return 'tabell', project / 'main.ddr', [str(path), 'team', 'cents'], verified.getvalue().encode()


def corpus(work):
    root = work / 'corpus'
    root.mkdir()
    main = ''.join(f'import m{i}\n' for i in range(20))
    for module in range(20):
        source = ''.join(f'pub fn item{f}(x: i64) -> i64 {{\n    mut value := x\n'
                         + '    value = value + 1\n' * 96
                         + '    return value\n}\n' for f in range(50))
        (root / f'm{module}.ddr').write_text(source)
    main += 'fn main() {\n    mut total: i64 = 0\n'
    for module in range(20):
        for f in range(50):
            main += f'    total = total + m{module}.item{f}({module * 50 + f})\n'
    main += '    print(total)\n}\n'
    (root / 'main.ddr').write_text(main)
    sources = [path.read_bytes() for path in sorted(root.glob('*.ddr'))]
    # This generated corpus has no strings or comments; this pattern exactly
    # counts its lexical tokens, including the two-character operators.
    token = rb'[A-Za-z_0-9]+|:=|->|==|!=|<=|>=|&&|\|\||\.\.|[^\s]'
    info = {'lines': sum(len(s.splitlines()) for s in sources),
            'source_bytes': sum(map(len, sources)), 'modules': 21, 'functions': 1001,
            'tokens': sum(len(re.findall(token, s)) for s in sources),
            'sha256': hashlib.sha256(b''.join(sources)).hexdigest()}
    return root / 'main.ddr', info


def source_hashes():
    paths = sorted(set((ROOT / 'selfhost').rglob('*.ddr')) | set(ROOT.glob('stdlib/*.ddr'))
                   | set(ROOT.glob('runtime/*.ddr')) | set(ROOT.glob('benchmarks/runtime/*.ddr'))
                   | {ROOT / 'benchmarks/runtime/reference.c'})
    return {str(p.relative_to(ROOT)): digest(p) for p in paths}


def order(variants, round_number):
    if len(variants) == 2:
        return variants if round_number % 2 == 0 else list(reversed(variants))
    return list(list(itertools.permutations(variants))[round_number % 6])


def save_csv(path, rows):
    if rows:
        with path.open('w', newline='') as output:
            writer = csv.DictWriter(output, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)



if __name__ == '__main__':
    raise SystemExit("Use dunder run tools/performance.ddr -- BASELINE CANDIDATE REPORT.")
