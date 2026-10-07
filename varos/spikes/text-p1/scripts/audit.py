#!/usr/bin/env python3
"""Read-only source/font audit plus reproducible vendor diff and artifact manifest."""
from pathlib import Path
import hashlib,json,difflib,subprocess,sys
root=Path(__file__).resolve().parents[1];out=Path(sys.argv[1]);base=next(Path.home().glob('.cargo/registry/src/*/cosmic-text-0.19.0'));vendor=root/'vendor/cosmic-text'
patch=[];inventory=[]
for p in sorted(vendor.rglob('*')):
 if not p.is_file():continue
 rel=p.relative_to(vendor);pristine=base/rel
 if not pristine.exists():raise RuntimeError(f'unexpected vendor file {rel}')
 if p.read_bytes()!=pristine.read_bytes():
  a=pristine.read_text().splitlines(keepends=True);b=p.read_text().splitlines(keepends=True)
  diff=list(difflib.unified_diff(a,b,fromfile='a/'+str(rel),tofile='b/'+str(rel)))
  patch.extend(diff);inventory.append({'file':str(rel),'added':sum(x.startswith('+') and not x.startswith('+++') for x in diff),'removed':sum(x.startswith('-') and not x.startswith('---') for x in diff)})
(root/'vendor/cosmic-text-0.19.0-p1b.patch').write_text(''.join(patch));(out/'patch-inventory.json').write_text(json.dumps(inventory,indent=2))
fonts=root/'../../crates/varos-app/assets/fonts';manifest=json.loads((fonts/'manifest.json').read_text());records=[]
for f in manifest['files']:
 if f['file'] in ['Inter-Regular.ttf','IBMPlexSansArabic-Regular.ttf']:
  actual=hashlib.sha256((fonts/f['file']).read_bytes()).hexdigest();assert actual==f['sha256'];records.append({'file':f['file'],'sha256':actual,'bytes':(fonts/f['file']).stat().st_size})
(out/'fonts-verified.json').write_text(json.dumps(records,indent=2))
old=out/'p1-preserved/corpus-layouts.txt';new=out/'corpus-layouts.txt'
if old.exists() and new.exists():
 def headers(p):return [x for x in p.read_text().splitlines() if 'pt width=' in x and 'input=' in x]
 assert headers(old)==headers(new),'original configuration identity changed'
 lines=len((out/'nonzero.tsv').read_text().splitlines())-1
 (out/'regression-identity.txt').write_text(f'{len(headers(old))} original configuration headers match exactly.\nOriginal line comparisons 611; revised {lines}. Grouping bidi/style fragments until paragraph UAX14 opportunities changes line count; no corpus row/input removed.\n')
metadata={}
for name,cmd in [('rustc',['rustc','-Vv']),('host',['uname','-a']),('os',['sw_vers']),('node',['node','--version'])]:
 metadata[name]=subprocess.run(cmd,text=True,stdout=subprocess.PIPE).stdout
metadata['sizes']={str(p.relative_to(root)):p.stat().st_size for p in [root/'target/release/varos-text-spike',root/'target/wasm32-unknown-unknown/release/varos_text_spike.wasm'] if p.exists()}
metadata['build_sha256']={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [root/'target/release/varos-text-spike',root/'target/wasm32-unknown-unknown/release/varos_text_spike.wasm'] if p.exists()}
sources=[root/'Cargo.toml',root/'Cargo.lock',root/'README.md',root/'vendor/cosmic-text-0.19.0-p1b.patch']
for folder in ['src','tests','examples','scripts']:
 sources.extend(p for p in (root/folder).rglob('*') if p.is_file() and '__pycache__' not in p.parts)
metadata['source_contract_sha256']={str(p.relative_to(root)):hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(sources)}
(out/'environment.json').write_text(json.dumps(metadata,indent=2))
# Build artifacts under throwaway harness targets are reproducible, not proof inputs.
artifacts={str(p.relative_to(out)):{'sha256':hashlib.sha256(p.read_bytes()).hexdigest(),'bytes':p.stat().st_size} for p in sorted(out.rglob('*')) if p.is_file() and 'target' not in p.relative_to(out).parts and p.name!='artifact-inventory.json'}
(out/'artifact-inventory.json').write_text(json.dumps(artifacts,indent=2))
print(json.dumps({'patch':inventory,'proof_files':len(artifacts)},indent=2))
