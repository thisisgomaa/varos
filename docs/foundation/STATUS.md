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

**Why `docs/studies/`:** the charter never lists allowed folders. It asks that every level-5 doc carry a stamp (§0, §3.5). F2b only decided that the `docs/` root holds current docs, and a subfolder keeps that true. `docs/audits/` is already a topic folder beside `history/` and `reference/`. So a new `docs/studies/` folder for dated level-5 proposals is allowed and moves nothing. Each study is stamped `reference` (an allowed value) instead of the non-charter `draft`. Once Ahmed decides on a study, the decision goes into an ADR or a work order, and the study stays as reference.
**INVENTORY.md left unchanged:** it is the frozen F1 baseline register (160 files at `1aff281`, and post-baseline docs such as ADRs and `P11_1_PERF.md` are deliberately not listed), so the studies are not added there. **Dashboard note:** the "First-party docs stamped" and "Link check" rows below were not re-measured for the three studies. All three carry a stamp, and none contains a relative Markdown link. `tools/check_links.ps1` cannot run on this Mac (no `pwsh`).

### Earlier

- **Program:** Architecture & Repository Foundation — charter ACCEPTED 2026-07-11 (`docs/foundation/FOUNDATION_CHARTER.md`).
- **F2a COMPLETE:** F2a.1 ✅ `673b5df` · F2a.2 ✅ `d0664d4` · F2a.3 ✅ `a10772e` (doc-truth P0s closed) · F2a.4 ✅ `76640fe` (vendor contract + machine check).
- **F2 COMPLETE 🎉** — F2a.1..4 ✅ · F2b ✅ `b6e3863` (docs root = 7 current docs; history/ 17 + reference/ 16) · F2c ✅ `3a0ad3b` (ADR-0005 edges machine-enforced).
- **F3 ✅** `b8c9ba6` · **F4.1 ✅** `bd8bc1f` — `EditCommand` lives in core; **zero direct document writes from the UI** (measured); hand-verified by the product owner on a branch release build.
- **P11.1 ✅** `1931c80` — 10.4× on the 300%-zoom scene, 9.6× on curves-100 (reviewer-reproduced), owner hand-verified. Tests: 239.
- **Active work order:** **P11.2** — (0) instant real-time zoom (glide easing removed — owner-authorized feel change, 2026-07-12); (1) viewport culling at path level (the scene-B/many-objects fix) **plus ring/edge-level clipping to the view rect** (the 4000%-zoom overdraw fix — owner-reported symptom (د); reuse the existing artboard clippers `scene.rs:617-736`); (2) cross-frame flatten cache with zoom buckets. F4.2 queued right after.
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
| `ui.rs` lines | 5,563 |
| `editor.rs` lines | 4,533 |
| Workspace tests | 232 (223 baseline + 6 F3 pins + 3 F4.1 boundary tests) |
| Tests on macOS (core+pdf+render) | 220 / 220 (2026-09-23) |
| `unsafe` sites (app crates) | 27 |
| Direct external deps | 23 |
| `cargo audit` | 3 vulns + 3 unmaintained (untriaged) |
| `.varos` refs in current docs | 0 |
| First-party docs stamped | 71 / 71 — current 28 · historical 18 · reference 25 |
| Link check | PASS — 70 docs, 71 relative links, 58 heading anchors |

## External action items (outside the repo)

- GitHub account billing verification — **deferred indefinitely by Ahmed, 2026-07-11.** Hosted CI stays unavailable (runs die with zero steps); the red ✗ on pushes is cosmetic. The program's real gates run locally (charter §4). Triggers stay enabled so CI self-activates if the hold ever clears. Repo visibility unchanged (going private would not lift the account-level hold).
- Branch protection + required checks — parked behind the item above.
- GitHub organization + second admin — parallel governance track (charter §9.5), no deadline. Owner: Ahmed.
