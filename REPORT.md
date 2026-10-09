p0-crash — feat/p0-crash, pre-review HEAD 96e9db1; fix changes are uncommitted.
REPORT.md was absent at handoff; reviewer final.txt, lane brief, CLAUDE.md and PLAN principles were read.
Slices 0.2/0.9 remain implemented, pending independent review and owner acceptance.
Fix round
P1 fixed — core editor.rs stores undo/redo/pending as immutable Arc<Document>; rollback copies live state and bounded history handles.
P1 test — 200-history checkpoint proves pointer sharing, pending/redo preservation and forced-panic restoration; existing crash tests retained.
P1 fixed — Bridge service.rs checkpoints mutations only; ten board-describe reads prove no additional checkpoint access.
P2 fixed — core clipping.rs caches menu enablement per editor revision/selection, bypasses active transactions, invalidates on command/begin.
P2 fixed — indexed node/leaf/z lookup and deduplicated selection units eliminate per-descendant subtree collection.
P2 tests — 512 selected paths, 100 mirror reads/one computation; partial selection, transaction invalidation/cancel, release/undo/replacement covered (7 new tests total).
Coverage note fixed — command Make/Undo/Redo matches content streams and page bounds of frozen native_rich.pdf; fixture unchanged.
Merge risks — burger key varos.object.clipping.enablement is namespaced; existing obj.clip/obj.release_clip IDs and Illustrator keys retained.
Shared changes confined to editor history/cache, command invalidation, Bridge checkpoint predicate and native/burger consumers; no schema/settings changes.
Integrator: retain fallible execute/execute_ui distinction, combine sibling API 1.2 verbs/schema/capability gates, preserve GPU submission guards.
Disagreements: none. Sibling branches were not modified; GPU loss/native interaction remains unverified under the no-GUI rule.
Gates — fmt PASS; dependency directions PASS; git diff --check PASS.
Workspace — cargo test --offline --workspace -j 2 --no-fail-fast: 1537 passed, 0 failed, 15 ignored (90 test/doc-test binaries).
Clippy — native and x86_64-pc-windows-msvc, workspace/all-targets, offline -j 2 -D warnings: both exit 0.
Ratchets — core 37/37 + Bridge 106/106 (4 tests passed); shell 3 tests passed; ui.rs 843/843; caps/source unchanged.
Bridge contracts — 72 passed, 0 failed, 4 ignored; API fixtures byte-unchanged against base f2066f1; PDF exports 36 passed.
Storage coverage — 114 storage tests passed, including recents/settings corruption quarantine and preserved earlier bad files.
Logs — /tmp/varos-p0-crash-fix-20261009-unique/{workspace-final,panic-ratchets,ui-ratchets,clippy-native,clippy-windows,deps,fmt}.log
No commit/push/merge/GUI/install; HEAD remains 96e9db1. Requested fixes complete locally; independent re-review/native acceptance remain.
