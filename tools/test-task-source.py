#!/usr/bin/env python3
"""Actual source task scopes: typing, effects, lifetime, isolated copies and targets.

No fake OS execution or latency measurements. First task provider supports
unprofiled ARM64/macOS. Other native targets are checked for refusal or
unreachable-feature compatibility, never reported as executed thread coverage.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[1]
WORKER = 'fn work(n:i64)->i64 { n*n }\n'
SCOPE = 'fn main(){task scope jobs(2){id:=spawn jobs work(7);print(await jobs(id))}}\n'
STATE = ('struct Workspace{values:[]i32,label:string}\n'
         'fn initialize()->Workspace{Workspace{values:[],label:"workers"}}\n'
         'fn collect(x:i32, ws:&mut Workspace)->[]i32{ws.values.push(x);ws.values.clone()}\n'
         'fn count(x:i32, ws:&mut Workspace)->i32{ws.values.len+x+ws.label.len}\n')
WORLD = ('struct World{values:[]i32}\n'
         'fn lookup(i:i32, world:&World)->i32{world.values[i]}\n'
         'fn update(i:i32, ws:&mut Workspace, world:&World)->i32{ws.values.push(world.values[i]);ws.values.len}\n')
VALID = {
 'borrowed_input_snapshot': ('fn work(n:&i64)->i64{n}\nfn main(){mut n:i64=41;task scope j(1){id:=spawn j work(n);n=0;print(await j(id))};print(n)}','41\n0\n'),
 'shared_snapshot': (STATE+WORLD+'fn main(){mut w:=World{values:[40,42]};task scope j(2,shared w.clone()){a:=spawn j lookup(1);w.values[0]=9;w.values.clear();b:=spawn j lookup(0);print(await! j(b));print(await! j(a))}}','40\n42\n'),
 'owned_and_shared': (STATE+WORLD+'fn main(){task scope j(1,initialize,shared World{values:[11]}){a:=spawn j update(0);b:=spawn j update(0);print(await! j(a));print(await! j(b))}}','1\n2\n'),
 'shared_clone': (STATE+WORLD+'fn copy(world:&World)->[]i32{mut v:=world.values.clone();v[0]=100;v}\nfn job(i:i32,world:&World)->[]i32{copy(&world)}\nfn main(){mut w:=World{values:[41]};task scope j(1,shared w.clone()){a:=spawn j job(0);b:=spawn j lookup(0);print((await! j(a))[0]);print(await! j(b))};print(w.values[0])}','100\n41\n41\n'),
 'persistent_workspace': (STATE+'fn main(){task scope j(1,initialize){first:=spawn j collect(11);second:=spawn j collect(22);mut a:=await! j(first);a[0]=99;out:=await! j(second);print(out[0]);print(out[1]);n:=spawn j count(3);print(await j(n))};task scope j(1,initialize){n:=spawn j count(0);print(await! j(n))}}','11\n22\n12\n7\n'),
 'isolated_workspaces': (STATE+'fn main(){task scope j(2,initialize){a:=spawn j collect(41);b:=spawn j collect(43);left:=await! j(a);right:=await! j(b);assert(left.len==1&&right.len==1);print(left[0]);print(right[0]);c:=spawn j count(0);d:=spawn j count(0);print(await j(c));print(await j(d))}}','41\n43\n8\n8\n'),

 'scalar_repeat': (WORKER + 'fn main(){task scope jobs(2){id:=spawn jobs work(7);copy:=id;print(await jobs(id));print(await jobs(copy))}}', '49\n49\n'),
 'key_fifo': (WORKER + 'fn main(){mut ids:[]TaskID<i64> = [];task scope jobs(1){for i in 0..20{ids.push(spawn jobs work(i64(i)))};mut sum:i64=0;for id in ids{sum=sum+await jobs(id)};print(sum)}}', '2470\n'),
 'snapshot_copies': ('fn work(move a:[]i64)->[]i64{mut b:=a;b[0]=b[0]+1;b}\nfn main(){mut a:[]i64=[41];task scope j(2){id:=spawn j work(a.clone());a[0]=0;mut first:=await! j(id);snapshot:=first.clone();first[0]=99;print(snapshot[0]);print(first[0]);print(a[0])}}', '42\n99\n0\n'),
 'records': ('struct Job{a:[]i64,n:i64}\nstruct Answer{a:[]i64,n:i64}\nfn work(x:Job)->Answer{mut a:=x.a.clone();a[0]=a[0]+x.n;Answer{a:a,n:x.n}}\nfn main(){mut a:[]i64=[40];mut x:=Job{a:a,n:2};task scope j(1){id:=spawn j work(x);x.a[0]=0;mut y:=await! j(id);snapshot:=y.clone();y.a[0]=99;print(snapshot.a[0]);print(y.a[0])}}','42\n99\n'),
 'nested_return': ('fn work(n:i64)->[]i64{[n,n+1]}\nfn result()->[]i64{task scope outer(2){a:=spawn outer work(40);task scope inner(1){b:=spawn inner work(42);ignored:=await! outer(a);return await! inner(b)}}}\nfn main(){mut a:=result();a[0]=a[0]+1;print(a[0]);print(a[1])}','43\n43\n'),
 'tail_return': ('fn work(n:i64)->[]i64{[n]}\nfn result()->[]i64{task scope jobs(1){id:=spawn jobs work(42);await! jobs(id)}}\nfn main(){print(result()[0])}', '42\n'),
 'break_continue': (WORKER + 'fn main(){mut sum:i64=0;for i in 0..6{task scope j(2){id:=spawn j work(i64(i));if i==1{continue};if i==4{break};sum=sum+await j(id)}};print(sum)}', '13\n'),
 'discard_owned': ('fn work(n:i64)->[]i64{[n]}\nfn main(){for i in 0..10{task scope j(2){a:=spawn j work(i64(i));b:=spawn j work(i64(i+1))}};print(42)}','42\n'),
 'local_borrow': ('fn add(n:&mut i64){n=n+1}\nfn work(n:i64)->i64{mut x:=n;add(&mut x);x}\nfn main(){task scope j(1){id:=spawn j work(41);print(await j(id))}}','42\n'),
 'local_string': ('fn work(n:i64)->i64{s:=n.str();assert(s.len>0);n+1}\nfn main(){task scope j(1){id:=spawn j work(41);print(await j(id))}}','42\n'),
 'local_runes_bytes': ('fn work(n:i64)->i64{s:=n.str();a:=s.runes();b:=s.bytes();i64(a.len+b.len)}\nfn main(){task scope j(1){id:=spawn j work(42);print(await j(id))}}','4\n'),
 'local_formatter': ('fn work(n:f64)->i64{s:=n.str();assert(s.len>0);42}\nfn main(){task scope j(1){id:=spawn j work(3.5);print(await j(id))}}','42\n'),
 'bool': ('fn work(n:bool)->bool{!n}\nfn main(){task scope j(1){id:=spawn j work(false);assert(await j(id));print(42)}}','42\n'),
 'unit': ('fn work(n:i64){assert(n==42)}\nfn main(){task scope j(1){id:=spawn j work(42);await j(id);await j(id)};print(42)}','42\n'),
 'tag_enum': ('enum Tag{A,B}\nfn work(n:Tag)->Tag{n}\nfn main(){task scope j(1){id:=spawn j work(Tag.B);assert(await j(id)==Tag.B);print(42)}}','42\n'),
 'bool_array': ('fn work(move a:[]bool)->[]bool{mut b:=a;b[0]=!b[0];b}\nfn main(){mut a:[]bool=[false];task scope j(1){id:=spawn j work(a.clone());a[0]=true;mut first:=await! j(id);snapshot:=first.clone();first[0]=false;assert(snapshot[0]);print(42)}}','42\n'),
 'fixed_array': ('fn work(move a:[u8;4])->[u8;4]{mut b:=a;b[0]=u8(42);b}\nfn main(){mut a:[u8;4]=[u8(1);4];task scope j(1){id:=spawn j work(a.clone());a[0]=u8(0);print(i32((await! j(id))[0]));print(i32(a[0]))}}','42\n0\n'),
 'c_record': ('@c_layout struct Pair{a:i32,b:i64}\nfn work(p:Pair)->Pair{Pair{a:p.a+1,b:p.b+2}}\nfn main(){mut p:=Pair{a:40,b:40};task scope j(1){id:=spawn j work(p);p.a=0;answer:=await j(id);print(answer.a);print(answer.b)}}','41\n42\n'),
 'float_results': ('fn work(n:f64)->f64{n+0.5}\nfn main(){task scope j(2){a:=spawn j work(2.0);b:=spawn j work(6.5);assert(await j(a)==2.5);assert(await j(b)==7.0);print(42)}}','42\n'),
 'empty_scope': ('fn main(){task scope jobs(1){mut x:=0;x=x+1;print(x)}}','1\n'),
 'scope_shadowing': ('fn work(n:i64)->i64{n}\nfn main(){mut jobs:i32=2;task scope jobs(jobs){id:=spawn jobs work(42);{mut jobs:=7;print(jobs)};print(await jobs(id))};print(jobs)}','7\n42\n2\n'),
 'consume_cached': ('fn work(n:i64)->i64{n}\nfn result()->i64{task scope j(1){a:=spawn j work(40);b:=spawn j work(41);c:=await j(a);assert(c==40);x:=await! j(a);return x+2}}\nfn main(){print(result())}','42\n'),
 'consume_active_unit': ('fn work(x:i32){assert(x==1)}\nfn main(){task scope j(1){k:=spawn j work(1);await! j(k)};print(42)}','42\n'),
 'arena_pool_stress': ('struct WorkItem{values:[]i32,seed:i32}\nstruct WorkOut{items:[]i32,total:i64}\nfn work(item:WorkItem)->WorkOut{s:=item.seed.str();mut out:[]i32=[];mut total:i64=i64(s.len);for v in item.values{w:=v+item.seed;out.push(w);total=total+i64(w)};WorkOut{items:out,total:total}}\nfn main(){mut sum:i64=0;for wave in 0..2{task scope jobs(4){mut keys:[]TaskID<WorkOut> = [];mut read:=0;for j in 0..120{i:=wave*120+j;keys.push(spawn jobs work(WorkItem{values:[i,i+1,i+2],seed:i+10}));if keys.len-read>=4{ans:=await! jobs(keys[read]);read=read+1;sum=sum+ans.total+i64(ans.items[0]+ans.items[2])}};while read<keys.len{ans:=await! jobs(keys[read]);read=read+1;sum=sum+ans.total+i64(ans.items[0]+ans.items[2])}}};print(sum)}','300630\n'),
}
INVALID = {
 'shared_mut_argument': (STATE+WORLD+'fn bad(i:i32,w:&mut World)->i32{0}\nfn main(){task scope j(1,shared World{values:[1]}){id:=spawn j bad(0)}}','read-only parameter for this shared input'),
 'shared_mutation': (STATE+WORLD+'fn bad(i:i32,w:&World)->i32{w.values[0]=9;0}\nfn main(){task scope j(1,shared World{values:[1]}){id:=spawn j bad(0)}}','mutable'),
 'shared_string': ('struct World{s:string}\nfn main(){task scope j(1,shared World{s:"hello"}){}}','shared task input requires isolated Data'),
 'shared_opaque': ('opaque struct World{a:i64}\nfn main(){task scope j(1,shared World{a:1}){}}','shared task input requires isolated Data'),
 'shared_value_argument': (STATE+WORLD+'fn copy(i:i32,move w:World)->i32{0}\nfn main(){task scope j(1,shared World{values:[1]}){id:=spawn j copy(0)}}','read-only parameter for this shared input'),
 'effectful_initializer': (STATE.replace('Workspace{values:[],label:', 'print(1);Workspace{values:[],label:')+'fn main(){task scope j(1,initialize){}}','initializer has effects outside'),
 'workspace_shape': (STATE+'fn main(){task scope j(1,initialize){id:=spawn j invalid(1)}}\nfn invalid(x:i32,s:&mut []i64)->i32{x}','mutable parameter for this workspace'),
 'missing_workspace_parameter': (STATE+'fn main(){task scope j(1,initialize){id:=spawn j invalid(1)}}\nfn invalid(x:i32)->i32{x}','mutable parameter for this workspace'),
 'workspace_resource': ('struct State{address:ptr}\nfn factory()->State{factory()}\nfn main(){task scope j(2,factory){}}','workspace contains a value outside isolated storage'),
 'workspace_initializer_args': (STATE+'fn main(){task scope j(1,count){}}','initializer without parameters'),
 'workspace_initializer_scalar': ('fn factory()->i32{42}\nfn main(){task scope j(1,factory){}}','initializer must return an owned record'),
 'input_string': ('fn work(n:string)->i64{1}\nfn main(){task scope j(1){id:=spawn j work("x")}}','input must be isolated Data'),
 'result_string': ('fn work(n:i64)->string{n.str()}\nfn main(){task scope j(1){id:=spawn j work(1)}}','result must be isolated Data'),
 'payload_enum': ('enum E{A(i64)}\nfn work(n:E)->E{n}\nfn main(){task scope j(1){id:=spawn j work(E.A(1))}}','input must be isolated Data'),
 'opaque': ('opaque struct E{n:i64}\nfn work(n:E)->E{n}\nfn main(){task scope j(1){id:=spawn j work(E{n:1})}}','input must be isolated Data'),
 'borrow_input': ('fn work(n:&mut i64)->i64{n}\nfn main(){mut n:i64=1;task scope j(1){id:=spawn j work(&mut n)}}','task input cannot borrow mutable parent storage'),
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
 'read_consumed_alias': (WORKER+'fn main(){task scope j(2){k:=spawn j work(7);alias:=k;print(await! j(k));print(await j(alias))}}',112),
 'consume_twice': (WORKER+'fn main(){task scope j(1){k:=spawn j work(7);print(await! j(k));print(await! j(k))}}',112),
 'zero_limit': ('fn main(){task scope j(0){}}',112),
 'negative_limit': ('fn main(){task scope j(-1){}}',112),
 'wrong_scope': (WORKER+'fn main(){task scope outer(1){id:=spawn outer work(1);task scope inner(1){value:=await inner(id)}}}',112),
 'expired_scope': (WORKER+'fn old()->TaskID<i64>{task scope j(1){return spawn j work(1)}}\nfn main(){id:=old();task scope j(1){value:=await j(id)}}',112),
 'scope_reentry': (WORKER+'fn main(){mut ids:[]TaskID<i64> = [];for i in 0..2{task scope j(1){id:=spawn j work(1);if i==1{value:=await j(ids[0])};ids.push(id)}}}',112),
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
        for target in ('arm64-linux','x86_64-linux'):
            for command in ('build','check'):
                cmd=[compiler,command,path,'--target',target]
                if command=='build':cmd+=['-o',out/'unsupported']
                text=run(cmd,1)
                assert 'isolated tasks require ARM64/macOS' in text
        dormant=out/'unreachable.ddr';dormant.write_text('fn unused(){task scope j(1){}}\nfn main(){print(42)}\n')
        for target in ('arm64-macos','arm64-linux','x86_64-linux'):
            run([compiler,'build',dormant,'--target',target,'-o',out/('unreachable-'+target)])
            run([compiler,'check',dormant,'--target',target])
        text=run([compiler,'build',path,'--runtime-profile','-o',out/'profiled'],1)
        assert 'isolated tasks require ARM64/macOS' in text
        summary={'result':'passed','valid':len(VALID),'invalid':len(INVALID),'traps':len(TRAPS),
                 'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest()}
        (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary))
    finally:(out/'commands.json').write_text(json.dumps(log,indent=2)+'\n')

if __name__=='__main__':main()
