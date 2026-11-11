#!/usr/bin/env python3
"""Check, calibrate, compare or attribute four independent Dunder dogfood cohorts."""
import argparse
import copy
import csv
import hashlib
import json
import math
import platform
from pathlib import Path
import re
import statistics
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
COHORT = ROOT / 'benchmarks/dogfood'
ENV = {'PATH': '/no-external-tools'}
COUNTERS = ('allocation_count requested_bytes size_class_payload_bytes fresh_allocations '
            'reused_allocations retain_calls release_calls array_clone_calls array_clone_bytes '
            'string_slice_calls string_slice_bytes record_construct_calls record_construct_bytes '
            'record_clone_calls record_clone_bytes copy_bytes').split()
KEYWORDS = set('fn pub var mut if else while for in return break continue struct enum match import as true false'.split())
TOKEN = re.compile(r'''(?P<space>[ \t\r]+)|(?P<newline>\n)|(?P<line>//[^\n]*)|
    (?P<quote>"(?:\\.|[^"\\\r\n])*"|'(?:\\.|[^'\\\r\n])*'|`(?:\\.|[^`\\\r\n])*`)|
    (?P<number>[0-9][0-9_]*(?:\.[0-9][0-9_]*)?(?:[eE][+-]?[0-9][0-9_]*)?)|
    (?P<name>[A-Za-z_\u0080-\U0010ffff][A-Za-z_0-9\u0080-\U0010ffff]*)|
    (?P<symbol>:=|->|=>|==|!=|<=|>=|&&|\|\||\.\.|[()\[\]{}:=,.;@+*/%<>!&-])''', re.VERBOSE)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def decimal(text):
    # Positive arbitrary-precision accumulation is independent of Dunder's
    # negative bounded accumulator. Keep left-to-right error precedence.
    negative = text.startswith('-')
    digits = text[1:] if text.startswith(('+','-')) else text
    if not digits:
        return False,0,1
    limit = 2**63 if negative else 2**63-1
    value = 0
    previous = False
    for i,char in enumerate(digits):
        if char == '_':
            if not previous or i+1 == len(digits) or not '0' <= digits[i+1] <= '9':
                return False,0,1
            previous = False
        elif '0' <= char <= '9':
            value = value*10+ord(char)-48
            if value > limit:
                return False,0,2
            previous = True
        else:
            return False,0,1
    return True,-value if negative else value,0


def quoted(text):
    # Decode byte escapes before UTF-8, rather than interpreting \xNN as a
    # Unicode character. Unicode scalar escapes use Python's independent codec.
    body = text[1:-1]
    output = bytearray()
    i = 0
    escapes = {'n':'\n','r':'\r','t':'\t','0':'\0','\\':'\\','"':'"',"'":"'",'`':'`'}
    while i < len(body):
        char = body[i]; i += 1
        if char != '\\':
            output.extend(char.encode()); continue
        escape = body[i]; i += 1
        if escape in escapes:
            output.extend(escapes[escape].encode())
        elif escape == 'x':
            output.append(int(body[i:i+2],16)); i += 2
        elif escape == 'u':
            if body[i] == '{':
                end = body.index('}',i); scalar = int(body[i+1:end],16); i = end+1
            else:
                scalar = int(body[i:i+4],16); i += 4
            output.extend(chr(scalar).encode())
        else:
            raise ValueError('unsupported escape in frozen lexer input')
    return output.decode('utf-8')


def tokens(source):
    offsets = [0]
    for char in source:
        offsets.append(offsets[-1]+len(char.encode()))
    result = []
    cursor = depth = 0
    while cursor < len(source):
        # Nested comments are recognized independently of the token regex.
        if source.startswith('/*',cursor):
            nesting = 1; cursor += 2
            while nesting:
                if cursor >= len(source): raise ValueError('unclosed comment')
                if source.startswith('/*',cursor): nesting += 1; cursor += 2
                elif source.startswith('*/',cursor): nesting -= 1; cursor += 2
                else:
                    if source[cursor] == '\n' and depth == 0:
                        result.append((8,'\n',offsets[cursor],offsets[cursor+1]))
                    cursor += 1
            continue
        match = TOKEN.match(source,cursor)
        if not match:
            raise ValueError(f'unrecognized frozen lexer input at {cursor}: {source[cursor:cursor+20]!r}')
        group = match.lastgroup; spelling = match.group(); first = cursor; cursor = match.end()
        if group in ('space','line'): continue
        if group == 'newline':
            if depth: continue
            kind = 8
        elif group == 'name': kind = 2 if spelling in KEYWORDS else 1
        elif group == 'number': kind = 4 if any(c in spelling for c in '.eE') else 3
        elif group == 'quote':
            kind = 6 if spelling[0] == '`' else 5
            spelling = quoted(spelling)
        else:
            kind = 7
            if spelling in ('(','['): depth += 1
            elif spelling in (')',']') and depth: depth -= 1
        result.append((kind,spelling,offsets[first],offsets[cursor]))
    result.append((0,'',offsets[-1],offsets[-1]))
    return result


def reference(name,count,manifest):
    info = manifest['cases'][name]
    if name == 'pod':
        def consume(value):
            ok,left,right,weight = value
            return left*3+right*5+weight*11+(7 if ok else -7)
        answer = [0,0,0,0]
        for i in range(count):
            value = manifest['seed']+i%97; tag = i%11
            first = (value%3 != 0,value*17+tag,value-11,tag*2+1)
            changed = (not first[0],first[1]+29,first[2]-13,first[3]+3)
            refined = (first[0],(first[1]+first[2])*(1 if first[0] else -1),first[1]-first[2],first[3]+2)
            answer = [total+consume(item) for total,item in zip(answer,[first,tuple(first),changed,refined])]
        return answer
    raw = (COHORT / info['input']).read_text()
    if name == 'decimal':
        entries = [decimal(text) for text in raw.split('\n')]
        answer = [0]*6
        for cycle in range(count):
            for i in range(len(entries)):
                ok,value,error = entries[(i+cycle)%len(entries)]
                if ok:
                    answer[0] += 1
                    residue = value%1000003 if value >= 0 else -((-value)%1000003)
                    answer[3] += residue; answer[4] += residue*(i+1)
                else: answer[2 if error == 2 else 1] += 1
                answer[5] += error*(i+1)
        return answer
    if name == 'lexer':
        scanned = tokens(raw)
        names = {}; ids = 0; content = 0
        for i,(kind,text,_,_) in enumerate(scanned):
            if kind in (1,2):
                if text not in names: names[text] = len(names)
                ids += (names[text]+1)*(i+1)
            for byte in text.encode(): content = (content*131+byte)%2147483647
        return [len(scanned)*count,sum(k*(i+1) for i,(k,*_) in enumerate(scanned))*count,
                sum(start*3+end*5 for _,_,start,end in scanned)*count,ids*((count+1)//2),content*count]
    keys = raw.split('\n')
    if len(set(keys)) != len(keys) or 'not a stored symbol!' in keys:
        raise ValueError('symbol input must have unique keys and exclude missing sentinel')
    answer = [0]*5
    for cycle in range(count):
        table = {key: (-(i*17+cycle)-7 if i%3 == 0 else i*17+cycle) for i,key in enumerate(keys)}
        saved = dict(table)
        for i,key in enumerate(keys):
            if i%4 == 0: del table[key]; answer[3] += 1
            if i%5 == 0: table[key] = cycle-i*9
        answer[2] += len(table); answer[4] += 999983
        answer[0] += sum(table.get(key,999983)*(i+1) for i,key in enumerate(keys))
        answer[1] += sum(saved[key]*(i+1) for i,key in enumerate(keys))
    return answer


def profile(stderr):
    records = []
    for line in stderr.splitlines():
        try: record = json.loads(line)
        except (ValueError,UnicodeDecodeError): continue
        if type(record) is dict and record.get('kind') == 'runtime_profile': records.append(record)
    if len(records) != 1:
        raise ValueError(f'expected exactly one runtime_profile record, got {len(records)}')
    record = records[0]
    required = {'version':1,'units':{'counts':'events','bytes':'bytes'},'counter_arithmetic':'uint64_modulo',
                'allocation_header_bytes':16,'pool_header_bytes':512,
                'requested_bytes_exclude_allocator_header':True,'size_class_payload_excludes_header':True}
    if any(type(record.get(key)) is not type(value) or record.get(key) != value for key,value in required.items()):
        raise ValueError(f'unsupported runtime profile semantics: {record}')
    counters = record.get('counters',{})
    if set(counters) != set(COUNTERS) or any(type(v) is not int or not 0 <= v < 2**64 for v in counters.values()):
        raise ValueError('invalid uint64 runtime counters')
    if (counters['fresh_allocations']+counters['reused_allocations'])%2**64 != counters['allocation_count']:
        raise ValueError('allocation classification does not match total')
    return record


def invoke(args, *, timed=False):
    started = time.perf_counter_ns() if timed else None
    result = subprocess.run([str(a) for a in args],cwd=ROOT,env=ENV,capture_output=True,timeout=120)
    elapsed = (time.perf_counter_ns()-started)/1_000_000 if timed else None
    if result.returncode:
        raise RuntimeError(f'{args}: exit {result.returncode}\n{result.stdout!r}\n{result.stderr.decode(errors="replace")}')
    return result,elapsed


def hashes():
    paths = [*COHORT.rglob('*.ddr'),*COHORT.rglob('*.txt'),*ROOT.glob('selfhost/*.ddr'),
             *ROOT.glob('stdlib/*.ddr'),*ROOT.glob('runtime/*.ddr'),Path(__file__)]
    return {str(p.relative_to(ROOT)):sha(p) for p in sorted(paths)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_subparsers(dest='mode',required=True)
    for mode in ('check','calibrate','attribute','compare'):
        sub = modes.add_parser(mode)
        if mode == 'compare':
            sub.add_argument('baseline',type=Path); sub.add_argument('candidate',type=Path)
            sub.add_argument('--rounds',type=int,default=30)
        else: sub.add_argument('compiler',type=Path)
        if mode != 'check': sub.add_argument('report',type=Path)
        sub.add_argument('--manifest',type=Path,default=COHORT/'workloads.json')
        sub.add_argument('--case',action='append',choices=['pod','decimal','lexer','symbols'])
        if mode != 'compare': sub.add_argument('--compiler-flag',action='append',default=[])
        else:
            sub.add_argument('--baseline-flag',action='append',default=[])
            sub.add_argument('--candidate-flag',action='append',default=[])
        if mode == 'calibrate': sub.add_argument('--target-ms',type=float,default=150)
        if mode == 'attribute': sub.add_argument('--smoke',action='store_true')
    args = parser.parse_args()
    manifest_path = args.manifest.resolve(strict=True)
    manifest = json.loads(manifest_path.read_text())
    if manifest.get('version') != 1 or set(manifest['cases']) != {'pod','decimal','lexer','symbols'}:
        parser.error('unsupported workload manifest')
    for relative,digest in manifest['inputs_sha256'].items():
        if sha(COHORT/relative) != digest: parser.error(f'frozen input changed: {relative}')
    if any(manifest['cases'][name].get('input') not in manifest['inputs_sha256'] for name in ('decimal','lexer','symbols')):
        parser.error('every workload input must have a frozen hash')
    if args.mode == 'compare' and (manifest.get('frozen') is not True or args.rounds < 1):
        parser.error('compare requires a frozen manifest and positive rounds; calibrate/review/freeze first')
    if args.mode == 'calibrate' and (not math.isfinite(args.target_ms) or args.target_ms <= 0):
        parser.error('--target-ms must be finite and positive')
    cases = list(dict.fromkeys(args.case or manifest['cases']))
    compilers = {'baseline':args.baseline.resolve(strict=True),'candidate':args.candidate.resolve(strict=True)} if args.mode == 'compare' else {'compiler':args.compiler.resolve(strict=True)}
    flags = {'baseline':args.baseline_flag,'candidate':args.candidate_flag} if args.mode == 'compare' else {'compiler':args.compiler_flag}
    source_hashes = hashes(); compiler_hashes = {v:sha(p) for v,p in compilers.items()}
    metadata = {'protocol':'dunder-dogfood-v1','mode':args.mode,'host':platform.uname()._asdict(),
                'manifest':manifest,'manifest_sha256':sha(manifest_path),'sources_sha256':source_hashes,
                'compiler_sha256':compiler_hashes,'compiler_paths':{v:str(p) for v,p in compilers.items()},
                'compiler_flags':flags,'environment':ENV,'workloads':{},
                'scope':'whole process including input, setup, workload and checksum output; warm OS file cache',
                'c_acceptance':'separate cohorts; does not change the four-kernel C acceptance suite'}
    for key,git_args in [('checkout',['rev-parse','HEAD']),('working_tree',['status','--short'])]:
        metadata[key] = subprocess.run(['git',*git_args],cwd=ROOT,capture_output=True,text=True,check=True).stdout.strip()
    if platform.system() == 'Darwin':
        metadata['hardware'] = subprocess.run(['/usr/sbin/sysctl','machdep.cpu.brand_string','hw.memsize'],capture_output=True,text=True,check=True).stdout.strip()
    elif Path('/proc/cpuinfo').exists():
        metadata['hardware'] = next((line for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),'')
    if args.mode == 'compare':
        metadata.update({'rounds':args.rounds,'order':'even baseline,candidate; odd candidate,baseline',
                         'warmup':'one unrecorded checked execution per binary'})
    if args.mode == 'calibrate':
        metadata['warmup'] = 'one unrecorded checked execution before the single sizing observation'
    report = None
    if args.mode != 'check':
        report = args.report.resolve(); report.mkdir(parents=True,exist_ok=True)
        if any(report.iterdir()): parser.error('report directory must be empty')
    rows = []; summaries = []; recommendations = copy.deepcopy(manifest)
    with tempfile.TemporaryDirectory(prefix='dunder-dogfood-') as temporary:
        work = Path(temporary)
        for name in cases:
            smoke = args.mode == 'check' or (args.mode == 'attribute' and args.smoke)
            count = manifest['smoke_count'] if smoke else manifest['cases'][name]['count']
            if type(count) is not int or not 0 < count <= 10_000_000: parser.error('work count out of range')
            values = reference(name,count,manifest)
            if any(not -2**63 <= value < 2**63 for value in values): parser.error('reference checksum exceeds i64')
            expected = ''.join(f'{value}\n' for value in values).encode()
            inputs = [str(count),str(manifest['seed']) if name == 'pod' else str(COHORT/manifest['cases'][name]['input'])]
            metadata['workloads'][name] = {'args':inputs,'count':count,'expected_stdout':expected.decode(),'binaries':{}}
            binaries = {}
            for variant,compiler in compilers.items():
                image = work/f'{name}-{variant}'
                build_flags = flags[variant]+(['--runtime-profile'] if args.mode == 'attribute' else [])
                invoke([compiler,'build',COHORT/f'{name}.ddr','-o',image,*build_flags])
                binaries[variant] = image
                metadata['workloads'][name]['binaries'][variant] = {'sha256':sha(image),'bytes':image.stat().st_size,'build_flags':build_flags}
            def run(variant,timed=False):
                result,elapsed = invoke([binaries[variant],*inputs],timed=timed)
                if result.stdout != expected: raise ValueError(f'{name}/{variant}: expected {expected!r}, got {result.stdout!r}')
                if args.mode != 'attribute' and result.stderr: raise ValueError(f'unexpected stderr: {result.stderr!r}')
                return result,elapsed
            if args.mode == 'attribute':
                result,_ = run('compiler')
                record = profile(result.stderr)
                (report/f'{name}.stderr.txt').write_bytes(result.stderr)
                (report/f'{name}.stdout.txt').write_bytes(result.stdout)
                (report/f'{name}.profile.json').write_text(json.dumps(record,indent=2)+'\n')
                metadata['workloads'][name]['runtime_profile'] = record
                print(f'ATTRIBUTED {name}: {record["counters"]["allocation_count"]} allocations, {record["counters"]["requested_bytes"]} requested bytes',flush=True)
            elif args.mode == 'calibrate':
                # Match comparison: assess/launch each freshly built image in
                # an unrecorded checked warmup before sizing its steady work.
                run('compiler')
                _,elapsed = run('compiler',True)
                proposed = max(1,min(10_000_000,math.ceil(count*args.target_ms/elapsed)))
                recommendations['cases'][name]['count'] = proposed
                rows.append({'case':name,'count':count,'elapsed_ms':f'{elapsed:.6f}','proposed_count':proposed})
                print(f'CALIBRATION {name}: count={count}, elapsed={elapsed:.3f}ms, proposed={proposed}',flush=True)
            elif args.mode == 'check':
                run('compiler'); print(f'PASS {name}: independent reference ({count} iterations)',flush=True)
            else:
                for variant in binaries: run(variant)  # unrecorded checked warmup
                for round_number in range(args.rounds):
                    order = ['baseline','candidate'] if round_number%2 == 0 else ['candidate','baseline']
                    for position,variant in enumerate(order):
                        _,elapsed = run(variant,True)
                        rows.append({'case':name,'variant':variant,'round':round_number,'order':position,'elapsed_ms':f'{elapsed:.6f}'})
                baseline = statistics.median(float(r['elapsed_ms']) for r in rows if r['case']==name and r['variant']=='baseline')
                for variant in binaries:
                    samples = sorted(float(r['elapsed_ms']) for r in rows if r['case']==name and r['variant']==variant)
                    median = statistics.median(samples); p95 = samples[math.ceil(len(samples)*.95)-1]
                    summaries.append({'case':name,'variant':variant,'samples':len(samples),'median_ms':f'{median:.6f}','p95_ms':f'{p95:.6f}','median_over_baseline':f'{median/baseline:.6f}'})
                print(f'COMPLETED {name}: {args.rounds} alternating pairs',flush=True)
    if hashes() != source_hashes or sha(manifest_path) != metadata['manifest_sha256'] or any(sha(p) != compiler_hashes[v] for v,p in compilers.items()):
        raise ValueError('workload sources, manifest or compiler changed during execution')
    if report:
        (report/'metadata.json').write_text(json.dumps(metadata,indent=2,ensure_ascii=False)+'\n')
        for filename,records in [('samples.csv',rows),('summary.csv',summaries)]:
            if records:
                with (report/filename).open('w',newline='') as output:
                    writer = csv.DictWriter(output,fieldnames=list(records[0])); writer.writeheader(); writer.writerows(records)
        if args.mode == 'calibrate':
            recommendations['frozen'] = False
            recommendations['calibration'] = 'Review these single-baseline proposals, then explicitly set frozen=true before comparing variants.'
            (report/'recommended-workloads.json').write_text(json.dumps(recommendations,indent=2,ensure_ascii=False)+'\n')


if __name__ == '__main__':
    main()
