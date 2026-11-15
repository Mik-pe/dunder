from pathlib import Path
import hashlib, json, subprocess
root=Path.cwd(); out=root/'build/compiler-dogfood/typed-nodes-identity'; out.mkdir(exist_ok=True)
compilers=[root/'build/compiler-dogfood/range-proofs-tuned',root/'build/selfhost/arm64-macos/dunder']
cases={
'arithmetic':'fn step(x:i64,y:i64)->i64{((x*3+y)/7)+(x%(-5))} fn yes(x:i64,y:i64)->bool{x!=0&&y/x>0} fn main(){print(step(123,17));print(yes(2,14))}',
'arrays':'fn walk(a:&mut []i64)->i64{var sum:i64=0;for i in 0..a.len{if i==2{continue};a[i]=a[i]+1;sum=sum+a[i]};var j:=0;while j<a.len{sum=sum+a[j];j=j+1};sum} fn main(){var a:[]i64=[1,2,3,4];print(walk(&mut a))}',
'spills':'fn sum(x:i64)->i64{a:=x+1;b:=x+2;c:=x+3;d:=x+4;e:=x+5;f:=x+6;g:=x+7;h:=x+8;i:=x+9;j:=x+10;k:=x+11;l:=x+12;m:=x+13;a+b+c+d+e+f+g+h+i+j+k+l+m} fn main(){print(sum(1))}',
'arguments':'fn mix(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{a+2*b+3*c+4*d+5*e+6*f+7*g+8*h} fn main(){print(mix(1,2,3,4,5,6,7,8))}',
'ownership':(root/'tests/fixtures/ownership_leases.ddr').read_text(),
'places':(root/'tests/fixtures/wasm_scalars.ddr').read_text(),
}
records=[]
for name,text in cases.items():
 source=out/(name+'.ddr'); source.write_text(text)
 for target in ('arm64-macos','arm64-linux','x86_64-linux','wasm32'):
  for range_mode in ('off','on'):
   flags=['--target',target,'--range-opt='+range_mode]
   images=[]; checks=[]
   for compiler in compilers:
    result=subprocess.run([compiler,'build',source,'-o',out/'image',*flags],capture_output=True,text=True)
    assert result.returncode==0,(name,target,range_mode,str(compiler),result.stderr)
    images.append((out/'image').read_bytes())
    checked=subprocess.run([compiler,'check',source,'--agent',*flags],capture_output=True)
    assert checked.returncode==0,(name,target,checked.stderr)
    checks.append((checked.stdout,checked.stderr))
   assert images[0]==images[1],('image mismatch',name,target,range_mode)
   assert checks[0]==checks[1],('check mismatch',name,target,range_mode)
   records.append({'case':name,'target':target,'range':range_mode,'image_sha256':hashlib.sha256(images[0]).hexdigest(),'bindings_sha256':hashlib.sha256(checks[0][0]).hexdigest()})
metadata={'compiler_sha256':[hashlib.sha256(c.read_bytes()).hexdigest() for c in compilers],'records':records}
(out/'report.json').write_text(json.dumps(metadata,indent=2))
print(str(len(records))+' all-target/range-mode exact image and binding comparisons passed')
