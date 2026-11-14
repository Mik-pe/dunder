#!/usr/bin/env python3
"""Actual joint Frame+Expr SSA images and exact 62-column transaction QA.

The independent physical interpreter knows the finite observer helper below,
not arbitrary calls. All three native targets are built; the native host executes.
There are no timings and no source/pipeline implementation changes here.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import re
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('expr_region_qa', ROOT/'tools/test-mir-regions.py')
region = importlib.util.module_from_spec(spec)
spec.loader.exec_module(region)
qa = region.qa
oracle = qa.oracle
MASK = (1 << 64) - 1
LOW = (1 << 32) - 1
LIMIT = 4194304
SP = 1 << 20
FAIL = -9999
NAMES = ('simple', 'nested_order', 'nested_nzcv', 'zero_upper_w', 'zero_register',
         'branch_phi', 'loop_value', 'unmatched_terminal', 'call_stack_address',
         'raw_backing_mutation', 'real_stack_argument', 'raw_indirect', 'raw_sp_read',
         'external_gp_observer', 'explicit_trap', 'mixed_families', 'guard_lower',
         'guard_upper', 'mixed_pressure', 'capture_and_stack', 'raw_padding_zero')
POSITIVE = {0,1,2,3,4,5,6,14,15,16,18,19}
PINNED = {7,8,9,10,11,12,13,17,20}
COUNTERS = ('changed','status','units','placed','pushes','pops','copies','loads','stores','captures','words')


def expected(case, value, control):
    if case == 0: result = value + 7
    elif case in (1,2): result = value - control + 7 + (8 * region.qa.overflow_sub(value,control) if case == 2 else 0)
    elif case == 3: result = (value & LOW) + 7
    elif case == 4: result = 7
    elif case == 5: result = value if control else control
    elif case == 6: result = value + max(0,control)
    elif case in (8,10): result = value + 99
    elif case == 9: result = control + 99
    elif case == 11: result = control
    elif case == 12: result = 16
    elif case in (13,14): result = value
    elif case == 15: result = value + control
    elif case == 16: result = 77
    elif case == 18: result = 24 * value + 78 * control - 154
    elif case == 19: result = (control & LOW) + 99
    else: result = 0
    return oracle.signed(result)


def corpus():
    pattern = oracle.signed(0xFEDCBA98800000FF)
    for case in range(len(NAMES)):
        if case == 2: values = [(-(1<<63),1),((1<<63)-1,-1),(0,0)]
        elif case in (5,14): values = [(pattern,0),(pattern,1)]
        elif case == 6: values = [(41,0),(-17,1),(pattern,7)]
        elif case in (16,17): values = [(41,0)]
        else: values = [(pattern,9),(41,-17)]
        for value,control in values: yield case,value,control


def transformed_model(model, helpers, case, value, control):
    operations = []
    starts = []
    places = [dict(x) for x in model['places']]
    op = region.instruction

    def constant(dst,bits): operations.append(op(1,dst=dst,imm=bits))
    def check(reg,bits):
        constant(100,bits); operations.append(op(5,opcode=2,a=reg,b=100,flags=2))
        operations.append(op(13,opcode=1,imm=1,flags=1,target=FAIL))
    def place(offset,width):
        p=dict(kind=2,offset=offset,width=width)
        if p not in places: places.append(p)
        return places.index(p)

    for identity in model['order']:
        starts.append(len(operations)); original = model['operations'][identity]
        if original['mode'] != 2 and original['kind'] == 12 and original['target'] == helpers[1]:
            check(0,value)
            for reg in range(9,18): check(reg,qa.seed(reg))
            operations.append(op(16,opcode=1,dst=31,a=29))
            operations.append(op(21,opcode=2,abi=1,effects=34))
            operations.append(op(15))
        elif original['mode'] != 2 and original['kind'] == 14:
            assert original['target'] == helpers[0] and case in (8,9,10,19), original
            for i,condition in enumerate((0,2,4,6)): operations.append(op(6,dst=101+i,imm=condition,flags=1))
            for reg in range(18):
                bits=qa.seed(reg)
                if reg==0: bits=SP+32
                if reg==4: bits=value
                if reg==5: bits=control
                check(reg,bits)
            diff=(value-control)&MASK
            flags=(diff==0,(value&MASK)>=(control&MASK),bool(diff>>63),qa.overflow_sub(value,control))
            for i,flag in enumerate(flags): check(101+i,int(flag))
            constant(100,96 if case==19 else (16 if case!=10 else 32))
            operations.append(op(4,opcode=2,dst=105,a=29,b=100))
            operations.append(op(5,opcode=2,a=31,b=105,flags=2)); operations.append(op(13,opcode=1,imm=1,flags=1,target=FAIL))
            if case in (9,10):
                operations.append(op(7,dst=105,a=31,aux=8,place=place(0,8))); check(105,value if case==9 else control)
                operations.append(op(7,dst=105,a=31,imm=8,aux=8,place=place(8,8))); check(105,0)
                if case==9: operations.append(op(8,a=5,b=31,aux=8,place=place(0,8)))
            if case==19:
                places.append(dict(kind=1,offset=-8,width=8));cell8=len(places)-1
                places.append(dict(kind=1,offset=-8,width=4));cell4=len(places)-1
                operations.append(op(7,dst=105,a=29,imm=-8,aux=8,place=cell8));check(105,value)
                operations.append(op(8,a=5,b=29,imm=-8,aux=4,place=cell4,type=1))
            for reg in range(18): constant(reg,9000007+reg*101)
            constant(100,0); operations.append(op(5,opcode=2,a=100,b=100,flags=2)); constant(0,99)
        elif original['mode'] != 2 and original['kind'] == 10:
            # Exact shared Push is STP value,XZR, not a single8B store.
            operations.append(dict(original))
            operations.append(op(8,a=-1,b=31,imm=8,aux=8,place=place(8,8)))
        elif original['mode'] != 2 and original['kind'] == 11:
            # LDP value,XZR also reads the second8B, although its bits discard.
            operations.append(op(7,dst=106,a=31,imm=8,aux=8,place=place(8,8)))
            operations.append(dict(original))
        else: operations.append(dict(original))
    starts.append(len(operations))
    labels=[(label,starts[at] if at>=0 else -1) for label,at in model['labels']]
    return dict(operations=operations,places=places,labels=labels,order=list(range(len(operations))))


def interpret(model, helpers, case, value, control):
    if case in (16,17): return {'gap':'resource bounds have no fabricated OS mapping model'}
    m=oracle.Interpreter(transformed_model(model,helpers,case,value,control),fuel=250000)
    m.variables.update({r:qa.seed(r)&MASK for r in range(31)})
    m.variables.update({0:SP+32,4:value&MASK,5:control&MASK,29:SP+32,31:SP})
    if case in (9,10,20): m.store(SP-(40 if case==10 else 24),qa.seed(17),8)
    diff=(value-control)&MASK
    m.nzcv=(bool(diff>>63),diff==0,(value&MASK)>=(control&MASK),qa.overflow_sub(value,control))
    trap=case==7 or (case==14 and control==0)
    try: result=oracle.signed(m.run())
    except oracle.CoverageGap as error:
        assert trap and str(error)==f'external branch {helpers[2]}',(NAMES[case],error)
        return {'trap':102,'steps':m.steps}
    assert not trap and result==expected(case,value,control),(NAMES[case],value,control,result)
    assert m.read(31)==SP and m.read(29)==SP+32
    for r in range(18,29): assert m.read(r)==qa.seed(r),(NAMES[case],r,m.read(r))
    proof={'result':result,'steps':m.steps,'coverage':dict(m.coverage)}
    if case==20:
        # Regression for the generic oracle's actual STP upper-XZR transfer.
        # Deliberately use the exported physical model WITHOUT our expansion.
        generic=oracle.Interpreter(model,fuel=250000)
        generic.variables.update({r:qa.seed(r)&MASK for r in range(31)})
        generic.variables.update({0:SP+32,4:value&MASK,5:control&MASK,29:SP+32,31:SP})
        generic.store(SP-24,qa.seed(17),8)
        got=oracle.signed(generic.run())
        assert got==0,('generic raw Push must overwrite prefilled padding with XZR',got)
        proof['generic_result']=got
    return proof


def parse(text):
    helper_text=text.split('HELPERS\n',1)[1].split('ENDHELPERS\n',1)[0]
    helpers=list(map(int,helper_text.splitlines()))
    before,after=text.split('ORIGINAL\n',1)[1].split('RESULT\n',1)
    after,counters=after.split('STACKCOUNTERS\n',1)
    def phase(raw):
        model,metadata=raw.split('META1\n',1); metadata,rest=metadata.split('ENDMETA\n',1)
        assert not rest.strip(); return oracle.decode_model(model),metadata
    b,bm=phase(before); a,am=phase(after)
    rows=counters.splitlines(); assert len(rows)==len(COUNTERS),(rows,COUNTERS)
    assert rows[0] in ('true','false')
    counts=dict(zip(COUNTERS,[rows[0]=='true',*map(int,rows[1:])]))
    return helpers,b,a,bm,am,counts


def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('compiler',type=Path)
    p.add_argument('--case',type=int,choices=range(len(NAMES)));p.add_argument('--output',type=Path);p.add_argument('--report',type=Path)
    args=p.parse_args();compiler=args.compiler.resolve(strict=True)
    host=0 if platform.system()=='Darwin' else (1 if platform.machine() in ('arm64','aarch64') else 2)
    fixture=ROOT/'tests/fixtures/mir-expr-oracle.ddr';adapter=ROOT/'tests/fixtures/mir_expr_qa_adapter.ddr';support=ROOT/'tests/fixtures/mir_region_qa_support.ddr'
    declaration=(ROOT/'selfhost/mir.ddr').read_text().split('pub struct Function {',1)[1].split('\n}',1)[0]
    fields=set(re.findall(r'\b([a-z][a-z0-9_]*)\s*:',re.sub(r'//[^\n]*','',declaration)))
    observed=set(re.findall(r'\bf\.([a-z][a-z0-9_]*)',support.read_text().split('pub fn metadata(',1)[1]))
    assert fields==observed,(fields-observed,observed-fields)
    report=dict(fields=len(fields),host=host,hashes={},images=[])
    for name,path in dict(compiler=compiler,fixture=fixture,adapter=adapter,support=support,ssa=ROOT/'selfhost/mir_ssa.ddr',expr=ROOT/'selfhost/mir_expr_stack.ddr',generic_oracle=ROOT/'tools/test-mir.py',tool=Path(__file__).resolve(),region_tool=ROOT/'tools/test-mir-regions.py').items(): report['hashes'][name]=region.digest(path)
    with tempfile.TemporaryDirectory(prefix='dunder-expr-qa-') as tmp:
        work=args.output.resolve() if args.output else Path(tmp)
        if args.output: work.mkdir(parents=True,exist_ok=True);assert not any(work.iterdir())
        def call(command,status=0):
            r=subprocess.run(list(map(str,command)),cwd=ROOT,capture_output=True,text=True,timeout=60)
            assert r.returncode==status,(command,r.returncode,r.stdout[-5000:],r.stderr[-5000:]);return r.stdout
        emitter=work/'emitter';call([compiler,'build',fixture,'-o',emitter]);report['hashes']['emitter']=region.digest(emitter)
        def one(case,value,control,target,mode,origin=1,storage=1,limit=LIMIT,terminal=1,label=''):
            image=work/f'{case}-{value}-{control}-{target}-{mode}-{origin}-{storage}-{terminal}-{limit}{label}'
            text=call([emitter,image,target,case,mode,value,control,expected(case,value,control),limit,origin,storage,terminal])
            helpers,b,a,bm,am,c=parse(text)
            assert 0<=c['units']<=limit,c
            before,after=qa.traffic(b),qa.traffic(a)
            pre,post=interpret(b,helpers,case,value,control),interpret(a,helpers,case,value,control)
            assert pre.get('result')==post.get('result') and pre.get('trap')==post.get('trap')
            for index,op in enumerate(b['operations']):
                if op['kind'] not in (7,10,11): assert a['operations'][index]==op,('pinned row changed',NAMES[case],index)
                if op['kind'] in (10,11) and (not storage or not origin or case in PINNED):
                    assert a['operations'][index]==op,('raw/observed stack changed',NAMES[case],index)
            if mode=='off' or not c['changed']: assert b==a and bm==am,('noncommit mutation',NAMES[case],c)
            if mode=='off': assert c['status']==c['units']==0
            elif limit<LIMIT: assert not c['changed'] and c['status']==-3,c
            elif case in POSITIVE and origin and storage:
                assert c['changed'] and c['status']==2 and c['placed']>0 and c['pushes']>0 and c['pops']>0,(NAMES[case],c)
                assert after['pushes']<before['pushes'] and after['pops']<before['pops'],(NAMES[case],before,after,c)
                if case in (15,18,19): assert c['loads']>0 and after['loads']<before['loads'],(NAMES[case],c,before,after)
                if case==19: assert c['captures']>0,c
            elif case not in (15,18,19): assert not c['changed'] and c['pushes']==c['pops']==0,(NAMES[case],c)
            trap=case==7 or (case==14 and control==0)
            ran=False
            if target==host and case!=17: call([image],102 if trap else 0);ran=True
            if args.output:image.with_suffix('.mir.txt').write_text(text)
            report['images'].append(dict(case=case,name=NAMES[case],input=value,control=control,target=target,mode=mode,origin=origin,storage=storage,terminal=terminal,limit=limit,executed=ran,sha256=region.digest(image),counters=c,before_traffic=before,after_traffic=after,before=pre,after=post))
            return c
        samples=[x for x in corpus() if args.case is None or x[0]==args.case]
        for index,(case,value,control) in enumerate(samples):
            if case==17: continue
            for target in range(3):
                one(case,value,control,target,'off')
                one(case,value,control,target,'on',storage=0)
                one(case,value,control,target,'on',origin=0)
                one(case,value,control,target,'on')
            print(f'PASS {index+1}/{len(samples)} {NAMES[case]} input{value} control{control}',flush=True)
        for case,value,control in ((1,41,9),(5,41,1),(6,41,7),(15,41,9),(18,41,9),(19,41,9)):
            if args.case is not None and args.case!=case:continue
            for target in (host,):
                full=one(case,value,control,target,'on',label='-budget-base')
                for limit in sorted({0,1,max(1,full['units']//2),max(1,full['units']-1)}):one(case,value,control,target,'on',limit=limit,label='-budget')
    report.update(images_built=len(report['images']),executions=sum(x['executed'] for x in report['images']),independent_pairs=sum('gap' not in x['before'] for x in report['images']),coverage_limits=['lower resource guard has no fabricated OS mapping model','Linux ARM/x86 build only on macOS host'])
    if args.report:args.report.parent.mkdir(parents=True,exist_ok=True);args.report.write_text(json.dumps(report,indent=2,sort_keys=True)+'\n')
    print(f'{report["images_built"]} images; {report["executions"]} native executions; {report["independent_pairs"]} independent exact-bit pairs; no timings')


if __name__=='__main__':main()
