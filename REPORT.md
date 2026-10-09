# Lane T — Text programme
Pure varos-text implementation: paragraph composition, kashida, bidi coverage, metrics, outlines; no core/app/Bridge/schema changes.
Original plan, provenance, corpus measurements and prior gates: docs/foundation/work_orders/text-programme-evidence/.
## Fix round — 2026-10-09
P2 quadratic scans FIXED: one grapheme/join/word index, cached safe slots and word counts; Off bypasses candidate discovery.
Space distribution FIXED: sorted visual prefix counts replace per-glyph space scans; reshape cap remains 64.
Tests: 20k-boundary/index and 20k-space prefix oracles, bidi/ties, long Arabic and space-heavy composition; 576 Arabic configurations retained.
P2 hyphenation hook FIXED: additive compose_with_hyphenation accepts an optional provider for Greedy and EveryLine.
Selected breaks measure/render virtual hyphens; original source/copy retained, glyph maps to preceding grapheme, diagnostic records source byte.
Tests: exact glyph/advance oracle, paragraph offsets, no-provider/wide fallback, overflow includes hyphen width, budget refusal, invalid grapheme rejection, narrow final-letter natural-break regression.
P2 non-test unwrap FIXED in composer/kashida/paths with checked selection/closure and indexed joining metadata; static scan: zero in all three.
Tests: both composer modes and empty paragraphs, candidate oracle, cubic-close parity plus new singleton/duplicate/repeated-close cases.
Merge risks: shared vendor/NOTICE/checker edits remain additive; no sibling core/app/Bridge hotspots, shortcuts, commands or settings touched.
MERGE.md and lane-files.json identify atomic convergence replacement, untracked files/fonts, vendor patch/checker coupling and per-glyph NonZero contract.
Tests: 7 new regressions; varos-text 62 passed / 0 failed; final workspace 1,533 passed / 0 failed / 15 existing ignored (406.26 s).
Gates PASS: fmt, dependency directions, native + Windows all-target clippy -D warnings, wasm32 library check, git diff --check.
Ratchets: 3 passed, no limits raised. Dedicated Bridge contracts/fixtures: 70 passed / 0 failed / 4 existing ignored.
Vendor: COSMIC full reconstruction PASS; egui_tiles SKIP (pristine archive uncached), permitted by brief.
Commands, exit codes, timings, logs and manifest: docs/foundation/work_orders/text-programme-evidence/fix-round/verification.json.
No finding disputed. Reviewer requests committed snapshot, but explicit user no-commit rule takes precedence; HEAD remains 7b48f2c.
No commit/push/GUI/install performed; independent fix re-review, owner visual acceptance and WASM runtime parity remain unverified.
