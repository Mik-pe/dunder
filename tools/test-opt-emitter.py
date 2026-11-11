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
weighted = '+'.join(f'p0[{i}]*{i+1}' for i in range(8))
cases.extend([
 ('store_rhs_reads','{p0[p1]=p0[p1+1]+p0[p1+2];p0[p1]}',2,21,[5,0,*default[2:]],4,0,8),
 ('store_index','{p0[p1+1]=p0[p1];p0[p1+1]}',2,21,[5,1,*default[2:]],-7,0,8),
 ('store_lhs_fault_first','{p0[p1]=p0[0]/p2;0}',2,21,[5,-1,0,*default[3:]],0,103,8),
 ('store_rhs_zero','{p0[p1]=p0[0]/p2;0}',2,21,[5,0,0,*default[3:]],0,102,8),
 ('store_rhs_bounds','{p0[p1]=p0[p1+8];0}',2,21,[5,0,*default[2:]],0,103,8),
 ('store_index_overflow','{p0[p1+1]=0;0}',2,21,[5,2147483647,*default[2:]],0,101,8),
 ('store_empty','{p0[p1]=0;0}',2,21,[5,0,*default[2:]],0,103,0),
 ('for_signed','{var total:=0;for i in -3..4{total=total+i};total}',1,2,default,0,0,8),
 ('for_empty','{var total:=0;for i in 5..5{total=total+1};total}',1,2,default,0,0,8),
 ('for_reversed','{var total:=0;for i in 5..2{total=total+1};total}',1,2,default,0,0,8),
 ('for_flow','{var total:=0;for i in 0..8{if i==2{continue};if i==6{break};total=total+i};total}',1,2,default,13,0,8),
 ('for_nested','{var total:=0;for i in 0..4{for j in 0..3{if j==1{continue};total=total+i}};total}',1,2,default,12,0,8),
 ('for_with_while','{var total:=0;for i in 0..4{var j:=0;while j<3{j=j+1;if j==2{continue};total=total+i};if i==1{continue};total=total+10};total}',1,2,default,42,0,8),
 ('for_return','{for i in 0..4{if i==2{return i}};return -1}',1,2,default,2,0,8),
 ('for_max','{var total:=0;for i in 2147483646..2147483647{total=total+1};total}',1,2,default,1,0,8),
 ('for_min','{var total:=0;for i in -2147483648..-2147483647{total=total+1};total}',1,2,default,1,0,8),
 ('for_high_snapshot','{var high:=4;var total:=0;for i in 0..high{high=0;total=total+1};total}',1,2,default,4,0,8),
 ('for_bound_home_reuse','{outer:=5;var total:=0;for i in outer-1..outer{total=total+i};total}',1,2,default,4,0,8),
 ('for_shadow_bounds','{var i:=5;var total:=0;for i in i-1..i+1{total=total+i};total+i}',1,2,default,14,0,8),
 ('for_low_fault_first','{for i in (p0+1)..(p0/0){};0}',2,1,[2147483647,*default[1:]],0,101,8),
 ('for_high_fault','{for i in 5..(p0/0){};0}',2,1,default,0,102,8),
 ('for_mutation','{var i:=0;for i in 0..p0.len{p0[i]=p0[i]+1};var total:i64=0;while i<p0.len{total=total+p0[i];i=i+1};total}',2,21,default,50,0,8),
 ('for_sort','{for i in 1..p0.len{value:=p0[i];var j:=i;while j>0 && p0[j-1]>value{p0[j]=p0[j-1];j=j-1};p0[j]=value};'+weighted+'}',2,21,default,311,0,8),
 ('for_reverse','{for i in 0..4{value:=p0[i];p0[i]=p0[7-i];p0[7-i]=value};'+weighted+'}',2,21,default,159,0,8),
])
for_pressure='{'+''.join(f'a{i}:i64={i+1};' for i in range(40))+'var total:i64=0;for i in 0..3{total=total+'+'+'.join(f'a{i}' for i in range(40))+'};total+'+'+'.join(f'p{i}' for i in range(8))+'}'
cases.append(('for_pressure',for_pressure,2,2,default,2584,0,8))
for name,body,ret,first,params,result,status,length in cases:
 source=work/(name+'.ddr');source.write_text('p0 p1 p2 p3 p4 p5 p6 p7 '+body)
 for platform in (0,1,2,3):
  out=work/(name+('.wasm' if platform==3 else f'-{platform}'))
  wrapper=out.with_name(out.name+'.wrapper')
  for mode,destination in [('wrapper',wrapper),('prepared',out)]:
   args=[harness,source,destination,str(platform),str(result),str(ret),str(first),*map(str,params),str(length),mode]
   built=subprocess.run(args,capture_output=True,timeout=20)
   assert built.returncode==0,(name,platform,mode,built.returncode,built.stdout,built.stderr)
  assert wrapper.read_bytes()==out.read_bytes(),(name,platform,'prepared/wrapper image mismatch')
  if platform==host:
   run=subprocess.run([out],capture_output=True,timeout=10)
   assert run.returncode==status,(name,platform,run.returncode,status,run.stdout,run.stderr)
  if platform==3:
   # Existing runner prints thrown Wasm traps as status125.
   run=subprocess.run(['node',root/'tools/runtime-contract-wasm.cjs',out],capture_output=True,timeout=10)
   expected=125 if status else 0
   assert run.returncode==expected,(name,platform,run.returncode,expected,run.stdout,run.stderr)
 print('PASS',name,flush=True)
print(len(cases),'cases native+Wasm executed; wrapper/prepared images byte-identical on all four targets')

scratch.cleanup()
