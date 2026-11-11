#!/usr/bin/env python3
"""Interpret actual Dunder IR and challenge every reached bounds proof.

This finite consumer safety check is independent of the solver's interval and
affine representation. Fuel exhaustion is INCOMPLETE (exit 2), never a pass.
It does not prove arbitrary inputs or loops, or validate target instruction code.
"""
import argparse
import hashlib
import itertools
import json
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
NODE_FIELDS = ('kind', 'type', 'left', 'right', 'third', 'next', 'value',
               'local', 'proof', 'effect', 'start', 'pending')
LOCAL_FIELDS = ('type', 'param', 'mutable', 'at')


def exporter(source_root, work):
    imports = []
    for module, alias in (('opt_ir', 'ir'), ('opt_parse', 'parse'), ('opt_pass', 'pass'),
                          ('opt_range', 'ranges'), ('opt_verify', 'verify'), ('lexer', 'lexer')):
        path = os.path.relpath(source_root / 'selfhost' / module, work).replace(os.sep, '/')
        imports.append('import ' + json.dumps(path) + ' as ' + alias)
    return '\n'.join(imports) + '''
import sys
import std.parse as decimal
fn same(f:&ir.Function,before:&ir.Function){
    assert(f.root==before.root && f.ret==before.ret && f.params==before.params)
    assert(f.supported==before.supported && f.visits==before.visits && f.rewrites==before.rewrites && f.budget_exhausted==before.budget_exhausted)
    assert(f.kinds.len==before.kinds.len && f.local_types.len==before.local_types.len)
    for i in 0..f.kinds.len{
        assert(f.kinds[i]==before.kinds[i] && f.types[i]==before.types[i] && f.left[i]==before.left[i] && f.right[i]==before.right[i] && f.third[i]==before.third[i] && f.next[i]==before.next[i])
        assert(f.values[i]==before.values[i] && f.locals[i]==before.locals[i] && f.proofs[i]==before.proofs[i] && f.effects[i]==before.effects[i] && f.starts[i]==before.starts[i])
    }
    for i in 0..f.local_types.len{
        assert(f.local_types[i]==before.local_types[i] && f.local_param[i]==before.local_param[i] && f.local_mutable[i]==before.local_mutable[i] && f.local_at[i]==before.local_at[i] && f.local_names[i]==before.local_names[i])
    }
}
fn main(){
    file:=sys.read_text(sys.arg(1));assert(file.ok)
    tokens:=lexer.scan("a b c n m x flag "+file.value,false)
    names:[]string=["a","b","c","n","m","x","flag"]
    types:[]i32=[21,20,21,1,1,2,3];positions:[]i32=[0,1,2,3,4,5,6]
    var f:=ir.new();assert(parse.parse(&mut f,&tokens,7,tokens.kinds.len-2,&names,&types,&positions,2))
    assert(verify.validate(&f,false))
    if sys.arg(3)=="simplified"{pass.run(&mut f);assert(!f.budget_exhausted)}
    assert(verify.validate(&f,true))
    before:=f
    var w:=ranges.new()
    // Reuse a consumed workspace before the budget-constrained observation.
    var warm:=f
    if ranges.analyze_into(&mut w,&warm){ranges.apply(&mut w,&mut warm)}
    allowance:=i32(decimal.integer(sys.arg(2)).value);ranges.set_budget(&mut w,allowance)
    ready:=ranges.analyze_into(&mut w,&f);same(&f,&before)
    if ready{assert(ranges.status(&w)==1);ranges.apply(&mut w,&mut f);assert(ranges.status(&w)==2);assert(verify.validate(&f,true))}
    else{assert(ranges.status(&w)==3||ranges.status(&w)==4)}
    print(1);print(f.root);print(f.ret);print(f.params);print(f.kinds.len);print(f.local_types.len)
    print(ranges.status(&w));print(ranges.units(&w));print(ranges.read_count(&w));print(ranges.store_count(&w));print(f.visits);print(f.rewrites)
    for i in 0..f.kinds.len{
        if ranges.produced(&w,i){assert(f.proofs[i]==before.proofs[i].bit_or(2))}else{assert(f.proofs[i]==before.proofs[i])}
        print(f.kinds[i]);print(f.types[i]);print(f.left[i]);print(f.right[i]);print(f.third[i]);print(f.next[i]);print(f.values[i]);print(f.locals[i]);print(f.proofs[i]);print(f.effects[i]);print(f.starts[i])
        if ranges.produced(&w,i){print(1)}else{print(0)}
    }
    for i in 0..f.local_types.len{
        print(f.local_types[i]);print(f.local_param[i]);if f.local_mutable[i]{print(1)}else{print(0)};print(f.local_at[i])
    }
}
'''


def decode(output):
    numbers = list(map(int, output.splitlines()))
    assert numbers[0] == 1 and len(numbers) >= 12, 'invalid export header'
    version, root, ret, params, nodes, locals_, status, units, reads, stores, visits, rewrites = numbers[:12]
    assert len(numbers) == 12 + nodes * len(NODE_FIELDS) + locals_ * len(LOCAL_FIELDS), 'invalid export size'
    at = 12
    ns = []
    for _ in range(nodes):
        ns.append(dict(zip(NODE_FIELDS, numbers[at:at + len(NODE_FIELDS)])))
        at += len(NODE_FIELDS)
    ls = []
    for _ in range(locals_):
        ls.append(dict(zip(LOCAL_FIELDS, numbers[at:at + len(LOCAL_FIELDS)])))
        at += len(LOCAL_FIELDS)
    assert status in (2, 3, 4) and 0 <= units <= 131072
    assert sum(n['pending'] for n in ns if n['kind'] == 6) == reads
    assert sum(n['pending'] for n in ns if n['kind'] == 18) == stores
    if status != 2:
        assert reads == stores == 0 and not any(n['pending'] for n in ns), 'failed analysis leaked proofs'
    for n in ns:
        if n['pending']:
            assert n['kind'] in (6, 18) and n['proof'] & 2
    return dict(root=root, ret=ret, params=params, nodes=ns, locals=ls,
                status=status, units=units, reads=reads, stores=stores)


class Trap(Exception):
    pass


class Fuel(Exception):
    pass


class Flow(Exception):
    def __init__(self, kind, value=None):
        self.kind, self.value = kind, value


def checked(value, ty):
    assert ty in (1, 2), ty
    bits = 32 if ty == 1 else 64
    if not -(1 << (bits - 1)) <= value < 1 << (bits - 1):
        raise Trap(101)
    return value


class Machine:
    def __init__(self, model, arguments, fuel):
        self.model, self.fuel, self.steps = model, fuel, 0
        self.values = {}
        # Preserve shared Python list identity for explicit alias challenges.
        clones = {}
        for i, local in enumerate(model['locals']):
            if local['param'] >= 0:
                value = arguments[local['param']]
                if local['type'] in (20, 21):
                    value = clones.setdefault(id(value), list(value))
                self.values[i] = value
        self.marked = set()
        self.accesses = 0

    def tick(self):
        self.steps += 1
        if self.steps > self.fuel:
            raise Fuel()

    def access(self, node, index):
        array = self.values[node['local']]
        valid = 0 <= index < len(array)
        self.accesses += 1
        if node['proof'] & 2:
            # Always use THIS node's array root, never any related root or a
            # dominating loop's length. Check before the ordinary trap path.
            assert valid, ('UNSOUND BOUNDS PROOF', node, index, len(array), self.values)
            self.marked.add(id(node))
        if not valid:
            raise Trap(103)
        return array

    def expression(self, index):
        self.tick()
        node = self.model['nodes'][index]
        kind, ty, op = node['kind'], node['type'], node['value']
        if kind == 1:
            return bool(op) if ty == 3 else op
        if kind == 2:
            return self.values[node['local']]
        if kind == 5:
            return len(self.values[node['local']])
        if kind == 6:
            i = self.expression(node['left'])
            return self.access(node, i)[i]
        if kind == 4:
            a = self.expression(node['left'])
            return not a if op == 2 else checked(-a, ty)
        assert kind == 3, node
        a = self.expression(node['left'])
        if op == 12:
            return bool(a and self.expression(node['right']))
        if op == 13:
            return bool(a or self.expression(node['right']))
        b = self.expression(node['right'])
        if op == 1:
            return checked(a + b, ty)
        if op == 2:
            return checked(a - b, ty)
        if op == 3:
            return checked(a * b, ty)
        if op in (4, 5):
            if b == 0:
                raise Trap(102)
            bits = 32 if ty == 1 else 64
            if a == -(1 << (bits - 1)) and b == -1:
                raise Trap(101)
            q = (abs(a) // abs(b)) * (-1 if (a < 0) != (b < 0) else 1)
            return q if op == 4 else a - q * b
        return {6: lambda: a == b, 7: lambda: a != b, 8: lambda: a < b,
                9: lambda: a <= b, 10: lambda: a > b, 11: lambda: a >= b}[op]()

    def statement(self, index):
        self.tick()
        node = self.model['nodes'][index]
        kind = node['kind']
        if kind == 10:
            child = node['left']
            while child >= 0:
                self.statement(child)
                child = self.model['nodes'][child]['next']
        elif kind in (11, 12):
            self.values[node['local']] = self.expression(node['left'])
        elif kind == 13:
            child = node['right'] if self.expression(node['left']) else node['third']
            if child >= 0:
                self.statement(child)
        elif kind == 14:
            while self.expression(node['left']):
                try:
                    self.statement(node['right'])
                except Flow as flow:
                    if flow.kind == 'break':
                        break
                    if flow.kind != 'continue':
                        raise
        elif kind == 19:
            # Both bounds execute before the iterator binding is introduced.
            low = self.expression(node['left'])
            high = self.expression(node['right'])
            self.values[node['local']] = low
            while self.values[node['local']] < high:
                try:
                    self.statement(node['third'])
                except Flow as flow:
                    if flow.kind == 'break':
                        break
                    if flow.kind != 'continue':
                        raise
                self.values[node['local']] = checked(self.values[node['local']] + 1, 1)
        elif kind == 15:
            raise Flow('return', self.expression(node['left']) if node['left'] >= 0 else None)
        elif kind in (16, 17):
            raise Flow('break' if kind == 16 else 'continue')
        elif kind == 18:
            i = self.expression(node['left'])
            array = self.access(node, i)  # Destination/bounds precede the RHS.
            value = self.expression(node['right'])
            array[i] = value
        else:
            raise AssertionError(('unknown statement', node))

    def run(self):
        try:
            self.statement(self.model['root'])
        except Trap as trap:
            return ('trap', trap.args[0])
        except Flow as flow:
            assert flow.kind == 'return', flow.kind
            return ('return', flow.value)
        return ('return', None)


def cases():
    bodies = {
        'sum': '{var total:i64=0;for i in 0..a.len{total=total+a[i]};return total}',
        'fill': '{for i in 0..a.len{a[i]=x};return x}',
        'sort': '{for i in 1..a.len{value:=a[i];var j:=i;while j>0&&a[j-1]>value{a[j]=a[j-1];j=j-1};a[j]=value};return x}',
        'reverse': '{var l:=0;var r:=a.len-1;while l<r{value:=a[l];a[l]=a[r];a[r]=value;l=l+1;r=r-1};return x}',
        'while_mutation': '{var i:=0;while i<a.len{a[i]=a[i]+1;i=i+1};return x}',
        'snapshot_grows': '{var high:=a.len;for i in 0..high{high=high+1;a[i]=x};return x}',
        'snapshot_shrinks': '{var high:=a.len;for i in 0..high{high=0;a[i]=x};return x}',
        'snapshot_low_shadow': '{var i:=n;for i in i..a.len{if i<0{break};a[i]=x};return x}',
        'nested': '{for i in 0..a.len{for j in 0..c.len{if j==1{continue};if j==3{break};c[j]=a[i]}};return x}',
        'continue_break': '{var i:=0;while i<a.len{if i==1{i=i+1;continue};a[i]=x;if i==3{break};i=i+1};return x}',
        'early_return': '{for i in 0..a.len{if i==n{return a[i]};a[i]=x};return x}',
        'if_else': '{var i:=n;if flag{if i>=0&&i<a.len{a[i]=x}}else{if i>=0&&i<c.len{c[i]=x}};return x}',
        'negation_guard': '{if !(n<0||n>=a.len){a[n]=x};return x}',
        'or_join': '{if (n>=0&&n<a.len)||(m>=0&&m<a.len){a[n]=x};return x}',
        'cross_root': '{for i in 0..a.len{c[i]=a[i]+b[i]};return x}',
        'two_root_guard': '{if n>=0&&n<a.len&&n<b.len{a[n]=b[n]};return x}',
        'wrong_guard_root': '{if n>=0&&n<b.len{a[n]=x};return x}',
        'offsets': '{for i in 1..a.len{a[i]=a[i-1]};return x}',
        'offset_upper': '{for i in 0..a.len{a[i]=a[i+1]};return x}',
        'negative': '{for i in -1..a.len{a[i]=x};return x}',
        'mutated_induction': '{var i:=0;while i<a.len{i=i-1;return a[i]};return x}',
        'mutated_before_read': '{var i:=n;if i>=0&&i<a.len{i=m;a[i]=x};return x}',
        'killed_relation': '{var i:=0;var guard:=0;while i<a.len&&guard<4{guard=guard+1;if flag{i=m};if i<0||i>=a.len{break};a[i]=x;i=i+1};return x}',
        'backedge': '{var i:=0;while i<a.len{a[i]=x;i=i+2};return x}',
        'empty_high_traps': '{for i in 1..n/0{a[i]=x};return x}',
        'low_trap_first': '{for i in n/0..m+1{a[i]=x};return x}',
        'high_overflow': '{for i in n..m+1{if i<0||i>=a.len{break};a[i]=x};return x}',
        'checked_index': '{if n>=0&&n<a.len{a[n+2147483647]=x};return x}',
        'store_order': '{a[n]=x/0;return x}',
        'index_order': '{a[n+1]=x/0;return x}',
        'load_order': '{a[n]=b[m];return x}',
        'short_and': '{if n>=0&&n<a.len&&a[n]>0{a[n]=x};return x}',
        'short_or': '{if n<0||n>=a.len||a[n]>0{return x};a[n]=x;return x}',
        'wide_overflow': '{if n>=0&&n<a.len{a[n]=x+1};return x}',
        'wide_negate': '{if n>=0&&n<a.len{a[n]=-x};return x}',
        'wide_min_div': '{if n>=0&&n<a.len{a[n]=x/(-1)};return x}',
        'wide_min_rem': '{if n>=0&&n<a.len{a[n]=x%(-1)};return x}',
        'nested_shadow': '{for i in 0..a.len{{i:=n;if i>=0&&i<c.len{c[i]=x}};a[i]=x};return x}',
        'signed_latch_high': '{for i in 2147483646..2147483647{if i<a.len{a[i]=x}};return x}',
        'signed_latch_low': '{for i in -2147483648..-2147483647{if i>=0&&i<a.len{a[i]=x}};return x}',
    }
    # A frozen generated family challenges affine signs, offsets and distinct
    # roots without mirroring the range solver's abstract representation.
    for offset in (-2, -1, 0, 1, 2):
        for root in ('a', 'b', 'c'):
            bodies[f'generated_{root}_{offset}'] = ('{if n>=0&&n<a.len{'
                + f'if n+({offset})>=0&&n+({offset})<{root}.len{{a[n]={root}[n+({offset})]}}'
                + '};return x}')
    return bodies


def inputs():
    xs = (-(1 << 63), -(1 << 63) + 1, -1, 0, 1, (1 << 63) - 1)
    small = (-2, -1, 0, 1, 2, 3, 4)
    for la, lb, lc, n, m, flag in itertools.product(range(4), range(4), range(4), small, small, (False, True)):
        a = [((i * 11 + la * 3) % 17) - 8 for i in range(la)]
        b = [((i * 7 + lb) % 13) - 6 for i in range(lb)]
        c = [((i * 3 + lc) % 11) - 5 for i in range(lc)]
        yield [a, b, c, n, m, xs[(la + lb + lc + n + m + flag) % len(xs)], flag]
    # Boundary scalar parameters never become Python-unbounded arithmetic.
    for n, m, x in itertools.product((-(1 << 31), -(1 << 31)+1, (1 << 31)-2, (1 << 31)-1), repeat=3):
        yield [[3, -1, 5], [], [9], n, m, x, False]
    # Same-storage root challenges are an overapproximation of source borrowing.
    for length in range(4):
        a = list(range(length))
        yield [a, a, a, length - 1, 0, 7, True]


def reference(name, args):
    a, b, c, n, m, x, flag = args
    if name == 'sum':
        return ('return', sum(a)), list(a)
    if name == 'fill':
        return ('return', x), [x] * len(a)
    if name == 'sort':
        return ('return', x), sorted(a)
    if name == 'reverse':
        return ('return', x), list(reversed(a))
    if name in ('store_order', 'index_order'):
        if name == 'index_order':
            if n == (1 << 31) - 1:
                return ('trap', 101), list(a)
            n += 1
        return ('trap', 102 if 0 <= n < len(a) else 103), list(a)
    if name in ('empty_high_traps', 'low_trap_first'):
        return ('trap', 102), list(a)
    if name == 'checked_index':
        if 0 <= n < len(a):
            return ('trap', 101 if n else 103), list(a)
        return ('return', x), list(a)
    if name in ('short_and', 'short_or'):
        after = list(a)
        valid = 0 <= n < len(a)
        if valid and ((a[n] > 0) if name == 'short_and' else (a[n] <= 0)):
            after[n] = x
        return ('return', x), after
    if name in ('wide_overflow', 'wide_negate', 'wide_min_div', 'wide_min_rem'):
        after = list(a)
        if 0 <= n < len(a):
            if (name == 'wide_overflow' and x == (1 << 63) - 1) or (name != 'wide_overflow' and x == -(1 << 63)):
                return ('trap', 101), after
            after[n] = x + 1 if name == 'wide_overflow' else 0 if name == 'wide_min_rem' else -x
        return ('return', x), after
    return None


def interpreter_selfcheck():
    # The safety assertion must reject a deliberately wrong proof rather than
    # quietly taking the ordinary checked-bounds trap.
    nodes = [dict(kind=1, type=1, value=0),
             dict(kind=6, type=2, value=0, left=0, local=0, proof=2),
             dict(kind=15, left=1)]
    model = dict(root=2, nodes=nodes, locals=[dict(type=20, param=0)])
    try:
        Machine(model, [[]], 100).run()
    except AssertionError as error:
        assert error.args[0][0] == 'UNSOUND BOUNDS PROOF', error
    else:
        raise AssertionError('interpreter accepted injected unsafe proof')
    # Nontermination must escape as Fuel, never a normal/checked-trap result.
    loop = dict(root=2, nodes=[dict(kind=1, type=3, value=1),
                              dict(kind=10, left=-1), dict(kind=14, left=0, right=1)], locals=[])
    try:
        Machine(loop, [], 12).run()
    except Fuel:
        pass
    else:
        raise AssertionError('interpreter accepted incomplete execution')
    for ty, minimum in ((1, -(1 << 31)), (2, -(1 << 63))):
        for value in (minimum - 1, -minimum):
            try:
                checked(value, ty)
            except Trap as trap:
                assert trap.args == (101,)
            else:
                raise AssertionError(('interpreter accepted overflow', ty, value))


def run(command, work):
    result = subprocess.run([str(x) for x in command], cwd=work, env={'PATH': '/no-external-tools'},
                            capture_output=True, timeout=120)
    assert result.returncode == 0, (command, result.returncode, result.stdout[-1000:], result.stderr)
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--source-root', type=Path, default=ROOT, help='checkout containing the actual opt_range module')
    parser.add_argument('--fuel', type=int, default=5000, help='node evaluations per input; exhausted inputs fail as INCOMPLETE')
    parser.add_argument('--case', help='case-name substring filter')
    parser.add_argument('--report', type=Path, help='optional JSON evidence including exact exported models')
    args = parser.parse_args()
    assert args.fuel > 0
    interpreter_selfcheck()
    compiler, source_root = args.compiler.resolve(strict=True), args.source_root.resolve(strict=True)
    hashes = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in
              [compiler, *(source_root / 'selfhost' / (n + '.ddr') for n in
                ('opt_ir', 'opt_parse', 'opt_pass', 'opt_range', 'opt_verify', 'lexer'))]}
    report = dict(complete=False, hashes=hashes, fuel=args.fuel, cases={},
                  limitations=['finite frozen input domain', 'fuel limit', 'interpreter, not target machine code',
                               'same-storage root inputs overapproximate source borrow capabilities'])
    total_runs = total_accesses = marked_reached = incomplete = maximum = 0
    chosen = {name: body for name, body in cases().items() if not args.case or args.case in name}
    assert chosen, 'no selected cases'
    with tempfile.TemporaryDirectory(prefix='dunder-range-qa-') as tmp:
        work = Path(tmp).resolve()
        fixture = work / 'export.ddr'; fixture.write_text(exporter(source_root, work))
        image = work / 'export'
        run([compiler, 'build', fixture, '-o', image], work)
        for name, body in chosen.items():
            source = work / (name + '.ddr'); source.write_text(body)
            models = {}
            for mode in ('raw', 'simplified'):
                for budget in (0, 64, 131072):
                    model = decode(run([image, source, budget, mode], work))
                    assert model['status'] != 4, (name, mode, budget, 'invalid model/invariant')
                    if budget == 131072:
                        assert model['status'] == 2, (name, mode, 'production budget exhausted')
                    models[f'{mode}_{budget}'] = model
            proof_nodes = {key: sum(bool(n['proof'] & 2) for n in model['nodes']) for key, model in models.items()}
            reached = set()
            case_incomplete = 0
            for arguments in inputs():
                outcomes = []
                for mode in ('raw', 'simplified'):
                    model = models[f'{mode}_131072']
                    machine = Machine(model, arguments, args.fuel)
                    try:
                        outcome = machine.run()
                    except Fuel:
                        incomplete += 1; case_incomplete += 1
                        continue
                    except AssertionError as error:
                        report['failure'] = dict(case=name, mode=mode, source=body,
                                                 arguments=arguments, model=model, error=repr(error))
                        if args.report:
                            args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
                        raise AssertionError((name, mode, arguments, error.args)) from error
                    expected = reference(name, arguments)
                    if expected:
                        assert (outcome, machine.values[0]) == expected, (name, arguments, outcome, machine.values[0], expected)
                    outcomes.append((outcome, [machine.values[i] for i in (0, 1, 2)]))
                    reached.update((mode, node) for node in machine.marked)
                    total_runs += 1; total_accesses += machine.accesses
                    maximum = max(maximum, machine.steps)
                if len(outcomes) == 2:
                    assert outcomes[0] == outcomes[1], (name, arguments, 'raw/simplified mismatch', outcomes)
            marked_reached += len(reached)
            report['cases'][name] = dict(source=body, models=models, proof_nodes=proof_nodes,
                                         reached_proof_nodes=len(reached), incomplete=case_incomplete)
            print('INCOMPLETE' if case_incomplete else 'PASS', name,
                  f'proofs(raw/simplified)={proof_nodes["raw_131072"]}/{proof_nodes["simplified_131072"]}',
                  f'reached={len(reached)} incomplete={case_incomplete}', flush=True)
    assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest() == h for p, h in hashes.items()), 'input compiler/module changed'
    assert args.case or marked_reached > 0, 'no bounds proof was dynamically exercised'
    report.update(complete=incomplete == 0, runs=total_runs, accesses=total_accesses,
                  reached_proof_nodes=marked_reached, incomplete=incomplete, maximum_completed_fuel=maximum)
    if args.report:
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(f'{total_runs} completed interpreter runs; {total_accesses} accesses; {marked_reached} reached proof nodes; '
          f'{incomplete} INCOMPLETE; maximum completed fuel {maximum}/{args.fuel}')
    raise SystemExit(2 if incomplete else 0)


if __name__ == '__main__':
    main()
