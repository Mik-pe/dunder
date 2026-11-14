#!/usr/bin/env python3
"""Actual source task scopes: typing, effects, lifetime, isolated copies and targets.

No fake OS execution or latency measurements. First task provider supports
unprofiled ARM64/macOS. Other native/Wasm targets are checked for refusal or
unreachable-feature compatibility, never reported as executed thread coverage.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[1]
WORKER = 'fn work(n:i64)->i64 { n*n }\n'
SCOPE = 'fn main(){task scope jobs(2){id:=spawn jobs work(7);print(await jobs(id))}}\n'
VALID = {
 'scalar_repeat': (WORKER + 'fn main(){task scope jobs(2){id:=spawn jobs work(7);copy:=id;print(await jobs(id));print(await jobs(copy))}}', '49\n49\n'),
 'key_fifo': (WORKER + 'fn main(){var ids:[]TaskID<i64> = [];task scope jobs(1){for i in 0..20{ids.push(spawn jobs work(i64(i)))};var sum:i64=0;for id in ids{sum=sum+await jobs(id)};print(sum)}}', '2470\n'),
 'snapshot_copies': ('fn work(a:[]i64)->[]i64{var b:=a;b[0]=b[0]+1;b}\nfn main(){var a:[]i64=[41];task scope j(2){id:=spawn j work(a);a[0]=0;var first:=await j(id);first[0]=99;print((await j(id))[0]);print(first[0]);print(a[0])}}', '42\n99\n0\n'),
 'records': ('struct Job{a:[]i64,n:i64}\nstruct Answer{a:[]i64,n:i64}\nfn work(x:Job)->Answer{var a:=x.a;a[0]=a[0]+x.n;Answer{a:a,n:x.n}}\nfn main(){var a:[]i64=[40];var x:=Job{a:a,n:2};task scope j(1){id:=spawn j work(x);x.a[0]=0;var y:=await j(id);y.a[0]=99;print((await j(id)).a[0]);print(y.a[0])}}','42\n99\n'),
 'nested_return': ('fn work(n:i64)->[]i64{[n,n+1]}\nfn result()->[]i64{task scope outer(2){a:=spawn outer work(40);task scope inner(1){b:=spawn inner work(42);ignored:=await outer(a);return await inner(b)}}}\nfn main(){var a:=result();a[0]=a[0]+1;print(a[0]);print(a[1])}','43\n43\n'),
 'tail_return': ('fn work(n:i64)->[]i64{[n]}\nfn result()->[]i64{task scope jobs(1){id:=spawn jobs work(42);await jobs(id)}}\nfn main(){print(result()[0])}', '42\n'),
 'break_continue': (WORKER + 'fn main(){var sum:i64=0;for i in 0..6{task scope j(2){id:=spawn j work(i64(i));if i==1{continue};if i==4{break};sum=sum+await j(id)}};print(sum)}', '13\n'),
 'discard_owned': ('fn work(n:i64)->[]i64{[n]}\nfn main(){for i in 0..10{task scope j(2){a:=spawn j work(i64(i));b:=spawn j work(i64(i+1))}};print(42)}','42\n'),
 'local_borrow': ('fn add(n:&mut i64){n=n+1}\nfn work(n:i64)->i64{var x:=n;add(&mut x);x}\nfn main(){task scope j(1){id:=spawn j work(41);print(await j(id))}}','42\n'),
 'local_string': ('fn work(n:i64)->i64{s:=n.str();assert(s.len>0);n+1}\nfn main(){task scope j(1){id:=spawn j work(41);print(await j(id))}}','42\n'),
 'local_runes_bytes': ('fn work(n:i64)->i64{s:=n.str();a:=s.runes();b:=s.bytes();i64(a.len+b.len)}\nfn main(){task scope j(1){id:=spawn j work(42);print(await j(id))}}','4\n'),
 'local_formatter': ('fn work(n:f64)->i64{s:=n.str();assert(s.len>0);42}\nfn main(){task scope j(1){id:=spawn j work(3.5);print(await j(id))}}','42\n'),
 'bool': ('fn work(n:bool)->bool{!n}\nfn main(){task scope j(1){id:=spawn j work(false);assert(await j(id));print(42)}}','42\n'),
 'unit': ('fn work(n:i64){assert(n==42)}\nfn main(){task scope j(1){id:=spawn j work(42);await j(id);await j(id)};print(42)}','42\n'),
 'tag_enum': ('enum Tag{A,B}\nfn work(n:Tag)->Tag{n}\nfn main(){task scope j(1){id:=spawn j work(Tag.B);assert(await j(id)==Tag.B);print(42)}}','42\n'),
 'bool_array': ('fn work(a:[]bool)->[]bool{var b:=a;b[0]=!b[0];b}\nfn main(){var a:[]bool=[false];task scope j(1){id:=spawn j work(a);a[0]=true;var first:=await j(id);first[0]=false;assert((await j(id))[0]);print(42)}}','42\n'),
 'fixed_array': ('fn work(a:[u8;4])->[u8;4]{var b:=a;b[0]=u8(42);b}\nfn main(){var a:[u8;4]=[u8(1);4];task scope j(1){id:=spawn j work(a);a[0]=u8(0);print(i32((await j(id))[0]));print(i32(a[0]))}}','42\n0\n'),
 'c_record': ('@c_layout struct Pair{a:i32,b:i64}\nfn work(p:Pair)->Pair{Pair{a:p.a+1,b:p.b+2}}\nfn main(){var p:=Pair{a:40,b:40};task scope j(1){id:=spawn j work(p);p.a=0;answer:=await j(id);print(answer.a);print(answer.b)}}','41\n42\n'),
 'float_results': ('fn work(n:f64)->f64{n+0.5}\nfn main(){task scope j(2){a:=spawn j work(2.0);b:=spawn j work(6.5);assert(await j(a)==2.5);assert(await j(b)==7.0);print(42)}}','42\n'),
 'empty_scope': ('fn main(){task scope jobs(1){var x:=0;x=x+1;print(x)}}','1\n'),
 'scope_shadowing': ('fn work(n:i64)->i64{n}\nfn main(){var jobs:i32=2;task scope jobs(jobs){id:=spawn jobs work(42);{var jobs:=7;print(jobs)};print(await jobs(id))};print(jobs)}','7\n42\n2\n'),
 'arena_pool_stress': ('struct WorkItem{values:[]i32,seed:i32}\nstruct WorkOut{items:[]i32,total:i64}\nfn work(item:WorkItem)->WorkOut{s:=item.seed.str();var out:[]i32=[];var total:i64=i64(s.len);for v in item.values{w:=v+item.seed;out.push(w);total=total+i64(w)};WorkOut{items:out,total:total}}\nfn main(){var sum:i64=0;for wave in 0..2{task scope jobs(4){var keys:[]TaskID<WorkOut> = [];var read:=0;for j in 0..120{i:=wave*120+j;keys.push(spawn jobs work(WorkItem{values:[i,i+1,i+2],seed:i+10}));if keys.len-read>=4{ans:=await jobs(keys[read]);read=read+1;sum=sum+ans.total+i64(ans.items[0]+ans.items[2])}};while read<keys.len{ans:=await jobs(keys[read]);read=read+1;sum=sum+ans.total+i64(ans.items[0]+ans.items[2])}}};print(sum)}','300630\n'),
}
INVALID = {
 'input_string': ('fn work(n:string)->i64{1}\nfn main(){task scope j(1){id:=spawn j work("x")}}','input must be isolated Data'),
 'result_string': ('fn work(n:i64)->string{n.str()}\nfn main(){task scope j(1){id:=spawn j work(1)}}','result must be isolated Data'),
 'payload_enum': ('enum E{A(i64)}\nfn work(n:E)->E{n}\nfn main(){task scope j(1){id:=spawn j work(E.A(1))}}','input must be isolated Data'),
 'opaque': ('opaque struct E{n:i64}\nfn work(n:E)->E{n}\nfn main(){task scope j(1){id:=spawn j work(E{n:1})}}','input must be isolated Data'),
 'borrow_input': ('fn work(n:&i64)->i64{n}\nfn main(){var n:i64=1;task scope j(1){id:=spawn j work(&n)}}','input must be isolated Data'),
 'key_input': ('fn leaf(n:i64)->i64{n}\nfn work(n:TaskID<i64>)->i64{1}\nfn main(){task scope j(1){a:=spawn j leaf(1);b:=spawn j work(a)}}','input must be isolated Data'),
 'print_dead': ('fn work(n:i64)->i64{if false{print(n)};n}\n'+SCOPE.replace('work(7)','work(7)'), 'effects outside'),
 'after_return_effect': ('fn work(n:i64)->i64{return n;print(9);n}\n'+SCOPE,'effects outside'),
 'method_argument_effect': ('fn bad(n:i64)->i64{print(n);n}\nfn work(n:i64)->i64{n.wrapping_add(bad(n))}\n'+SCOPE,'effects outside'),
 'short_circuit_effect': ('fn bad(n:i64)->bool{print(n);true}\nfn work(n:i64)->i64{x:=false&&bad(n);n}\n'+SCOPE,'effects outside'),
 'print_transitive': ('fn bad(n:i64)->i64{print(n);n}\nfn work(n:i64)->i64{bad(n)}\n'+SCOPE,'effects outside'),
 'ambient': ('import std.sys as sys\nfn work(n:i64)->i64{i64(sys.pid())+n}\n'+SCOPE,'effects outside'),
 'unsafe_empty': ('capability unsafe\nfn work(n:i64)->i64{unsafe{};n}\n'+SCOPE,'effects outside'),
 'unknown_indirect': ('fn leaf(n:i64)->i64{n}\nfn work(n:i64)->i64{f:=leaf;f(n)}\n'+SCOPE,'effects outside'),
 'nested_spawn': ('fn leaf(n:i64)->i64{n}\nfn work(n:i64)->i64{task scope nested(1){id:=spawn nested leaf(n);await nested(id)}}\n'+SCOPE,'effects outside'),
 'unsafe_worker': ('capability unsafe\nunsafe fn work(n:i64)->i64{n}\n'+SCOPE,'one safe by-value'),
 'two_arguments': ('fn work(a:i64,b:i64)->i64{a+b}\n'+SCOPE,'one safe by-value'),
 'scope_escape': ('fn main(){task scope j(1){copy:=j}}','scope capability'),
 'scope_borrow': ('fn use(n:&i64){}\nfn main(){task scope j(1){use(&j)}}','scope capability'),
 'missing_scope': (WORKER+'fn main(){id:=spawn missing work(1)}','live task scope'),
 'wrong_key': ('fn main(){task scope j(1){x:=await j(7)}}','typed TaskID'),
 'wrong_key_type': (WORKER+'fn main(){task scope j(1){id:TaskID<i32> = spawn j work(1)}}','type mismatch'),
 'key_arithmetic': (WORKER+'fn main(){task scope j(1){id:=spawn j work(1);x:=id+id}}','arithmetic requires'),
 'capacity_type': ('fn main(){task scope j(true){}}','type mismatch'),
 'named_worker_shadow': (WORKER+'fn main(){f:=work;task scope j(1){id:=spawn j f(1)}}','named Dunder'),
}
TRAPS = {
 'zero_limit': ('fn main(){task scope j(0){}}',112),
 'negative_limit': ('fn main(){task scope j(-1){}}',112),
 'wrong_scope': (WORKER+'fn main(){task scope outer(1){id:=spawn outer work(1);task scope inner(1){value:=await inner(id)}}}',112),
 'expired_scope': (WORKER+'fn old()->TaskID<i64>{task scope j(1){return spawn j work(1)}}\nfn main(){id:=old();task scope j(1){value:=await j(id)}}',112),
 'scope_reentry': (WORKER+'fn main(){var ids:[]TaskID<i64> = [];for i in 0..2{task scope j(1){id:=spawn j work(1);if i==1{value:=await j(ids[0])};ids.push(id)}}}',112),
}


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('compiler',type=Path)
    ap.add_argument('--source-root',type=Path,default=ROOT)
    ap.add_argument('--output',type=Path,default=ROOT/'build/task-source-qa')
    args=ap.parse_args(); root=args.source_root.resolve(); compiler=args.compiler.resolve(); out=args.output.resolve();out.mkdir(parents=True,exist_ok=True)
    log=[]
    def run(cmd,status=0):
        r=subprocess.run([str(x) for x in cmd],cwd=root,text=True,capture_output=True,timeout=60)
        log.append({'command':[str(x) for x in cmd],'status':r.returncode,'stdout':r.stdout,'stderr':r.stderr})
        assert r.returncode==status,log[-1]
        return r.stdout+r.stderr
    try:
        for name,(source,expected) in VALID.items():
            path=out/(name+'.ddr');path.write_text(source+'\n');image=out/name
            run([compiler,'build',path,'-o',image]);assert run([image])==expected,(name,log[-1])
            run([compiler,'check',path,'--agent'])
        for name,(source,message) in INVALID.items():
            path=out/(name+'.ddr');path.write_text(source+'\n')
            for command in ('build','check'):
                cmd=[compiler,command,path]
                if command=='build':cmd+=['-o',out/name]
                text=run(cmd,1);assert message in text,(name,message,text)
        for name,(source,status) in TRAPS.items():
            path=out/(name+'.ddr');path.write_text(source+'\n');image=out/name
            run([compiler,'build',path,'-o',image]);run([image],status)
        # Scoped capability shadows a module only inside its body. Capacity
        # resolves the outer module, as do references after the closing brace.
        (out/'jobs.ddr').write_text('pub fn capacity()->i32{2}\n')
        (out/'worker.ddr').write_text('pub fn work(n:i64)->i64{n}\n')
        namespace=out/'namespace.ddr'
        namespace.write_text('import jobs\nimport worker\nfn main(){task scope jobs(jobs.capacity()){id:=spawn jobs worker.work(42);print(await jobs(id))};print(jobs.capacity())}\n')
        run([compiler,'build',namespace,'-o',out/'namespace']);assert run([out/'namespace'])=='42\n2\n'
        # Actual optimization switches remain available after certification.
        # Cached final fragments are deliberately disabled for task programs.
        safe='fn helper(n:i64)->i64{n+1}\nfn work(n:i64)->i64{helper(n)}\nfn main(){task scope j(1){id:=spawn j work(41);print(await j(id))}}\n'
        warm=out/'incremental.ddr'
        for scalar in ('on','off'):
            for inline in ('on','off'):
                warm.write_text(safe)
                flags=['--scalar-opt='+scalar,'--inline-opt='+inline,'--incremental']
                previous=None
                for repeat in range(2):
                    image=out/f'cached-{scalar}-{inline}-{repeat}'
                    text=run([compiler,'build',warm,'-o',image,*flags,'--timings=json'])
                    records=[json.loads(line) for line in text.splitlines() if line.startswith('{')]
                    assert records and records[0]['functions']['reused']==0
                    assert run([image])=='42\n'
                    data=image.read_bytes()
                    if previous is not None:assert data==previous
                    previous=data
                warm.write_text(safe.replace('n+1','print(n);n+1',1))
                text=run([compiler,'build',warm,'-o',out/'stale',*flags],1);assert 'effects outside' in text
        path=out/'scalar_repeat.ddr' 
        for target in ('arm64-linux','x86_64-linux','wasm32'):
            for command in ('build','check'):
                cmd=[compiler,command,path,'--target',target]
                if command=='build':cmd+=['-o',out/'unsupported']
                text=run(cmd,1)
                assert 'isolated tasks require ARM64/macOS' in text
        dormant=out/'unreachable.ddr';dormant.write_text('fn unused(){task scope j(1){}}\nfn main(){print(42)}\n')
        for target in ('arm64-macos','arm64-linux','x86_64-linux','wasm32'):
            run([compiler,'build',dormant,'--target',target,'-o',out/('unreachable-'+target)])
            run([compiler,'check',dormant,'--target',target])
        text=run([compiler,'build',path,'--runtime-profile','-o',out/'profiled'],1)
        assert 'isolated tasks require ARM64/macOS' in text
        summary={'result':'passed','valid':len(VALID),'invalid':len(INVALID),'traps':len(TRAPS),
                 'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest()}
        (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
    finally:(out/'commands.json').write_text(json.dumps(log,indent=2)+'\n')

if __name__=='__main__':main()
