# Lane B — Phase 5 completed scope
Branch: feat/w2-gradients; resumed WIP 2a5ea20; all resumed changes remain uncommitted.
5.0: additive Appearance read-view; paint readers, hashing, validation and exports use resolved appearance.
5.1: linear/radial gradients, stops/opacity/midpoint, Pad/Reflect/Repeat, placement/focal; SwatchRef + document table.
GPU: cached 1024-texel LUT/dither; CPU sampling; SVG gradients; PDF axial/radial shadings with alpha/knockout.
Hit-inside, transforms, duplicate/clipboard/eyedropper/Pathfinder preserve owned paint; live swatches invalidate cache.
5.2: enabled Gradient picker and G annotator; stop/midpoint/aspect/focal handles; one undo per gesture, Escape restores.
5.3: real Swatches panel; document/global/groups, built-in library; GPL, RGB ASE and native JSON import/export.
5.4: core harmony/variation maths, picker grid; Recolor Artwork with Lab k-means and delta E2000 matching.
5.0–5.4 implemented (provisional UI, owner design review pending); within-stroke gradients implemented.
Optional 5.5 along/across/freeform deferred; no corresponding UI or fallback is advertised.
Format: NEXT_GRADIENT_VERSION=6 provisional; moderator renumbers after images/before text.
Pure migration: migrate_v5_to_next_gradients; frozen accepted/refusal fixtures and old-version key refusal.
Keys: optional doc.swatches {id,name,paint,global,group}; fill/stroke tagged type/value gradient or swatch_ref.
Gradient keys: kind, stops {offset,colour,opacity,midpoint}, spread, placement, focal; reference key: id.
Null/solid encodings unchanged; old v4/v5 fixture trees and all 16 legacy Bridge fixture files unchanged.
API 1.2 colour verb registered in list_verbs/schema; bounded swatch/palette reads; CLI apply and palette IO.
PDF reports sampled gradients/stroke baking; GPL/ASE refuse unsupported alpha/gradients, native JSON preserves them.
Baked/transformed gradient references materialize by value; solid global references remain linked.
New coverage: 11 core format fixtures, 9 PNG/SVG/PDF goldens, 12 SVG/CPU parity cases, command/tool/IO tests.
Gates: cargo fmt --all --check PASS; dependency directions PASS; git diff --check PASS.
Workspace --offline -j 3 --no-fail-fast: PASS, 1917 primary passed / 0 failed / 15 ignored; 3 extra subprocess passes.
Native + x86_64-pc-windows-msvc workspace/all-targets clippy --offline -j 3 -D warnings: PASS, zero warnings.
Ratchets: 3 shell + 6 Bridge PASS; old 1.0/1.1 tools/list byte fixtures PASS; ui.rs 816/843 lines.
Evidence: varos/target/lane-b-final-{fmt,deps,workspace,clippy-native,clippy-windows}.log.
Attribution headers/NOTICE updated; no added dependencies or missing local-registry crates.
No new commit, push, merge, GUI launch or install; GPU runtime/owner UI acceptance and independent review unverified.
