#!/usr/bin/env python3
"""Check scalar optimization through consumer programs and a legacy compiler."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import random
import shutil
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
ENV = {'PATH': '/no-external-tools'}
WASM_RUNNER = '''const fs=require('node:fs');
const print=value=>console.log(value.toString());
const module=new WebAssembly.Module(fs.readFileSync(process.argv[1]));
const program=new WebAssembly.Instance(module,{dunder:{
    print_i32:print,print_i64:print,print_bool:value=>print(Boolean(value)),
}}).exports;
try{const status=program.main();if(status!==undefined)process.exitCode=Number(status)}
catch(error){
    if(!(error instanceof WebAssembly.RuntimeError)||error.message!=='unreachable')throw error;
    process.exitCode=125;
}'''


def divide(a, b):
    return (abs(a) // abs(b)) * (-1 if (a < 0) != (b < 0) else 1)


def positive_cases():
    # Freeze both generated calls and references independently of compiler output.
    rng = random.Random(271828)
    arithmetic = 'fn calculate(a:i64,b:i64,c:i64)->i64{var v:=a+b*c;v=v-(a-b);v/c+a%c}\n'
    calls, expected = [], []
    for _ in range(48):
        a, b = rng.randrange(-100, 101), rng.randrange(-100, 101)
        c = rng.choice([-13, -7, -3, -1, 1, 2, 9, 17])
        calls.append(f'print(calculate({a},{b},{c}))')
        expected.append(divide(a+b*c-(a-b), c) + a-divide(a,c)*c)
    arithmetic += 'fn main(){' + ';'.join(calls) + '}\n'
    yield 'arithmetic', arithmetic, expected

    control = '''fn calculate(n:i32)->i32{
        var i:=0; var total:=0
        while i<n {
            i=i+1
            if i%2==0 {continue}
            if i>11 {break}
            {x:=i+2;total=total+x}
        }
        if n<0{return -7}else{return total}
    }
    fn earlier(n:i32)->i32{
        var i:=0;var total:=0
        while i<n{if i==7{return total};total=total+i;i=i+1}
        return total
    }
    fn shadow(x:i32)->i32{var total:=x;{x:=x+2;total=total+x};return total+x}
    fn both(x:i32)->i32{if x<0{return -1}else{return 1}}
    fn flip(n:i32)->bool{var i:=0;var value:=false;while i<n{value=!value;i=i+1};return value}
    fn nested(n:i32)->i32{
        var i:=0;var total:=0
        while i<n{var j:=0;while j<n{j=j+1;if j==2{continue};if j==5{break};total=total+i+j};i=i+1}
        return total
    }
    fn main(){'''
    expected = []
    for n in [-17, -1, 0, 1, 2, 3, 6, 7, 8, 11, 12, 17]:
        control += f'print(calculate({n}));print(earlier({n}));print(shadow({n}));print(both({n}));print(nested({n}));print(flip({n}));'
        total = -7 if n < 0 else sum(i+2 for i in range(1, min(n, 11)+1) if i % 2)
        nested = sum(i+j for i in range(max(n,0)) for j in range(1,min(n,4)+1) if j != 2)
        expected += [total, sum(range(min(max(n,0),7))), n*3+2, -1 if n < 0 else 1, nested, max(n,0)%2 == 1]
    yield 'control', control + '}\n', expected

    # Eighteen mutable values remain live across every backedge and at exit.
    # Distinct weighted incoming parameters also catch entry-copy overlap.
    spills = 'fn pressure(n:i64,seed:i64)->i64{'
    spills += ''.join(f'var a{i}:=seed+{i};' for i in range(18))
    spills += 'var i:i64=0;while i<n{'
    spills += ''.join(f'a{j}=a{j}+i+{j+1};' for j in range(18))
    spills += 'i=i+1};return ' + '+'.join(f'a{i}*{i+1}' for i in range(18)) + '}\n'
    spills += 'fn eight(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{a+b*2+c*3+d*4+e*5+f*6+g*7+h*8}\nfn main(){'
    expected = []
    for n in [-3, 0, 1, 2, 9, 16]:
        for seed in [-7, 0, 13]:
            spills += f'print(pressure({n},{seed}));'
            values = [seed+i for i in range(18)]
            for i in range(max(n,0)):
                values = [value+i+j+1 for j,value in enumerate(values)]
            expected.append(sum(value*(j+1) for j,value in enumerate(values)))
    for _ in range(12):
        values = [rng.randrange(-21,22) for _ in range(8)]
        spills += 'print(eight(' + ','.join(map(str,values)) + '));'
        expected.append(sum(value*(i+1) for i,value in enumerate(values)))
    yield 'register_pressure', spills + '}\n', expected

    arrays = '''fn reduce(values:&[]i64)->i64{
        var i:=0;var total:i64=0
        while i<values.len{total=total+values[i];i=i+1}
        return total
    }
    fn choose(values:&[]i64,n:i32)->i64{
        var i:=0;var total:i64=0
        while i<values.len{if i==n{return total};if i%2==0{total=total+values[i]};i=i+1}
        return total
    }
    // This requires legacy fallback: reading a borrowed array into an owned
    // binding must keep value semantics rather than copying its raw reference.
    fn copied(values:&[]i64)->i64{var copy:=values;copy[0]=123;return values[0]+copy[0]}
    fn main(){'''
    expected = []
    for values in [[], [0], [-9], [-4,7,0,19,-31], [i*7-31 for i in range(35)]]:
        arrays += '{var values:[]i64=[' + ','.join(map(str,values)) + '];'
        arrays += 'print(reduce(&values));'
        expected.append(sum(values))
        for n in [-1,0,1,3,len(values),len(values)+1]:
            arrays += f'print(choose(&values,{n}));'
            expected.append(sum(v for i,v in enumerate(values) if i%2==0 and (n<0 or i<n)))
        if values:
            arrays += 'saved:=values;values[0]=99;print(reduce(&saved));print(reduce(&values));print(copied(&saved));'
            expected += [sum(values), 99+sum(values[1:]), values[0]+123]
        arrays += '}'
    yield 'readonly_arrays', arrays + '}\n', expected
    yield 'fallback_owned_copy', '''fn copied(a:&[]i64)->i64{var b:=a;b[0]=123;return a[0]+b[0]}
        fn main(){a:[]i64=[7,11];print(copied(&a));print(a[0])}\n''', [130,7]
    yield 'fallback_stack_arguments', '''fn nine(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64,i:i64)->i64{
        a+b*2+c*3+d*4+e*5+f*6+g*7+h*8+i*9}
        fn main(){print(nine(1,2,3,4,5,6,7,8,9))}\n''', [285]

    booleans = '''fn guard(x:i64,y:i64)->bool{(x==0||y/x>2)&&(x!=0||y>=0)}
    fn other(x:i64)->bool{ten:i64=10;x!=0&&ten/x>0}
    fn minimum()->i32{-2147483648}
    fn main(){'''
    expected = []
    for x in [-7,-1,0,1,7]:
        for y in [-21,-1,0,1,21]:
            booleans += f'print(guard({x},{y}));'
            expected.append((x==0 or divide(y,x)>2) and (x!=0 or y>=0))
        booleans += f'print(other({x}));'
        expected.append(x!=0 and divide(10,x)>0)
    booleans += 'print(minimum());}\n'
    expected.append(-2147483648)
    yield 'short_circuit', booleans, expected


def trap_cases():
    # Native exit codes distinguish which checked operation trapped first. Wasm
    # has a single unreachable trap, so its independent check is the prefix.
    cases = [
        ('add_i32', 101, 'fn f(x:i32)->i32{x+1}', 'f(2147483647)'),
        ('sub_i32', 101, 'fn f(x:i32)->i32{x-1}', 'f(-2147483648)'),
        ('neg_i32', 101, 'fn f(x:i32)->i32{-x}', 'f(-2147483648)'),
        ('add_i64', 101, 'fn f(x:i64)->i64{x+1}', 'f(9223372036854775807)'),
        ('mul_i64', 101, 'fn f(x:i64)->i64{x*x}', 'f(3037000500)'),
        ('neg_i64', 101, 'fn f(x:i64)->i64{-x}', 'f(-9223372036854775808)'),
        ('division_min_i32', 101, 'fn f(x:i32,y:i32)->i32{x/y}', 'f(-2147483648,-1)'),
        ('division_min_i64', 101, 'fn f(x:i64,y:i64)->i64{x/y}', 'f(-9223372036854775808,-1)'),
        ('remainder_min', 101, 'fn f(x:i64,y:i64)->i64{x%y}', 'f(-9223372036854775808,-1)'),
        ('division_zero', 102, 'fn f(x:i64)->i64{19/x}', 'f(0)'),
        ('overflow_before_zero', 101, 'fn f(x:i64)->i64{(9223372036854775807+x)/(x-x)}', 'f(1)'),
        ('zero_before_overflow', 102, 'fn f(x:i64)->i64{1/(x-x)+(9223372036854775807+x)}', 'f(1)'),
        ('empty_index', 103, 'fn f(a:&[]i64)->i64{a[0]}', 'f(&empty)'),
        ('negative_index', 103, 'fn f(a:&[]i64,x:i32)->i64{a[x]}', 'f(&values,-1)'),
        ('index_overflow_first', 101, 'fn f(a:&[]i64,x:i32)->i64{a[x+1]}', 'f(&values,2147483647)'),
        ('modified_induction', 103, 'fn f(a:&[]i64)->i64{var i:=0;while i<a.len{i=i-1;return a[i]};return 0}', 'f(&values)'),
        ('off_by_one', 103, 'fn f(a:&[]i64)->i64{var i:=0;var total:i64=0;while i<a.len{total=total+a[i+1];i=i+1};return total}', 'f(&values)'),
        ('checked_reduction', 101, 'fn f(a:&[]i64)->i64{var i:=0;var total:i64=0;while i<a.len{total=total+a[i];i=i+1};return total}', 'f(&large)'),
    ]
    for name,status,function,expression in cases:
        source = function + '\nfn main(){empty:[]i64=[];values:[]i64=[3,5];large:[]i64=[9223372036854775807,1,-1];print(17);value:=' + expression + ';print(99)}\n'
        yield name, source, status


def invalid_cases():
    bodies = {
        'dead_type': 'if false{return true};return x',
        'dead_immutable': 'value:=x;if false{value=3};return value',
        'dead_unknown': 'if false{return missing};return x',
        'dead_literal_width': 'if false{return 2147483648};return x',
        'dead_duplicate': 'if false{value:=1;value:=2};return x',
        'dead_reserved': 'if false{i32:=x};return x',
        'dead_non_ascii': 'if false{å:=x};return x',
        'dead_syntax': 'if false{var value:=};return x',
        'dead_else_syntax': 'if true{return x}else{var value:=};return x',
        'dead_loop_body': 'while false{var value:bool=x};return x',
        'after_return_type': 'return x;return true',
        'dead_condition': 'if false{if 1{return x}};return x',
        'dead_break': 'if false{break};return x',
        'dead_continue': 'if false{continue};return x',
        'parameter_assignment': 'if false{x=0};return x',
        'parameter_shadow': 'x:=x+1;return x',
        'missing_return': 'if x>0{return 1}',
        'escaped_scope': '{value:=x};return value',
        'grouped_min_literal': 'return -(2147483648)',
        'wrong_local_annotation': 'var value:bool=x;return x',
        'bool_ordering': 'if true<false{return x};return x',
    }
    for name,body in bodies.items():
        yield name, f'fn f(x:i32)->i32{{{body}}}\nfn main(){{print(f(1))}}\n'
    yield 'duplicate_parameters', 'fn f(x:i32,x:i32)->i32{x}\nfn main(){print(f(1,2))}\n'
    yield 'shared_array_assignment', 'fn f(a:&[]i64)->i64{if false{a[0]=1};return 0}\nfn main(){a:[]i64=[0];print(f(&a))}\n'
    yield 'array_boolean_index', 'fn f(a:&[]i64)->i64{if false{return a[true]};return 0}\nfn main(){a:[]i64=[0];print(f(&a))}\n'
    yield 'legacy_literal_context', 'fn f(x:i64)->bool{if false{return 10/x>0};return true}\nfn main(){print(f(1))}\n'
    yield 'excessive_nesting', 'fn f(x:i32)->i32{' + '('*140 + 'x' + ')'*140 + '}\nfn main(){print(f(1))}\n'
    # Each level enters both expression() and primary(). Checking only the
    # parser's primary depth would incorrectly accept this foldable program.
    nested = '1'
    for _ in range(43):
        nested = '1+(' + nested + ')'
    yield 'combined_expression_depth', f'fn f()->i32{{{nested}}}\nfn main(){{print(f())}}\n'
    yield 'dead_combined_expression_depth', f'fn unused()->i32{{{nested}}}\nfn main(){{print(42)}}\n'


def call(args, *, cwd, env=None):
    return subprocess.run([str(a) for a in args], cwd=cwd, env=env,
                          capture_output=True, timeout=60)


def build(compiler, source, image, target, work, flags=()):
    result = call([compiler, 'build', source, '--target', target, '-o', image, *flags], cwd=work, env=ENV)
    if result.returncode:
        raise AssertionError(f'{compiler} build {source.name} ({target}): {result.returncode}\n{result.stderr.decode(errors="replace")}')
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler', type=Path)
    parser.add_argument('--baseline', type=Path,
                        help='older authoritative compiler; default: current compiler with --scalar-opt=off')
    parser.add_argument('--node', default='node')
    parser.add_argument('--verify-selection', action='store_true',
                        help='also compare candidate --scalar-opt=off and assert optimizer timing counters')
    args = parser.parse_args()
    candidate = args.compiler.resolve(strict=True)
    compilers = {'baseline': args.baseline.resolve(strict=True) if args.baseline else candidate,
                 'candidate': candidate}
    variants = [('baseline', compilers['baseline'], [] if args.baseline else ['--scalar-opt=off']),
                ('candidate', compilers['candidate'], [])]
    if args.verify_selection and args.baseline:
        variants.append(('candidate_off', compilers['candidate'], ['--scalar-opt=off']))
    node = shutil.which(args.node)
    if not node:
        parser.error('Node is required for Wasm execution')
    machine = platform.machine()
    native = 'arm64-macos' if platform.system() == 'Darwin' else ('arm64-linux' if machine in ('arm64','aarch64') else 'x86_64-linux')
    targets = list(dict.fromkeys([native, 'wasm32', 'arm64-linux', 'x86_64-linux']))
    hashes = {v: hashlib.sha256(p.read_bytes()).hexdigest() for v,p in compilers.items()}
    executed = crossed = diagnostics = 0
    with tempfile.TemporaryDirectory(prefix='dunder-optimizer-') as tmp:
        work = Path(tmp)
        programs = list(positive_cases())
        programs.append(('fixture', (ROOT / 'tests/contracts/scalar_optimizer.ddr').read_text(), [42]))
        for name,source,expected in programs:
            path = work / (name + '.ddr'); path.write_text(source)
            output = ''.join((str(value).lower() if type(value) is bool else str(value))+'\n' for value in expected).encode()
            for variant,compiler,flags in variants:
                for target in targets:
                    image = work / f'{name}-{variant}-{target}'
                    build_flags = flags + (['--timings=json'] if args.verify_selection and variant != 'baseline' else [])
                    built = build(compiler,path,image,target,work,build_flags)
                    if args.verify_selection and variant != 'baseline':
                        optimizer = json.loads(built.stderr)['optimizer']
                        assert optimizer['enabled'] is (variant == 'candidate'), optimizer
                        if variant == 'candidate' and not name.startswith('fallback_'):
                            minimum = 2 if name == 'register_pressure' else 1
                            assert optimizer['modeled'] >= minimum and optimizer['emitted'] >= minimum, (name,target,optimizer)
                        else:
                            assert optimizer['modeled'] == 0 and optimizer['emitted'] == 0, (name,target,optimizer)
                    if target == native or target == 'wasm32':
                        command = [image] if target == native else [node, '-e', WASM_RUNNER, image]
                        result = call(command,cwd=work)
                        if result.returncode != 0 or result.stdout != output:
                            raise AssertionError(f'{variant} {name} {target}: exit {result.returncode}, expected {output!r}, got {result.stdout!r}\n{result.stderr.decode(errors="replace")}')
                        executed += 1
                    else:
                        data = image.read_bytes()
                        assert data[:6] == b'\x7fELF\x02\x01'
                        assert struct.unpack_from('<H',data,18)[0] == (183 if target == 'arm64-linux' else 62)
                        crossed += 1
            for target in targets:
                baseline_records = None
                for variant,compiler,flags in variants:
                    checked = call([compiler,'check',path,'--target',target,'--agent',*flags],cwd=work,env=ENV)
                    if checked.returncode != 0 or checked.stderr:
                        raise AssertionError(f'{variant} check --agent {name} {target}: {checked.returncode}\n{checked.stderr!r}')
                    records = [json.loads(line) for line in checked.stdout.splitlines()]
                    assert records[-1]['kind'] == 'ok', records
                    if baseline_records is None:
                        baseline_records = records
                    elif records != baseline_records:
                        raise AssertionError(f'binding records changed for {variant} {name} {target}:\n{baseline_records!r}\n{records!r}')
            print('PASS results',name,flush=True)
        for name,source,status in trap_cases():
            path = work / (name + '.ddr'); path.write_text(source)
            for variant,compiler,flags in variants:
                for target in targets:
                    image = work / f'{name}-{variant}-{target}'
                    build(compiler,path,image,target,work,flags)
                    if target == native or target == 'wasm32':
                        command = [image] if target == native else [node, '-e', WASM_RUNNER, image]
                        result = call(command,cwd=work)
                        expected_status = status if target == native else 125
                        if result.returncode != expected_status or result.stdout != b'17\n':
                            raise AssertionError(f'{variant} {name} {target}: exit {result.returncode}, expected {expected_status}; stdout {result.stdout!r}; stderr {result.stderr!r}')
                        executed += 1
                    else:
                        crossed += 1
            print('PASS traps',name,flush=True)
        for name,source in invalid_cases():
            path = work / (name + '.ddr'); path.write_text(source)
            for target in targets:
                for command in ('check','check-agent','build'):
                    outcomes = []
                    for variant,compiler,extra in variants:
                        image = work / 'previous-output'; image.write_bytes(b'previous successful output')
                        mode = 'check' if command == 'check-agent' else command
                        flags = ['--agent'] if command == 'check-agent' else []
                        if command == 'build': flags += ['-o',str(image)]
                        result = call([compiler,mode,path,'--target',target,*flags,*extra],cwd=work,env=ENV)
                        if result.returncode != 1:
                            raise AssertionError(f'{variant} unexpectedly accepted invalid {name} {command} {target}: {result.returncode}\n{result.stdout!r}\n{result.stderr!r}')
                        if command == 'check-agent':
                            record = json.loads(result.stdout)
                            assert record['kind'] == 'error' and Path(record['path']).resolve() == path.resolve(), record
                            assert 0 <= record['start'] <= record['end'] <= path.stat().st_size
                        assert image.read_bytes() == b'previous successful output'
                        outcomes.append((result.returncode,result.stdout,result.stderr))
                        diagnostics += 1
                    if any(outcome != outcomes[0] for outcome in outcomes[1:]):
                        raise AssertionError(f'diagnostics changed for {name} {command} {target}:\n{outcomes!r}')
            print('PASS diagnostics',name,flush=True)
    assert all(hashlib.sha256(p.read_bytes()).hexdigest() == hashes[v] for v,p in compilers.items()), 'compiler changed during verification'
    print(f'{executed} native/Wasm executions; {crossed} Linux cross-builds (not executed); {diagnostics} matching invalid diagnostics')


if __name__ == '__main__':
    main()
