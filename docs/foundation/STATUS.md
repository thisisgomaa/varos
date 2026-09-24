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
- **Next piece: wire cursor set v1 into `varos-app/src/cursors.rs`** — replace the local Illustrator reference set on Mac and Windows with `assets/cursors/v1/` (study §6: `varos_svg(ck)` table from `hotspots.json`); add `CK::ZoomIn` / `ZoomOut` / `Artboard` only when those tools exist; make the cursors Retina-sharp on macOS (2× bitmaps).
- **In progress: icon-library study** (`docs/studies/2026-09-23-ICON_LIBRARY_STUDY.md`, being written by another agent).
- **Next small piece: P11.2 item (0), instant zoom** — remove the A13 glide easing in `varos-app/src/main.rs` (owner-authorized, 2026-07-12). Now unblocked: the Mac port has landed, so `main.rs` builds and runs here. Follow-up measurement: harness scene F (many same-colour translucent strokes — structural cost of the P1-1 fix, not yet measured).

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
11. (from Mac cursors) Hover each tool over the canvas and check the cursor changes: the pen nib (and its + / − states over a segment / an anchor), the selection arrows, the hand while Space is held. You can also open the installed `/Applications/Varos.app`.
    - Expect the cursors to look a little soft on the Retina screen (32-pixel bitmaps shown at 32 points) — known, not a bug of this piece.
    - On your Mac the local Illustrator reference set is present, so all 28 states are custom. Without that folder (a fresh clone), 17 states (resize ×4, move, hand, grab, copy, no-drop, rotate ×8) use the standard Mac cursors — by design until our own set lands.
    - The original Varos cursor set v1 has landed as design files (`d30365c`) but is **not wired yet**. After the wiring piece, all these cursors will change to the Varos set — re-check this item then.
12. (from the stroke-join fix) Draw a closed curvy path, stroke width 80, zoom to ~327% and 4000% — the band must be solid, no spokes.
13. (from macOS chrome) One bar + traffic lights; drag the empty bar to move the window; ⌘R and View▸Rulers toggle exactly once; File▸Save; Quit ⌘Q.

**Known Mac gaps (expected, not bugs of this piece):** screen eyedropper disabled; window position/size not remembered; traffic lights sit ≈9pt above the bar's control centre; Quit/Close do not prompt for unsaved changes (same as ✕ today); menu items without an existing shortcut are omitted (New, Export, Cut/Copy/Paste, Duplicate, Select All, Deselect, Zoom In/Out, Delete); “Ctrl” labels should read ⌘ on Mac — next small piece.

**Why `docs/studies/`:** the charter never lists allowed folders. It asks that every level-5 doc carry a stamp (§0, §3.5). F2b only decided that the `docs/` root holds current docs, and a subfolder keeps that true. `docs/audits/` is already a topic folder beside `history/` and `reference/`. So a new `docs/studies/` folder for dated level-5 proposals is allowed and moves nothing. Each study is stamped `reference` (an allowed value) instead of the non-charter `draft`. Once Ahmed decides on a study, the decision goes into an ADR or a work order, and the study stays as reference.
**INVENTORY.md left unchanged:** it is the frozen F1 baseline register (160 files at `1aff281`, and post-baseline docs such as ADRs and `P11_1_PERF.md` are deliberately not listed), so the studies are not added there. **Dashboard note:** the "First-party docs stamped" and "Link check" rows below were not re-measured for the three studies. All three carry a stamp, and none contains a relative Markdown link. `tools/check_links.ps1` cannot run on this Mac (no `pwsh`).

### Earlier

- **Program:** Architecture & Repository Foundation — charter ACCEPTED 2026-07-11 (`docs/foundation/FOUNDATION_CHARTER.md`).
- **F2a COMPLETE:** F2a.1 ✅ `673b5df` · F2a.2 ✅ `d0664d4` · F2a.3 ✅ `a10772e` (doc-truth P0s closed) · F2a.4 ✅ `76640fe` (vendor contract + machine check).
- **F2 COMPLETE 🎉** — F2a.1..4 ✅ · F2b ✅ `b6e3863` (docs root = 7 current docs; history/ 17 + reference/ 16) · F2c ✅ `3a0ad3b` (ADR-0005 edges machine-enforced).
- **F3 ✅** `b8c9ba6` · **F4.1 ✅** `bd8bc1f` — `EditCommand` lives in core; **zero direct document writes from the UI** (measured); hand-verified by the product owner on a branch release build.
- **P11.1 ✅** `1931c80` — 10.4× on the 300%-zoom scene, 9.6× on curves-100 (reviewer-reproduced), owner hand-verified. Tests: 239.
- **Active work order:** **P11.2** — items (1)+(2) ✅ `b15d2bf` (2026-09-23); item (0) is next. Original order: (0) instant real-time zoom (glide easing removed — owner-authorized feel change, 2026-07-12); (1) viewport culling at path level (the scene-B/many-objects fix) **plus ring/edge-level clipping to the view rect** (the 4000%-zoom overdraw fix — owner-reported symptom (د); reuse the existing artboard clippers `scene.rs:617-736`); (2) cross-frame flatten cache with zoom buckets. F4.2 queued right after.
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
| Workspace tests | 289 — whole workspace, run on macOS 2026-09-24 on merged `main` at `e212cf0` (0 failed) |
| Tests on macOS | 289 / 289 — whole workspace incl. `varos-app` (2026-09-24, merged `main` at `e212cf0`) |
| `unsafe` sites (app crates) | 27 |
| Direct external deps | 23 |
| `cargo audit` | 4 vulns + 6 warnings (triaged 2026-09-23, `docs/audits/2026-09-23-CARGO_AUDIT_TRIAGE.md`; fixes await owner approval) |
| `.varos` refs in current docs | 0 |
| First-party docs stamped | 71 / 71 — current 28 · historical 18 · reference 25 |
| Link check | PASS — 70 docs, 71 relative links, 58 heading anchors |

## External action items (outside the repo)

- GitHub account billing verification — **deferred indefinitely by Ahmed, 2026-07-11.** Hosted CI stays unavailable (runs die with zero steps); the red ✗ on pushes is cosmetic. The program's real gates run locally (charter §4). Triggers stay enabled so CI self-activates if the hold ever clears. Repo visibility unchanged (going private would not lift the account-level hold).
- Branch protection + required checks — parked behind the item above.
- GitHub organization + second admin — parallel governance track (charter §9.5), no deadline. Owner: Ahmed.
