> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos STATUS — single current-state page

Authority level 3 (charter §3). Anything that contradicts this page is stale. Updated at every gate.

## Now

- **Program:** Architecture & Repository Foundation — charter ACCEPTED 2026-07-11 (`docs/foundation/FOUNDATION_CHARTER.md`).
- **F2a COMPLETE:** F2a.1 ✅ `673b5df` · F2a.2 ✅ `d0664d4` · F2a.3 ✅ `a10772e` (doc-truth P0s closed) · F2a.4 ✅ `76640fe` (vendor contract + machine check).
- **Active work order:** F2b (physical doc layout) — order issued; layout per the decision below. F2c ✅ `3a0ad3b` (ADR-0005 edges machine-enforced locally + in CI).
- **F2b layout DECIDED 2026-07-11:** product owner delegated the choice ("اختار انت الصح"); planner selected **`docs/history/` + `docs/reference/`**, root `docs/` keeps current docs only; `design-reference/` stays in place.
- **ADRs:** ADR-0001..0007 **Accepted** by the product owner 2026-07-11 — highest authority per charter §3. F2a.3's Constitution/CLAUDE.md corrections and F2c are unblocked.
- **Last gate:** F2c — PASS (zero defects) — merged `3a0ad3b` (GATE_LOG §F2c).
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
| Workspace tests | 223 |
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
