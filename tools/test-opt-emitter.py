#!/usr/bin/env python3
"""Exercise scalar backend ABI, liveness, spill paths and checked arithmetic."""
import argparse
from pathlib import Path
import subprocess
import tempfile
import platform as host_platform

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('compiler', type=Path)
args = parser.parse_args()
compiler = args.compiler.resolve(strict=True)
root = ROOT
host = 0 if host_platform.system() == 'Darwin' else (1 if host_platform.machine() in ('arm64', 'aarch64') else 2)
scratch = tempfile.TemporaryDirectory(prefix='dunder-opt-emitter-')
work = Path(scratch.name)
harness = work / 'harness'
built = subprocess.run([compiler, 'build', ROOT / 'tests/fixtures/scalar-emitter.ddr', '-o', harness], capture_output=True, timeout=60)
assert built.returncode == 0, built.stderr.decode(errors='replace')
default=[5,7,11,13,17,19,23,29]
cases=[
 ('params','{p4+p0+p7}',2,2,default,51,0,8),
 ('i32_sign','{p0}',1,1,[4294967295,*default[1:]],-1,0,8),
 ('nested','{(p0+p1)*(p2-p3)+p7}',2,2,default,5,0,8),
 ('if_then','{var answer:i64=0;if p0<p1{answer=p2}else{answer=p3};answer}',2,2,default,11,0,8),
 ('if_else','{var answer:i64=0;if p0>p1{answer=p2}else{answer=p3};answer}',2,2,default,13,0,8),
 ('loop','{var i:=0;var result:i64=0;while i<10{result=result+3;i=i+1};result}',2,2,default,30,0,8),
 ('flow','{var i:=0;var result:=0;while i<10{i=i+1;if i==2{continue};result=result+i;if i==5{break}};result}',1,2,default,13,0,8),
 ('sum','{var i:=0;var result:i64=0;while i<p0.len{result=result+p0[i];i=i+1};result}',2,20,default,42,0,8),
 ('empty','{var i:=0;var result:i64=0;while i<p0.len{result=result+p0[i];i=i+1};result}',2,20,default,0,0,0),
 ('last','{p0[p1]}',2,20,default,6,0,8),
 ('negative','{p0[p1]}',2,20,[5,-1,*default[2:]],0,103,8),
 ('upper','{p0[p1]}',2,20,[5,8,*default[2:]],0,103,8),
 ('short_and','{false && p0/p1>0}',3,2,[5,0,*default[2:]],0,0,8),
 ('short_or','{true || p0/p1>0}',3,2,[5,0,*default[2:]],1,0,8),
 ('bool','{(p0<p1 && p2<p3) || !(p4<p5)}',3,2,default,1,0,8),
 ('wide','{9223372036854775807}',2,2,default,9223372036854775807,0,8),
 ('i64_overflow','{p0+1}',2,2,[9223372036854775807,*default[1:]],0,101,8),
 ('i32_overflow','{p0+1}',1,1,[2147483647,*default[1:]],0,101,8),
 ('div_overflow','{p0/p1}',2,2,[-9223372036854775808,-1,*default[2:]],0,101,8),
 ('rem_overflow','{p0%p1}',2,2,[-9223372036854775808,-1,*default[2:]],0,101,8),
 ('zero','{p0/p1}',2,2,[5,0,*default[2:]],0,102,8),
 ('void','{var answer:i64=5;}',0,2,default,0,0,8),
 ('bare_return','{return}',0,2,default,0,0,8),
]
pressure='{'+''.join(f'a{i}:i64={i+1};' for i in range(40))+'+'.join([*(f'p{i}' for i in range(8)),*(f'a{i}' for i in range(40))])+'}'
cases.append(('pressure',pressure,2,2,default,944,0,8))
for name,body,ret,first,params,result,status,length in cases:
 source=work/(name+'.ddr');source.write_text('p0 p1 p2 p3 p4 p5 p6 p7 '+body)
 for platform in (0,1,2,3):
  out=work/(name+('.wasm' if platform==3 else f'-{platform}'))
  args=[harness,source,out,str(platform),str(result),str(ret),str(first),*map(str,params),str(length)]
  built=subprocess.run(args,capture_output=True,timeout=20)
  assert built.returncode==0,(name,platform,built.returncode,built.stdout,built.stderr)
  if platform==host:
   run=subprocess.run([out],capture_output=True,timeout=10)
   assert run.returncode==status,(name,platform,run.returncode,status,run.stdout,run.stderr)
  if platform==3:
   # Existing runner prints thrown Wasm traps as status125.
   run=subprocess.run(['node',root/'tools/runtime-contract-wasm.cjs',out],capture_output=True,timeout=10)
   expected=125 if status else 0
   assert run.returncode==expected,(name,platform,run.returncode,expected,run.stdout,run.stderr)
 print('PASS',name,flush=True)
print(len(cases),'cases native+Wasm executed; all four targets built')

scratch.cleanup()
