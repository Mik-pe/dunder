from pathlib import Path
from collections import deque
import json
work=Path(__file__).resolve().parent
variants=[]
for line in (work/'call-input-words.txt').read_text().splitlines():
 p=line.split()
 if p[0]=='BEGIN':
  variant={'target':int(p[1]),'profile':p[2]=='true','entries':{},'ops':{}};variants.append(variant)
 elif p[0]=='ENTRY':variant['entries'][p[1]]=int(p[2])
 elif p[0]=='OP':
  at,width,kind,reads,writes,a,b,c,flow,target,word=map(int,p[1:])
  variant['ops'][at]=(width,kind,reads,writes,a,b,c,flow,target,word)
MASK=sum(1<<r for r in (*range(9,16),17))
def role(r):return 1<<r if r>=0 else 0
results=[]
for v in variants:
 ops=v['ops']; roots=set(v['entries'].values())
 for at,row in ops.items():
  if row[7]==1 and row[8]>=0:roots.add(row[8])
 needs={root:0 for root in roots}; witnesses={}
 def function(root):
  incoming={root:MASK};q=deque([root]);used=0;witness=[]
  while q:
   at=q.popleft();undef=incoming[at]
   width,kind,reads,writes,a,b,c,flow,target,word=ops[at]
   # These6 Wasm custom memory operations were separately audited from the
   # actual wasm_memory producer. All other unknown words refuse this audit.
   if kind==0:
    assert v['target']==3 and word in (4278194176,4278194177),(v['target'],at,word)
    if word==4278194176:reads=1<<12;writes=0
    else:reads=sum(1<<r for r in (10,11,12));writes=reads
   elif kind==14:
    reads=role(a)|role(b)|role(c)
    if flow==1:
     assert target in needs,(at,target)
     reads|=needs[target]
    # Never assume a call overwrites/defines a scratch register. It could
    # preserve the incoming bits, so later local uses remain conservative.
    writes=0
   bad=reads & undef & MASK
   if bad:witness.append((at,[r for r in range(32) if bad&(1<<r)]));used|=bad
   outgoing=undef & ~writes
   if kind in (15,20):continue
   if flow==0:successors=[target]
   elif 2<=flow<=19:successors=[target,at+width]
   else:successors=[at+width]
   for successor in successors:
    if successor==max(ops)+ops[max(ops)][0]:continue
    assert successor in ops,(root,at,successor)
    old=incoming.get(successor)
    if old is None or outgoing|old != old:
     incoming[successor]=(old or 0)|outgoing;q.append(successor)
  return used,witness
 iterations=0
 while True:
  iterations+=1;changed=False
  for root in sorted(roots):
   used,witness=function(root)
   if used|needs[root] != needs[root]:needs[root]|=used;changed=True
   witnesses[root]=witness
  if not changed:break
  assert iterations<=len(roots)*8+1
 bad={name:[r for r in range(32) if needs[root]&(1<<r)] for name,root in v['entries'].items() if needs[root]}
 private_bad={str(root):[r for r in range(32) if mask&(1<<r)] for root,mask in needs.items() if mask}
 results.append({'target':v['target'],'profile':v['profile'],'public_entries':len(v['entries']),
  'all_direct_call_entries':len(roots),'instruction_rows':len(ops),'fixpoint_rounds':iterations,
  'public_scratch_inputs':bad,'all_scratch_inputs':private_bad,
  'witnesses':{str(root):witnesses[root] for root in needs if needs[root]}})
print(json.dumps(results,indent=2))
(work/'call-input-analysis.json').write_text(json.dumps(results,indent=2)+'\n')
