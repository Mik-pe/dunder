from pathlib import Path
import importlib.util, subprocess, sys, json, hashlib
sys.dont_write_bytecode = True
root=Path(__file__).resolve().parents[2]
work=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('provider_oracle',root/'tools/test-task-native.py')
provider=importlib.util.module_from_spec(spec);spec.loader.exec_module(provider)
compiler=Path(sys.argv[1]).resolve()
def run(args, output=None):
 r=subprocess.run(list(map(str,args)),cwd=root,capture_output=True,text=True,timeout=30)
 assert r.returncode==0,(args,r.returncode,r.stdout,r.stderr)
 if output is not None:assert r.stdout==output,(args,r.stdout)
 return r
(work/'oracle.S').write_text(provider.asm_probe())
(work/'oracle.c').write_text('''#include <stdint.h>
int64_t sum12(int64_t a,int64_t b,int64_t c,int64_t d,int64_t e,int64_t f,
int64_t g,int64_t h,int64_t i,int64_t j,int64_t k,int64_t l){return a+b+c+d+e+f+g+h+i+j+k+l;}
''')
library=work/'oracle.dylib'
run(['clang','-dynamiclib','-O2',work/'oracle.c',work/'oracle.S','-o',library])
source='''capability unsafe
@extern("@LIB@","abi_probe") unsafe fn probe(f:extern fn(ptr)->ptr,p:ptr)->i32
@extern("@LIB@","sum12") unsafe fn c_sum12(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64,i:i64,j:i64,k:i64,l:i64)->i64
fn internal12(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64,i:i64,j:i64,k:i64,l:i64)->i64{
 var total:i64=0;var index:=0
 while index<50{total=total+a+b+c+d+e+f+g+h+i+j+k+l+i64(index);index=index+1}
 total
}
fn text_total(text:string)->i64{var total:i64=0;for scalar in text.runes(){total=total+i64(scalar)};total}
fn entry(p:ptr)->ptr{
 assert(p==ptr(0))
 assert(internal12(1,2,3,4,5,6,7,8,9,10,11,12)==5125)
 text:="α"+1729.str()+"🌍"
 assert(text_total(text)==128913)
 unsafe{assert(c_sum12(1,2,3,4,5,6,7,8,9,10,11,12)==78)}
 ptr(0)
}
fn main(){unsafe{assert(probe(callback(entry),ptr(0))==1)};print(42)}
'''.replace('@LIB@',str(library))
# ptr() and raw pointer comparisons are ordinary unsafe operations here.
source=source.replace('fn entry(p:ptr)->ptr{','unsafe fn entry(p:ptr)->ptr{')
(work/'probe.ddr').write_text(source)
image=work/'probe'
run([compiler,'build',work/'probe.ddr','-o',image])
run([image],output='42\n')
asm=run(['otool','-tvV',image]).stdout
(work/'probe.asm').write_text(asm)
# The independent ASM oracle resides in a dylib, so25..28 in this disassembly
# belong to actual Dunder functions, never the oracle's sentinel instructions.
lines=[line for line in asm.splitlines() if any('x'+str(reg) in line for reg in range(25,29))]
assert any('stur' in line for line in lines) and any('ldur' in line for line in lines)
receipt={'compiler_sha256':hashlib.sha256(compiler.read_bytes()).hexdigest(),
 'image_sha256':hashlib.sha256(image.read_bytes()).hexdigest(),
 'preserved':'X19..28 + D8..15 + FP/SP; source callback, actual promoted internal12/text_total, C12 args',
 'status':'passed','dunder25_28_instruction_lines':len(lines)}
(work/('result-'+receipt['compiler_sha256'][:12]+'.json')).write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps(receipt,indent=2))
