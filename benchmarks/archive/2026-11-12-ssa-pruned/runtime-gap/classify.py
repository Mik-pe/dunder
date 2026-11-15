from pathlib import Path
from collections import Counter
import json,re,hashlib
root=Path(__file__).resolve().parents[3]; out=Path(__file__).resolve().parent

def image(name):
 rows={}
 for line in (out/(name+'.asm')).read_text().splitlines():
  m=re.match(r'^([0-9a-f]{16})\s+(\S+)\s*(.*)$',line)
  if m:rows[int(m[1],16)]={'address':m[1],'mnemonic':m[2],'operands':m[3],'raw':line}
 return rows

def region(start,end):return list(range(int(start,16),int(end,16)+4,4))
def old_path(name):
 return [int(m[1],16) for line in (root/'benchmarks/2026-11-11-whole-function-optimization/runtime-attribution'/(name+'-selected-path.asm')).read_text().splitlines() if (m:=re.match(r'^([0-9a-f]{9,16})\s',line))]

def classify(name,addresses,description,closed=True):
 rows=image(name); selected=[rows[a] for a in addresses]; assert selected,name
 # Mechanically ensure the written selected path follows real branch/fallthrough
 # edges. Conditional outcomes are explicit hypotheses, not sampled frequencies.
 if closed:
  for i,a in enumerate(addresses):
   row=rows[a]; following=addresses[(i+1)%len(addresses)]
   targets=re.findall(r'0x[0-9a-f]+',row['operands'])
   if row['mnemonic']=='b':assert targets and following==int(targets[-1],16),(name,row,hex(following))
   elif row['mnemonic'].startswith('b.') or row['mnemonic'] in ('cbz','cbnz','tbz','tbnz'):
    assert targets and following in (a+4,int(targets[-1],16)),(name,row,hex(following))
   else:assert following==a+4,(name,row,hex(following))
 counts=Counter(r['mnemonic'] for r in selected)
 memory=Counter()
 for row in selected:
  mn=row['mnemonic']; op=row['operands']
  if not mn.startswith(('ldr','ldur','str','stur','stp','ldp')):continue
  domain='frame' if '[x29' in op else 'expression_stack' if '[sp' in op else 'input_or_managed_heap'
  direction='load' if mn.startswith(('ld','ldr')) else 'store'
  memory[domain+'_'+direction+'_instructions']+=1
 result={'image':name,'path_hypothesis':description,'classification':'selected feasible static instruction path; no execution/frequency/timing claim' if closed else 'static address interval; includes all contained branch alternatives, not a periteration count','instruction_count':len(selected),'mnemonics':dict(counts),'memory':dict(memory),'calls':[r['address'] for r in selected if r['mnemonic'] in ('bl','blr')],'addresses':[r['address'] for r in selected]}
 return result

paths={
 'text-off-ASCII-inside':classify('text-baseline',old_path('text'),'ASCII nonseparator, already inside word, cursor<len, checked sum does not overflow'),
 'text-on-ASCII-inside':classify('text-candidate',region('100000e30','100000e58')+region('100000ec0','100000f1c')+region('100000f34','100000f40')+[int('100000f6c',16)],'ASCII nonseparator, already inside word, cursor<len, checked sum does not overflow'),
 'text-C-ASCII-inside':classify('text-c',region('1000004f4','100000500')+region('10000058c','100000598')+region('1000004ec','1000004f0'),'ASCII byte>32, already inside word, nonzero byte, checked sum does not overflow'),
 'sort-off-shift':classify('sort-baseline',old_path('sort'),'successful insertion shift with j>0 and next j>0'),
 'sort-on-shift':classify('sort-candidate',region('10000101c','10000105c'),'successful insertion shift with j>0 and next j>0'),
 'sort-C-shift':classify('sort-c',region('100000550','100000568'),'successful insertion shift with next j>0'),
 'sort-on-checksum':classify('sort-candidate',region('1000004c0','100000580'),'checksum loop has another element, index valid and all checked arithmetic succeeds'),
 'sort-C-checksum':classify('sort-c',region('10000057c','1000005a0'),'checksum loop has another element and all checked arithmetic succeeds'),
}
# Avoid overwriting named candidate path artifacts when one image has two paths.
for key,result in paths.items():
 rows=image(result['image'])
 (out/(key+'.asm')).write_text('// '+result['classification']+'\n// '+result['path_hypothesis']+'\n'+'\n'.join(rows[int(a,16)]['raw'] for a in result['addresses'])+'\n')
(out/'selected-paths.json').write_text(json.dumps(paths,indent=2)+'\n')
for key,r in paths.items():print(key,r['instruction_count'],r['memory'],'NOP',r['mnemonics'].get('nop',0),'calls',len(r['calls']))
# Preserve exact evidence for the current implementation limitations.
meta=json.loads((root/'build/ssa-pruned/user-profile/metadata.json').read_text())
(out/'source').mkdir(exist_ok=True)
for rel in ('selfhost/mir_ssa.ddr','selfhost/mir_ssa_cells.ddr','selfhost/mir_value_prop.ddr','selfhost/mir_memory.ddr','selfhost/mir_pipeline.ddr','benchmarks/runtime/text.ddr','benchmarks/runtime/sort.ddr','benchmarks/runtime/reference.c'):
 p=root/rel; digest=hashlib.sha256(p.read_bytes()).hexdigest(); assert digest==meta['sources_sha256'][rel],rel
 (out/'source'/p.name).write_bytes(p.read_bytes())
