> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos STATUS — single current-state page

Authority level 3 (charter §3). Anything that contradicts this page is stale. Updated at every gate.

## Now

### 2026-09-23 — Mac move + autonomous-run decisions (owner)

Owner decisions, recorded as facts on 2026-09-23:

1. **Mac is the primary machine.** Primary working copy: `~/Documents/AI workspace/varos`; the old external-disk copy is a read-only mirror. Rust 1.98.1. `varos-core`, `varos-pdf`, `varos-render-wgpu` build and pass **220/220** tests on macOS/Metal (re-run 2026-09-23: `cargo test -p varos-core -p varos-pdf -p varos-render-wgpu` → 33 result lines, 220 passed, 0 failed). `varos-app` is Windows-only today; its **Mac port is in flight on a separate branch (branch pending)**.
2. **Process change (amends the "Ahmed hand-tests every stage before the next" law for the current autonomous run):** a piece may merge to `main` when `cargo test --workspace` + `clippy -D warnings` + `fmt --check` are green **and** an independent Codex review approves. Ahmed hand-tests **afterwards, in batches**, from a kept "for Ahmed to try" list. Bad pieces are reverted.
3. **Push policy:** `main` and work branches are pushed to GitHub after each good commit.
4. **Priority after the Mac shell port:** P11.2, then the remaining `docs/PAINS_LOG.md` items. Waiting for explicit owner decisions: MCP (needs a new ADR-0008), online/web (needs an ADR-0001 amendment), Compositor-UI picks (Ahmed's eye).
5. **Claude = moderator only:** plans, delegates to Claude subagents + Codex, reviews, lands. It does not implement by hand.

Studies landed (level-5 proposals, stamped `reference`; none is an accepted decision):

- `docs/studies/2026-09-23-MCP_CONTROL_STUDY.md` — letting AI agents drive Varos over MCP through `EditCommand`; asks for ADR-0008.
- `docs/studies/2026-09-23-COMPOSITOR_UI_STUDY.md` — UI behaviours worth borrowing from Compositor, conflicts with `UI_DIRECTION.md`, and which pixel features fit v1.
- `docs/studies/2026-09-23-ONLINE_AND_MAC_STUDY.md` — feasibility and cost of native Mac, in-browser (WASM/WebGPU), and Figma-style cloud Varos; browser work needs an ADR-0001 amendment.

Landed later on 2026-09-23/24:

- **A26/A32 acceptance tests merged** `a468a8b` (tests only; both pains were already fixed in `1bc4f3c`). Gates green on macOS (core+pdf+render **226/226**, clippy, fmt) and Windows-target clippy of the whole workspace; Codex APPROVE WITH NITS, nit fixed. Side finding logged as `PAINS_LOG.md` P12 (Pen anchor-delete opens the path) — needs Ahmed's decision. Details: `GATE_LOG.md`.
- **cargo-audit triage landed** `a5e437c` — `docs/audits/2026-09-23-CARGO_AUDIT_TRIAGE.md`: 4 vulns + 6 warnings, 8/10 clear with seven lockfile bumps; bumps await owner approval.
- **Mac port of `varos-app` merged** `9f8ec1d` (branch `worktree-agent-a8668165735dd9308`, pushed as `feat/mac-shell-port`; Codex REQUEST CHANGES → 4 fixes in `c3093dd` → APPROVE WITH NITS; nits fixed in `b0972e7`). The **whole workspace now builds and tests on macOS: 250/250**, clippy and fmt clean, Windows-target clippy clean, release build launches on Metal (8 s smoke test, no panic). Details: `GATE_LOG.md`.
- **Owner decision (2026-09-23, later): the official build is Mac-only for now** ("بلاش نتعب في الويندوز دلوقتي"). Windows is kept compiling through the cheap gate `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`, so it is not silently broken. No Windows hand-testing and no Windows-specific work until this is revisited. Mac polish comes first.
- **P11.2 items (1)+(2) merged** `b15d2bf` — viewport culling, view-rect clipping and a cross-frame flatten cache (branch `worktree-agent-a20eb2401bf6ec4b2`, pushed as `perf/p11-2-culling-cache`; Codex REQUEST CHANGES → 2 P1s fixed in `ba6e5e0` → APPROVE WITH NITS; nits fixed in `4ea9dcd`). Whole workspace **267/267** on macOS, clippy/fmt clean, Windows-target clippy clean, release build launches. 4000% scene: ~30× fewer vertices (harness D: 1,068 / 2,388 / 3,384). Details: `GATE_LOG.md`, `P11_2_PERF.md`.
- **Mac custom tool cursors merged** `0a6974d` (branch `feat/mac-cursors`, commits `ba71ae3` + `378c808`; Codex REQUEST CHANGES → 3 fixes (P2: 17 states collapsed to the arrow without the local set; P3 doc honesty; P3 broken-file test) → APPROVE). Each of the 28 cursor states is a winit `CustomCursor` built from the same bitmaps as Windows. Source per state: the local Illustrator reference set (gitignored, never committed) when present → our 11 legal built-in glyphs → the system cursor. Whole workspace **271/271** on macOS, clippy/fmt clean, Windows-target clippy clean. Startup log with the local set: `[varos] cursors: 28 custom (28 from cursors-ai, 0 built-in) + 0 system fallbacks of 28`. `/Applications/Varos.app` rebuilt and reinstalled. Details: `GATE_LOG.md`.
- **Original Varos cursor set v1 merged** `d30365c` (branch `design/cursors-v1`, commits `fcc235f` + `e7a471e`; doc nits fixed in `b8bc2ec`) — 30 original SVGs + `hotspots.json` in `varos/crates/varos-app/assets/cursors/v1/` and the study `docs/studies/2026-09-23-CURSOR_SET_V1.md`. Assets + docs only, no code. Codex originality review against all 326 local Adobe reference SVGs: round 1 all 10 Original, round 2 all 20 Original (APPROVE WITH NITS, doc nits fixed). Ahmed approved the set visually. Tests 271/271 unchanged; nothing from `cursors-ai` is tracked (`git ls-files | grep -c cursors-ai` → 0). Details: `GATE_LOG.md`.
- **Stroke-join wedge fix merged** `b37f78b` (branch `fix/stroke-fan-artifact`, commits `045c956` + `ccd92ea`; Codex REQUEST CHANGES (disc fallback left gaps) → fixed → APPROVE WITH NITS; doc nit fixed in `35443b8`). Thick curved strokes at high zoom no longer render as "spokes": P11.1's 5° join elision (`1931c80`) had left an open wedge on the outer side of every curve point; now every turn is closed by a round fan within 0.25 screen px (up to 128 steps per join). Whole workspace **276/276** on macOS (render-wgpu 16 → 21), clippy/fmt clean, Windows-target clippy clean. Harness D stroke vertices 2,388 → 3,363, E 51,624 → 27,996. **`/Applications/Varos.app` refreshed at 23:25 (2026-09-23)** and launches. Leftover: round caps at open path ends are still 24-gons (`PAINS_LOG.md` P13). Details: `GATE_LOG.md`.
- **macOS chrome merged (2026-09-24)** `e212cf0` (branch `feat/mac-chrome`, commits `efd29ef`, `209705a`, `1c05ac0`) — one bar, opaque title strip, native menu bar via `muda` 0.20 (macOS only). Codex REQUEST CHANGES (P2 caption drag through floating UI; P2 false double-click zoom) → both fixed, Codex gates **284/284**. Moderator gates on merged `main`: **289/289**, clippy/fmt clean, Windows-target clippy clean. **`/Applications/Varos.app` refreshed 2026-09-24** via `tools/mac/bundle.sh`; Ahmed's hand test pending. Details: `GATE_LOG.md`, `MAC_CHROME.md`.
- **Varos cursor set v1 wired + Retina NSCursor merged (2026-09-24)** `ac7d633` (branch `feat/cursors-v1-wired`, commit `1ca727d`) — 30 embedded SVGs cover all 28 `CK` states; v1 defaults on all platforms, Illustrator references only via `VAROS_CURSORS_AI=1`. Mac cursors use a 32-pt `NSImage` with 1× + 2× reps. Codex REQUEST CHANGES (P2 unscoped `NSCursor::set`; P2 cursor restoration stall) → fixed → merged; mac-chrome conflicts resolved keeping both. Moderator gates: **294/294**, clippy Mac + Windows clean, fmt clean. Installed startup: 28 v1, 28 Retina NSCursor, no reference overrides or fallbacks; hand test pending. Details: `GATE_LOG.md`.
- **Stroke joins round 3 merged (2026-09-24)** `d6095f0` (branch `fix/stroke-fan-artifact`, commit `f751cb2`) — f64 geometry, shared once-cast corners, every non-zero segment kept and f64 cover bounds/padding close the remaining radial hairlines on huge circles far from the origin. Nine GPU-free regressions fail before and pass after. Codex REQUEST CHANGES (P2 f32 cover bounds crop; P3 perf docs) → fixed → merged. Moderator gates: **303/303**, clippy Mac + Windows clean, fmt clean. **`Varos.app` refreshed 2026-09-24**; hand test pending. Details: `GATE_LOG.md`.
- **Instant zoom + Mac small fixes merged (2026-09-24)** `36d04d4` (branch `feat/mac-small-fixes`, commit `4c7cba9`) — P11.2 item (0): instant zoom on all platforms, A13 glide/easing removed (`eased_step` deleted from `varos-core/src/geom.rs`), anchor-at-cursor math unchanged; owner decision 2026-07-12 reaffirmed 2026-09-23. ⌘ labels on macOS via single-home helpers in `shell/tokens.rs` (labels only; key handling unchanged). Mac top bar 28 pt (Windows stays 46 pt), to centre controls on the native traffic lights. Codex REQUEST CHANGES (P2 Windows should be instant too; P3 geometry test overclaimed) → both fixed → APPROVE. Geometry test uses production control rects (±1 pt); real-window check pending. Moderator gates on merged `main`: **306/306**, clippy Mac + Windows clean, fmt clean. **`Varos.app` rebuilt and reinstalled 2026-09-24 (~03:5x)**; hand test pending. Details: `GATE_LOG.md`.
- **Owner decisions D1–D3 (2026-09-24, Ahmed: "آه للتلاتة"):** the Document & File System proposal `docs/specs/DOCUMENT_FILE_SYSTEM.md` is ACCEPTED as one system — D1 (first complete system, absorbs F4.2, real sessions replace fake tabs, narrow Windows exception for S4 = compile gate only), D2 (Start page by default, boardless New, recovery on at 30 s with two generations, recover-as-copy, explicit discard), D3 (format v2 for all new saves, conservative version refusals, bounded migrations, pure-PDF export; companion ADR-0008 to be drafted and accepted before S5 code lands). Execution: stage work orders in `docs/foundation/work_orders/` (S1 first; pure modules of S3/S5/S6 in parallel), run in the cloud by a Fable moderator with Opus/Sonnet subagents; Ahmed hand-tests in batches afterwards; independent review before any merge to `main`. Owner also authorised (same message) a long autonomous run: unlimited subagents under clear plans, moderator reviews only.
- **Afternoon build wave merged to `claude/sweet-cerf-1sg30t` (2026-09-24, cloud):** 5 pieces merged after Astra batch 1, each behind its own independent code review (3 REQUEST CHANGES → fixed, 2 APPROVE WITH NITS → fixed) — S3-A storage foundation `580436f` (macOS `F_FULLFSYNC` ENOTSUP fallback fixed in `8cc5ac3`), S1-A workspace/content checkpoint `24aa2e0` (Pen-resume dirty bug fixed in `c09e94d`; `content_eq` 3.9 ms on 10k paths), QW3 layer rename `3c208fa` (Astra F10), QW1 thick-stroke hit `b9a8b2c` (Astra F08; hover 13.0 → 5.9 ms), S6-A pure PDF export `9bd3389`. Moderator gates on the merged head `9bd3389` (Linux): **459/459** tests, clippy clean, fmt clean. Independent Codex review not run (cloud) and hand test still pending (Mac) — both required before merge to `main`. `docs/adr/ADR-0008-vrs-format-versioning.md` drafted, status **Proposed**, awaiting Ahmed's acceptance. In flight next: S1-B/C/D, S3-B/C, S5-B/E, QW4/QW5. A separate UI system initiative also started: 5 audits landed in `docs/audits/ui-2026-09-24/`. Details: `GATE_LOG.md`.
- **Cloud run, 2026-09-24 evening — DFS wave 1 nearly complete on the branch (head `6999970`, 111 commits over `main`):** merged after the afternoon: QW5 `66dafe2`, QW4 `4af375f`, S1-B `ce87bbf`, S3-B `130bc85`, S5-E `ea3be27`, S3-C `d8908e6`, S2-E1 `30e7384`, S3-D `40b93af`, **S1-C+S1-D `e5b9315` (real tabs, one dispatch — S1 complete pending hand test)**, S5-B `6999970` (format v2 — ADR-0008 still Proposed, stays off `main` until accepted). Gates on the head: **624/624 passed, 0 failed, 4 ignored by design (old-reader harness ×2 pre-existing, timing, corpus check)** (Linux), clippy Linux/Windows/Mac clean, fmt clean. Independent code reviews caught and fixed before merge: macOS fsync fallback (S3-A), hidden mask-ring leak + 13× PDF size (S6-A), straight-edge hit error 22 → 0.07 units (QW1), Pen-resume dirty bug (S1-A), **scan deleting user files through symlinks (S3-C)**, serde text in user messages (S5-B). Three merges carry "INDEPENDENT REVIEW PENDING" (S2-E1, S3-D, S1-C+D) because the owner asked for a budget hold; review them before `main`. UI system initiative: 5 audits + spec v1 + 5 reviews (all REQUEST CHANGES → spec v2 needed; no code started). Details: `GATE_LOG.md`.

### 2026-09-24 evening — Mac verification of the cloud wave

- **Owner acceptances:** ADR-0008 is Accepted. The UI system is one priority after DFS S1 and after spec v2; bundle IBM Plex Sans/Mono + Plex Sans Arabic, with a bidi spike for multi-word Arabic names; remove drag ease, glide and azure glow but keep the direction bar, with ADR-0009 to be drafted for the glide; use the mockup's “on” looks (tool = azure block, toggle = small azure bar, tab = surface fill); rename FAINT → MUTED with QW6 sizes; defer layout persistence and put Reset Workspace only in the Window menu; open an old file with a broken clip mask with the mask released plus a notice; when the document is itself a `.pdf`, export as `<name> export.pdf`.
- **Integration branch:** `integrate/cloud-wave-1`, not `main` yet. Three landings passed the Mac moderator gates: clippy compatibility fix `2415902` (**629/629**); S3-D scheduler review fixes merged as `f59da81` from `cce22e6`, with §3.6 updated in `542790a` (**633/633**); S2-E1 Start-model review fixes merged as `e2298f9` from `26030d1` + `c3a099c` (**634/634**). Clippy is clean on Mac and Windows targets; fmt is clean. Details: `GATE_LOG.md`.
- **In flight:** `fix/s1cd-tabs-review` — Codex review of `e5b9315` REQUEST CHANGES: typed field values crossed tabs; command dispatch was not FIFO (⌘S then ⌘Z saved the undone state); overflow tab dragging reordered incorrectly; docs overclaimed. Opus is fixing it; the first P1 landed in `1115e76`.
- **Owner process rules restated:** Opus implements; Fable moderates; Codex (Sol) handles reviews, docs and ordinary work; Astra is reserved for very hard problems or plan consultation. **Close what is broken or missing before starting anything new.**

### 2026-09-26 — the cloud wave is on `main`

- **Merged to `main` in three waves** (each gated on macOS before the next): wave 1 Astra batch 1 `c85dc9c` (351 tests); wave 2 DFS wave 1 from the cloud run + Mac clippy fix `2415902` → `d1d8ee3`'s parent (629 tests); wave 3 Mac verification fixes + the 2026-09-25 owner round `d1d8ee3` (685 tests; tree identical to `integrate/cloud-wave-1`).
- **Owner verified by hand on the installed build:** live tab drag (P16 — the tab follows the pointer, others reflow instantly; root cause egui's focus flag stayed false on macOS bundle launches, now seeded from winit — also restores text carets and typed buffers).
- **Astra batch review (first Codex review of that batch):** REQUEST CHANGES → fixed on `main` `19eed19` (P17 hole anchors in the Direct inspector, P18 Cut deleting hidden/locked art, P19 Duplicate Artboard demoting clip groups, P20 Pen/Convert/Alt+Direct crash on hole anchors; one central "nothing hidden or locked is ever edited" invariant). Implemented by Codex Sol, reviewed by Opus (REQUEST CHANGES → APPROVE WITH NITS). Follow-ups in `PAINS_LOG.md` P21 (locked clip mask on copy, hole with 1–2 anchors, group_sel routes, board move vs hidden mask).
- **Gates on `main` `19eed19`:** **708 passed / 0 failed / 4 ignored**, clippy Mac + Windows target clean, fmt clean. `/Applications/Varos.app` rebuilt from `main`.
- **Open after this:** P21 decisions; the name field drops typed text on click-away (commit only on Enter); the old local worktree `codex-astra-1` holds uncommitted duplicate Astra fixes (superseded; kept until the owner says delete). **Next system:** UI System spec v2 (tab strip, Home button instead of the burger menu, icons instead of text labels), then the icon library.

### 2026-09-25 — owner look at the integration build + fixes (branch `integrate/cloud-wave-1`, not `main` yet)

- **Owner verified by hand:** independent tabs, tab numbering, dirty marking on content edits only, field edits never cross tabs, ⌘S/⌘Z order, ⌘S while typing, Pathfinder buttons, tab drag after P15, cursor arrows.
- **Process change (owner):** Ahmed no longer runs long checklists. Codex computer-use runs them (launched from the Codex desktop app); Ahmed spot-checks what needs a designer's eye. Codex run on this build: 8 pass, 6 partial (tool limits), 4 reported fails — 3 were false alarms (native NSAlert dialogs are invisible to its screenshots; X toggles fill/stroke focus, ⇧X swaps, per Illustrator), 1 real (tab drag, P15).
- **Landed on the branch, each with gates + Codex Sol review:** P14 Ctrl+Tab cycles every tab + Space survives a tab switch (`358e482`, REQUEST CHANGES → APPROVE WITH NITS); cursor system v1.1 — new arrow family approved by Ahmed, hover badges kept, a tool badge on every crosshair (Rect/Ellipse/Triangle/Polygon/Rotate/Scale), drag-locked bbox cursors, Artboard cursor (`6f2292d`, `fbe2ae4`); P15 tab drag moved the whole Mac window — AppKit native drag region disabled, one caption hit-test for Mac + Windows (`6f606ad`, APPROVE WITH NITS); Pathfinder click-path regression tests (`6cc6e8c`, owner's "Shape Builder broke" report not reproducible, later confirmed working by him). Gates on the merged branch: **668 passed / 0 failed / 4 ignored**, clippy Mac + Windows clean, fmt clean. `/Applications/Varos.app` rebuilt from the branch.
- **Owner UI decisions routed to their systems (no patches):** remove the burger menu beside the tabs (native Mac menu bar only) and put a Home button there → DFS S2-E2 Start page + UI System spec v2; icons instead of text labels on buttons → icon library moves up right after stabilisation; tab-drag visuals are ugly (P16) → tab-strip component in UI System spec v2 (instant, no animation, azure drop marker).
- **Next:** chunked merge of the branch to `main` (Astra batch + quick wins → S1 + S3 → S5 + S6-A), then UI System spec v2.

**For Ahmed to try (batch 3)** — on the Mac, from branch `claude/sweet-cerf-1sg30t` (`git fetch origin claude/sweet-cerf-1sg30t && git checkout claude/sweet-cerf-1sg30t`, then `tools/mac/bundle.sh` or `cargo run --release -p varos-app`). Batch 2 (in this file, above) still applies; batch 3 adds:

1. (S1) ⌘N twice → two independent tabs `Untitled-1`, `Untitled-2` (numbers never reused after closing); draw red in A, blue in B; switch tabs by click — each keeps its own drawing, view, selection and undo.
2. (S1) Edit A, ⌘Z back to the saved state → the dirty dot and the title `*` disappear; pan/zoom/fit/switch artboards never mark a tab dirty.
3. (S1) ⌘W on a dirty tab → Save / Don't Save / Cancel; Cancel keeps it; closing the last tab leaves a fresh Untitled. ⌘Q with two dirty tabs → asked "Document 1 of 2" then "2 of 2"; Cancel on the second keeps both open and the first's save stays.
4. (S1) ⌘O a file already open → its tab is focused, not reopened; Save As onto another open tab's file → refused with a notice.
5. (S1) Drag a tab to reorder; middle-click closes; the `+` chip is always visible; with many tabs the active one is always visible.
6. (S1) File menu rows New/Open/Close Tab/Save/Save As/Quit work with and without a text field focused; ⌘S while typing in a field saves; Ctrl+Tab / Ctrl+⇧Tab switch tabs; the red traffic light runs the same multi-tab Quit.
7. (S1) Open the colour picker, click on empty canvas, press Cancel → the colour reverts (batch-1 bug). Burger menu: New/Open/Save/Save As act; Export and Share are greyed with a reason; the search pill no longer shows "⌘ K".
8. (QW5) ⌘A selects all visible unlocked objects (Direct tool: all anchors; Artboard tool: all boards); ⇧⌘A deselects; Edit ▸ Delete deletes the selection but Backspace in a text field still deletes text.
9. (QW4) An open path with round caps, width 80, at 100 % and 4000 %: the cap is a true half-disc, no 24-gon facets; a closed curvy path shows no seam; a two-anchor dot path draws a full disc.
10. (S5) Open an old `.vrs` (batch-1 file) → it opens with "Opened an older file. Saving will update its format."; Save → the file now says format 2; the old build (`main`) refuses it with "needs a newer Varos". Run hand test 0: `VAROS_CORPUS_DIR=~/Documents cargo test -p varos-pdf --test corpus_check -- --ignored --nocapture` and check every one of your files reads OK.
11. (S3-A) Save to a USB stick or an SMB share → succeeds (no "Couldn't save"); save to the internal disk → no "not confirmed" notice.

**Decisions waiting for Ahmed (each with the recommended default):** P12 Pen anchor-delete — split the behaviours to match Illustrator (Pen deletion reconnects neighbours; Direct Selection + Delete opens the path); the seven dependency bumps from the cargo-audit triage — apply the bounded, pre-checked patch set; icon stroke weight — 1.5 px after the S1 side-by-side; copy cursor — keep the arrow + “+” OS convention.
- **Astra hand test DONE (2026-09-24)** — `docs/audits/2026-09-24-ASTRA_USER_TEST.md`: 1 P0 (unsaved quit), 3 P1 (new tab, Export, copy/paste), 6 P2, 1 P3; batch-1 results in its §A. The Document & File System proposal `docs/specs/DOCUMENT_FILE_SYSTEM.md` absorbs F01/F02/F03/F09 and waits for Ahmed's D1–D3.
- **Astra batch 1 on branch `claude/sweet-cerf-1sg30t` (2026-09-24, cloud session, NOT merged to `main`)** head `0e369a6` — first run done entirely in the cloud: Fable moderator + four Opus subagents, each in its own worktree from `bd24627`, every diff moderator-reviewed and merged `--no-ff`. Pieces: (1) **Quit guard, F01/P0** `953bca2` — Save / Don't Save / Cancel on ⌘Q, the red traffic light and the caption ✕; (2) **⌘± zoom the canvas, ⌘1 keeps the place, F05+F06** `389e542` — egui's own keyboard UI-zoom switched off, ×1.5 key step, View ▸ Zoom In/Out rows; (3) **Direct-selection inspector, F07** `9d0665e` — "Anchor" / "N anchors" / path name with real X/Y/W/H, editable through `SetObjectBounds`; (4) **Duplicate Artboard copies its art, F09** + board click no longer marks the file dirty + stroke weight carries to the next Pen path `abfbaa4`; (5) **In-app clipboard, F04/P1** `0e369a6` — ⌘C/⌘X/⌘V/⇧⌘V + Edit menu rows, structure-preserving, one undo step each; OS clipboard is a later piece. Gates on the cloud (Linux): workspace **346/346**, clippy Linux + Windows target + Mac target (`aarch64-apple-darwin`, type-check only) clean, fmt clean. **Independent Codex review NOT run** (unavailable in the cloud) and **no hand test yet** — both required before merging to `main`. Details: `GATE_LOG.md`.
- **Next queued work:** finish and re-review `fix/s1cd-tabs-review`, then close the remaining broken or missing DFS work before starting anything new. ADR-0008 is Accepted and no longer blocks the S5 format-v2 work. Astra F11, UI-system implementation and icon-library S1 wait behind that close-first rule.
- **Follow-up measurement:** harness scene F (many same-colour translucent strokes — structural cost of the P1-1 fix, not yet measured).
- **Astra hand test:** was BLOCKED by the macOS app-control permission on 2026-09-24 morning; unblocked and run the same day (see above). Batch 1 of the "for Ahmed to try" list remains Ahmed's own hand test.

**For Ahmed to try (batch 1)** — on the Mac, from the release build `varos/target/release/varos`:

1. Open the app from the release binary.
2. Draw with the Pen and with the shape tools.
3. Hold Space and move across a panel splitter — the cursor must stay a hand.
4. Zoom to 4000%.
5. Resize the window to full screen.
6. Open and save a `.vrs` file.
7. (from A26/A32) Boolean shapes keep sharp corners; deleting an anchor opens the path.
8. (from P11.2) On a complex file, zoom to 4000% and pan and zoom around — it should stay smooth, with nothing missing at the window edges.
9. (from P11.2) Overlapping translucent strokes (e.g. two 50% red lines crossing) look the same as before — the crossing is darker.
10. (from P11.2) Masked objects look right while you pan them partly off screen and back.
11. hover every tool: the Varos v1 cursors (pen nib, arrows, hand…) should be sharp on Retina.
12. (from the stroke-join fix) Draw a closed curvy path, stroke width 80, zoom to ~327% and 4000% — the band must be solid, no spokes.
13. (from macOS chrome) One bar + traffic lights; drag the empty bar to move the window; ⌘R and View▸Rulers toggle exactly once; File▸Save; Quit ⌘Q.
14. thick stroke on a huge circle far from the artboard at 100–400% zoom: no radial hairlines.
15. zoom with wheel/⌘± is instant, no glide.
16. top bar: traffic lights centred, labels show ⌘.

**For Ahmed to try (batch 2)** — on the Mac, from branch `claude/sweet-cerf-1sg30t` (`git fetch origin claude/sweet-cerf-1sg30t && git checkout claude/sweet-cerf-1sg30t`, then `tools/mac/bundle.sh` or `cargo run --release -p varos-app`):

1. (F01) Save a file, draw one more shape, press ⌘Q → a dialog asks Save / Don't Save / Cancel. Cancel keeps the app open with the shape; Save then reopen keeps the shape; Don't Save loses only that shape. Same via the red traffic light. A clean file quits at once, no dialog.
2. (F01) Untitled dirty document → ⌘Q → Save → the Save As dialog; cancel it → the app stays open.
3. (F05) At any zoom press ⌘= / ⌘− a few times: only the artwork zooms (×1.5 per press), panels and text never change size; View ▸ Zoom In / Zoom Out do the same. ⌘⇧= (⌘+) also zooms in.
4. (F06) Zoom into a shape far from the artboard, then ⌘1: 100 % and the same spot stays in the middle of the canvas.
5. (F07) Deselect, press A, grab an anchor of a shape and move it: the floating bar says "Anchor" with its X/Y and W/H = 0 (greyed with a reason); marquee two anchors → "2 anchors" with real W/H; scrub X → the anchors move, ⌘Z undoes once. Click a path with A → its name and its bounds.
6. (F09) A populated artboard, "Move artwork" on → Duplicate → the copy carries the artwork (groups, masks, rotated objects), one ⌘Z removes board + copies. With "Move artwork" off → an empty copy.
7. Click another artboard (Artboard tool) without dragging → the title shows no `*`; drag it → `*` appears and one ⌘Z undoes.
8. Set stroke weight 80 on a selected path, then draw a new Pen path → it is 80. Mid-Pen-path, change the weight → the path being drawn changes.
9. (F04) Select a group, ⌘C, click the second artboard, ⌘V → a copy centred in the view, selected, on the active layer; ⌘V again → another independent copy; ⇧⌘V → in place; ⌘X then ⌘Z → one undo brings it back. Edit menu shows Cut/Copy/Paste/Paste in Place. In a text field (e.g. a name), ⌘C/⌘V still copy/paste text.
10. Plain V still = Selection tool; plain X still swaps fill/stroke.

**Known Mac gaps (expected, not bugs of this piece):** screen eyedropper disabled; window position/size not remembered; Quit/Close prompt for unsaved changes only on branch `claude/sweet-cerf-1sg30t` (still not on `main`); menu items without an existing shortcut are omitted (New, Export, Duplicate, Select All, Deselect, Delete — Cut/Copy/Paste and Zoom In/Out exist on the branch).

**Why `docs/studies/`:** the charter never lists allowed folders. It asks that every level-5 doc carry a stamp (§0, §3.5). F2b only decided that the `docs/` root holds current docs, and a subfolder keeps that true. `docs/audits/` is already a topic folder beside `history/` and `reference/`. So a new `docs/studies/` folder for dated level-5 proposals is allowed and moves nothing. Each study is stamped `reference` (an allowed value) instead of the non-charter `draft`. Once Ahmed decides on a study, the decision goes into an ADR or a work order, and the study stays as reference.
**INVENTORY.md left unchanged:** it is the frozen F1 baseline register (160 files at `1aff281`, and post-baseline docs such as ADRs and `P11_1_PERF.md` are deliberately not listed), so the studies are not added there. **Dashboard note:** the "First-party docs stamped" and "Link check" rows below were not re-measured for the three studies. All three carry a stamp, and none contains a relative Markdown link. `tools/check_links.ps1` cannot run on this Mac (no `pwsh`).

### Earlier

- **Program:** Architecture & Repository Foundation — charter ACCEPTED 2026-07-11 (`docs/foundation/FOUNDATION_CHARTER.md`).
- **F2a COMPLETE:** F2a.1 ✅ `673b5df` · F2a.2 ✅ `d0664d4` · F2a.3 ✅ `a10772e` (doc-truth P0s closed) · F2a.4 ✅ `76640fe` (vendor contract + machine check).
- **F2 COMPLETE 🎉** — F2a.1..4 ✅ · F2b ✅ `b6e3863` (docs root = 7 current docs; history/ 17 + reference/ 16) · F2c ✅ `3a0ad3b` (ADR-0005 edges machine-enforced).
- **F3 ✅** `b8c9ba6` · **F4.1 ✅** `bd8bc1f` — `EditCommand` lives in core; **zero direct document writes from the UI** (measured); hand-verified by the product owner on a branch release build.
- **P11.1 ✅** `1931c80` — 10.4× on the 300%-zoom scene, 9.6× on curves-100 (reviewer-reproduced), owner hand-verified. Tests: 239.
- **Active work order:** **P11.2** — items (1)+(2) ✅ `b15d2bf` (2026-09-23); item (0) **DONE** ✅ `36d04d4` (2026-09-24); batch hand test pending. Original order: (0) instant real-time zoom (glide easing removed — owner-authorized feel change, 2026-07-12); (1) viewport culling at path level (the scene-B/many-objects fix) **plus ring/edge-level clipping to the view rect** (the 4000%-zoom overdraw fix — owner-reported symptom (د); reuse the existing artboard clippers `scene.rs:617-736`); (2) cross-frame flatten cache with zoom buckets. F4.2 queued right after.
- **P11 remaining after P11.2:** hover/snap interaction costs (P11.3, only if still felt), undo-clone storage (measure first).
- **Upcoming decision for the product owner:** fate of `codex/p6-header` — must be decided before F5 (charter precondition).
- **F2b layout DECIDED 2026-07-11:** product owner delegated the choice ("اختار انت الصح"); planner selected **`docs/history/` + `docs/reference/`**, root `docs/` keeps current docs only; `design-reference/` stays in place.
- **ADRs:** ADR-0001..0007 **Accepted** by the product owner 2026-07-11 — highest authority per charter §3. F2a.3's Constitution/CLAUDE.md corrections and F2c are unblocked.
- **Last gate:** F4.1 — PASS (zero implementer defects; one planner process defect recorded honestly in GATE_LOG §F4.1) — merged `bd8bc1f`.
- **Risk register:** `docs/audits/2026-07-11-CODEX-FULL-PROJECT-AUDIT.md` + `docs/audits/2026-07-11-CLAUDE-COUNTER-REVIEW.md`.

## Trigger flags (flipped only by Ahmed, with a date — charter §8)

| Flag | Value | Since |
|---|---|---|
| `flag.design-work-started` | false | 2026-07-11 |
| `flag.external-testers` | false | 2026-07-11 |
| `flag.release-milestone` | false | 2026-07-11 |
| `flag.dogfooding` | false | 2026-07-11 |

## Health dashboard (baseline `05b6dc7`)

| Metric | Value |
|---|---:|
| `ui.rs` lines | 5,826 (re-measured `wc -l` 2026-09-23, after `9f8ec1d`) |
| `editor.rs` lines | 4,537 (re-measured `wc -l` 2026-09-23, after `b15d2bf`) |
| Workspace tests | **708 passed / 0 failed / 4 ignored** — macOS 2026-09-26, `main` at `19eed19` · earlier: **634/634** — whole workspace, run on macOS 2026-09-24 on integration branch `integrate/cloud-wave-1` at `e2298f9` (0 failed); not on `main` yet · merged `main` at `36d04d4`: 306/306 on macOS · cloud branch head `6999970`: 624/624 on Linux (4 ignored by design) |
| Tests on macOS | **634 / 634** — whole workspace incl. `varos-app` (2026-09-24, integration branch `integrate/cloud-wave-1` at `e2298f9`; not on `main` yet) |
| `unsafe` sites (app crates) | 30 — re-measured 2026-09-24 via `grep -rc "unsafe" varos/crates/varos-app/src \| awk -F: '{s+=$2} END {print s}'` (Linux checkout of `claude/sweet-cerf-1sg30t`; was 27) |
| Direct external deps | 23 |
| `cargo audit` | 4 vulns + 6 warnings (triaged 2026-09-23, `docs/audits/2026-09-23-CARGO_AUDIT_TRIAGE.md`; fixes await owner approval) |
| `.varos` refs in current docs | 0 |
| First-party docs stamped | 71 / 71 — current 28 · historical 18 · reference 25 |
| Link check | PASS — 70 docs, 71 relative links, 58 heading anchors |

## External action items (outside the repo)

- GitHub account billing verification — **deferred indefinitely by Ahmed, 2026-07-11.** Hosted CI stays unavailable (runs die with zero steps); the red ✗ on pushes is cosmetic. The program's real gates run locally (charter §4). Triggers stay enabled so CI self-activates if the hold ever clears. Repo visibility unchanged (going private would not lift the account-level hold).
- Branch protection + required checks — parked behind the item above.
- GitHub organization + second admin — parallel governance track (charter §9.5), no deadline. Owner: Ahmed.
