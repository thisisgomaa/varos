#!/usr/bin/env python3
from pathlib import Path
import subprocess,sys,resource,platform
if sys.argv[1]=='--one':
 r=subprocess.run(sys.argv[2:]);u=resource.getrusage(resource.RUSAGE_CHILDREN)
 print(f'peak_rss_bytes={u.ru_maxrss*(1 if platform.system()=="Darwin" else 1024)}');sys.exit(r.returncode)
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]);failed=False
for count in [10000,100000]:
 for kind in ['long','many']:
  for control in ['width','font','language']:
   r=subprocess.run([sys.executable,__file__,'--one',str(root/'target/release/examples/control_bench'),str(count),kind,control],stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
   name=f'control-{count}-{kind}-{control}';(out/(name+'.log')).write_text(r.stdout);print(name,r.returncode,r.stdout.splitlines()[-2:],flush=True);failed|=bool(r.returncode)
sys.exit(1 if failed else 0)
