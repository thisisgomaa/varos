# Lane A — Appearance / FORMAT v10
State: implemented (provisional UI, owner design review pending); committed baseline `29b46a7`, fix round uncommitted.
Scope: ordered fills/strokes, container looks, Expand, nested clip/alpha masks; kit Properties/row hooks; Bridge/CLI; GPU/CPU/PDF/SVG.
Format: pure v9→v10 migration, legacy byte identity, frozen output/refusal fixtures; moderator owns v10–v14 rechaining.
Guards: GPU depth 6 / 512 MiB with reported flattening; CPU 128 MiB / depth 12 with refusal; existing ratchets unchanged.

## Fix round
Accepted all five findings; no disagreements.
P1 geometry: bake every distinct affected transform unit, including separately rotated Layer children, before reparenting.
P1 topology: stage and structurally/semantically validate masks before publication; Layer sources and targets containing sublayers refuse atomically.
P1 rebuild: eliminate per-entry Editor clones / full export scenes / document-wide scene cache; reuse one evaluator, leaf geometry/stroke caches, culling and aggregate export budget.
P2 drawing: mask-row selection/toggle/activation restores its authoritative Group drawing child.
P2 swatches: preserve authored base/extra paint references separately from resolved display colours during opacity/visibility edits.
Regressions: four added core tests + one UI test; existing pan test now measures real leaf-cache misses; focused core 13/13 passed.
300-leaf debug probe: plain 13.8 ms / appearance 13.0 ms (reviewer's appearance probe: 922 ms); actual thermals remain unmeasured.
Gates PASS: fmt, dependency directions, workspace (2273 logged passed / 0 failed / 15 ignored), native+Windows all-target clippy -D warnings; UI ratchets 3/3, Bridge 113 passed / 4 ignored; frozen 1.0/1.1 match, 1.2 tools/list 23,912/24,000 B; ui.rs 812/843.
Integration: sibling effects/live order+Expand, CMYK/text Forms, GPU pool/CPU parity, Arabic/a11y hooks, format chain and combined UI/Bridge budgets still require moderator checks on the combined tree.
Evidence: `/tmp/varos-appearance-fix.TyEBdP/`; GUI/GPU pixels, PDF viewer pixels, thermals and owner acceptance unverified.
No git writes, commit, push, merge, GUI run or app installation; no dependencies or ratchet increases.
