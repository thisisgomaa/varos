# Lane F — Phase 9 application handoff
Base d403aca; original work committed by coordinator; this fix round remains uncommitted.
9.1–9.6 implemented (provisional UI, owner design review pending): Preferences, registry, shortcuts, History, Actions, Help/preview.
API 1.2 + CLI paths are wired; API 1.0/1.1 fixtures remain byte-frozen; no new dependencies or prior-art lifts.
## Fix round — 2026-10-09
Accepted all seven reviewer findings; no disagreements.
P1 preview: next format provisional 6; matching JSON/catalog stamps, pure migrate_v5_to_next_preview, frozen containers/refusals, VRS_FORMAT updated.
Added keys: /VAROS_Preview and /VAROS_PreviewVersion=1; no authored JSON fields added; original v1–v5 fixtures untouched.
Moderator must renumber preview migration/stamps/fixtures after images, gradients, text and Live Corners in actual merge order.
P2 shortcuts: Reset excludes held Space; old persisted defaults pass to incumbent dispatch; rebound document keys retain repetition.
P2 recording coverage: every changed commit checks semantic coverage; direct gestures/checked creation/Undo/Redo visibly discard unsupported recordings.
P2 target binding: one selection/first supported target set per recording; Bridge checks across requests and refuses partial export on changes.
P2 cache bounds: reads ≤2 MiB plus sentinel, strict 544×246 PNG dimensions and 8 MiB decoder budget; validated bytes reused.
P2 cache failures: generate/embed in memory; optional directory/write/durability failures cannot block native Save.
P2 History: ceiling restored to 200 in core, Preferences and Bridge schema; higher depths await image-memory evidence.
Headless regressions cover every finding, Reset→Apply dispatch, cross-request Bridge targets and direct Undo.
PASS: targeted regressions, frozen v5 hashes/visual goldens, updated old-reader harness 9/9, fmt and dependency directions.
PASS: native + Windows workspace/all-targets clippy -D warnings; app ratchets 3/3 and Bridge ratchets/fixtures 6/6.
PASS: cargo test --offline --workspace -j 3 --no-fail-fast: 1932 passed / 0 failed / 15 ignored.
Evidence: /tmp/w2-app-fix-{workspace-final,clippy-native-final,clippy-windows-final,ratchets-app,ratchets-bridge}.log.
Ratchet files/limits and legacy Bridge fixtures unchanged; ui.rs remains 772 lines (cap 843).
Pending integration: sibling BlobStore preview/save path, combined Bridge size budget, independent re-review and owner native acceptance.
No git writes, push, merge, GUI launch, install or signed Quick Look packaging performed.
