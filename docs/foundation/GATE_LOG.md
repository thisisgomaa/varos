> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Gate Log

Every work-order gate review is recorded here (charter §4). Format: order, branch, commit range, checks run, defects, verdict, merge commit.

## F1 — Inventory & classification

- **Date:** 2026-07-11. **Branch:** `codex/f1-inventory` (range `1aff281..2aa1c2f`, i.e. commits `74fdad7`, `cf59527`, `d3bbc17`, `96deeaf`, `2aa1c2f`). **Reviewer:** planner.
- **Checks run:**
  - Completeness: table paths extracted and set-compared against `git ls-tree -r 1aff281 --name-only` → 160/160 identical, 0 duplicates, 0 missing.
  - Dead-candidate safety: `git grep` for root `icon.png` and `varos/varos.ico` → zero real references (the two `include_bytes!("../icon.png")` hits resolve to `crates/varos-app/icon.png`).
  - Evidence sampling: 6 `Last` commit values re-derived via `git log -1 --format=%h -- <path>` → all matched.
  - DEPENDENCY_MAP: internal edges vs all 5 manifests, 8 external-dep purposes vs source, fork sweep vs independently verified 5-file list — verified (secondary reviewer, sonnet-tier).
  - OWNERSHIP_MAP: 28/28 modules present, 12 line-range samples read against source, 4 ownership truths verified (secondary reviewer, sonnet-tier).
  - Hygiene: diff scope = ci.yml + foundation docs only; `git diff --check` clean; no Rust source/fixture changes.
- **Defects found (round 1):** (1) fabricated wgpu evidence citation `tess.rs:256,636`; (2) `editor.rs` table gap 2760-2868; (3) forward-looking §7 in an as-is document; (4) "future command-boundary pressure point" wording; plus cosmetic line-number nits.
- **Fixes:** `96deeaf`, `2aa1c2f` (§7 content moved to `NOTES_FOR_CHARTER.md`). Point re-verified: tess.rs citation gone (bytemuck `tess.rs:7` citation verified real), tables contiguous 1-5563 / 1-4533, as-is purity restored.
- **Verdict:** PASS. **Merged:** `05b6dc7` to `main`, pushed.
- **Post-merge observation:** push auto-triggered CI run `29162831354`; job died in 2s with `runner_id: 0`, zero steps — GitHub account verification hold blocks runners (environmental, not a code failure). Ahmed's action item.
- *Honesty note: this entry was written retroactively the same day, after a charter self-review flagged that the F1 gate had no repository evidence trail. This file exists so every future gate leaves one.*
- Sign-off: planner — PASS — 2026-07-11

## Charter mutual review

- **Date:** 2026-07-11. **Document:** `FOUNDATION_CHARTER.md` draft v1 → v2.
- Planner self-review (sonnet-tier) found 7 defects in v1 (fixed same day). Codex counter-review found 8 blocking points — all verified and accepted: untracked risk register; golden round-trip invariant claimed but not implemented (`golden.rs` is load+assert only); F3 self-contradiction (private `apply_ops` untestable without scaffolding); F4 two-level `EditCommand`/`AppCommand` replaces the core-vs-app binary; F2a oversized → split into F2a.1-4 with defined denominators; `.varos` dashboard metric incoherent (current docs already at 0 — verified by grep across all 12 current-classified docs); per-commit gate claim unenforceable → branch-tip/merge-commit wording; unmeasurable/person-bound commitments moved to §4b or given markers, flags, and sign-off formats.
- **Verdict:** v2 ACCEPTED by Ahmed ("اعتمد"), including tracking both audit reports as the risk register.
- Sign-off: planner — v2 ready — 2026-07-11
- Sign-off: product owner (Ahmed) — ACCEPTED — 2026-07-11

## F2a.1 — Policy scaffolding

- **Date:** 2026-07-11. **Branch:** `codex/f2a1-policy` (range `27bcba8..9f89003`, commits `4988f65`, `76f8b31`, `e34648b`, `9f89003`). **Reviewer:** planner.
- **Checks run:**
  - Scope: diff = 8 `docs/adr/` files + `STATUS.md` + `tools/check_links.ps1`; zero `.rs`/`.toml`/`.lock` changes; no moves/deletes/stamping; `git diff --check` clean.
  - ADR content: all 7 drafts read in full by the reviewer — every Decision traces 1:1 to charter §6/§9/§10 (zero invented decisions); Supersedes lines correct (Constitution 14/17-20, CLAUDE.md:10, stale manifest comment); charter line citations spot-checked against the file.
  - Link checker: independently executed → `PASS (70 first-party docs, 71 relative links, 58 heading anchors)`, matching the implementer's report exactly; negative test (broken link injected into a tracked doc, then restored) → exit 1 with a named failure.
  - STATUS truthfulness: stamped denominator now defined (0/70); link-check row live.
- **Defects:** none — first zero-defect gate in the program.
- **Verdict:** PASS. **Merged:** `673b5df` to `main`.
- **Pending:** the 7 ADRs remain `Proposed` until the product owner accepts them (blocker for F2c and for F2a.3's Constitution/CLAUDE.md corrections).
- Sign-off: planner — PASS — 2026-07-11

## ADR acceptance

- **Date:** 2026-07-11. ADR-0001..0007 flipped `Proposed` → `Accepted` on the product owner's assent (relayed while dispatching F2a.2-4; the ADRs contain only decisions Ahmed had already made — recorded in charter §9 and the mutual-review record §10). Reversible by product-owner word before any dependent work merges.
- Sign-off: product owner (Ahmed, via planner) — ACCEPTED — 2026-07-11

## F2a.2 — Stamping

- **Date:** 2026-07-11. **Branch:** `codex/f2a2-stamping` (range `84463a7..c43ecf3`, commits `bab8001`, `a89e935`, `c43ecf3`). **Reviewer:** planner.
- **Checks run:**
  - Diff purity: 70 files / +70 / −0; every added line matches `^> \*\*Status:\*\*` (zero non-stamp additions); no `.rs`/`.toml`/`.lock` touched; `git diff --check` clean.
  - Classification fidelity: per-file stamp class compared programmatically against `INVENTORY.md` for every baseline doc → **0 mismatches**. Post-baseline docs judged sensibly: foundation+ADRs = current, `NOTES_FOR_CHARTER.md` = historical (absorbed into the charter), `ADR-0000-template.md` = reference, both audits = current (active risk register). Arithmetic closes: 27+18+25 = 70.
  - HTML safety: every HTML stamp sits immediately after `<body>` (never inside `<style>`/`<script>`); `UI_VISION_MOCKUP.html` has no body tag → line 1. Renders as a visible provenance line on archived prototypes — harmless.
  - Link check independently re-run: `PASS (70 docs, 71 links, 58 anchors)`.
  - Note (no action): `ELEMENTS_CATALOG.md` and `VISUAL_POLISH_PLAN.md` each contain one pre-existing legacy line that coincidentally starts with the stamp marker; file-level counts are unaffected.
- **Defects:** none.
- **Verdict:** PASS. **Merged:** `d0664d4` to `main`.
- Sign-off: planner — PASS — 2026-07-11

## F2a.3 — Current-doc corrections

- **Date:** 2026-07-11. **Branch:** `codex/f2a3-current-docs` (range `ab75a35..9faa162`, commits `5182cf2`, `57b25b6`, `9faa162`). **Reviewer:** planner.
- **Checks run:**
  - Scope: exactly 4 current docs (README, CONTRIBUTING, CLAUDE.md, VAROS_CONSTITUTION.md); zero `.rs`/`.toml`/`.lock`; `git diff --check` clean.
  - Full diff read line-by-line by the reviewer: every change traces to an accepted ADR (0001/0004/0007) or an audit-flagged falsehood — no invented decisions. Constitution amended the right way: original 2026-06-23 lock date preserved, amendment dated and linked to its ADRs; item 18's web-fallback clause removed as an ADR-0001 contradiction.
  - Audit closures: P0.3 (SVG/PNG export no longer claimed as working — moved to Coming next), P0.4 (test count now 223 dated to the audited baseline; README/CONTRIBUTING state hosted-CI reality with STATUS links), P0.5 over-claim absent.
  - Link check independently re-run: `PASS (70 docs, 79 links, 58 anchors)` — the +8 links are exactly the 8 new ADR/STATUS references (CLAUDE 3, Constitution 3, README 1, CONTRIBUTING 1).
  - `.varos` and personal/temp paths in the edited docs: zero.
  - Implementer ran fmt/clippy/test (223/223) at `-j 4` — docs-only diff, belt-and-suspenders.
- **Defects:** none — third consecutive zero-defect gate.
- **Verdict:** PASS. **Merged:** `a10772e` to `main`.
- Sign-off: planner — PASS — 2026-07-11

## F2a.4 — Vendor contract

- **Date:** 2026-07-11. **Branch:** `codex/f2a4-vendor` (range `0abc3da..e158e9e`, commits `fa2c278`, `dbd863b`, `e158e9e`). **Reviewer:** planner.
- **Checks run:**
  - Scope: `docs/VENDOR_PATCHES.md` (new, stamped current) + `tools/check_vendor_patches.ps1` (new) + `varos/Cargo.toml` — manifest diff verified **comments-only**; vendor sources and `Cargo.lock` untouched; `git diff --check` clean.
  - Checker independently executed: `PASS — egui_tiles 0.16.0 (62ac747…), archive SHA-256 9EB8FE…A174, comparable files 17, modified files 5` — identical to the implementer's report and to the F1-verified ledger.
  - Negative test performed by the reviewer: appended a comment to `src/container/grid.rs` (a 6th modified file) → checker FAILED naming the file, exit 1; file restored via `git checkout`.
  - VENDOR_PATCHES.md content matches the DEPENDENCY_MAP §4 ledger row-for-row, adds upstream identity with hash evidence, an isolation contract, and a 6-step rebase procedure gated on a superseding ADR.
  - Link check independently re-run: `PASS (71 docs, 81 links, 58 anchors)` — the new doc is counted and clean.
  - Implementer ran fmt/clippy/test (223/223) at `-j 4` — required since `.toml` was touched (comments-only).
- **Defects:** none — fourth consecutive zero-defect gate. **F2a is complete.**
- **Verdict:** PASS. **Merged:** `76640fe` to `main`.
- Sign-off: planner — PASS — 2026-07-11

## F2c — Dependency-direction gate

- **Date:** 2026-07-11. **Branch:** `codex/f2c-dep-check` (range `820f173..0bade77`, commits `40f90c7`, `0bade77`). **Reviewer:** planner.
- **Checks run:**
  - Scope: `tools/check_dep_directions.ps1` (new) + one named `ci.yml` step; zero `.rs`/`.toml`/`.lock`; `git diff --check` clean; CI step path `../tools/...` verified against the job's `working-directory: varos`.
  - Script read in full: derives truth from `cargo metadata` (not text-matching manifests), strips Rust comments before the `egui_tiles` source scan, exact-set assertions for all ADR-0005 edges incl. core's forbidden wgpu/winit/egui/windows families and renderer's no-winit rule.
  - Independently executed: PASS with the expected edge summary. Negative test by the reviewer: `use egui_tiles::Tree;` appended to `shell/mod.rs` → FAIL naming `[shell/boxtree.rs, shell/mod.rs]` vs expected, exit 1; restored.
  - Implementer ran fmt/clippy/test (223/223) at `-j 4`.
- **Defects:** none — fifth consecutive zero-defect gate.
- **Verdict:** PASS. **Merged:** `3a0ad3b` to `main`.
- Sign-off: planner — PASS — 2026-07-11

## F2b — Physical doc layout

- **Date:** 2026-07-11. **Branch:** `codex/f2b-doc-layout` (range `71ba062..6346bec`, commits `da3c924`, `6386814`, `6346bec`). **Reviewer:** planner.
- **Checks run:**
  - 33 renames stamp-derived (17 historical → `docs/history/`, 16 reference → `docs/reference/`): 28 at R100, 5 at R093-R099 whose deltas are their own internal link fixes; docs root now exactly the 7 current docs; foundation/adr/audits/design-reference untouched; NOTES_FOR_CHARTER kept in place as ordered.
  - 6 modified docs verified at word level: every changed token is a path string (including inside the tracked audit — evidentiary content untouched); 56 insertions vs 56 deletions.
  - Stamps unchanged (71 stamped docs). Both checkers independently re-run: `check_links PASS (71/81/58)`, `check_dep_directions PASS`. `git log --follow` traces `plan.html` to its creation commit `720c2b4`; implementer demonstrated two more.
  - Implementer ran fmt/clippy/test (223/223) at `-j 4`.
- **Non-blocking note:** `INVENTORY.md` path cells were updated in place to post-move locations, which slightly strains its "as returned at baseline" header sentence. Rider on the next order: add one clarifying line to INVENTORY's header ("path cells reflect post-F2b locations; baseline identity lives in the `Last` column"). Not a defect — the update keeps the classification register usable, which is how F2a.2/F2b consumed it.
- **Defects:** none — sixth consecutive zero-defect gate. **F2 (a+b+c) is COMPLETE.**
- **Verdict:** PASS. **Merged:** `b6e3863` to `main`.
- Sign-off: planner — PASS — 2026-07-11

## F3 — Characterization tests

- **Date:** 2026-07-11. **Branch:** `codex/f3-characterization` (range `50e2e7f..a36ef8a`, commits `1753e9d`, `41579de`, `988f338`, `a36ef8a`). **Reviewer:** planner.
- **Checks run:**
  - All 6 tests read line-by-line: golden round-trip implements the §4 law verbatim for all three fixtures (blob-level A/B byte stability; original fixture bytes never compared); healthy drag→commit→undo→redo pinned; the known mid-drag undo defect FROZEN as-is with a risk-register pointer (counter-review §4.6) — characterization, not repair; Op dispatch pins clamps + revision counting; direct-write Ops pinned; menu/shortcut parity via the extracted callback vs `apply_key("KeyU")`.
  - Sole production change verified mechanical: `snap.smart = !snap.smart` moved verbatim into private `toggle_smart_guides`, menu calls it. Scaffolding declared in `F3_CHARACTERIZATION.md` (in-file `#[cfg(test)]` per the color_tests precedent; no visibility widened; no deps added).
  - Red/green evidence: per-test mutation table with concrete failure values recorded by the implementer (e.g. "ids 13 vs 12", "revision 3 vs 2"); reviewer verified the table's plausibility against the test code rather than re-running mutations — basis: specific failure outputs + independent full-suite run + line-by-line reading.
  - Honest coverage boundary documented (File menu rows = would-be false coverage until F4; Ctrl+S/O = native dialogs, not headless-testable; parity test scope limits).
  - Reviewer ran the gates independently: `cargo test --workspace -j 4` = **229/229**, `fmt --check` clean, `clippy --all-targets -D warnings` clean. INVENTORY rider line landed.
- **Defects:** none — seventh consecutive zero-defect gate.
- **Verdict:** PASS. **Merged:** `b8c9ba6` to `main`.
- Sign-off: planner — PASS — 2026-07-11
