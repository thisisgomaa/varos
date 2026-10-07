#!/usr/bin/env python3
"""Sequential fresh processes; run without concurrent builds/raster jobs."""
from pathlib import Path
import subprocess,sys,resource,platform,json,re
if sys.argv[1]=='--one':
 r=subprocess.run([sys.argv[2],'edits',*sys.argv[3:]],check=False)
 u=resource.getrusage(resource.RUSAGE_CHILDREN)
 print(f'peak_rss_bytes={u.ru_maxrss*(1 if platform.system()=="Darwin" else 1024)}',flush=True)
 sys.exit(r.returncode)
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
exe=root/'target/release/varos-text-spike';results=[]
for count in [10000,100000]:
 for kind in ['long','many']:
  for position in ['start','middle','end']:
   cmd=[sys.executable,__file__,'--one',str(exe),str(count),kind,position]
   r=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
   name=f'edits-{count}-{kind}-{position}';(out/(name+'.log')).write_text(r.stdout)
   match=re.search(r'p95_ms=([\d.]+) max_ms=([\d.]+)',r.stdout)
   passed=r.returncode==0 and match and float(match[1])<=(8 if count==10000 else 50) and (count==10000 or float(match[2])<=100)
   print(name,'PASS' if passed else 'FAIL',r.stdout.splitlines()[-2:],flush=True)
   results.append({'workload':name,'exit':r.returncode,'timing_gate':'PASS' if passed else 'FAIL'})
for count in [1000,10000,100000]:
 r=subprocess.run([sys.executable,str(root/'scripts/measure.py'),str(exe),str(count)],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 (out/f'legacy-41-{count}.log').write_text(r.stdout);print('legacy',count,r.returncode,flush=True)
(out/'benchmark-exits.json').write_text(json.dumps(results,indent=2))
sys.exit(0 if all(r['timing_gate']=='PASS' for r in results) else 1)
