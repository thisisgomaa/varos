#!/usr/bin/env python3
"""Fixed policy: exact identities; <=.001pt floats; per-image mean<=1, no alpha delta>32."""
from pathlib import Path
import sys,json,itertools
native=Path(sys.argv[1]).read_text().splitlines();wasm=Path(sys.argv[2]).read_text().splitlines()
assert len(native)==len(wasm),(len(native),len(wasm))
max_float=0.;max_alpha=0;worst_mean=0.;fixtures=0;images=0
for i,(a,b) in enumerate(zip(native,wasm)):
 aa=a.split('\t');bb=b.split('\t');assert aa[0]==bb[0],i
 if aa[0]=='float':
  assert len(aa)==len(bb)
  delta=max(abs(float(x)-float(y)) for x,y in zip(aa[1:],bb[1:]));max_float=max(max_float,delta);assert delta<=.001,(i,a,b)
 elif aa[0]=='alpha':
  # Compare RLE without expanding page-sized buffers.
  ar=[list(map(int,x.split(':'))) for x in aa[1:]];br=[list(map(int,x.split(':'))) for x in bb[1:]]
  ai=bi=0;total=error=severe=0
  while ai<len(ar) and bi<len(br):
   n=min(ar[ai][1],br[bi][1]);d=abs(ar[ai][0]-br[bi][0]);total+=n;error+=n*d;severe+=n*(d>32);max_alpha=max(max_alpha,d if n else 0)
   ar[ai][1]-=n;br[bi][1]-=n
   if not ar[ai][1]:ai+=1
   if not br[bi][1]:bi+=1
  assert ai==len(ar) and bi==len(br),(i,'pixel lengths')
  mean=error/max(total,1);worst_mean=max(worst_mean,mean);images+=1;assert mean<=1 and severe==0,(i,mean,severe)
 else:
  assert a==b,(i,a,b)
  fixtures+=aa[0]=='fixture'
print(json.dumps(dict(result='PASS',fixtures=fixtures,images=images,max_float_pt=max_float,max_alpha=max_alpha,worst_mean_alpha=worst_mean),indent=2))
