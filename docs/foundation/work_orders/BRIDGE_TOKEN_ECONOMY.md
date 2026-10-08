# Bridge token economy — proposed work order, 2026-10-08

Design only, against `243f9fd`; **not commissioned** until Ahmed approves the [ADR amendment](../../adr/ADR-0009-varos-bridge.md#amendment--token-economy-proposed-2026-10-08).
No implementation, commit, push or owner acceptance is claimed.

## Measured result and limits

Wireframe cumulative bytes: **28,092 → 23,677 defaults → 18,627 optional names → 9,910 compact → 7,960 repeat → 7,855 bars**.
Clone alternative: **7,998**, so rejected for this sample. Inline group: zero saving because the source has no groups.
Future text: zero credited; no real strings/font/layout were supplied. The 199 unnamed leaves remain editable and ordered.
Poster ops: **708 → 508**; story ops: **738 → 618** using compact creation only, preserving every name/local and all other operations.
These last two are offline fixture encodings, not new agent runs.

The embedded script verifies source hashes, counts UTF-8 bytes including new defaults/ops wrappers,
round-trips tuples, and expands structural candidates back to the original operations except intentionally omitted names.
It preserves all coordinates/paint/order, not just a similar-looking image. It does not implement or execute Bridge operations.
The 9,364-byte ceiling is deliberately above the measured 7,960 repeat result; bars/group/clone are not prerequisites.
No tokenizer is available in this experiment: the owner's “about 7,000” is a heuristic.
Dividing by four would suggest about 1,964 at 7,855 bytes, **not measured tokens**; numeric arrays may have a different ratio.

Receipt assumptions: one diff per batch; 70/91/38 new paths, four-digit mock path IDs, node:1 parent, short board/request IDs,
empty selection, one changed layer, three changed revisions, one new page/local, ordinary finite coordinates.
From `service.rs::projection/changes/edit_receipt_reserved`, each created-path proxy is 188–208 bytes:
13,832 / 18,208 / 7,604 bytes before headers. The first two exceed the 12,288-minus-locals fallback threshold.
Budget roughly 9–11 KiB structured across the actual three paginated receipts, plus 6–9 KiB compact text
(`mcp.rs::tool_result` sends both). This is an estimate, not an observed transcript; no detail-follow-up calls are assumed.
IDs-mode proxy totals 2,959 structured bytes; allow about 5–6 KiB with its readable projection. New local bindings can increase it.
The third full receipt fits; receipts are neither 201 identical acknowledgments nor all 39,644 bytes of unpaginated detail.
Actual IDs, floating-point output, board header, selection, page changes and client projection can change these estimates.

Snapshot assumptions: all three reported 1024-px shots target the 1440×3000 page.
Code uses nearest-integer aspect fit: each 492×1024 vs proposed 246×512, totaling 1,511,424 vs 377,856 pixels.
Raw RGBA totals are 6,045,696 vs 1,511,424 bytes; **neither is PNG size or image-token cost**.
PNG data were not supplied; base64 wire bytes = 4×ceil(PNG bytes/3); current source-PNG cap = 783,360 bytes/image.
If the original shots covered the whole board or explicit square dimensions, record that separately and recompute.
Economy profile is an explicit new API 1.1 option; unprofiled requests preserve 544×246 board / 1024×1024 page defaults.
The three failed describes and agent planning are unknown; the separately fixed describe composition bug earns no savings here.

## Same-task measurement and gate

Freeze copies of the three original wireframe files + generator, hashes, poster/story 1.0 fixtures, prompt, initial board,
model/client build, API/schema, rendering settings and hardware in new benchmark fixtures when implementation is authorized.
Run each task on a clean equivalent board before/after, preserving the wireframe's three transaction boundaries and the poster/story's one.
Keep final intended content, order, parent tree, paint, geometry and editable objects equal except the explicitly omitted wireframe names.
Replay deterministic fixtures first; then run at least three paired agent attempts per task with the same prompt/model/settings.
Record all attempts, median/range, completion rate, repair calls, visual quality and owner feedback; never discard costly failures.

For each task report: compact ops+defaults bytes; complete request and receipt bytes; describe/error/schema text bytes;
actual provider/model tokenizer counts for each text category; raw PNG and base64 wire bytes; provider-reported image usage.
Use client traces to establish whether text, structuredContent or both enter context; measure rather than double-count or assume suppression.
Report model input/output/cache usage and planning separately; compare warm discovery and cold discovery, including tools/list schema overhead.
Images are reported separately from text tokens unless provider usage permits an honest combined total; unavailable counts stay “unknown”.
Use the same three snapshot checkpoints for wireframe; report low-resolution quality/repair cost and compare explicit 1024 previews for equivalence.
Poster/story use the same read/snapshot checkpoints on both sides. Do not request new snapshots merely to inflate a baseline.

Gate: deterministic wireframe payload **≤9,364 bytes** with exact expansion equivalence; no new mutation semantics or altered art.
Poster/story must not regress request bytes. End-to-end median measured total model tokens must decrease on each task,
with no lower completion/visual/editability quality; if image accounting is unavailable, token gate remains unverified.
Verify one undo/redo per original batch, failed nested-op rollback, unchanged selection/defaults, stale revisions,
same-request retry, changed-spelling conflict, and boundary/adversarial expansion limits through MCP and CLI.
Only add versioned fixtures; all 1.0 frozen fixtures remain byte-identical. CPU checks never construct GPU Renderer/EventLoop.
Owner live review is separate from passing automated gates; no owner acceptance is inferred from this encoding experiment.

## Implementation slices, after approval

1. **Encoding and response economy:** additive 1.1 negotiation/schemas/CLI parity, creation defaults, existing optional names plus stable auto-label policy,
   rect/ellipse/path tuples, IDs receipts, opt-in economy snapshots and summary budget, one capabilities hint.
   Gate: **9,910 wireframe ops bytes**, poster/story 508/618, tuple/default/null/override/schema rejection and old-fixture parity;
   same-task receipt/read/image accounting, idempotency and rollback. Most savings with no new document operation.
2. **Exact repeat:** bounded creation-only expansion, nested grids, translated handles, suffix/local resolution and error paths.
   Gate: **7,960 bytes**, ≤9,364 ceiling, original 201 expanded ops across 72/91/38 batches, deterministic same-art proof,
   nested collision/overflow/cancellation/rollback and one-step undo. Re-run end-to-end token and owner quality gate.
3. **Optional structural conveniences:** bars, inline group and clone, each separately advertised; no general template/layout language.
   Gate: bars **7,855 bytes**, clone candidate explicitly **7,998** (do not replace the cheaper fixture), group tree/clone fresh-ID
   and inherited-transform/paint parity, editable placeholder rectangles, strict expansion/target bounds.
   Defer this slice if its modest sample savings do not justify effort; ADR-0010 add_text remains the real text work.

## Reproducer (documentation only)

Run from repository root without creating a script file:
`python3 -c "$(python3 -c 'from pathlib import Path; print(Path("docs/foundation/work_orders/BRIDGE_TOKEN_ECONOMY.md").read_text().split(chr(126)*3+"python"+chr(10),1)[1].split(chr(126)*3,1)[0])')" "/path/to/token-sample"`
The original sample directory is:
`/private/tmp/claude-501/-Users-gomaa-Documents-AI-workspace-varos/90bf49a1-794a-43e8-b7df-7f72ceea7227/scratchpad/cycle2/token-sample`.
Standard library only; prints counts/proxies, asserts equivalence and ceiling; no network, board edits or files written.

<details>
<summary>Exact Python measurement script</summary>

~~~python
import json, sys, hashlib, copy
from pathlib import Path
from collections import Counter
root = Path(sys.argv[1])
expected=["beae95bb86eda3d40d4a12162d9a449bd392676cc9df76a5cc48afa794a18952",
          "343b46d1528181b8bc47dfc58be233e189e3d3970b5b1ef033d7ba64e0a20ebd",
          "c2fcc7ee305ee9f2b31d32992afe876a34c92477044d12e67a9f4111b0ed1b66"]
assert [hashlib.sha256((root/f"b{i}.json").read_bytes()).hexdigest() for i in (1,2,3)]==expected
raw = [json.loads((root/f"b{i}.json").read_text()) for i in (1,2,3)]
def wire(v): return json.dumps(v,ensure_ascii=False,separators=(",",":"))
def size(v): return len(wire(v).encode())
def total(bs): return sum(map(size,bs))
def creation(o): return o["verb"] in ("add_shape","add_path")
def defaults(ops):
    d = {k:Counter(wire(o[k]) for o in ops if creation(o)).most_common(1)[0][0]
         for k in ("parent","fill")}
    d = {k:json.loads(v) for k,v in d.items()}
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
def bars(ops):
    out=[]; i=0
    while i<len(ops):
        o=ops[i]
        if o["verb"]=="repeat":
            out.append({**o,"ops":bars(o["ops"])}); i+=1; continue
        def eligible(a):
            return (a.get("kind")=="rect" and set(a)<=
                    {"verb","kind","bounds","fill","radius"} and
                    a.get("radius")==a["bounds"][3]/2)
        candidates=[]
        if eligible(o) and i+1<len(ops) and eligible(ops[i+1]):
            x,y,w,h=o["bounds"]; step=ops[i+1]["bounds"][1]-y
            for j in range(i+1,len(ops)):
                a=ops[j]
                if not eligible(a) or a.get("fill")!=o.get("fill"): break
                ax,ay,aw,ah=a["bounds"]
                if (ax,ay,ah)!=(x,y+(j-i)*step,h) or step<h: break
                rows=j-i+1; widths=[b["bounds"][2] for b in ops[i:j+1]]
                b={"verb":"bars","rows":rows,"x":x,"y":y,"w":w,"h":h,"gap":step-h}
                if "fill" in o: b["fill"]=o["fill"]
                if any(v!=w for v in widths): b["widths"]=widths
                saving=size([compact(z) for z in ops[i:j+1]])-size([b])
                if saving>0: candidates.append((saving,rows,b))
        if candidates:
            _,rows,b=max(candidates); out.append(b); i+=rows
        else: out.append(o); i+=1
    return out
def expand(ops):
    out=[]
    for o in ops:
        if o["verb"]=="repeat":
            for i in range(o["count"]): out+=expand([shift(x,i*o["dx"],i*o["dy"]) for x in o["ops"]])
        elif o["verb"]=="bars":
            for i in range(o["rows"]):
                a={"verb":"add_shape","kind":"rect","bounds":[o["x"],o["y"]+i*(o["h"]+o["gap"]),o.get("widths",[o["w"]]*o["rows"])[i],o["h"]],"radius":o["h"]/2}
                if "fill" in o: a["fill"]=o["fill"]
                out.append(a)
        else: out.append(o)
    return out
d=[defaults(b) for b in raw]; names=[nameless(b) for b in d]
r=[{**b,"ops":repeats(b["ops"])} for b in names]
s=[{**b,"ops":bars(b["ops"])} for b in r]
for original, encoded in zip(names,s): assert expand(encoded["ops"])==original["ops"]
# Same-batch existing objects: Plan 3 is exactly Plan 1 translated +880.
cl=copy.deepcopy(names)
src=[i for i,o in enumerate(raw[1]) if o.get("name","").startswith("Plan 1 ")]
dst=[i for i,o in enumerate(raw[1]) if o.get("name","").startswith("Plan 3 ")]
assert len(src)==len(dst)==13 and dst==list(range(dst[0],dst[-1]+1))
assert [shift(names[1]["ops"][i],880,0) for i in src]==[names[1]["ops"][i] for i in dst]
for k,i in enumerate(src): cl[1]["ops"][i]["local"]=f"$c{k}"
clone={"verb":"clone","ids":[f"$c{k}" for k in range(13)],"dx":880,"dy":0,"count":1}
cl[1]["ops"][dst[0]:dst[-1]+1]=[clone]
cl=[{**b,"ops":bars(repeats(b["ops"]))} for b in cl]
# Resolve clone sources in staged order; erase newly introduced local bindings.
def resolve_clones(ops):
    out=[]; bound={}
    for o in expand(ops):
        if o["verb"]=="clone":
            out.extend(shift(bound[k],o["dx"],o["dy"]) for k in o["ids"])
        else:
            o=copy.deepcopy(o); local=o.pop("local",None)
            if local: bound[local]=o
            if local=="$web": o["local"]=local
            out.append(o)
    return out
for a,b in zip(names,cl): assert resolve_clones(b["ops"])==a["ops"]

stages=[("baseline ops",raw),("defaults",d),("optional names",names),
        ("compact",list(map(packed,names))),("repeat",list(map(packed,r))),
        ("bars after repeat",list(map(packed,s))),("clone alternative",list(map(packed,cl)))]
for label,bs in stages:
    print(label,total(bs),[size(b) for b in bs])
print("ops",list(map(len,raw)),"top-level", [len(b["ops"]) for b in s])
print("repeat top-level",[len(b["ops"]) for b in r])
print("sha256",[(f"b{i}.json",hashlib.sha256((root/f"b{i}.json").read_bytes()).hexdigest()) for i in (1,2,3)])
# Receipt proxy only: exact geometry/host IDs/transcript were not supplied.
lens=[]
for batch in raw:
    costs=[]
    for o in batch:
        if not creation(o): continue
        p={"id":"path:1001","kind":"path","parent":"node:1","name":o["name"],
           "bounds":o.get("bounds",[3600,-186,1440,3000]),"fill":o.get("fill"),
           "stroke":o.get("stroke"),"stroke_width":o.get("stroke_width",0.0),
           "opacity":o.get("opacity",1.0),"hidden":False,"locked":False}
        costs.append(size(p))
    lens.append((len(costs),sum(costs),min(costs),max(costs)))
print("receipt created-object proxy: count/bytes/min/max",lens)
terse=[]
for i,n in enumerate([70,91,38],1):
    v={"ok":True,"board":"b1","request_id":f"r{i}","rev":i,"undo_steps":1,
       "result":{"created":[f"path:{1001+10*j}" for j in range(n)],"changed":["node:1"],
       "removed":[],"artboards_created":["artboard:1"] if i==1 else [],
       "artboards_removed":[],"locals":{"$web":"artboard:1"} if i==1 else {}}}
    terse.append(v)
print("terse receipt proxy structured bytes",total(terse))
assert total(raw)==28092
assert total(list(map(packed,s)))<=9364

fixtures=Path("varos/crates/varos-bridge/tests/fixtures")
for task in ("poster","story"):
    req=json.loads((fixtures/f"{task}-request-1.0.json").read_text())
    before=req["ops"]; after=[compact(o) for o in before]
    assert [unpack(o) for o in after]==before
    print(task,"ops baseline/compact",size(before),size(after))
~~~

</details>

