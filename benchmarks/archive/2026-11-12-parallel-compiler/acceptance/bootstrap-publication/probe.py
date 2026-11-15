from pathlib import Path
import subprocess,hashlib,gzip,os,json,shutil,tarfile,io
OUT=Path(__file__).resolve().parent;ROOT=Path('/Users/mikpe/dev/dunder-language');REV='60b9c6beebef0047256330ca11795aab6bf07467'
LOG=[];RESULT=[]
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def call(args,cwd,env=None,status=0):
 r=subprocess.run([str(x) for x in args],cwd=cwd,env=env,text=True,capture_output=True,timeout=120)
 row={'command':[str(x) for x in args],'cwd':str(cwd),'exit':r.returncode,'stdout':r.stdout,'stderr':r.stderr};LOG.append(row)
 assert r.returncode==status,row
 return r
scripts={name:subprocess.check_output(['git','show',REV+':tools/'+name],cwd=ROOT) for name in ('bootstrap.sh','bootstrap-parallel.sh')}
stub=b'''#!/usr/bin/env python3
from pathlib import Path
import sys,os,json
args=sys.argv[1:]
with open(os.environ['QA_EVENTS'],'a') as f:f.write(json.dumps({'compiler':str(Path(__file__).resolve()),'args':args})+'\\n')
assert args[0]=='build' and '-o' in args,args
out=Path(args[args.index('-o')+1])
jobs=args[args.index('--jobs')+1] if '--jobs' in args else 'serial'
late=args[1]=='selfhost/main_parallel.ddr' and jobs=='4'
if late and os.environ.get('QA_FAIL_STAGE')=='exit':
 print('injected jobs4 failure after serial and jobs1 fixed points',file=sys.stderr);sys.exit(73)
data=Path(__file__).read_bytes()
if late and os.environ.get('QA_FAIL_STAGE')=='mismatch':data+=b'\\n# injected fixed-point mismatch\\n'
out.write_bytes(data);out.chmod(0o755)
print('built '+str(out))
'''
def events(path):
 return [json.loads(x) for x in path.read_text().splitlines()] if path.exists() else []
def fixture(name,system,machine,host,fail=''):
 d=OUT/name;d.mkdir();(d/'tools').mkdir();(d/'bootstrap').mkdir();(d/'bin').mkdir()
 for n,b in scripts.items():(d/'tools'/n).write_bytes(b);(d/'tools'/n).chmod(0o755)
 uname=d/'bin/uname';uname.write_text('#!/bin/sh\ncase "$1" in -s) echo "$QA_UNAME_S";; -m) echo "$QA_UNAME_M";; *) echo "$QA_UNAME_S $QA_UNAME_M";; esac\n');uname.chmod(0o755)
 packed=gzip.compress(stub,mtime=0);(d/'bootstrap'/(host+'.gz')).write_bytes(packed)
 (d/'bootstrap'/(host+'.sha256')).write_text(hashlib.sha256(packed).hexdigest()+'  '+host+'.gz\n')
 dest=d/'build/selfhost'/host;dest.mkdir(parents=True)
 shutil.copy2(OUT/'dunder',dest/'dunder');(dest/'dunder-parallel').write_text('old parallel sentinel\n')
 env={**os.environ,'PATH':str(d/'bin')+':'+os.environ['PATH'],'QA_UNAME_S':system,'QA_UNAME_M':machine,'QA_EVENTS':str(d/'events.jsonl'),'QA_FAIL_STAGE':fail}
 return d,dest,env
try:
 for system,machine,host in [('Darwin','arm64','arm64-macos'),('Linux','aarch64','arm64-linux'),('Linux','x86_64','x86_64-linux')]:
  d,dest,env=fixture('serial-'+host,system,machine,host)
  before=(sha(dest/'dunder'),(dest/'dunder').stat().st_ino,(dest/'dunder').stat().st_mtime_ns)
  parallel_before=(dest/'dunder-parallel').read_bytes()
  r=call(['./tools/bootstrap.sh','--serial'],d,env)
  after=(sha(dest/'dunder'),(dest/'dunder').stat().st_ino,(dest/'dunder').stat().st_mtime_ns)
  ev=events(d/'events.jsonl');assert before==after and len(ev)==2,(before,after,ev)
  assert r.stdout==str(dest/'dunder-serial')+'\n',r.stdout
  assert (dest/'dunder-serial').read_bytes()==stub
  assert (dest/'dunder-parallel').read_bytes()==parallel_before
  assert all(e['args'][1]!='selfhost/main_parallel.ddr' for e in ev)
  RESULT.append({'case':'serial-'+host,'scope':'stubbed shell flow; no Linux binary execution','standard_sha_inode_mtime_unchanged':True,'serial_fixpoint_builds':len(ev),'returned':r.stdout.strip()})
 for failure,exit_code in [('exit',73),('mismatch',1)]:
  d,dest,env=fixture('late-'+failure,'Darwin','arm64','arm64-macos',failure)
  before=(sha(dest/'dunder'),(dest/'dunder').stat().st_ino,(dest/'dunder').stat().st_mtime_ns);oldparallel=(dest/'dunder-parallel').read_bytes()
  r=call(['./tools/bootstrap.sh'],d,env,status=exit_code)
  after=(sha(dest/'dunder'),(dest/'dunder').stat().st_ino,(dest/'dunder').stat().st_mtime_ns)
  ev=events(d/'events.jsonl');assert before==after and len(ev)==5,(before,after,ev)
  assert r.stdout=='' and (dest/'dunder-serial').read_bytes()==stub and (dest/'dunder-parallel').read_bytes()==oldparallel
  assert ev[-1]['args'][1]=='selfhost/main_parallel.ddr' and ev[-1]['args'][ev[-1]['args'].index('--jobs')+1]=='4'
  retained=list(dest.glob('.parallel.*'));assert len(retained)==1 and (retained[0]/'one-worker').read_bytes()==stub
  assert not list(dest.glob('.rebuild.*'))
  RESULT.append({'case':'late-'+failure,'scope':'stubbed shell flow, real cmp/SHA/gzip/mv tools','serial_fixed_point_reached':True,'jobs1_fixed_point_reached':True,'standard_sha_inode_mtime_unchanged':True,'parallel_sentinel_unchanged':True,'stdout':r.stdout,'exit':r.returncode,'retained':str(retained[0])})
 print('STUB_PUBLICATION_CONTRACTS_PASS',flush=True)
 # Native compiler images are real and frozen. Only serial bootstrap delegation
 # is stubbed, to avoid repeating seed self-builds already covered above.
 real=OUT/'real-standalone';real.mkdir()
 archive=subprocess.check_output(['git','archive',REV],cwd=ROOT)
 with tarfile.open(fileobj=io.BytesIO(archive)) as tf:tf.extractall(real,filter='data')
 delegate=real/'tools/bootstrap.sh'
 delegate.write_text('#!/bin/sh\nset -eu\nprintf "%s\\n" "$*" >> "$QA_DELEGATE"\n[ "$#" -eq 1 ] && [ "$1" = --serial ]\nprintf "%s\\n" "$QA_REAL_SERIAL"\n');delegate.chmod(0o755)
 env={**os.environ,'QA_DELEGATE':str(real/'delegate.log'),'QA_REAL_SERIAL':str(OUT/'dunder-serial')}
 r=call(['./tools/bootstrap-parallel.sh'],real,env)
 image=real/'build/selfhost/arm64-macos/dunder-parallel'
 assert r.stdout==str(image)+'\n' and image.exists(),r.stdout
 assert (real/'delegate.log').read_text()=='--serial\n'
 assert sha(image)==sha(OUT/'dunder'),(sha(image),sha(OUT/'dunder'))
 assert not list(image.parent.glob('.parallel.*'))
 # Execute its public CLI and build/run a consumer with the published image.
 help_result=call([image,'--help'],real);assert 'default 4' in help_result.stdout
 source=real/'smoke.ddr';source.write_text('fn main(){print(42)}\n');program=real/'smoke'
 call([image,'build',source,'-o',program],real);run=call([program],real);assert run.stdout=='42\n'
 RESULT.append({'case':'standalone-noarg','scope':'three real ARM64 macOS compiler producer builds and consumer execution; only bootstrap.sh delegation stubbed','delegate_args':['--serial'],'published_sha256':sha(image),'expected_sha256':sha(OUT/'dunder'),'smoke_stdout':run.stdout,'returned':r.stdout.strip(),'recursion':False})
 print('REAL_STANDALONE_BBD_PASS',flush=True)
finally:
 (OUT/'commands.json').write_text(json.dumps(LOG,indent=2)+'\n')
 (OUT/'results.json').write_text(json.dumps(RESULT,indent=2)+'\n')
 print('ARTIFACTS',OUT,flush=True)
