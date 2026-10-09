# w2-export-paths — Lane C
Status: implemented (provisional UI, owner design review pending); committed lane plus uncommitted fix-round changes.
1.8: persisted Advanced state; ranges/bleed/colour/whole-board; card × row exports, presets, names/sub-folders, PDF modes, SVG options.
1.4: kit New Document dialog, categories/units/count/layout/bleed/ppi; physical dimensions preserved across unit changes.
2.3 / 4F: Outline/Offset/Expand, scale-strokes preference, live corner widgets/typed radius, undoable commands.
Hosts: EditCommand/AppCommand, API 1.2 verbs/schema/discovery, CLI new-document/export-screens/apply; frozen 1.0/1.1 retained.
Writer: next version (locally 6), doc.paths[].corners {radius,kind}; migrate_v5_to_live_corners and frozen/refusal fixtures.
Attribution: VectorCraft adaptation headers and NOTICE; no new dependencies.
Merge: integrator assigns version/migration/fixture numbers with sibling writers; union Bridge 1.2 and reconcile shared host changes.
## Fix round
Astra fdd7ec6 review: all five P1/P2 findings accepted and fixed; no disagreements.
P1 corners: shared stroke evaluator resolves live corners; rounded/inverted/chamfer, styled/dashed Outline and Expand match baked geometry.
P1 compositing: Expand refuses mixed fill/stroke with object opacity <1 or stroke alpha <1 before mutation; single-paint expansion remains supported.
P1 Selection PDF: UI disables single mode with a reason, jobs export each snapshot separately; shared planner refuses mismatched combined snapshots.
P2 host parity: app/Bridge/CLI use shared SVG options encoder; headless worker/Bridge and real CLI byte comparisons pass.
P2 precision: 0–8 decimals applied at initial numeric serialization, including stroke/clip geometry; legacy core API preserves frozen goldens.
Cheap coverage fix: corner cache test now checks actual hits, misses, fresh equivalence and Arc reuse after invalidation.
Workspace: cargo test --offline --workspace -j 3 --no-fail-fast PASS: 1,922 passed / 0 failed / 15 existing ignored; 114 suites.
Gates: fmt, dependency directions, native + Windows all-target Clippy -D warnings, git diff --check PASS.
Ratchets 3/3 PASS; Bridge contracts/fixtures 88 passed / 4 existing ignored; no fixture updates.
Audit: all 247 original core/Bridge fixtures byte-identical to b3d39ee; ratchet source unchanged; ui.rs 816 ≤843.
Evidence: /tmp/w2-export-fix-final-*.log, /tmp/w2-export-fix-final-gates.json, /tmp/w2-export-fix-final-audit.json.
No git writes, push, merge, GUI launch, build installation or new independent review performed.
Still pending: independent re-review, integration/version assignment, owner design/native acceptance, Windows runtime, idle heat/undo RAM measurements.
