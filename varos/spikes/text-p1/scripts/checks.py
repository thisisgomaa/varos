#!/usr/bin/env python3
from pathlib import Path
import subprocess,sys,json,shutil,platform
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
combined=out/'harnesses/combined/Cargo.toml'
commands=[
 ('native-tests',['cargo','--offline','test','--no-fail-fast','--release','--manifest-path',str(root/'Cargo.toml')]),
 ('native-clippy',['cargo','--offline','clippy','--manifest-path',str(root/'Cargo.toml'),'--all-targets','--','-D','warnings']),
 ('windows-clippy',['cargo','--offline','clippy','--manifest-path',str(root/'Cargo.toml'),'--all-targets','--target','x86_64-pc-windows-msvc','--','-D','warnings']),
 ('wasm-build',['cargo','--offline','build','--release','--lib','--target','wasm32-unknown-unknown','--manifest-path',str(root/'Cargo.toml')]),
 ('combined-native',['cargo','--offline','clippy','--manifest-path',str(combined),'--all-targets','--','-D','warnings']),
 ('combined-windows',['cargo','--offline','clippy','--manifest-path',str(combined),'--all-targets','--target','x86_64-pc-windows-msvc','--','-D','warnings']),
 ('combined-wasm',['cargo','--offline','check','--manifest-path',str(combined),'--lib','--target','wasm32-unknown-unknown']),
 ('engine-features',['cargo','--offline','tree','--manifest-path',str(root/'Cargo.toml'),'-e','features']),
 ('combined-features',['cargo','--offline','tree','--manifest-path',str(combined),'-e','features','-i','fontdb']),
]
results={}
for name,cmd in commands:
 r=subprocess.run(cmd,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
 (out/(name+'.log')).write_text('$ '+' '.join(cmd)+'\n'+r.stdout)
 results[name]={'exit':r.returncode,'command':cmd}
 (out/'gate-exits.json').write_text(json.dumps(results,indent=2))
 print(name,r.returncode,flush=True)
 if any(x in r.stdout for x in ['attempting to make an HTTP request','no matching package named','failed to download']):
  print(r.stdout,flush=True);sys.exit(2)

sys.exit(1 if any(r["exit"] for r in results.values()) else 0)
