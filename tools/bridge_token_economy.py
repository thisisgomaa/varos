#!/usr/bin/env python3
"""Deterministic slices 1-2 encoder; UTF-8 bytes are not tokenizer measurements."""
import copy
import json
import sys
from pathlib import Path
from collections import Counter

def wire(v): return json.dumps(v,ensure_ascii=False,separators=(",",":"))
def size(v): return len(wire(v).encode())
def total(bs): return sum(map(size,bs))
def creation(o): return o["verb"] in ("add_shape","add_path")
def defaults(ops):
    creations = [o for o in ops if creation(o)]
    d = {}
    for k in ("parent", "fill"):
        if creations and all(k in o for o in creations):
            d[k] = json.loads(Counter(wire(o[k]) for o in creations).most_common(1)[0][0])
    return {"defaults":d,"ops":[{k:v for k,v in o.items()
            if not (creation(o) and k in d and v==d[k])} for o in ops]}
def nameless(b):
    b=copy.deepcopy(b)
    for o in b["ops"]:
        if creation(o): o.pop("name",None)
    return b
def compact(o):
    o=copy.deepcopy(o); v=o.pop("verb")
    if v=="repeat":
        o["ops"]=[compact(x) for x in o["ops"]]
        return {"verb":v,**o}
    if v not in ("add_shape","add_path"): return {"verb":v,**o}
    if v=="add_shape":
        kind=o.pop("kind"); a=[kind,o.pop("bounds")]
    else:
        anchors=o.pop("anchors")
        assert all(set(p)=={"p"} for p in anchors)
        a=["path",[p["p"] for p in anchors],o.pop("closed")]
    if "fill" in o:
        a.append(o.pop("fill"))
        if v=="add_shape" and kind=="rect" and "radius" in o: a.append(o.pop("radius"))
    if o: a.append(o)
    return a
def unpack(a):
    if isinstance(a,dict):
        return {**a,"ops":[unpack(x) for x in a["ops"]]} if a["verb"]=="repeat" else a
    a=copy.deepcopy(a); v=a.pop(0)
    o=({"verb":"add_path","anchors":[{"p":p} for p in a.pop(0)],"closed":a.pop(0)}
       if v=="path" else {"verb":"add_shape","kind":v,"bounds":a.pop(0)})
    if a and not isinstance(a[0],dict):
        o["fill"]=a.pop(0)
        if a and isinstance(a[0],(int,float)): o["radius"]=a.pop(0)
    if a: o.update(a.pop(0))
    assert not a
    return o
def packed(b):
    p={**b,"ops":[compact(o) for o in b["ops"]]}
    assert [unpack(a) for a in p["ops"]]==b["ops"]
    return p
def shift(o,dx,dy):
    o=copy.deepcopy(o)
    if o["verb"]=="add_shape":
        o["bounds"][0]+=dx; o["bounds"][1]+=dy
    elif o["verb"]=="add_path":
        for a in o["anchors"]:
            for key in ("p","hin","hout"):
                if key in a and a[key] is not None:
                    a[key][0]+=dx; a[key][1]+=dy
    elif o["verb"]=="repeat":
        o["ops"]=[shift(x,dx,dy) for x in o["ops"]]
    elif o["verb"]=="bars": o["x"]+=dx; o["y"]+=dy
    else: raise ValueError("not translatable")
    return o
def origin(o):
    if o["verb"]=="add_shape": return o["bounds"][:2]
    if o["verb"]=="add_path": return o["anchors"][0]["p"]
    if o["verb"]=="repeat": return origin(o["ops"][0])
    if o["verb"]=="bars": return [o["x"],o["y"]]
    raise ValueError("not translatable")
def repeats(ops):
    # Dynamic programming: preserve order and exact geometry; no approximate cards.
    n=len(ops); best=[None]*(n+1); best[n]=[]
    for i in range(n-1,-1,-1):
        best[i]=[ops[i]]+best[i+1]
        for length in range(1,(n-i)//2+1):
            base=ops[i:i+length]
            try:
                p,q=origin(base[0]),origin(ops[i+length])
                dx,dy=q[0]-p[0],q[1]-p[1]
                count=1
                while i+(count+1)*length<=n:
                    if [shift(o,count*dx,count*dy) for o in base]!=ops[i+count*length:i+(count+1)*length]: break
                    count+=1
                    r={"verb":"repeat","ops":base,"count":count,"dx":dx,"dy":dy}
                    candidate=[r]+best[i+count*length]
                    if size([compact(x) for x in candidate])<size([compact(x) for x in best[i]]): best[i]=candidate
            except ValueError: pass
    return best[0]

def expand(ops):
    out=[]
    for o in ops:
        if o.get("verb") == "repeat":
            for i in range(o["count"]):
                out += expand([shift(x, i*o["dx"], i*o["dy"]) for x in o["ops"]])
        else:
            out.append(o)
    return out

def encode(raw):
    names=[nameless(defaults(b)) for b in raw]
    repeated=[{**b,"ops":repeats(b["ops"])} for b in names]
    for a,b in zip(names,repeated):
        assert expand(b["ops"])==a["ops"]
    return list(map(packed,repeated))

if __name__ == "__main__":
    fixture=Path(sys.argv[1])
    raw=json.loads(fixture.read_text())
    encoded=encode(raw)
    assert total(raw)==28092
    assert total(encoded)<=9364
    requests=[{"api":"1.1","board":"b1","request_id":f"r{i+1}","expected_rev":i,**b,"receipt":"ids"} for i,b in enumerate(encoded)]
    tasks={}
    for task in ("poster","story"):
        before=json.loads((fixture.parent/f"{task}-request-1.0.json").read_text())["ops"]
        after=[compact(o) for o in before]
        assert [unpack(o) for o in after]==before
        assert size(after)<=size(before)
        tasks[task]=[size(before),size(after)]
    print(wire({"requests":requests,"payload_bytes":total(encoded),"batch_bytes":[size(b) for b in encoded],"baseline_bytes":total(raw),"poster_story_bytes":tasks}))
