Lane H — Phase 7 import resume, feat/w2-import, 2026-10-09
Base b3d39ee; existing WIP 14db577 retained; resume fixes are uncommitted.
Status: implemented (provisional UI, owner design review pending), supported subsets below.
Kept existing PDF/AI/DXF adapters, clipboard trait/NSPasteboard, Open/Place/drop, CLI and Bridge wiring.
Fixed CLI import output replacement: complete temporary file published with no-replace hard link.
Fixed PDF dash length/phase under similarity transforms; added orthogonal page rotation.
Fixed PDF implicit subpath closure before even-odd hole detection; unsupported combined cases refuse.
Extended DXF to exact multi-span clamped nonrational degree 1–3 B-spline conversion in new dxf_spline.rs.
DXF rejects polyline width/count mismatches and preserves entity invisibility.
Host cancellation now reaches SVG/DXF conversion checkpoints; PDF/AI remains process-isolated.
Import completion bypasses generic gesture settlement; busy/refused/cancelled results preserve human edits/UI caches.
Place publication uses checked EditCommand::PlaceArtwork and reports command failures.
Six added headless regressions cover rotation/dashes, holes, splines, DXF refusal/visibility, cancellation and host atomicity.
Existing CLI test also proves a second import cannot replace its destination; source files remain untouched.
PDF/AI: cached lopdf 0.43 (MIT); hayro/hayro-interpret absent; classic-xref RGB/Gray static paths/clipping supported.
PDF text has no verified glyph-outline adapter: explicit accepted omission; images omitted because image/blob node absent.
Bitmap-only clipboard refuses that absent prerequisite; no silent vector-to-bitmap fallback.
PDF Form XObjects, object/xref streams, encryption, UserUnit and unsupported print/transparency semantics refuse.
DXF rational/periodic/high-degree splines, INSERT/3D/text/hatches and binary DXF refuse; DWG has a readable refusal.
Desktop multi-page PDF/unitless DXF options sheet remains unavailable; explicit page/unit options exist in CLI/Bridge.
No persisted keys/format bump; native .vrs reader firewall retained; attribution/NOTICE from WIP retained.
PASS: cargo fmt --all --check; python3 ../tools/check_dep_directions.py; git diff --check (exit 0 each).
PASS: cargo test --offline --workspace -j 3 --no-fail-fast: 1903 passed, 0 failed, 15 existing ignored (exit 0).
PASS: cargo clippy --offline --workspace --all-targets -j 3 [native / --target x86_64-pc-windows-msvc] -- -D warnings (exit 0 each).
PASS: ratchets 3/3; dedicated Bridge contracts/phase1/import_file/import_svg 98 passed, 4 ignored (exit 0).
Compatibility: all 250 checked Bridge/core fixtures + protected UI/token/ratchet files byte-identical to b3d39ee; ui.rs 811/843.
Evidence: /tmp/w2-import-{workspace,clippy-native,clippy-windows}-final3.log; /tmp/w2-import-{fmt,deps,ratchets,bridge}.log; /tmp/w2-import-compat.json.
No new commit/push/merge/GUI/install; independent review, owner design/native interoperability and complex visual oracles remain pending.
