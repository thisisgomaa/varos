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

## F4.1 — Core EditCommand boundary

- **Date:** 2026-07-12. **Branch:** `codex/f41-editcommand` (range `2379e6a..46321fd`, commits `db8413d`, `68d584b`, `49dfbd7`, `9065f19`, `46321fd`). **Reviewer:** planner.
- **Checks run:**
  - Design-sketch-first honored: `F4_DESIGN.md` with a complete 48/48 `Op` migration table and written rulings for every gray state (tool/selection/preview/view stay declared `Editor` interfaces; pointer/gesture stays input; no blanket begin/commit — each command keeps its exact history policy). The sketch discovered a **4th hidden direct write** (`ed.doc.snap = snap_cfg`, frame-level) beyond the three named in the order, and closed it.
  - `command.rs` read in full by the reviewer: every variant delegates to the existing core semantic method; migrated writes (ruler origin, snap toggles, stroke width) keep their old semantics verbatim — including staying non-undoable (parked risk untouched, as ordered).
  - Reviewer's independent scans: **zero** direct `ed.doc` writes in `ui.rs` production region; **zero** direct doc assignments in `main.rs`. `apply_ops` is translation-only; editing shortcuts in `main.rs` route through `execute`.
  - F3 immunity: `golden.rs`/`history_lifecycle.rs` zero diff; ui.rs test modules untouched by any hunk (implementer also recorded a normalized SHA-256 match).
  - 3 new headless boundary tests (232 total) with per-test red-proof values; reviewer-run gates: `cargo test --workspace -j 4` = **232/232**, fmt clean, clippy `-D warnings` clean.
  - **Hand verification by the product owner** on a release build of the branch tip: normal editing, undo/redo, groups, layers, shortcuts all behave unchanged. One observation logged as P11 (pre-existing selected-state performance drag on complex art — `PAINS_LOG.md`), explicitly NOT a regression: all F3/F4.1 changes execute on click/shortcut, not per-frame.
- **Process defect (planner's own, recorded honestly):** the gate summary message claimed "merged" before the merge was actually executed; caught when a follow-up docs commit landed on the branch instead of `main`. Corrected same hour: proper `--no-ff` merge of the reviewed tip `46321fd`, docs commit cherry-picked to `main`, branch pointer restored to the delivered tip. Rule reinforced: **the merge command runs before the word "merged" is written.**
- **Defects (implementer):** none — eighth consecutive zero-defect delivery.
- **Verdict:** PASS. **Merged:** `bd8bc1f` to `main`.
- Sign-off: planner — PASS — 2026-07-12

## P11.1 — Performance surgery, stage 1

- **Date:** 2026-07-12. **Branch:** `codex/p111-perf` (range `8da2d8c..893a0b9`, 9 commits). **Reviewer:** planner + product owner (hand test).
- **Checks run:**
  - Scope compliance: snap engine, undo storage, PresentMode, and all F3/F4.1 test files untouched (verified by diff); out-of-scope items deferred exactly as ordered.
  - Corner-join elision (`tess.rs:80-116`) read line-by-line: 5° threshold with documented rationale, caps always kept, unit ring built once; the vertex columns expose the win honestly (A foreground 93,672→7,344; C 756,000→72,000 — ~90% of stroke geometry was redundant discs).
  - Conservative scene signature (`scene.rs:71-172`) read in full: doc rev, view, frame, tool/gesture/drag discriminants, selections, hover, live selected-path geometry incl. handles, snap guides/HUD, previews, modifiers, pivot — plus the `cursor_drives_scene` rule that feeds raw cursor into the hash during ANY active drag/preview, so marquee/pen/guide overlays can never freeze on a stale cache.
  - Reviewer reproduced the harness independently: identical vertex counts, A cold **0.272ms** (baseline 2.696ms), C cold 2.034ms (baseline 23.931ms); reviewer-run gates 239/239, fmt, clippy clean; exe SHA-256 matched the report byte-for-byte.
  - **Product-owner hand test: PASS** — "حلو وأحسن كتير فعلا"; zoom/drag on the 300%-zoom complex scene now smooth; corner rendering visually clean.
- **Product feedback carried forward:** the A13 zoom-glide easing now *feels* like lag to the owner — he wants real-time immediate zoom. Authorized as a deliberate behavior change; scheduled as item 0 of P11.2.
- **Verdict:** PASS. **Merged:** `1931c80` to `main`.
- Sign-off: planner — PASS — 2026-07-12
- Sign-off: product owner (Ahmed, hand test) — PASS — 2026-07-12

## A26/A32 — Acceptance tests for two already-fixed pains

- **Date:** 2026-09-23. **Branch:** `worktree-agent-ad798213814c7febc` (range `61786a0..775bbb3`, commits `e21702d`, `8118894`, `775bbb3`). **Reviewer:** Codex (gpt-6-astra, read-only) + moderator gates.
- **Scope:** tests + docs only — `varos-core/tests/boolean_corners.rs` (A26, boolean keeps sharp corners), `varos-core/tests/delete_anchor.rs` (A32, deleting an anchor opens the path), and the two `PAINS_LOG.md` rows annotated. Zero production code: both pains were already fixed on `main` in `1bc4f3c` (July); these tests pin the fix. Implementer's red-proof: with the old code temporarily restored, 4/4 A26 and 10/11 A32 tests failed; all pass on the current code.
- **Codex review:** APPROVE WITH NITS. The nit (A26 tests must prove the Pathfinder op actually ran, not pass vacuously on an empty result) was fixed in `775bbb3`. Codex also confirmed a side observation, now logged as `PAINS_LOG.md` P12 (Pen click on a middle anchor opens the path; Illustrator's Pen joins the neighbours). It needs an owner decision and is not a defect of this branch.
- **Checks run (moderator, on the merged `main`, macOS, Rust toolchain `~/.cargo/bin`):**
  - `cargo test -p varos-core -p varos-pdf -p varos-render-wgpu -j 4` → 33 result lines, **226 passed, 0 failed, 0 ignored** (was 220; +6 new tests).
  - `cargo clippy -p varos-core -p varos-pdf -p varos-render-wgpu --all-targets -- -D warnings` → clean (0 warnings, exit 0).
  - `cargo fmt --all --check` → clean (exit 0, no output).
  - `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` → clean (0 warnings, exit 0) — the whole workspace, `varos-app` included, type-checked for Windows.
  - **Not run:** `varos-app` tests. The app crate does not build on macOS until the Mac port lands (`cargo check -p varos-app` fails in the `windows-future` crate, 16 errors). The full `cargo test --workspace` must run on Windows or after the port.
- **Conflicts:** none (`ort` merge, 3 files).
- **Hand test:** not yet — per the owner decision of 2026-09-23, Ahmed tests in batches after merge. For his list: boolean shapes keep sharp corners; deleting an anchor opens the path.
- **Verdict:** PASS. **Merged:** `a468a8b` to `main`.
- Sign-off: moderator — PASS — 2026-09-23

## cargo-audit triage report

- **Date:** 2026-09-23. **Commit:** `a5e437c` on `main` — `docs/audits/2026-09-23-CARGO_AUDIT_TRIAGE.md`, written from a real `cargo audit` run (it corrects an earlier Codex draft).
- **Findings:** 4 vulnerabilities + 6 warnings (4 unmaintained, 1 unsound, 1 yanked); 8 of 10 clear with seven lockfile bumps (candidate audit on a throw-away copy → only the two font warnings `0192`/`0206` remain). Windows reach of the 4 vulnerabilities: quick-xml `0194`/`0195` — not in the Windows build; webbrowser `0257` — crate compiled, vulnerable Unix code not; crossbeam-epoch `0204` — **in the Windows build** (PDF loading via lopdf → rayon), no trigger demonstrated, patch recommended.
- **Action:** no dependency was changed. The bumps wait for owner approval.
- Sign-off: moderator — report landed — 2026-09-23

## Mac port — `varos-app` builds and runs on macOS

- **Date:** 2026-09-23. **Branch:** `worktree-agent-a8668165735dd9308` (range `61786a0..c3093dd`, commits `34391ab`, `c3093dd`; pushed as `feat/mac-shell-port`). **Reviewer:** Codex (independent, two rounds) + moderator gates.
- **Scope:** `varos-app` builds and launches on macOS. Every Win32 call stays as it was, behind `#[cfg(windows)]`; each Windows-only shell function got a `#[cfg(not(windows))]` twin with the same signature (`cursors.rs`); the `windows` crate moved to `[target.'cfg(windows)'.dependencies]`. Two shared renderer fixes in `varos-render-wgpu` also run on Windows: the real adapter texture-size limit (the 2048px downlevel cap panicked on a Retina window) and egui texture upload before frame acquisition (a skipped frame dropped the font atlas, then the next frame panicked). Design of record: `docs/foundation/MAC_SHELL_PORT.md`.
- **Codex review, round 1:** REQUEST CHANGES — 4 findings, all fixed in `c3093dd`:
  1. Cursor ownership (P2): off Windows, egui-winit also sets the OS cursor, so e.g. Space-hand across a panel splitter lost the tool cursor. Fix: pure `resolve_ck` + `cursor_apply_needed`; the tool cursor is re-asserted every frame after `gui.run` on macOS (Windows keeps set-on-change). 2 GPU-free tests.
  2. Texture frees lost on a skipped frame (leak). Fix: `FreeQueue` parks them and releases them after the next real submit. Tested.
  3. No upper clamp on the surface size. Fix: readable startup `Err` above the device texture cap; `resize` clamps to the cap (logged once) instead of a wgpu validation panic. Tested (`fit_to_limit`).
  4. `MAC_SHELL_PORT.md` scope wording inaccurate and two Windows-only sites missing (`main.rs:1` `windows_subsystem`, `ui.rs:1392` `C:/Windows/Fonts`). Fixed.
- **Codex review, round 2:** APPROVE WITH NITS — two P3 doc nits: (a) `MAC_SHELL_PORT.md` — egui-winit deduplicates unchanged cursor icons rather than writing the OS cursor every frame, and the "Two renderer bugs" heading sat above three entries; (b) the `FreeQueue` comment needed "once acquisition succeeds again". Fixed after the merge in `b0972e7` (docs + one comment, no code).
- **Checks run (moderator, on the merged `main` at `b0972e7`, macOS/Apple M5/Metal, Rust `~/.cargo/bin`).** This is the first time the whole workspace, `varos-app` included, ran on this Mac:
  - `cargo test --workspace -j 4` → 36 result lines, **250 passed, 0 failed, 0 ignored** (core 200 · pdf 13 · render-wgpu 15 · app 22 = 6 lib + 16 bin). Matches the implementer's 244 on the branch + the 6 A26/A32 tests on `main`.
  - `cargo clippy --workspace --all-targets -- -D warnings` → clean (exit 0, 0 warnings).
  - `cargo fmt --all --check` → clean (exit 0, no output).
  - `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` → clean (exit 0, 0 warnings) — Windows still compiles.
  - `cargo build --release -p varos-app` → OK (3m 21s); `varos/target/release/varos` 17.9 MB.
  - **Launch smoke test:** release binary run 8 s, still alive, then killed. stderr (complete): `[varos] adapter: "Apple M5" | backend: Metal` / `[varos] present: Immediate | format: Bgra8Unorm | msaa: 8`. No "panic" in stdout/stderr, no macOS crash report, no process left. (The app's `crash.txt` lives under `%APPDATA%`, which does not exist on macOS, so "no panic file" is shown by the clean stderr, not by that file.)
- **Conflicts:** none (`ort` merge, 7 files; `main` had only gained tests and docs since `61786a0`).
- **Hand test:** not yet — batch 1 list in `STATUS.md`.
- **Verdict:** PASS. **Merged:** `9f8ec1d` to `main`.
- Sign-off: moderator — PASS — 2026-09-23

## P11.2 (1)+(2) — viewport culling, view-rect clipping, flatten cache

- **Date:** 2026-09-23. **Branch:** `worktree-agent-a20eb2401bf6ec4b2` (range `61786a0..bcb7cb8`, commits `3acbeaf`, `86244fa`, `40c96ee`, `b5a13b0`, `90dc794`, `ba6e5e0`, `bcb7cb8`; pushed as `perf/p11-2-culling-cache`). **Reviewer:** Codex (independent, two rounds) + moderator gates.
- **Scope:** `scene::build_scene_in_view` culls whole paths outside the window (control-point bbox test) and cuts partly visible fill rings, stroke runs, mask rings, skeleton and snap outline to the view rect grown by 32 px, reusing the existing artboard clippers. A cross-frame flatten cache (`varos-core/src/flatten.rs`) keyed by path id + by-value geometry inputs + quarter-octave zoom bucket. `varos-app` changed only at its two scene call sites (`b5a13b0`), which now pass the physical window size (`window.inner_size()`). Design and evidence of record: `docs/foundation/P11_2_PERF.md`.
- **Codex review, round 1:** REQUEST CHANGES — 2 P1s, both fixed in `ba6e5e0` with red-proof tests (all 4 new core tests and the new `tess.rs` test fail on the pre-fix `scene.rs`):
  1. **P1-1 — coverage batching crossed objects.** When the object between two same-colour translucent strokes was culled, the two strokes merged into one `StrokeCov` batch and their crossing painted once (50%) instead of twice (75%). Fix: the scene closes the opaque run at an object boundary whenever the next object's first prim would merge with the previous object's last one.
  2. **P1-2 — object treatment depended on the view.** A 50%-opacity filled and stroked object whose stroke lay in the off-screen margin flipped from isolated to folded alpha, and back as a small pan brought the stroke into the pad. Fix: treatment is decided from the uncut fill and stroke.
- **Codex review, round 2:** APPROVE WITH NITS — two P3 doc nits, fixed after the merge in `4ea9dcd` (docs only): the off-screen-mask test claim narrowed to what it asserts (empty `mask_rings`; translucent-stroke and knockout members can still draw, pre-existing renderer gap), and "view-independent" narrowed to object **treatment**. The same commit adds a "Known cost" note: 500 consecutive unfilled same-colour translucent rectangles now give 500 `StrokeCov` batches (1,000 draw calls) instead of 1 — structural, not measured; follow-up harness **scene F** (many same-colour translucent strokes).
- **Conflicts:** none. `ort` merge, 10 files; `main.rs` auto-merged cleanly with the Mac-port changes (the import and the two `build_scene` → `build_scene_in_view` call sites, lines 676 and 1067, are the only P11.2 hunks).
- **Checks run (moderator, on the merged `main`, macOS/Apple M5/Metal, Rust `~/.cargo/bin`):**
  - `cargo test --workspace -j 4` → 37 result lines, **267 passed, 0 failed, 0 ignored** (core 216 · pdf 13 · render-wgpu 16 · app 22 = 6 lib + 16 bin). 250 before + 17 new (16 in `varos-core/tests/view_cull.rs`, 1 CPU test in `tess.rs`).
  - `cargo clippy --workspace --all-targets -- -D warnings` → clean (exit 0, 0 warnings).
  - `cargo fmt --all --check` → clean (exit 0, no output).
  - `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` → clean (exit 0).
  - `cargo build --release -p varos-app` → OK; cargo reported every crate `Fresh` (the release binary had been rebuilt from the merged sources at 22:28 by a concurrent `tools/mac/bundle.sh` run). `varos/target/release/varos` 17.9 MB.
  - **Launch smoke test:** release binary run 8 s, still alive, then killed. stderr (complete): `[varos] adapter: "Apple M5" | backend: Metal` / `[varos] present: Immediate | format: Bgra8Unorm | msaa: 8`. stdout empty, no "panic", no new macOS crash report, no process left.
  - **Perf harness** (`cargo run -p varos-render-wgpu --release --example perf_harness -j 4`, one run, load average 3.0; vertex counts identical to `P11_2_PERF.md`):
    - `D curved-150 selected ppu=40  scene_cold=0.119ms scene_warm=0.089ms cold=0.136ms warm=0.106ms vertices=1068/2388/0 overlay_vertices=3384`
    - `E rectangles-500 ppu=4.0 partial  scene_cold=1.440ms scene_warm=1.428ms cold=1.600ms warm=1.588ms vertices=2835/51624/0`
- **Process note:** a concurrent agent committed `7f3d883` (`tools/mac/bundle.sh`, script + doc only) on top of this merge in the shared checkout and pushed `main` at 22:29, before this gate run finished. All gates above came back green, and `7f3d883` touches no Rust code, so nothing unverified reached `origin`; recorded for honesty.
- **Not verified:** GPU fill-rate gain at 4000% (inferred from vertex counts only); the scene-F draw-call cost; the look of translucent overlapping strokes and masked objects in the real window.
- **Hand test:** not yet — batch 1 list in `STATUS.md`.
- **Verdict:** PASS. **Merged:** `b15d2bf` to `main`.
- Sign-off: moderator — PASS — 2026-09-23

## Mac custom tool cursors — winit `CustomCursor`

- **Date:** 2026-09-23. **Branch:** `feat/mac-cursors` (worktree `agent-abde3c60632d6ed46`, range `c3093dd..378c808`, commits `ba71ae3`, `378c808`). **Reviewer:** Codex (independent, two rounds) + moderator gates.
- **Scope:** on macOS each of the 28 cursor states (CK) gets a winit `CustomCursor` built once at startup (`cursors::create_custom_cursors`), using the same 32 px straight-alpha bitmaps and hotspots as the Win32 HCURSORs (`svg_file_rgba` is now shared with Windows). Source per state: the local Illustrator reference SVG in `assets/cursors-ai/svg/` when present (gitignored, never committed or shipped), otherwise one of the 11 legal built-in glyphs, otherwise the system `CursorIcon`. `main.rs` gained only 4 Mac-only lines. Design of record: `docs/foundation/MAC_SHELL_PORT.md` § "Tool cursors on macOS".
- **Codex review, round 1:** REQUEST CHANGES — 3 findings, all fixed in `378c808`:
  1. **P2 — 17 states collapsed to the arrow in a fresh clone.** Without `cursors-ai`, the built-in `svg()` is only the arrow placeholder for the 17 interaction/rotate states, so resize, hand, grab, copy, no-drop and rotate all showed the arrow. Fix: an explicit, exhaustive `has_builtin(ck)` table (the 11 real glyphs); `cursor_rgba` returns `None` when there is neither a `cursors-ai` file nor a distinct built-in, and those states keep the system `CursorIcon`. Two GPU-free tests: `has_builtin` equals the SVG set and each glyph differs from the arrow; with `use_ai=false` every CK has a distinct bitmap or a non-default system icon.
  2. **P3 — doc honesty.** `MAC_SHELL_PORT.md` now says implemented + startup-verified, not yet hand-tested on screen; Retina softness unverified.
  3. **P3 — broken-file test.** The Illustrator test no longer skips a file that is present but fails to render: absent → skip, present → must render at 32×32.
- **Codex review, round 2:** APPROVE.
- **Implementer's gates (on the branch):** 248/248 tests; clippy (macOS + Windows target) and fmt clean; startup log `28 custom (28 from cursors-ai, 0 built-in) + 0 system fallbacks of 28` with the local set, `11 custom (0 from cursors-ai, 11 built-in) + 17 system fallbacks of 28` without it.
- **Conflicts:** none. `ort` merge, 3 files. `main.rs` auto-merged: the P11.2 `build_scene_in_view` call sites (3 occurrences) and the 4 new Mac-only cursor lines (`#[cfg(not(windows))] cursors::create_custom_cursors(&event_loop);` after `cursors::bind_window`) both present. `MAC_SHELL_PORT.md` auto-merged: the cursors section (line 53) and the bundle.sh "Run on macOS" section (line 136) both present.
- **Local reference set:** copied into the main checkout (`rsync` of the worktree's `varos/crates/varos-app/assets/cursors-ai/`, 326 SVGs). `git check-ignore -v …/cursors-ai/svg/CUR_PEN.svg` → `.gitignore:22:**/cursors-ai/`; `git status --short` stayed empty. Not committed.
- **Checks run (moderator, on the merged `main` at `0a6974d`, macOS/Apple M5/Metal, Rust `~/.cargo/bin`):**
  - `cargo test --workspace -j 4` → 37 result lines, **271 passed, 0 failed, 0 ignored** (core 216 · pdf 13 · render-wgpu 16 · app 26 = 6 lib + 20 bin). 267 before + 4 new cursor tests in `cursors.rs` (the brief's "+2" estimate was low; the branch adds 4 `#[test]` functions, all 4 ran).
  - `cargo clippy --workspace --all-targets -- -D warnings` → clean (exit 0).
  - `cargo fmt --all --check` → clean (exit 0, no output).
  - `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` → clean (exit 0).
  - `tools/mac/bundle.sh` → release build OK, ad-hoc signed (`com.varos.editor`), installed `/Applications/Varos.app` (binary 22:45, contains the new `system fallbacks of` log string). `open /Applications/Varos.app` → `pgrep` showed `/Applications/Varos.app/Contents/MacOS/varos` running after 3 s.
  - **Launch smoke test:** release binary `varos/target/release/varos` run 8 s, still alive, then killed (both it and the bundle; no process left). stderr (complete): `[varos] cursors: 28 custom (28 from cursors-ai, 0 built-in) + 0 system fallbacks of 28` / `[varos] adapter: "Apple M5" | backend: Metal` / `[varos] present: Immediate | format: Bgra8Unorm | msaa: 8`. No "panic" or "error".
- **Process note:** the first test log was overwritten in the shared scratchpad by a concurrent agent's run (it showed 267, no cursor tests, timestamp after this run). The tests were re-run into a uniquely named log; the numbers above come from that re-run.
- **Not verified:** the cursors on screen (shape, hotspot, Retina sharpness — winit sizes the NSImage in points = bitmap pixels, so 32 px bitmaps give a 32 pt cursor that is expected to look soft on Retina); the fresh-clone path (11 built-in + 17 system) was checked by the implementer and by the `use_ai=false` test, not by the moderator's launch.
- **Hand test:** not yet — batch 1 list in `STATUS.md`.
- **Verdict:** PASS. **Merged:** `0a6974d` to `main`.
- Sign-off: moderator — PASS — 2026-09-23

## Varos original cursor set v1 — design assets + study (no code)

- **Date:** 2026-09-23. **Branch:** `design/cursors-v1` (worktree `agent-a0f43430ff2f6bc02`, range `4bf0adc..e7a471e`, commits `fcc235f` — 10 cursors + comparison study, `e7a471e` — 20 more, completing the 28-state set; ink weights unified). **Reviewer:** Codex (independent legal/originality review, two rounds) + moderator gates.
- **Scope:** 30 original SVG cursors in `varos/crates/varos-app/assets/cursors/v1/` (28 `CK` states — `Move` reuses `select.svg` — plus proposed zoom-in, zoom-out and artboard), `hotspots.json` (`ck` 28 · `proposed_ck` 3 · `files` 30), and the study `docs/studies/2026-09-23-CURSOR_SET_V1.md` (stamped `reference`). No Rust code changed; nothing is wired yet — the app still uses the local Illustrator reference set / built-in glyphs.
- **Codex originality review, round 1:** the 10 round-1 files compared against all 326 Adobe reference SVGs (2,039 paths, normalised) → no identical or near-identical path data; all 10 **Original**. One nit (inconsistent ink weights: pen slit 1.25, zoom handle 2.5) → fixed in `e7a471e` (every ink stroke 1.5).
- **Codex originality review, round 2:** the 20 round-2 files compared against the same 326 SVGs → all 20 **Original**. **APPROVE WITH NITS**, two doc-only nits in the study, fixed after the merge in `b8bc2ec`: (a) "every badge is 7.5px" → one shared anchor at (21.25, 21.25), optical size 7.5–9 px, measured ink boxes listed (7.5×7.5 · 7.5×1.5 · 7.56×8.5 · 8×8 · 9×5.25); (b) "round caps everywhere" and "authored on integers" → round joins/caps on free ends with the butt-cap exception listed (bars on a white field: `pen-add`, `copy`, `pen-delete`, `shape-rect`, `artboard` ×2, `zoom-in`, `zoom-out`), and the real 0.25 px grid described. Moderator re-checked (b) with a script: 121 horizontal/vertical path segments, all un-rotated ones centred on whole or half coordinates (the only 4 exceptions are in the 45°-rotated `eyedropper`). The same commit records the round-2 result in the study (it still said "recommended before merge").
- **Owner visual approval:** Ahmed approved the whole set on the review page ("تحف فنية", "ركبهم كلهم").
- **Conflicts:** none. `ort` merge, 32 files, 894 insertions, all new files.
- **Checks run (moderator, on the merged `main`, macOS/Apple M5/Metal, Rust `~/.cargo/bin`):**
  - `cargo test --workspace -j 4` → **271 passed, 0 failed** (unchanged, as expected for assets + docs).
  - `cargo clippy --workspace --all-targets -- -D warnings` → clean (exit 0). `cargo fmt --all --check` → clean (exit 0).
  - `xmllint --noout` on all 30 SVGs → 30 parse, 0 BAD.
  - `hotspots.json` → valid JSON; 4 top-level keys (`_about`, `ck`, `proposed_ck`, `files`); `files` has 30 entries and matches the 30 SVG filenames exactly.
  - **Legal hygiene:** `git ls-files | grep -c cursors-ai` → **0**; `git ls-files | grep -ic 'cursors-ai\|illustrator\|adobe'` → 0. The comparison gallery (holds Adobe files) lives only under gitignored `varos/target/`.
- **Not verified:** the cursors on screen — they are not wired into `cursors.rs` yet (next piece). 1× crispness on non-Retina screens (the set is tuned for 2×; study §7).
- **Hand test:** visual approval given on the review page; in-app hand test comes with the wiring piece.
- **Verdict:** PASS. **Merged:** `d30365c` to `main`.
- Sign-off: moderator — PASS — 2026-09-23

## Stroke-join wedge fix — close the outer wedges left by P11.1's join elision

- **Date:** 2026-09-23. **Branch:** `fix/stroke-fan-artifact` (worktree `agent-a297d1d6b4f0838ea`, range `7f3d883..ccd92ea`, commits `045c956` — close every join, `ccd92ea` — round outer sector within 0.25 px). **Reviewer:** Codex (independent, two rounds) + moderator gates.
- **Symptom:** thick strokes at high zoom rendered as radial "spokes" (Ahmed saw it live in the installed macOS app: an 80.4-wide closed curvy path at 327%).
- **Root cause:** P11.1 (`1931c80`, July) skipped round joins at turns < 5°, on the premise that segment quads overlap there. They overlap only on the inner side of a turn; on the outer side every flattened curve vertex left an open wedge about r·θ wide (≈ 7 px in the owner's case).
- **Bisect** (headless CPU tessellation, uncovered band pixels at 327%, whole path in view): `8da2d8c` (before P11.1) 15 → `1931c80` (P11.1) **47,677**; `61786a0`, `9f8ec1d` and `main` equal to P11.1. P11.1 caused it; P11.2's view clipping and the macOS port did not.
- **Fix** (`varos-render-wgpu/src/tess.rs` `stroke_poly` / `stroke_join`): every turn is closed on its outer side by a round fan anchored on the exact corners of the incoming and outgoing quads (no crack). Steps: chord sagitta ≤ 0.25 screen px, at least 1 step per 45°, at most 128 steps; a single triangle (bevel) for gentle turns; a small-angle-exact angle formula. Corners no longer use the 24-gon disc. Design and numbers of record: `P11_1_PERF.md` ("Correction") and `P11_2_PERF.md` ("After the stroke-join fix").
- **Codex review, round 1:** REQUEST CHANGES — the first version fell back to the full 24-triangle disc at sharper turns; that disc is inscribed and sits up to r·(1 − cos 7.5°) ≈ 13.7 px inside the band at 4000%, leaving gaps. Fixed in `ccd92ea` (the outer sector replaced the disc fallback).
- **Codex review, round 2:** APPROVE WITH NITS — one P3 doc nit: both perf docs promised ≤ 0.25 px without the 128-step cap (a 180° join passes the tolerance above r ≈ 3,320 screen px; r = 10,000 px → 0.753 px). Fixed after the merge in `35443b8` (docs only; the code comment at `JOIN_MAX_STEPS` already said so). Moderator re-derived both numbers: 0.25 / (1 − cos(π/256)) = 3,320.1; 10,000 · (1 − cos(π/256)) = 0.753.
- **Implementer's gates (on the branch):** 272/272 tests, clippy (macOS + Windows target) and fmt clean, 8 s launch. Harness: D stroke vertices 2,388 → 3,363 (time about equal); E 51,624 → 27,996 and 1.21 → 1.18 ms; C +40% vertices, +11% time.
- **Conflicts:** none. `ort` merge, 3 files (`tess.rs`, `P11_1_PERF.md`, `P11_2_PERF.md`); `main` had not touched any of the three since the branch base `7f3d883` (the earlier P11.2 Codex-nit edits, `4ea9dcd`, were already in that base), so there was nothing to reconcile.
- **Checks run (moderator, on the merged `main` at `35443b8`, macOS/Apple M5/Metal, Rust `~/.cargo/bin`):**
  - `cargo test --workspace -j 4` → 37 result lines, **276 passed, 0 failed, 0 ignored** (core 216 · pdf 13 · render-wgpu 21 · app 26 = 6 lib + 20 bin). 271 before + 5 new GPU-free tests in `tess.rs`.
  - `cargo clippy --workspace --all-targets -- -D warnings` → clean (exit 0).
  - `cargo fmt --all --check` → clean (exit 0, no output).
  - `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` → clean (exit 0).
  - **Perf harness** (`cargo run -p varos-render-wgpu --release --example perf_harness -j 4`, one run, 15 measured runs each, median, load average 5.1 — timings noisy; vertex counts match the implementer's):
    - `A curved-150 selected ppu=3.0  scene_cold=0.100ms scene_warm=0.094ms cold=0.283ms warm=0.275ms vertices=3078/9480/0 overlay_vertices=38520`
    - `B rectangles-500 ppu=0.3  scene_cold=2.222ms scene_warm=2.170ms cold=2.391ms warm=2.336ms vertices=10500/12000/0`
    - `C curves-100 ppu=1.0  scene_cold=0.248ms scene_warm=0.212ms cold=0.551ms warm=0.517ms vertices=29700/100500/0`
    - `D curved-150 selected ppu=40  scene_cold=0.094ms scene_warm=0.070ms cold=0.109ms warm=0.085ms vertices=1068/3363/0 overlay_vertices=4170`
    - `E rectangles-500 ppu=4.0 partial  scene_cold=1.070ms scene_warm=1.057ms cold=1.164ms warm=1.152ms vertices=2835/27996/0`
  - `tools/mac/bundle.sh` → release build OK, ad-hoc signed (`com.varos.editor`, `Signature=adhoc`), installed `/Applications/Varos.app` (binary 23:25). `open /Applications/Varos.app` → `pgrep` showed `/Applications/Varos.app/Contents/MacOS/varos` running after 3 s; killed.
  - **Launch smoke test:** release binary `varos/target/release/varos` run 8 s, still alive, then killed (no process left). stderr (complete): `[varos] cursors: 28 custom (28 from cursors-ai, 0 built-in) + 0 system fallbacks of 28` / `[varos] adapter: "Apple M5" | backend: Metal` / `[varos] present: Immediate | format: Bgra8Unorm | msaa: 8`. stdout empty, no "panic" or "error".
- **Process note:** the first 8 s launch run returned an odd shell exit (144) before printing its result; its stderr file was complete (same three lines). It was re-run into uniquely named logs and the result above comes from that re-run. A concurrent session sharing the scratchpad had its own `varos` binary running; it was not touched.
- **Not verified:** the look in the real window (no screenshot at 327% / 4000%); GPU time (harness is CPU-only). Known leftover: round **caps** at open path ends are still 24-gons — visible faceting (~14 px) at 4000% with width 80 (`PAINS_LOG.md` P13, follow-up).
- **Hand test:** not yet — batch 1 list in `STATUS.md`.
- **Verdict:** PASS. **Merged:** `b37f78b` to `main`.
- Sign-off: moderator — PASS — 2026-09-23

## macOS chrome — one bar, opaque strip, native menu bar (muda)

- **Date:** 2026-09-24. **Branch:** `feat/mac-chrome` (worktree `agent-a4662a65bd9f34389`, commits `efd29ef` — design note `docs/foundation/MAC_CHROME.md`, `209705a` — chrome + native menu bar, `1c05ac0` — Codex-review fixes). **Reviewer:** Codex (independent, with build rights) + moderator gates.
- **Scope:** macOS window chrome + native menu bar — new `chrome.rs` and `mac_menu.rs`, with `muda` 0.20 as a macOS-only dependency (Apache-2.0 OR MIT). One bar replaces the two title bars; the title strip is opaque.
- **Codex review:** REQUEST CHANGES — P2 caption drag through floating UI; P2 false double-click zoom. Both fixed in `1c05ac0`: `mac_caption.rs` extracts the pure functions `caption_drag_allowed` / `caption_double_click`, with 3 tests.
- **Codex gates (on the branch, after the fixes):** 284/284 tests.
- **Checks run (moderator, on the merged `main` at `e212cf0`, macOS):**
  - `cargo test --workspace` → **289 passed, 0 failed**.
  - `cargo clippy --workspace --all-targets -- -D warnings` → clean.
  - `cargo fmt --all --check` → clean.
  - `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` → clean.
  - `tools/mac/bundle.sh` → `Varos.app` rebuilt and reinstalled (2026-09-24).
- **Process note:** merged early on 2026-09-24, after the 2am limit reset.
- **Known gaps:** traffic lights sit ≈9pt above the bar's control centre; Quit/Close do not prompt for unsaved changes (same as ✕ today); menu items without an existing shortcut are omitted (New, Export, Cut/Copy/Paste, Duplicate, Select All, Deselect, Zoom In/Out, Delete). “Ctrl” labels should read ⌘ on Mac — next small piece.
- **Hand test:** not yet — batch 1 list in `STATUS.md`.
- **Verdict:** PASS. **Merged:** `e212cf0` to `main`.
- Sign-off: moderator — PASS — 2026-09-24

## Varos cursor set v1 wired + Retina NSCursor

- **Date:** 2026-09-24. **Branch:** `feat/cursors-v1-wired` (commit `1ca727d`). **Reviewer:** Codex + moderator gates.
- **Scope:** 30 v1 SVGs embedded at compile time, covering all 28 `CK` states; v1 is the default on all platforms. The Illustrator reference set is available only behind `VAROS_CURSORS_AI=1`. On macOS, each `NSCursor` comes from a 32-pt `NSImage` with 1× + 2× representations.
- **Codex review:** REQUEST CHANGES — P2 unscoped `NSCursor::set`; P2 cursor restoration stall. Both fixed before merge: `native_cursor_apply_needed` limits application to the pointer inside the focused client area, after egui output; redraw on `CursorEntered` / `Focused` and on egui repaint restores the cursor promptly.
- **Conflicts:** with mac-chrome in `Cargo.toml`, `Cargo.lock`, `THIRD_PARTY_NOTICES` and `main.rs`; resolved by Codex keeping both changes.
- **Checks run (moderator, on the merged `main`, macOS):**
  - Workspace tests → **294/294 passed**.
  - Clippy (macOS + Windows target) → clean; fmt → clean.
  - **Startup log after install:** `[varos] cursors: 28 v1 (+ 0 reference overrides); 28 Retina NSCursor (32 pt, 1x + 2x), 0 winit 1x, 0 system fallbacks`.
- **Hand test:** pending — batch 1 list in `STATUS.md`.
- **Verdict:** PASS. **Merged:** `ac7d633` to `main`.
- Sign-off: moderator — PASS — 2026-09-24

## Stroke joins round 3 — close radial hairlines far from the origin

- **Date:** 2026-09-24. **Branch:** `fix/stroke-fan-artifact` (commit `f751cb2`). **Reviewer:** Codex + moderator gates.
- **Symptom:** Ahmed still saw ≈1 px radial hairline gaps on the outer half of thick strokes at extreme zoom on a large circle far from the origin.
- **Root causes:** fan/quad T-junctions (edges not shared exactly), f32 cancellation far from the origin, tiny-turn elision and skipped short segments.
- **Fix:** f64 stroke geometry with shared corners cast once; stable sagitta used only for subdivision; every non-zero segment kept; f64 cover bounds and padding for translucent strokes / knockout.
- **Codex review:** REQUEST CHANGES — P2 f32 cover bounds crop; P3 perf docs. Both fixed before merge.
- **Regression evidence:** 9 GPU-free regressions using a radial-seam sampler fail on the old code and pass after the fix. Uncovered samples before → after: near circle **3/0/0 → 0**, far circle **8/0/0 → 0**, seven-point **15/1 → 0**, short-segment **38 → 0**.
- **Perf harness:** alternating before/after runs, 10 rounds, load ≈0.95 — A **+5.8%**, B **−0.05%**, C **+3.8%**, D **+3.1%**, E **0.0%**.
- **Checks run (moderator, on the merged `main`, macOS):**
  - Workspace tests → **303/303 passed** (294 before + 9 regressions).
  - Clippy (macOS + Windows target) → clean; fmt → clean.
  - `Varos.app` rebuilt (2026-09-24).
- **Hand test:** pending — batch 1 list in `STATUS.md`.
- **Verdict:** PASS. **Merged:** `d6095f0` to `main`.
- Sign-off: moderator — PASS — 2026-09-24

## P11.2 (0) instant zoom + macOS shortcut labels and top-bar alignment

- **Date:** 2026-09-24. **Branch:** `feat/mac-small-fixes` (commit `4c7cba9`). **Reviewer:** Codex + moderator gates.
- **Scope:** P11.2 item (0) — instant zoom on all platforms; A13 glide/easing removed (`eased_step` deleted from `varos-core/src/geom.rs`), with anchor-at-cursor math unchanged. Owner decision of 2026-07-12 reaffirmed 2026-09-23. macOS shortcut labels show ⌘ via single-home helpers in `shell/tokens.rs` (labels only; key handling unchanged). The macOS top bar is 28 pt (Windows stays 46 pt), to centre its controls vertically on the native traffic lights.
- **Codex review:** REQUEST CHANGES — P2 Windows should be instant too; P3 geometry test overclaimed. Both fixed → APPROVE. The geometry test now uses production control rects (±1 pt); real-window alignment remains unverified.
- **Checks run (moderator, on the merged `main` at `36d04d4`, macOS):**
  - Workspace tests → **306/306 passed**.
  - Clippy (macOS + Windows target) → clean; fmt → clean.
  - `Varos.app` rebuilt and reinstalled 2026-09-24 (~03:5x).
- **Hand test:** pending — batch 1 list in `STATUS.md`. Astra (Codex computer-use) was **BLOCKED** by macOS app-control permission for Varos; the owner must allow control of Varos in the Codex app, then retry. Brief staged at `varos/target/astra-brief.txt`.
- **Verdict:** PASS. **Merged:** `36d04d4` to `main`.
- Sign-off: moderator — PASS — 2026-09-24

## Astra batch 1 — cloud session (Linux moderator + Opus subagents), branch `claude/sweet-cerf-1sg30t`

- **Date:** 2026-09-24. **Branch:** `claude/sweet-cerf-1sg30t` (head `0e369a6`). **Reviewer:** moderator review of every diff + gates on the cloud (Linux). **Independent Codex review: NOT run** (not available in the cloud session) — required before any merge to `main`, per STATUS process law.
- **Process note:** first session run entirely in the cloud. Each piece was implemented by an Opus subagent in its own git worktree from `bd24627`, reviewed by the moderator, then merged (`--no-ff`) into the work branch. No `docs/` edits by subagents.
- **Pieces:**
  1. **Quit guard (Astra F01, P0)** `953bca2` — ⌘Q / Quit Varos, the OS close request (red traffic light / ✕) and the caption ✕ share `OpenDocContext::confirm_quit`: clean → quit; dirty → Save / Don't Save / Cancel. Save quits only when the write completed. Save/Save As extracted into `OpenDocContext::save` (unchanged behaviour). 4 headless tests (`quit_guard_tests`).
  2. **⌘± zooms the canvas, ⌘1 keeps the place (Astra F05 + F06)** `389e542` (merge of `23f8e92`) — root cause verified in egui 0.35: `Options::zoom_with_keyboard` (on by default) scaled the whole UI on ⌘+/⌘−/⌘0; now off (`disable_ui_keyboard_zoom`), with a control test proving a stock context does scale. ⌘= / ⌘⇧= / keypad ± zoom the canvas around the canvas centre (`canvas_px`), instantly, by `ZOOM_KEY_STEP` ×1.5 (moderator change from the wheel's ×1.12 notch). ⌘1 goes to 100 % keeping the world point at the canvas centre (`zoom_to`). View menu gains Zoom In ⌘= / Zoom Out ⌘− on the same key path. Tests: 3 in `menu_mirror_tests` + 1 in `ui_zoom_tests`.
  3. **Direct-selection inspector (Astra F07)** `9d0665e` (merge of `0d5947b`) — `Editor::direct_bbox` / `direct_label` (core) measure grabbed anchors ("Anchor", "N anchors") or a Direct path ("Path" / name) when there is no object selection; X/Y/W/H edit the Direct selection through `SetObjectBounds` → `set_direct_bbox` (one undo step, no-op when unchanged, rotated units baked first). W/H disabled with a reason when the extent is zero; rotate/flip disabled for a Direct selection with a tooltip reason. 9 tests in `varos-core/tests/direct_inspector.rs`.
  4. **Artboards + paint (Astra F09 + two "additional observations")** `abfbaa4` (merge of `a2c3cb4`, `83363df`, `a1e0e3c`) — Duplicate Artboard copies the art on the page when "Move artwork" is on (same `paths_on_ab` + `dup_paths` helpers as Alt+drag; copies offset by the board delta, not selected, one undo step). Clicking a board to activate it no longer bumps `rev` (a same-spot move event during the click set `dirty`); confirmed as the cause of the "switching boards marks the document dirty" observation. Stroke-weight field now always sets `cur_sw` and targets `selected_pids()` (covers the Pen's in-progress path) — confirmed cause of "new Pen strokes return to width 2". 7 tests (`boards.rs` +5, `edit_command.rs` +2).
  5. **In-app clipboard (Astra F04, P1)** `0e369a6` (merge of `8032455`) — `varos-core/src/clipboard.rs`: deep copies of the selected paths with their Group ancestry (groups, clip masks, live rotations preserved, as `dup_paths`). `EditCommand::Copy` (no `rev` bump), `Cut` (copy + delete, one undo step), `Paste { offset }` (fresh ids, front of the active layer, copies selected, one undo step; empty clipboard = no-op). ⌘C/⌘X in `apply_key`; ⌘V centred on the canvas centre, ⇧⌘V in place, in `OpenDocContext::shortcut`. Edit menu rows Cut/Copy/Paste/Paste in Place on the same key path; text fields still get egui Copy/Cut/Paste events via `forward_shortcut` (`text_clipboard_event`). Not the OS clipboard (documented). 15 core tests (`tests/clipboard.rs`) + 5 app key tests + 1 text-field test + 1 menu test. Moderator note: the subagent's commit carries its own model attribution trailer; the merge commit carries the session's.
- **Open design question for Ahmed (from piece 4, behaviour unchanged):** `doc.active` is persisted in the file, but activating a board no longer shows the unsaved `*`. Whether that should count as unsaved is his call. Two other empty commits noted, not changed: a board-handle drag without movement and leaving a board's name field unchanged both bump `rev`.
- **Checks run (moderator, cloud, Linux, on the merged branch head):**
  - `cargo test --workspace` → **346 passed, 0 failed** (Linux; Mac-only tests not compiled here).
  - Clippy (Linux + `x86_64-pc-windows-msvc` target) → clean; fmt → clean.
  - `cargo clippy -p varos-app --all-targets --target aarch64-apple-darwin -- -D warnings` → clean (Mac-only arms type-checked, not run).
- **Hand test:** pending on the Mac — list "For Ahmed to try (batch 2)" in `STATUS.md`.
- **Verdict:** gates PASS on the branch. **Not merged to `main`**: waits for the Codex review + Ahmed's hand test.

## S3-A — storage foundation (app-data resolver + durable replacement writer)

- **Date:** 2026-09-24. **Branch:** `claude/sweet-cerf-1sg30t`, merging `feat/dfs-s3-a-storage` (base `2c4dd6e`, commits `96fd2cb`, `fd8fa93`, `8cc5ac3`). **Reviewer:** independent code review (`docs/foundation/work_orders/reviews/DFS_S3_A_STORAGE.code-review.md`) + moderator gates.
- **Scope:** `AppLayout` app-data path resolver, `FsPort`/`FaultFs`, the durable replacement writer (create → write → sync → rename → dir-sync) with its per-OS fault matrix, an in-house CRC-32, `time_text`.
- **Independent code review:** REQUEST CHANGES — one P1: the macOS `F_FULLFSYNC` fallback (from plan-review P1-1, commit `fd8fa93`) never fires on the error macOS actually returns. `ENOTSUP` (45) decodes to `ErrorKind::Uncategorized` in std, so a save to an SMB/USB volume would still fail with "couldn't confirm". Fixed in `8cc5ac3`: any `F_FULLFSYNC` error now falls back to a plain `fsync` (SQLite's `if (rc) rc = fsync(fd)` pattern), the same helper drives both the file sync and the directory sync, and a per-OS unsupported-errno table (macOS 22/25/45/78/102, Linux 22/25/38/95; macOS numbers const-asserted against libc) decides Durable vs ReplacedUnconfirmed. Also folded in from the review's P3 nits: permissions are copied onto the temp file right after `create_new`, before any byte is written; the dead `#[allow(deprecated)]` on `home_dir()` removed; the unused `remove_dir` port method dropped.
- **Checks run:** reviewer, pre-fix at `fd8fa93`: `cargo test --workspace -j 4` → 372 passed, 0 failed (storage lib 26 passed); clippy/fmt clean. Implementer, post-fix (`8cc5ac3` commit message): storage tests 28/28; `cargo test --workspace -j 4` → **374 passed, 0 failed** (39 suites); `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all --check` clean; `cargo clippy -p varos-app --all-targets` on `x86_64-pc-windows-msvc` and `aarch64-apple-darwin` clean (compile-only — the macOS `F_FULLFSYNC`/libc paths are compiled, not exercised, here). Merge commit: "Storage tests 28/28 re-run by the moderator; full gates run on the batch."
- **Verdict:** gates PASS on the branch. **Merged:** `580436f` into `claude/sweet-cerf-1sg30t`.
- Independent Codex review: not run (cloud); required before merge to `main`. Hand test: pending (Mac).

## S1-A — workspace/content checkpoint (frozen AppCommand/Workspace/Lifecycle API)

- **Date:** 2026-09-24. **Branch:** `claude/sweet-cerf-1sg30t`, merging `feat/dfs-s1-a-workspace` (base `c1c7de9`, commits `9b8c741`, `c09e94d`). **Reviewer:** independent code review (`docs/foundation/work_orders/reviews/DFS_S1_A_WORKSPACE.code-review.md`) + moderator gates.
- **Scope:** core `content_eq` checkpoint (exhaustive `Document`/`DocUnits` destructure, `paths` compared by Vec order, `ids` ignored), the frozen `AppCommand`/`Workspace`/`Lifecycle` API surface, and the workspace/tab model behind it.
- **Independent code review:** APPROVE WITH NITS — no P1. One real hole, P2-1: the Pen tool's "continue path" resume, when the clicked end is the path's first anchor, reversed anchor order (saved content) via `self.reverse(pi)` without setting `self.dirty`, so the tab memoised clean (no dot, no `*`) while content had actually changed, and ⌘W asked to save on a tab that showed no dot. Fixed in `c09e94d`: the reversal now sets `dirty` inside the pointer-down transaction, committing as one undo step; a P3 companion fix makes `DocumentSession::settle` also finish a Pen add/delete-anchor transaction opened with no drag. Cost measured by the reviewer (throwaway 10,000-path × 8-anchor probe, release): `content_eq` (equal, full walk) **3.9 ms**; `Document::clone` 3.8 ms; `begin()` already clones per gesture, so the checkpoint roughly doubles per-edit overhead, paid once per `rev` change on the active tab, never per frame.
- **Checks run:** reviewer, pre-fix at `9b8c741`: `cargo test --workspace -j 4` → 365 passed, 0 failed; clippy/fmt clean on host + Windows + Mac targets. Implementer, post-fix (`c09e94d` commit message): `cargo test --workspace -j 4` → **366 passed, 0 failed** (40 suites); `cargo clippy --workspace --all-targets -j 4 -- -D warnings` clean; `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` clean (compile-only); `cargo clippy -p varos-app --all-targets --target aarch64-apple-darwin -- -D warnings` clean (compile-only). Merge commit: "Gates on the batch."
- **Verdict:** gates PASS on the branch. **Merged:** `24aa2e0` into `claude/sweet-cerf-1sg30t`.
- Independent Codex review: not run (cloud); required before merge to `main`. Hand test: pending (Mac).

## QW3 — layer/object rename you can find (Astra F10, P4, FB6)

- **Date:** 2026-09-24. **Branch:** `claude/sweet-cerf-1sg30t`, merging `fix/qw3-layer-rename` (base `c9067d4`, commits `0679979`, `d23aa50`). **Reviewer:** independent code review (`docs/foundation/work_orders/reviews/QW3_LAYER_RENAME.code-review.md`) + moderator gates.
- **Scope:** root cause of Astra F10 fixed as an `EditCommand` — a `<Path>` layer row now writes `Path::name` (the name the row actually displays) through a new `EditCommand::RenamePath`, instead of the old `RenameNode`, which wrote the invisible `Node::name`; the inline-rename editor state machine hardened against stuck focus on egui 0.35; the properties dock made honest while the Pen is drawing (FB6); invisible bidi/format marks trimmed from pasted Arabic names.
- **Independent code review:** APPROVE WITH NITS — no P1, two P2s fixed before merge in `d23aa50`: (1) mid-draft the dock read the stale object selection's zeros while its fields still wrote to that stale selection — fixed by having the Pen clear `objsel` when a new draft starts, so a new path deselects other art (matching Illustrator); (2) the new `layer_rename_tests` cost 10.8 s for 9 tests because `row_y` rebuilt 120 fresh egui contexts × 5 frames per lookup — fixed by stopping the probe once the hit run ends, down to 0.8 s. Invisible-mark trimming for Arabic names was folded in alongside.
- **Checks run:** reviewer, pre-fix at `0679979`: `cargo test --workspace -j 4` → 359 passed, 0 failed (40 result lines); `cargo test -p varos-app layer_rename_tests` 9/9 in 10.80 s; clippy/fmt clean on host + Windows + Mac targets. Implementer, post-fix (`d23aa50` commit message): `cargo test --workspace` → **362 passed, 0 failed** (39 suites; `layer_rename_tests` now 0.80 s); `cargo clippy --workspace --all-targets -- -D warnings` clean; `fmt --check` clean; `cargo clippy -p varos-app` for `x86_64-pc-windows-msvc` and `aarch64-apple-darwin` clean (compile-only). Merge commit: "Gates on the batch."
- **Verdict:** gates PASS on the branch. **Merged:** `3c208fa` into `claude/sweet-cerf-1sg30t`.
- Independent Codex review: not run (cloud); required before merge to `main`. Hand test: pending (Mac).

## QW1 — thick-stroke hit-testing (Astra F08)

- **Date:** 2026-09-24. **Branch:** `claude/sweet-cerf-1sg30t`, merging `fix/qw1-stroke-hit` (base `5ef0687`, commits `cada338`, `00cfc41`, `1f19bfc` + merges). **Reviewer:** independent code review (`docs/foundation/work_orders/reviews/QW1_STROKE_HIT.code-review.md`) + moderator gates.
- **Scope:** hit-testing reaches the true painted band of a thick stroke (true-curve distance via `cubic_nearest`, not 25-point sampling), the top-down occlusion walk, the marquee's "filled-only" touch rule extended to hole rims, and the rotate-ring guard no longer blocked by the selection's own thick band (plan-review P1).
- **Independent code review:** REQUEST CHANGES — one P1: `cubic_nearest`'s Gauss-Newton step included a curvature term that stalled on **straight segments** (every rectangle/polygon edge, where handles collapse to the anchors), so a click exactly on the centreline near a corner still measured up to **21.9 units** off on a 20,000-unit edge, with some probes missing entirely (`path_under` → `None`). Fixed in `1f19bfc` by dropping the curvature term (pure Gauss-Newton, always convergent near the residual): the same 20,000-edge worst case fell from **22.0 to 0.07 units**. One P2 also fixed: the marquee's hole-rim touch branch had no test and a mutation disabling it passed the whole suite — added `marquee_touching_only_a_donut_hole_rim_selects_it`. Net effect on hover cost: the bbox cull added alongside the fix made hover **faster than before — 13.0 ms → 5.9 ms** on the 500-path × 50-segment release benchmark.
- **Checks run:** reviewer, pre-fix at `00cfc41`: `cargo test --workspace -j 4` → 360 passed, 0 failed (`hit_thick_stroke.rs` 13/13, `occlusion.rs` +1); clippy/fmt clean on host + Windows + Mac targets; hover benchmark (release, 500 rings × 50 cubics) **13.0 / 13.5 ms → old 10.2 / 10.9 ms** (≈1.3× slower, the P1 defect). Implementer, post-fix (`1f19bfc` commit message): `cargo test --workspace` → **364 passed, 0 failed**, adding `big_rect_edge_near_corner_centreline_is_hit`, `pen_add_anchor_near_a_corner_lands_on_the_line`, `marquee_touching_only_a_donut_hole_rim_selects_it`, and `occlusion.rs::lower_thick_band_does_not_steal_from_thin_top_path`; clippy/fmt clean; hover benchmark with the added bbox cull → **5.9 ms/call**, down from the pre-fix **13.0 ms/call**. Merge commit: "Gates on the batch."
- **Verdict:** gates PASS on the branch. **Merged:** `b9a8b2c` into `claude/sweet-cerf-1sg30t`.
- Independent Codex review: not run (cloud); required before merge to `main`. Hand test: pending (Mac).

## S6-A — pure PDF export writer (no embedded model) + page-scope planner + PDF clip masks

- **Date:** 2026-09-24. **Branch:** `claude/sweet-cerf-1sg30t`, merging `feat/dfs-s6-a-pure-pdf` (base `c9067d4`, commits `c91b634`, `69c34c4`, `7e3d190`, `9e04e7c`). **Reviewer:** independent code review (`docs/foundation/work_orders/reviews/DFS_S6_A_PURE_PDF.code-review.md`) + moderator gates.
- **Scope:** PDF export writer refactored to a page-scope planner producing a pure PDF (no embedded Varos model); plan-review F1–F3 applied — masks are clipped with `q … W* n … Q` (per `MASKS_PLAN` Stage 5) instead of being refused, a bounded byte scan, and `outline_bbox`. `native_demo.pdf` stays byte-identical to the pre-refactor writer.
- **Independent code review:** REQUEST CHANGES — one P1: a mask group emitted **every** mask ring for **every** member's clip, with no check that a ring could actually affect that member; a hidden path standing only on a hidden board leaked its exact coordinates into an export that promises "no hidden or editable data" (reproduced). Fixed in `9e04e7c`: rings are culled against the member/run's bbox before emission, and clipping was restructured to one `q <rings> W* n … Q` scope per contiguous run of a clip group's members (matching `scene.rs`'s own run structure) instead of re-emitting the mask per member — file size on a 50-member/64-anchor-mask case fell from **156,889 bytes (≈156 KB) to ≈9 KB**. Mask-ring tests were added (multi-ring even-odd, nested clips, knockout-inside-clip) to close the review's P2 "a mutation that keeps only the first ring passes all 27 tests" finding. `native_demo.pdf` stayed byte-identical; `native_rich.pdf` was re-blessed once for the new clip-run bytes.
- **Checks run:** reviewer, pre-fix on the diff `c9067d4...HEAD`: `cargo test --workspace -j 4` → 373 passed, 0 failed, 0 ignored (`export_pdf.rs` 27 passed); clippy/fmt clean on host + Windows + Mac targets. Implementer, post-fix (`9e04e7c` commit message): `cargo test --workspace -j 4` → **377 passed, 0 failed** (`export_pdf.rs` 31), adding `hidden_mask_path_far_away_never_reaches_the_file`, `clip_uses_every_mask_ring_even_odd`, `nested_clip_uses_nearest_mask_only`, `one_clip_scope_per_run_of_members`; clippy/fmt clean; `cargo clippy -p varos-app` for `x86_64-pc-windows-msvc` and `aarch64-apple-darwin` clean (compile-only). Merge commit: "Gates on the batch."
- **Verdict:** gates PASS on the branch. **Merged:** `9bd3389` into `claude/sweet-cerf-1sg30t`.
- Independent Codex review: not run (cloud); required before merge to `main`. Hand test: pending (Mac).

## Cloud process, 2026-09-24 (afternoon)

- **Planning wave:** 5 work orders staged in `docs/foundation/work_orders/` — `DFS_S1_LIFECYCLE_TABS.md`, `DFS_S2_S3_START_RECENTS_RECOVERY.md`, `DFS_S4_S6_ASSOCIATION_EXPORT.md`, `DFS_S5_FORMAT_V2.md`, `QUICK_WINS_2026-09-24.md` — each given an independent plan review before any implementation started. All 5 plan reviews (`docs/foundation/work_orders/reviews/*.review.md`) came back **APPROVE WITH CHANGES**; every amendment was applied to the work order before the corresponding subagent began (e.g. one resolver, simpler recovery locks, mask clip and S6-D folded into S6-A, QW2 dropped, QW7 folded into S1-C, `MenuCmd::Plain` routing for QW5).
- **Standing rule adopted (owner instruction):** independent code review before every merge — no piece lands, to a work branch or to `main`, without one. The five pieces above each got that independent code review before merging into `claude/sweet-cerf-1sg30t`; three (S3-A, QW1, S6-A) came back REQUEST CHANGES and were fixed before merge, two (S1-A, QW3) came back APPROVE WITH NITS and had their nits fixed before merge.
- **Moderator gate run on the merged head `9bd3389`** (Linux): `cargo test --workspace` → **459/459 passed, 0 failed**; `cargo clippy --workspace --all-targets -- -D warnings` → clean; `cargo fmt --all -- --check` → clean.
- Independent Codex review: not run (cloud); required before merge to `main`. Hand test: pending (Mac).

## Cloud process, 2026-09-24 (evening) — DFS wave 1 + quick wins + UI system initiative

- **Date:** 2026-09-24 (evening). **Branch:** `claude/sweet-cerf-1sg30t` (head `6999970`, 111 commits over `main`). **Reviewer:** Opus code reviewers per piece where marked; moderator review only where marked "INDEPENDENT REVIEW PENDING" (owner asked for a budget hold at 23:xx: finish what runs, launch nothing new).
- **Merged after the afternoon entry (each `--no-ff`; review verdict → fixes → merge):**
  - **QW5** `66dafe2` — Edit ▸ Select All ⌘A / Deselect ⇧⌘A / click-only Delete row; `Editor::select_all`; `MenuCmd::Plain`. Review APPROVE WITH NITS → the one-caller `EditCommand::SelectAll` wrapper dropped.
  - **QW4** `4af375f` — round caps as half-discs by the joins' rule; seam join on closed rings; incremental fan rotation. Review APPROVE WITH NITS → recurrence kept; the fixed emit-buffer reserve was tried, measured (w80 probe 4× slower) and DROPPED by moderator decision; scene C 0.9 → 2.5 ms on Linux is glibc-allocator-bound (86,400 < 100,500 vertices) → follow-up: renderer vertex-buffer reuse. E: 27,996 → 6,744 vertices.
  - **S1-B** `ce87bbf` — lifecycle rules (New/Open/Save/Save As/Close/Quit transaction, "Document i of n") with fake dialog/store ports (30 tests) + real rfd/disk ports. Review APPROVE WITH NITS → plain-English error reasons; collision check before the Replace prompt.
  - **S3-B** `130bc85` — recents + settings store (atomic JSON, 20 items, path-or-inode dedupe, `.bad` aside kept). Review APPROVE WITH NITS → version-direction wording.
  - **S5-E** `ea3be27` — 16 frozen v1 fixtures generated by the `ecf67f5` writer, old-reader harness (11-line verbatim gate), round-trip law over the corpus, `corpus_check` (hand test 0), `docs/reference/VRS_FORMAT.md`. Review APPROVE.
  - **S3-C** `d8908e6` — recovery snapshot store (two generations, manifest published last, one `session.lock`, retire by rename to `.discarded-<nonce>`). Review REQUEST CHANGES → **P1: scan followed symlinks and could delete the user's own files** — fixed with a flat, name-gated remover + real-symlink test; lock held through retire; unknown folders kept as Damaged; two untested safety rules now tested. 26 tests.
  - **S2-E1** `30e7384` — Start page pure model (`StartModel`, focus traversal, `elide_middle`). Drawing deferred to the UI-system kit. Moderator review only — INDEPENDENT REVIEW PENDING.
  - **S3-D** `40b93af` — recovery scheduler (30 s deadline not reset by later edits, one job per session, waits for open transactions, back-off) + generic I/O worker thread (`Job<T>`, panic-safe, drains on shutdown). 15 tests. Moderator review only — INDEPENDENT REVIEW PENDING.
  - **S1-C + S1-D** `e5b9315` — real tab strip (dirty dot, drag-reorder as insertion slot, middle-click close, `+`), burger rows as `AppCommand`s, Export/Share honestly disabled, search pill no longer advertises ⌘K, dead-control test; native File menu as `MenuCmd::File(FileCmd)`; one `Workspace` + one `dispatch` for every lifecycle / window command source (corrected after the independent review: document shortcut keys and menu rows — Key / Plain / Snap — did not go through it and could overtake a queued ⌘S; fixed on `fix/s1cd-tabs-review` with one FIFO queue, where a document action runs at once only when nothing is queued ahead of it; pointer input and panel edits stay outside the queue); batch-1 quit guard replaced by `Lifecycle::run`; scene cache keyed by `SessionId`; a mouse release goes to whoever got the press (picker Cancel now reverts after a click on the chrome); file keys decided before the text-field check. 581/581 on the implementer's private build dir. Moderator review only — INDEPENDENT REVIEW PENDING (both).
  - **S5-B** `6999970` — `.vrs` format v2: version gate before decode, `deny_unknown_fields`, declared limits (40,000 nodes/paths, measured), typed plain-English errors, iterative cycle/depth checks, v1→v2 migration in memory, save runs the same checks. Review APPROVE WITH NITS → serde text hidden from users; dangling mask reference refused. All 16 v1 fixtures load byte-identical to the old reader; the two old-reader tests now pass. **ADR-0008 still Proposed — this stays off `main` until Ahmed accepts.**
- **Not merged / open:** S3-F1/F2 (autosave wiring + recovery UI), S2-E2 (Start page host wiring), S4-A/B (file association), S5-C/D (semantic validator, bounded PDF reader), S6-B/C (Export home + entry points), QW6 (readability, superseded by UI system U0), QW2-lite (Mac window memory). Astra F03 (Export) and F11 remain open; F02 closes with S1 pending the hand test.
- **UI system initiative (owner ask 2026-09-24 evening):** 5 audits in `docs/audits/ui-2026-09-24/` (878 lines), spec v1 `docs/specs/UI_SYSTEM.md` (34 laws, stages U0–U6), 5 independent spec reviews in `docs/foundation/work_orders/reviews/UI_SYSTEM.*.review.md` — **all five REQUEST CHANGES**: direction right, spec needs a v2 (kit crate boundary; core live-span primitive; per-frame `Snap` cost measured at 360 ms on a 10k-path Select All; responder/scope rule for shortcuts; egui Tab focus; box size model + Varos-owned layout format; glide removal needs ADR-0009; Arabic shaping/bidi and accessibility missing; honest cost ≈30–34 agent-days). No UI-system code was started.
- **Checks run (moderator, cloud, Linux, on `6999970`):** `cargo test --workspace` → **624/624 passed, 0 failed, 4 ignored by design (old-reader harness ×2 pre-existing, timing, corpus check)**; clippy Linux + Windows target + Mac target (type-check) → clean; fmt → clean.
- **Process notes:** shared cargo build dirs between worktrees clobbered artifacts (cargo hashes workspace crates by workspace-relative path) — two agents reported false test results until sources were touched; rule changed to a private build dir per piece. Independent Codex review: not run (cloud); required before any merge to `main`. Hand test: pending (Mac) — batch 3 list in `STATUS.md`.
- Sign-off: moderator — gates PASS — 2026-09-24
