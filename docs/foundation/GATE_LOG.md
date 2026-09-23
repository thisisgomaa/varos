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
