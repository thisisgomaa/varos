> **Status:** partial — implementation prepared; T1 is not landed.
# Text T1 results — 2026-10-07

## Scope
- Added leaf `varos/crates/varos-text`: byte-fed FaceId/FontSet, SHA-256 font/snapshot
  identity, fixed script fallback, language/script/features, legal line fitting,
  UBA/caret affinity, exact cubic Y-down outlines, bounded elision/caret/selection.
- Converge/incremental promotion is prepared, pending owner acceptance of
  [ADR-0012](../../adr/ADR-0012-ui-text-engine.md). The owner verbally selected
  “option 2 = path B” on 2026-10-07; acceptance is not recorded and no T1 work-order
  file exists. This results record does not override the ADR's T4 deferral.
  P1c's resource/cancellation limits remain explicit. No core->text edge is added.
- Corpus/language/wrapping/incremental tests migrated; four production contract tests
  added. Proof/stencil/export/parity/bench scripts remain in the standalone spike.
- App->text edge prepared pending that acceptance; dependency checker enforces the isolated transitive graph.

## Feature and patch proof
- COSMIC 0.19.0: defaults OFF; std + shape-run-cache ON; discovery/sys-locale/
  swash/cosmic fontconfig OFF. Isolated fontdb: std only.
- Real workspace fontdb union: std, fs, memmap, memmap2, fontconfig, fontconfig-parser
  through resvg. COSMIC byte-fed mode refuses file sources. No observed E0004.
- Duplicated HarfRust: COSMIC 0.5.2 / epaint 0.7.0; Skrifa 0.40.0 / 0.42.1;
  read-fonts 0.37.0 / 0.39.2. Dedupe when upstream aligns these versions.
- Vendor ledger: docs/VENDOR_PATCHES.md; varos/vendor/patches includes complete
  patch, pristine hashes/base. Offline checker reproduces 110 files / 7 modified.
- Archive SHA-256: be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73.

## Migrated versus spike-only proof
- Crate-local `winding_preserves_glyph_counter` rasterizes Inter O with
  tiny-skia `FillRule::Winding`: the counter is empty and both strokes are filled.
- `winding_overlapping_contours` and `winding_corpus_gate` remain spike-only:
  they compare native winding against the spike's stencil/nonzero reference
  raster harness, which is not part of the layout leaf. Their renderer parity
  proof was not migrated and does not certify the production crate.
- The rotated-path assertion in `mixed_metrics_rotation_limits_and_language`
  stays spike-only: it exercises renderer transform plumbing. Production keeps
  the mixed metrics/language/limits assertions; rotation is a host responsibility.

## Budgets (release, --offline -j 2, one run)
- Font initialization 2.629 ms, excluded from layout timings. Fresh Engine
  construction is also outside each cold-first sample's timer.
- Cold-first 120-scalar mixed names, 41 fresh Engines: first 0.642875 ms;
  p50 0.117458; p95 0.177417; max 0.642875 ms. Each sample starts without
  engine shape-run/span caches; first face use and process initialization are visible.
- Warm-engine label misses, 41 distinct generated words after unrelated Latin/Arabic
  warmup: first 0.092042 ms; p50 0.062250; p95 0.083292; max 0.092042 ms.
  Engine caches can reuse shared runs; these are not fully cold samples.
- Correction to the previous run: the reported 41 “cold” names differed only in
  three digits, warming the shared shape-run/span caches after sample 0. Its
  0.843083 ms first sample included first face use; its 0.066375 ms p50 described
  warm-engine label misses, not cold-first latency. Those figures are superseded.
- Both new p95 values meet the ADR's <=0.25 ms per-label target; cold-first max
  exceeds it. 200 Recent rows × warm-engine p50 = **12.45 ms**, above the ADR's
  **8 ms ceiling** (cold-first p50 gives 23.4916 ms). Spreading misses over frames
  is mandatory; these layout numbers exclude rendering.
- 1,000 warm lookup + CPU quad-position emissions: mean 0.038292 us/label.
  This is a proxy: atlas lookup, UV/colour/index mesh emission and GPU work are T2.
- `LabelCache::layout()` warm hit with a FULL 4,096-entry cache, 1,000 calls in
  one frame: mean **0.020292 us/label**; retained payload 2,732,032 bytes.
  Once-per-frame expiration is performed before this timer, not per hit.
- Label cache caps: 4,096 entries / 4,194,304 estimated payload bytes; LRU/age tests
  pass. Small sample: 1 entry / 2,065 bytes; engine caches separately 1,763,169 bytes.
  Caller-held Arcs, HashMap/allocator overhead, shaping/paragraph caches are separate.

## Gates (all Cargo builds offline, -j 2; no ratchet changed)
- Full workspace tests: PASS, 1,282 passed / 0 failed / 16 existing ignored.
  varos-text: 40 passed / 0 failed / 0 ignored; production contracts: 4 passed.
- Development-profile opt-level 2 overrides: varos-text, cosmic-text, HarfRust
  0.5.2, Skrifa 0.42.1 and read-fonts 0.39.2. Exhaustive sweeps remain enabled.
  Single measured incremental suite: **71.95 s**, down from 288.72 s (~4.01×).
  Whole workspace command including rebuild: 129.62 s wall time.
- Native + Windows x86_64-pc-windows-msvc workspace all-target clippy: PASS,
  zero warnings. Windows is a cross-target lint gate, not a native runtime test.
- fmt --all --check, dependency graph: PASS; Python checker tests: 15 PASS
  (12 dependency checks, 3 vendor status checks).
- varos-text wasm32-unknown-unknown check: PASS; no dependency missing offline.
- COSMIC vendor: PASS (110 files / 7 deltas); egui_tiles: SKIP (archive not
  cached; not verified). Combined vendor exit 2 as expected; no download attempted.
- Requested gate totals: **7 PASS / 1 expected SKIP / 0 FAIL**.
  PowerShell runtime is unavailable on this Mac; its updated wrapper was not executed.
- Raw local gates: `varos/target/t1-fix-gates/` (build artifacts, not checked in).

## Open items
- Plex Arabic Medium/SemiBold absent after case-insensitive ignored/hidden-file
  search of prior-art, spike and ~/.cargo; stopped. Existing Regular/assets untouched.
- Independent patch/converge review, owner's Arabic proof review, maintenance-owner
  designation and upstream submission pending; this offline run sent nothing.
- Full warm textured-mesh budget, egui-chain coverage comparison and linked whole-app
  binary delta unmeasured. P1c 1 MiB slices, temporary/hostile-font bounds remain open.
- T2/T3 deferred: share font bytes through `fontdb::Source::Binary(Arc)`;
  `local_line_levels` O(text); caret_move word boundary binary search,
  WordLeft/WordRight and visual Home/End semantics; `selection_rects` caret snapping;
  WASM and vendor checker in CI.
- No commit/push, GUI, /Applications changes or real Keychain writes.
