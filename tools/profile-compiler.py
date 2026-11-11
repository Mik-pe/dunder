#!/usr/bin/env python3
"""Attribute compiler phases and allocation traffic with separate observers."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import re
import statistics
import subprocess
import sys

# Imported helper modules must not create caches outside the report directory.
sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
ENV = {'PATH':'/no-external-tools'}
PHASES = ['declarations','model_parse','verify_pre','simplify','verify_post',
          'allocation','scalar_emit','legacy_body','fragment_cache','finalize','range_analysis']
UNITS = ['functions','tokens','nodes','visits','nodes','frame_values','words',
         'tokens','operations','bytes','operations']
PIPELINE = {'module_load_lex_resolve','build_setup','compiler','image_format_sign','publication'}


def helper(name, filename):
    spec = importlib.util.spec_from_file_location(name,ROOT/'tools'/filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PERFORMANCE = helper('dunder_performance','performance.py')
DOGFOOD = helper('dunder_dogfood_profiles','profile-dogfood.py')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def record(stderr, kind):
    lines = stderr.splitlines()
    if len(lines) != 1:
        raise ValueError(f'expected one {kind} JSON line, got {len(lines)}; observers must remain separate')
    value = json.loads(lines[0])
    if type(value) is not dict or value.get('kind') != kind:
        raise ValueError(f'expected {kind} record, got {value!r}')
    return value


def numbers(value, keys, name):
    if type(value) is not dict or set(value) != set(keys) or any(type(n) is not int or n < 0 for n in value.values()):
        raise ValueError(f'invalid {name} counters or keys')


def compiler_timing(stderr, target):
    value = record(stderr,'timings')
    required = {'version':1,'operation':'build','scope':'compiler_pipeline',
                'clock':'monotonic','clock_ok':True,'unit':'ns','target':target}
    if any(type(value.get(k)) is not type(v) or value.get(k) != v for k,v in required.items()):
        raise ValueError('unsupported compiler timing schema or unavailable host clock')
    numbers(value.get('phases'),PIPELINE,'pipeline phase')
    numbers(value.get('functions'),['reused','emitted'],'function')
    if type(value.get('modules')) is not int or value['modules'] < 1:
        raise ValueError('invalid module count')
    if type(value.get('total')) is not int or value['total'] != sum(value['phases'].values()):
        raise ValueError('pipeline phases do not partition total')
    if value.get('skipped_phases') != []:
        raise ValueError('successful build unexpectedly skipped a phase')
    optimizer = value.get('optimizer',{})
    if type(optimizer) is not dict or type(optimizer.get('enabled')) is not bool:
        raise ValueError('missing optimizer state')
    numbers({k:v for k,v in optimizer.items() if k != 'enabled'},
            ['modeled','emitted','rewrites','proven_bounds','fallbacks'],'optimizer')
    detail = value.get('compiler_detail',{})
    if type(detail) is not dict or detail.get('enabled') is not True or detail.get('clock_ok') is not True or detail.get('unit') != 'ns':
        raise ValueError('missing compiler details or unavailable compiler clock')
    numbers(detail.get('phases'),PHASES,'compiler phase')
    numbers(detail.get('calls'),PHASES,'compiler phase call')
    numbers(detail.get('work'),[f'{p}_{u}' for p,u in zip(PHASES,UNITS)],'compiler work')
    if any(type(detail.get(k)) is not int or detail[k] < 0 for k in ('covered','other')):
        raise ValueError('missing covered/other duration; null is not zero')
    if detail['covered'] != sum(detail['phases'].values()) or detail['covered']+detail['other'] != value['phases']['compiler']:
        raise ValueError('compiler details do not partition compiler interval')
    if detail['calls']['declarations'] != 1 or detail['calls']['finalize'] != 1:
        raise ValueError('expected one declaration and finalization interval')
    for phase,unit in zip(PHASES,UNITS):
        if detail['calls'][phase] == 0 and (detail['phases'][phase] != 0 or detail['work'][f'{phase}_{unit}'] != 0):
            raise ValueError(f'uncalled phase {phase} has time/work')
    return value


def allocation_profile(stderr):
    record(stderr,'runtime_profile')  # Reject combined observer output first.
    return DOGFOOD.profile(stderr)


def host():
    system,machine = platform.system(),platform.machine()
    if system == 'Darwin' and machine == 'arm64': return 'arm64-macos','main'
    if system == 'Linux' and machine in ('arm64','aarch64'): return 'arm64-linux','linux'
    if system == 'Linux' and machine == 'x86_64': return 'x86_64-linux','x86_linux'
    raise ValueError(f'unsupported native host: {system}/{machine}')


def import_path(raw, parent):
    if '/' in raw:
        path = raw
    else:
        dots = len(raw)-len(raw.lstrip('.'))
        path = ('./' if dots == 1 else '../'*(dots-1) if dots else '')+raw[dots:].replace('.','/')
    if path == 'sys': return ROOT/'stdlib/sys.ddr'
    if path.startswith('std/') and (ROOT/'stdlib'/(path[4:]+'.ddr')).is_file():
        return ROOT/'stdlib'/(path[4:]+'.ddr')
    return parent/(path+'.ddr')


def source_graph(source, resolutions=None):
    # The existing independent lexer avoids treating import-looking text in
    # strings/comments as dependencies. Use the compiler's documented path
    # spelling rules and canonical files; inability to discover a file is fatal.
    pending = [source]; found = {}
    while pending:
        requested = pending.pop()
        path = requested.resolve(strict=True)
        if resolutions is not None: resolutions[str(requested.absolute())] = str(path)
        if str(path) in found: continue
        data = path.read_bytes(); text = data.decode('utf-8')
        found[str(path)] = {'sha256':sha(path),'bytes':len(data),
                            'physical_lines_including_comments':len(data.splitlines())}
        scanned = DOGFOOD.tokens(text)
        for i,(kind,spelling,_,_) in enumerate(scanned[:-1]):
            if kind != 2 or spelling != 'import': continue
            following = scanned[i+1]
            if following[0] == 5: raw = following[1]
            else:
                start = following[2]; end = start
                while end < len(data) and data[end] > 32 and data[end] != 59: end += 1
                raw = data[start:end].decode('utf-8')
            pending.append(import_path(raw,path.parent))
    return found


class Recorder:
    def __init__(self, directory, metadata, timeout):
        self.directory,self.metadata,self.timeout = directory,metadata,timeout
        (directory/'raw').mkdir()

    def checkpoint(self):
        (self.directory/'metadata.json').write_text(json.dumps(self.metadata,indent=2,ensure_ascii=False)+'\n')

    def run(self, purpose, args, cwd=None):
        command = {'purpose':purpose,'argv':[str(a) for a in args],
                   'cwd':str(cwd or ROOT),'environment':ENV,'stdin':'null'}
        self.metadata['commands'].append(command)
        prefix = f'{len(self.metadata["commands"]):04d}'
        command['stdout'] = f'raw/{prefix}.stdout'
        command['stderr'] = f'raw/{prefix}.stderr'
        self.checkpoint()
        try:
            result = subprocess.run(command['argv'],cwd=command['cwd'],env=ENV,stdin=subprocess.DEVNULL,
                                    capture_output=True,timeout=self.timeout)
        except subprocess.TimeoutExpired as error:
            (self.directory/command['stdout']).write_bytes(error.stdout or b'')
            (self.directory/command['stderr']).write_bytes(error.stderr or b'')
            command.update({'timed_out':True,'returncode':None})
            self.checkpoint()
            raise
        (self.directory/command['stdout']).write_bytes(result.stdout)
        (self.directory/command['stderr']).write_bytes(result.stderr)
        command['returncode'] = result.returncode
        self.checkpoint()
        if result.returncode:
            raise RuntimeError(f'{purpose}: exit {result.returncode}; see {command["stderr"]}')
        return result


def plain(result, expected=None):
    if result.stderr:
        raise ValueError('unexpected stderr from uninstrumented command; see raw logs')
    if expected is not None and result.stdout != expected:
        raise ValueError(f'incorrect program result: expected {expected!r}, got {result.stdout!r}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compiler',type=Path)
    parser.add_argument('report',type=Path)
    parser.add_argument('--rounds',type=int,default=5,help='attribution repetitions, not a latency benchmark')
    parser.add_argument('--case',action='append',type=Path,help='source file; replaces default generated/selfhost/Tabell cases')
    parser.add_argument('--expected-stdout',type=Path,help='exact stdout reference for one custom source')
    parser.add_argument('--program-arg',action='append',default=[],help='runtime argument for that custom program')
    parser.add_argument('--input-file',action='append',type=Path,default=[],help='additional runtime input to hash and check for changes')
    parser.add_argument('--timeout',type=int,default=120)
    args = parser.parse_args()
    if args.rounds < 1 or args.timeout < 1: parser.error('rounds and timeout must be positive')
    if (args.expected_stdout or args.program_arg) and (not args.case or len(args.case) != 1):
        parser.error('custom execution options require exactly one --case')
    if args.program_arg and not args.expected_stdout:
        parser.error('--program-arg requires --expected-stdout to verify the run')
    expected_stdout = args.expected_stdout.resolve(strict=True) if args.expected_stdout else None
    compiler = args.compiler.resolve(strict=True)
    target,entry = host(); selfhost = (ROOT/f'selfhost/{entry}.ddr').resolve()
    tabell = (ROOT/'projects/tabell/main.ddr').resolve()
    directory = args.report.resolve()
    directory.mkdir(parents=True,exist_ok=True)
    if any(directory.iterdir()): parser.error('report directory must be empty')
    for name in ('images','validation'): (directory/name).mkdir()
    metadata = {'protocol':'dunder-compiler-attribution-v1','complete':False,
                'host':platform.uname()._asdict(),'native_target':target,'rounds':args.rounds,
                'input_compiler':{'path':str(compiler),'sha256':sha(compiler)},
                'selfhost_source':str(selfhost),'commands':[],'cases':{},
                'observer_policy':'compiler clocks and executed allocation traffic collected in separate processes',
                'scope':'attribution only; profiled durations and counters are not uninstrumented latency or C acceptance measurements'}
    recorder = Recorder(directory,metadata,args.timeout)
    try:
        cases = []
        if not args.case:
            generated,corpus = PERFORMANCE.corpus(directory)
            metadata['generated_corpus'] = corpus
            cases = [('generated',generated,'generated'),('selfhost',selfhost,'selfhost'),('tabell',tabell,'tabell')]
        else:
            for i,path in enumerate(args.case):
                source = path.resolve(strict=True)
                validation = 'selfhost' if source == selfhost else 'tabell' if source == tabell else 'custom'
                cases.append((f'{i+1:02d}_'+re.sub(r'[^A-Za-z0-9_-]','_',source.stem),source,validation))
        if args.expected_stdout and cases[0][2] != 'custom':
            raise ValueError('known selfhost/Tabell cases use their authoritative validation')
        resolutions = {}
        sources = source_graph(selfhost,resolutions)
        for name,source,validation in cases:
            graph = ({str(p.resolve()):{'sha256':sha(p),'bytes':p.stat().st_size,
                       'physical_lines_including_comments':len(p.read_bytes().splitlines())}
                      for p in sorted(source.parent.glob('*.ddr'))} if validation == 'generated' else source_graph(source,resolutions))
            sources.update(graph)
            metadata['cases'][name] = {'source':str(source),'validation':validation if validation != 'custom' or args.expected_stdout else 'image_identity_only',
                'source_files':list(graph),'source_bytes':sum(p['bytes'] for p in graph.values()),'observations':[]}
        # Bundled standard-library source lives in the compiler image; record
        # both its exact image identity and the current rebuild sources.
        for path in [*ROOT.glob('selfhost/*.ddr'),*ROOT.glob('stdlib/*.ddr'),*ROOT.glob('runtime/*.ddr')]:
            resolved = path.resolve()
            resolutions[str(path.absolute())] = str(resolved)
            sources[str(resolved)] = {'sha256':sha(path),'bytes':path.stat().st_size,
                                     'physical_lines_including_comments':len(path.read_bytes().splitlines())}
        for path in [Path(__file__),ROOT/'tools/performance.py',ROOT/'tools/profile-dogfood.py',
                     ROOT/'examples/hello.ddr',ROOT/'projects/tabell/expenses.csv',ROOT/'projects/tabell/expected.csv']:
            resolutions[str(path.absolute())] = str(path.resolve())
            sources[str(path.resolve())] = {'sha256':sha(path),'bytes':path.stat().st_size}
        if expected_stdout:
            path = expected_stdout
            sources[str(path)] = {'sha256':sha(path),'bytes':path.stat().st_size}
            metadata['custom_reference'] = {'path':str(path),'sha256':sha(path),'program_args':args.program_arg}
        metadata['additional_runtime_inputs'] = []
        for path in args.input_file:
            resolved = path.resolve(strict=True)
            resolutions[str(path.absolute())] = str(resolved)
            sources[str(resolved)] = {'sha256':sha(resolved),'bytes':resolved.stat().st_size}
            metadata['additional_runtime_inputs'].append(str(resolved))
        metadata['sources'] = sources
        metadata['source_resolutions'] = resolutions
        recorder.checkpoint()
        # Require the caller's compiler to match this checkout before building
        # a different observer from it. Otherwise observations could describe
        # two distinct compiler revisions despite coincidentally equal outputs.
        identity = directory/'images/source-identity'
        plain(recorder.run('verify input compiler source identity',
              [compiler,'build',selfhost,'--target',target,'-o',identity]))
        if sha(identity) != metadata['input_compiler']['sha256']:
            raise ValueError('input compiler is not the current source fixed point')
        metadata['source_identity_image'] = {'path':str(identity),'sha256':sha(identity)}
        profiled = directory/'images/allocation-compiler'
        plain(recorder.run('build allocation observer once',
              [compiler,'build',selfhost,'--target',target,'--runtime-profile','-o',profiled]))
        metadata['allocation_compiler'] = {'path':str(profiled),'sha256':sha(profiled)}
        recorder.checkpoint()

        def validate(name,image,kind,suffix):
            if kind == 'generated':
                plain(recorder.run(f'verify {name}/{suffix}',[image],directory/'validation'),b'595500\n')
            elif kind == 'selfhost':
                if sha(image) != metadata['input_compiler']['sha256']:
                    raise ValueError('observed selfhost result differs from source fixed point')
                hello = directory/f'validation/{name}-{suffix}-hello'
                plain(recorder.run(f'build hello with {name}/{suffix}',
                      [image,'build',ROOT/'examples/hello.ddr','--target',target,'-o',hello],directory/'validation'))
                plain(recorder.run(f'verify hello from {name}/{suffix}',[hello],directory/'validation'),b'42\ntrue\n')
            elif kind == 'tabell':
                plain(recorder.run(f'verify {name}/{suffix}',
                      [image,ROOT/'projects/tabell/expenses.csv','team','cents'],directory/'validation'),
                      (ROOT/'projects/tabell/expected.csv').read_bytes())
            elif expected_stdout:
                plain(recorder.run(f'verify {name}/{suffix}',[image,*args.program_arg],directory/'validation'),
                      expected_stdout.read_bytes())

        for name,source,validation in cases:
            expected_image = None
            for round_number in range(args.rounds):
                # Alternate collection order without mixing instrumentation.
                order = ['clock','allocation'] if round_number%2 == 0 else ['allocation','clock']
                for observer in order:
                    image = directory/f'images/{name}-{round_number:03d}-{observer}'
                    executable = compiler if observer == 'clock' else profiled
                    flags = ['--profile-compiler','--timings=json'] if observer == 'clock' else []
                    result = recorder.run(f'observe {name}/{round_number}/{observer}',
                            [executable,'build',source,'--target',target,'-o',image,*flags])
                    payload = compiler_timing(result.stderr,target) if observer == 'clock' else allocation_profile(result.stderr)
                    digest = sha(image)
                    if expected_image is None: expected_image = digest
                    if digest != expected_image:
                        raise ValueError(f'{name} emitted different images across observers/rounds')
                    observation = {'round':round_number,'observer':observer,'image':str(image),
                                   'image_sha256':digest,'image_bytes':image.stat().st_size,'record':payload}
                    metadata['cases'][name]['observations'].append(observation)
                    recorder.checkpoint()
                    validate(name,image,validation,f'{round_number:03d}-{observer}')
            metadata['cases'][name]['image_sha256'] = expected_image
            print(f'ATTRIBUTED {name}: {args.rounds} separate clock/allocation pairs; identical output images',flush=True)
        if sha(compiler) != metadata['input_compiler']['sha256'] or sha(profiled) != metadata['allocation_compiler']['sha256']:
            raise ValueError('compiler image changed during collection')
        for path,info in sources.items():
            if sha(Path(path)) != info['sha256']: raise ValueError(f'source/input/helper changed: {path}')
        for requested,resolved in resolutions.items():
            if str(Path(requested).resolve(strict=True)) != resolved:
                raise ValueError(f'source resolution changed: {requested}')
        summary = {}
        for name,case in metadata['cases'].items():
            clocks = [o['record'] for o in case['observations'] if o['observer'] == 'clock']
            allocations = [o['record']['counters'] for o in case['observations'] if o['observer'] == 'allocation']
            summary[name] = {'image_sha256':case['image_sha256'],'rounds':args.rounds,
                'clock_detail_median_ns':{phase:statistics.median(c['compiler_detail']['phases'][phase] for c in clocks) for phase in PHASES},
                'clock_other_median_ns':statistics.median(c['compiler_detail']['other'] for c in clocks),
                'allocation_counter_median_low':{counter:statistics.median_low(a[counter] for a in allocations) for counter in DOGFOOD.COUNTERS},
                'validation':case['validation']}
        (directory/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
        metadata['complete'] = True
    except Exception as error:
        metadata['failure'] = str(error)
        raise
    finally:
        recorder.checkpoint()


if __name__ == '__main__':
    main()
