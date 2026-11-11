#!/usr/bin/env python3
"""Interpret actual Dunder IR and challenge reached bounds/i32 overflow proofs.

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
               'local', 'proof', 'effect', 'start', 'pending', 'pending_mask')
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
    assert(f.nodes.kinds.len==before.nodes.kinds.len && f.local_types.len==before.local_types.len)
    for i in 0..f.nodes.kinds.len{
        assert(f.nodes.kinds[i]==before.nodes.kinds[i] && f.nodes.types[i]==before.nodes.types[i] && f.nodes.left[i]==before.nodes.left[i] && f.nodes.right[i]==before.nodes.right[i] && f.nodes.third[i]==before.nodes.third[i] && f.nodes.next[i]==before.nodes.next[i])
        assert(f.nodes.values[i]==before.nodes.values[i] && f.nodes.locals[i]==before.nodes.locals[i] && f.nodes.proofs[i]==before.nodes.proofs[i] && f.nodes.effects[i]==before.nodes.effects[i] && f.nodes.starts[i]==before.nodes.starts[i])
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
    print(2);print(f.root);print(f.ret);print(f.params);print(f.nodes.kinds.len);print(f.local_types.len)
    print(ranges.status(&w));print(ranges.units(&w));print(ranges.read_count(&w));print(ranges.store_count(&w));print(f.visits);print(f.rewrites)
    print(ranges.arithmetic_count(&w))
    for i in 0..f.nodes.kinds.len{
        mask:=ranges.proof_mask(&w,i)
        assert(ranges.produced(&w,i)==(mask.bit_and(2)!=0))
        assert(f.nodes.proofs[i]==before.nodes.proofs[i].bit_or(mask))
        print(f.nodes.kinds[i]);print(f.nodes.types[i]);print(f.nodes.left[i]);print(f.nodes.right[i]);print(f.nodes.third[i]);print(f.nodes.next[i]);print(f.nodes.values[i]);print(f.nodes.locals[i]);print(f.nodes.proofs[i]);print(f.nodes.effects[i]);print(f.nodes.starts[i])
        if ranges.produced(&w,i){print(1)}else{print(0)}
        print(mask)
    }
    for i in 0..f.local_types.len{
        print(f.local_types[i]);print(f.local_param[i]);if f.local_mutable[i]{print(1)}else{print(0)};print(f.local_at[i])
    }
}
'''


def decode(output):
    numbers = list(map(int, output.splitlines()))
    assert numbers[0] == 2 and len(numbers) >= 13, 'invalid export header'
    version, root, ret, params, nodes, locals_, status, units, reads, stores, visits, rewrites, arithmetic = numbers[:13]
    assert len(numbers) == 13 + nodes * len(NODE_FIELDS) + locals_ * len(LOCAL_FIELDS), 'invalid export size'
    at = 13
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
    assert sum(bool(n['pending_mask'] & 1) for n in ns) == arithmetic
    if status != 2:
        assert reads == stores == arithmetic == 0 and not any(n['pending_mask'] for n in ns), 'failed analysis leaked proofs'
    for n in ns:
        assert 0 <= n['pending_mask'] <= 3
        assert n['pending'] == bool(n['pending_mask'] & 2)
        if n['pending']:
            assert n['kind'] in (6, 18) and n['proof'] & 2
        if n['pending_mask'] & 1:
            assert n['type'] == 1 and n['proof'] & 1
            assert (n['kind'] == 3 and n['value'] in (1, 2, 3)) or (n['kind'] == 4 and n['value'] == 1)
    return dict(root=root, ret=ret, params=params, nodes=ns, locals=ls,
                status=status, units=units, reads=reads, stores=stores, arithmetic=arithmetic)


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
        self.arithmetic_marked = set()
        self.arithmetic_checks = 0
        self.new_arithmetic_marked = set()
        self.new_arithmetic_checks = 0
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

    def arithmetic(self, node, value):
        # Operands have already completed. Check the unbounded mathematical
        # result BEFORE the usual source overflow trap can mask a bad proof.
        # i64 remains checked and receives no range-analysis exemption here.
        if node['type'] == 1 and node.get('proof', 0) & 1:
            self.arithmetic_checks += 1
            assert -(1 << 31) <= value < 1 << 31, (
                'UNSOUND ARITHMETIC PROOF', node, value, self.values)
            self.arithmetic_marked.add(id(node))
            if node.get('pending_mask', 0) & 1:
                self.new_arithmetic_marked.add(id(node))
                self.new_arithmetic_checks += 1
        return checked(value, node['type'])

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
            return not a if op == 2 else self.arithmetic(node, -a)
        assert kind == 3, node
        a = self.expression(node['left'])
        if op == 12:
            return bool(a and self.expression(node['right']))
        if op == 13:
            return bool(a or self.expression(node['right']))
        b = self.expression(node['right'])
        if op == 1:
            return self.arithmetic(node, a + b)
        if op == 2:
            return self.arithmetic(node, a - b)
        if op == 3:
            return self.arithmetic(node, a * b)
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


def arithmetic_cases():
    # Separate from cases()/inputs(): do not silently enlarge or renumber the
    # published 55-case, 697400-run bounds cohort.
    return {
        'i32_add': '{value:=n+m;return x}',
        'i32_sub': '{value:=n-m;return x}',
        'i32_mul': '{value:=n*m;return x}',
        'i32_neg': '{value:=-n;return x}',
        'i32_guarded_add': '{if n>=-10&&n<=10&&m>=-10&&m<=10{value:=n+m};return x}',
        'i32_guarded_sub': '{if n>=-10&&n<=10&&m>=-10&&m<=10{value:=n-m};return x}',
        'i32_guarded_mul': '{if n>=-46340&&n<=46340&&m>=-46340&&m<=46340{value:=n*m};return x}',
        'i32_unsafe_mul_guard': '{if n>=-46341&&n<=46341&&m>=-46341&&m<=46341{value:=n*m};return x}',
        'i32_guarded_neg': '{if n>-2147483648{value:=-n};return x}',
        'i32_child_add_then_zero': '{value:=(n+1)*0;return x}',
        'i32_child_sub_then_restore': '{value:=(n-1)+1;return x}',
        'i32_double_neg': '{value:=-(-n);return x}',
        'i32_const_child_mul_zero': '{value:=(2147483647*2147483647)*0;return x}',
        'i32_const_zero_mul_child': '{value:=0*(2147483647*2147483647);return x}',
        'i32_const_child_mul_neg': '{value:=-(2147483647*2147483647);return x}',
        'i32_const_child_mul_add': '{value:=(2147483647*2147483647)+1;return x}',
        'i32_const_child_mul_sub': '{value:=1-(2147483647*2147483647);return x}',
        'i32_const_min_mul_zero': '{value:=(-2147483648*(-1))*0;return x}',
        'i32_const_min_neg_zero': '{value:=(-(-2147483648))*0;return x}',
        'i32_const_child_add_zero': '{value:=(2147483647+1)*0;return x}',
        'i32_const_child_sub_zero': '{value:=(-2147483648-1)*0;return x}',
        'i32_const_child_before_divisor': '{value:=(2147483647*2147483647)/(n+1);return x}',
        'i32_bounds_after_child': '{if n>=0&&n<a.len{value:=(n+m)*0;a[value]=x};return x}',
        'i32_later_backedge': '{var value:=n;for i in 0..3{value=value+1};return x}',
        'i32_guarded_backedge': '{if n<=2147483645{var value:=n;for i in 0..3{value=value+1}};return x}',
        'i32_later_multiply': '{var value:=n;var i:=0;while i<3{value=value*2;i=i+1};return x}',
        'i32_short_or': '{if flag||n+1>0{if n>=0&&n<a.len{a[n]=x}};return x}',
        'i32_short_and': '{if !flag&&n*m>=0{if n>=0&&n<a.len{a[n]=x}};return x}',
        'i32_unreachable': '{if false{value:=n+1};return x}',
        'i32_budget_prefix': '{if n>=0&&n<100{var value:=n;for i in 0..8{value=value+1};if value<a.len{a[value]=x}};return x}',
        'i64_checked_add': '{value:=x+1;return value}',
        'i64_checked_sub': '{value:=x-1;return value}',
        'i64_checked_mul': '{value:=x*3;return value}',
        'i64_checked_neg': '{value:=-x;return value}',
    }


def arithmetic_inputs():
    values = (-(1 << 31), -(1 << 31)+1, -(1 << 31)+2, -1073741824,
              -65536, -46341, -46340, -2, -1, 0, 1, 2, 46340, 46341,
              65536, 1073741823, (1 << 31)-3, (1 << 31)-2, (1 << 31)-1)
    arrays = (([], [], []), ([3, -1, 7], [9], [5, 8]), ([0, -3], [4, 6, 9], []))
    wide = (-(1 << 63), -(1 << 63)+1, -1, 0, 1, (1 << 63)-1)
    for i, (n, m, group, flag) in enumerate(itertools.product(values, values, arrays, (False, True))):
        yield [*group, n, m, wide[i % len(wide)], flag]


def reference(name, args):
    a, b, c, n, m, x, flag = args
    if name.startswith('i32_const_'):
        return ('trap', 101), list(a)
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
    calculations = {'i32_add': (1, n+m), 'i32_sub': (1, n-m),
                    'i32_mul': (1, n*m), 'i32_neg': (1, -n),
                    'i64_checked_add': (2, x+1), 'i64_checked_sub': (2, x-1),
                    'i64_checked_mul': (2, x*3), 'i64_checked_neg': (2, -x)}
    if name in calculations:
        ty, result = calculations[name]
        width = 32 if ty == 1 else 64
        if not -(1 << (width-1)) <= result < 1 << (width-1):
            return ('trap', 101), list(a)
        return ('return', result if ty == 2 else x), list(a)
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
    # Each arithmetic opcode must reject a bad proof BEFORE the ordinary
    # overflow trap could accidentally make a broken proof look correct.
    for kind, op, a, b in ((3, 1, (1 << 31)-1, 1), (3, 2, -(1 << 31), 1),
                           (3, 3, -(1 << 31), -1), (3, 3, 46341, 46341),
                           (4, 1, -(1 << 31), 0)):
        expression = dict(kind=kind, type=1, value=op, left=0, right=1, proof=1)
        nodes = [dict(kind=1, type=1, value=a), dict(kind=1, type=1, value=b),
                 expression, dict(kind=15, left=2)]
        model = dict(root=3, nodes=nodes, locals=[])
        try:
            Machine(model, [], 100).run()
        except AssertionError as error:
            assert error.args[0][0] == 'UNSOUND ARITHMETIC PROOF', error
        else:
            raise AssertionError('interpreter accepted injected arithmetic proof')
    # A parent may be safe on normal continuation even when a child traps.
    # Proving the parent's (n+1)*0 must never skip the child's source overflow.
    child = dict(kind=3, type=1, value=1, left=0, right=1, proof=0)
    nodes = [dict(kind=1, type=1, value=(1 << 31)-1), dict(kind=1, type=1, value=1),
             child, dict(kind=1, type=1, value=0),
             dict(kind=3, type=1, value=3, left=2, right=3, proof=1), dict(kind=15, left=4)]
    machine = Machine(dict(root=5, nodes=nodes, locals=[]), [], 100)
    assert machine.run() == ('trap', 101) and machine.arithmetic_checks == 0
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
              [compiler, Path(__file__).resolve(), *(source_root / 'selfhost' / (n + '.ddr') for n in
                ('opt_ir', 'opt_parse', 'opt_pass', 'opt_range', 'opt_verify', 'lexer'))]}
    report = dict(complete=False, hashes=hashes, fuel=args.fuel, cases={}, cohorts={},
                  limitations=['finite frozen input domain', 'fuel limit', 'interpreter, not target machine code',
                               'same-storage root inputs overapproximate source borrow capabilities'])
    total_runs = total_accesses = marked_reached = incomplete = maximum = 0
    total_arithmetic = total_new_arithmetic = 0
    chosen = [(cohort, name, body) for cohort, collection in
              (('bounds_original', cases()), ('arithmetic_boundaries', arithmetic_cases()))
              for name, body in collection.items() if not args.case or args.case in name]
    assert chosen, 'no selected cases'
    with tempfile.TemporaryDirectory(prefix='dunder-range-qa-') as tmp:
        work = Path(tmp).resolve()
        fixture = work / 'export.ddr'; fixture.write_text(exporter(source_root, work))
        image = work / 'export'
        run([compiler, 'build', fixture, '-o', image], work)
        for cohort, name, body in chosen:
            stats = report['cohorts'].setdefault(cohort, dict(cases=0, runs=0, accesses=0,
                reached_bounds_nodes=0, reached_arithmetic_nodes=0, reached_new_arithmetic_nodes=0,
                arithmetic_checks=0, new_arithmetic_checks=0, incomplete=0, maximum_completed_fuel=0, outcomes={}))
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
            arithmetic_proof_nodes = {key: sum(n['type'] == 1 and bool(n['proof'] & 1) for n in model['nodes'])
                                      for key, model in models.items()}
            reached = set()
            arithmetic_reached, new_arithmetic_reached = set(), set()
            case_incomplete = 0
            case_arithmetic = case_new_arithmetic = 0
            case_outcomes = {}
            for arguments in (inputs() if cohort == 'bounds_original' else arithmetic_inputs()):
                outcomes = []
                for mode in ('raw', 'simplified'):
                    model = models[f'{mode}_131072']
                    machine = Machine(model, arguments, args.fuel)
                    try:
                        outcome = machine.run()
                    except Fuel:
                        incomplete += 1; case_incomplete += 1
                        stats['incomplete'] += 1
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
                    arithmetic_reached.update((mode, node) for node in machine.arithmetic_marked)
                    new_arithmetic_reached.update((mode, node) for node in machine.new_arithmetic_marked)
                    case_arithmetic += machine.arithmetic_checks
                    case_new_arithmetic += machine.new_arithmetic_checks
                    total_runs += 1; total_accesses += machine.accesses
                    outcome_key = 'return' if outcome[0] == 'return' else f'trap_{outcome[1]}'
                    case_outcomes[outcome_key] = case_outcomes.get(outcome_key, 0) + 1
                    stats['outcomes'][outcome_key] = stats['outcomes'].get(outcome_key, 0) + 1
                    stats['runs'] += 1; stats['accesses'] += machine.accesses
                    stats['maximum_completed_fuel'] = max(stats['maximum_completed_fuel'], machine.steps)
                    maximum = max(maximum, machine.steps)
                if len(outcomes) == 2:
                    assert outcomes[0] == outcomes[1], (name, arguments, 'raw/simplified mismatch', outcomes)
            marked_reached += len(reached)
            stats['cases'] += 1
            stats['reached_bounds_nodes'] += len(reached)
            stats['reached_arithmetic_nodes'] += len(arithmetic_reached)
            stats['reached_new_arithmetic_nodes'] += len(new_arithmetic_reached)
            stats['arithmetic_checks'] += case_arithmetic
            stats['new_arithmetic_checks'] += case_new_arithmetic
            total_arithmetic += case_arithmetic
            total_new_arithmetic += case_new_arithmetic
            report['cases'][name] = dict(cohort=cohort, source=body, models=models, proof_nodes=proof_nodes,
                                         arithmetic_proof_nodes=arithmetic_proof_nodes,
                                         reached_proof_nodes=len(reached), reached_arithmetic_nodes=len(arithmetic_reached),
                                         reached_new_arithmetic_nodes=len(new_arithmetic_reached),
                                         arithmetic_checks=case_arithmetic, new_arithmetic_checks=case_new_arithmetic,
                                         outcomes=case_outcomes, incomplete=case_incomplete)
            print('INCOMPLETE' if case_incomplete else 'PASS', name,
                  f'proofs(raw/simplified)={proof_nodes["raw_131072"]}/{proof_nodes["simplified_131072"]}',
                  f'bounds_reached={len(reached)} arithmetic_reached={len(arithmetic_reached)}',
                  f'new_arithmetic_checks={case_new_arithmetic} incomplete={case_incomplete}', flush=True)
    assert all(hashlib.sha256(Path(p).read_bytes()).hexdigest() == h for p, h in hashes.items()), 'input compiler/module changed'
    assert args.case or incomplete or marked_reached > 0, 'no bounds proof was dynamically exercised'
    assert args.case or incomplete or total_new_arithmetic > 0, 'no new arithmetic range proof was dynamically exercised'
    report.update(complete=incomplete == 0, runs=total_runs, accesses=total_accesses,
                  reached_proof_nodes=marked_reached, arithmetic_checks=total_arithmetic,
                  new_arithmetic_checks=total_new_arithmetic, incomplete=incomplete, maximum_completed_fuel=maximum)
    if args.report:
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
    print(f'{total_runs} completed interpreter runs; {total_accesses} accesses; {marked_reached} reached proof nodes; '
          f'{total_arithmetic} arithmetic proof checks ({total_new_arithmetic} new range); '
          f'{incomplete} INCOMPLETE; maximum completed fuel {maximum}/{args.fuel}')
    for name, stats in report['cohorts'].items():
        print(name, json.dumps(stats, sort_keys=True))
    raise SystemExit(2 if incomplete else 0)


if __name__ == '__main__':
    main()
