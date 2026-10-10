> **Status:** current — single current-state page, governed by [FOUNDATION_CHARTER](FOUNDATION_CHARTER.md) §3.
# Varos — cycle 2 closed 2026-10-07

Updated 2026-10-09: **colour picker v3 complete** (`214e998`, 1465 tests): modeless live Wheel + Sliders + Harmony + drawer tabs + Mini on the page-colour chip; owner tested slices 1–3 («ممتازة»), slice 4 pending. **2026-10-10 03:24: wave 3 merged and installed (`f1dbf21`; 2493 tests; format v14)** — the programme plan's Phases 0–14 are implemented except the items listed under GATE_LOG 2026-10-10 "Known open items"; every UI added since 2026-10-09 is PROVISIONAL and awaits the owner's design review. Earlier: **2026-10-09 night: wave 2 merged and installed (`0dee21e` stage 1, `8d21c1f` stage 2; 2245 tests; format v9 = images v6 · gradients v7 · text v8 · corners/preview v9)** — Phases 0–5 plus 7, 8, 9.1–9.6, 2.3, 4B/4C/4F, 3.8 and the text product (Type tool, kashida) are in the installed app; wave 3 running (appearance+masks v10, live effects v11, CMYK v12, renderer layers/blur/blend, live nodes v13, Arabic UI, a11y+updates, text P5–P8 v14). **All new UI is provisional pending the owner's design review.** Earlier: night-shift groups 1+2 merged and installed (`6d03c90`, `5a09800`; 1881 tests)** — Phase 0 complete; Phase 1 except 1.4/1.8; Phase 2 stroke engine v5; Phase 4 tools (4A/4D/4E, 4B/4C core); 3.8 engine; 7.1 SVG import; text composer + kashida. All new UI provisional pending the owner's design review. **Programme plan approved 2026-10-09** (`docs/PLAN.md`: 15 phases, format bump schedule v5…v14, lanes, ADR order); execution starts with Phase 0 (render fixes, crash safety, export wiring, file-menu basics). Gap inventories: `docs/reference/gap/`. Prior-art corpus (VectorCraft/PhotoCraft) kept locally; borrowing rules in `docs/reference/EXTERNAL_PRIOR_ART.md`, gap survey in `EXTERNAL_PRIOR_ART_SURVEY.md`; appearance/masks study in progress under `docs/reference/study/`.

Updated 2026-10-08 (close). **Landed on `main` after `76b57ed` (GATE_LOG 2026-10-08 close): workspace layout remembered between launches, colour picker live preview fixed, control bar hidden when idle; 1402 tests.** Earlier at `4e3f771` (GATE_LOG 2026-10-08 night): on-canvas agent presence (orange), Bridge token economy slices 1–2 (API 1.1, −72% op bytes on the real sample), describe composition; 1364 tests.** Earlier the same day at `189ff0e` (GATE_LOG 2026-10-08): panel controls are icon buttons (direction B, toggle ON = black well, no azure bar) and the Bridge needs no Keychain and no pairing (file key store, open local trust, ADR-0011 Amendments 2–3); 1321 tests.** Earlier: cycle 3 first pair at `83819a1` (GATE_LOG "Cycle 3 first pair"): T1 `varos-text` leaf crate with vendored patched cosmic-text, and Bridge slice 4 (add_path, rounded rect, artboard reorder/duplicate, `files` scope default OFF); 1304 tests.** Cycle 2 closed on `main` at `31e45b6` (tag `cycle-2026-10-07`; see GATE_LOG "Cycle 2 close"): Bridge slices 1–3 + C1 connection system landed and owner-driven from Claude Code; text engine spike P1→P1c passed every headless gate; frame pacing fixed the heat; format v4 (artboard ids). Previous close (2026-10-06): baseline `07ff058` plus cleanup (tag `cycle-2026-10-06`).** The last three code landings are close-out 1 (`736feba`: band Search and Custom… removed, launch recovery scan awaited before frame 0 with a bounded 250 ms wait and polling if still pending), trackpad control (`2f2bcc0`: pinch, smart zoom, 1:1 pan) and recovery card / in-place Review (`1532637`). These three await the owner's eye. VIEW_ROTATION is planned/parked, not implemented.

Local `feat/bridge-token-economy` review fixes remain uncommitted: tools/list 23,119 bytes; 1,338 tests pass, native/Windows clippy clean; sockets blocked by sandbox EPERM. [Verification](../../varos/crates/varos-bridge/VERIFICATION.md#token-economy-independent-review-fixes--2026-10-08).

**Open after cycle close:** owner checks of recovery card + trackpad + frame-0 notice; QW6 panel icon size; remaining P4, P5/P7 and P6/F7/F8 measurements; P21 (4); Arabic gate §8; Windows S4-A; CI billing lock (last inspected run 2026-09-27; current remote state unverified). Smaller reviewer follow-ups and retained product/acceptance gaps are listed in [GATE_LOG — Cycle close](GATE_LOG.md#2026-10-06--cycle-close). Closing this cycle does not claim these items are finished.

Owner hand test in the real window, 2026-10-06: top bar 4b (`19611ae`) and polish (`7adee37`) — "كله شغال"; recovery after Force Quit works (before the new timing/card changes); Finder double-click on a `.vrs` works; File ▸ Export ▸ PDF works. Everything else below is tests-only unless it says otherwise. Baseline gates freshly run at `07ff058`: **1086 passed / 0 failed / 8 ignored**, Mac + Windows-target clippy and fmt clean. Cleanup gates are recorded below and in GATE_LOG.

## Product and platform

Varos is an early native vector editor in Rust. macOS is the primary and only official build for now. Windows receives compile checks only (owner decision, reconfirmed 2026-10-06); Windows runtime testing is deferred. Arabic typography is the long-term product focus; editable text is not implemented yet, and the Arabic UI gate ([UI_SYSTEM §8](../specs/UI_SYSTEM.md#8-arabic--rtl-gate-owner-piece)) has not passed.

Keep the four-crate architecture: pure `varos-core`, scene renderer `varos-render-wgpu`, PDF container `varos-pdf`, desktop `varos-app`. No rewrite or additional framework is needed for this work.

## What is actually available

| Area | Current state |
|---|---|
| Drawing | Shapes, Pen/Direct editing, boolean operations, layers, transforms, snapping and artboard clipping are implemented. P12 (Pen delete reconnects neighbours) and P21 (1–3) merged in `abf8ec1`; P21 (4) is open. Mask model/render/PDF foundations exist; creation/release gestures are pending. Edge cases remain in the pain log. |
| Documents | Independent tabs, dirty tracking, undo/redo, save/close/quit guards and `.vrs` save/open are integrated. A document is a Board with name, description and tags (`380bf05`). |
| File format | `.vrs` remains a PDF containing editable document data. `FORMAT_VERSION = 4` ([format/mod.rs](../../varos/crates/varos-core/src/format/mod.rs)) with v2→v3 and v3→v4 (artboard ids) migrations and frozen v1/v2/v3/v4 fixtures; ADR-0008 accepted and amended 2026-10-04 for format 3 and 2026-10-07 for format 4 (Bridge slice 3 work branch; both amendments' independent reviews pending). S5-C semantic validation, S5-D bounded PDF reading and S5-E refusal corpus merged in `2cb072a`. A real `.vrs` opened from Finder was owner-seen 2026-10-06; refusal messages and the v1 broken-mask notice are tests-only. |
| Start and recent files | E2 merged in `2cb072a`; its page was replaced by [Start v2 — Boards](work_orders/START_V2_BOARDS.md) (`6405dc9`: board cards with thumbnails, tag filters, list view, presets, Board section in Properties). Owner acceptance of the Start v2 page is not recorded (GATE_LOG 2026-10-05: not run in a real window). Custom… removed in `736feba`; New board opens a free canvas. Cycle cleanup removes the unreachable search model; tag filters and grid/list remain. |
| Recovery | [F1](work_orders/DFS_S3_F1_INTEGRATION.md) and [F2](work_orders/DFS_S3_F2_INTEGRATION.md) merged in `2cb072a` (`recovery_host.rs`). Owner-seen 2026-10-06: recovery after Force Quit works. Frame-0 scan guarantee landed in `736feba`; the old strip became the recovery card / Review in `1532637`. Timing and card acceptance await the owner. Cancel/failure paths are tests-only. |
| File association | S4 on macOS merged in `221310f` (`mac_open.rs` + `os_open.rs` → one `OpenPaths`). Owner-seen 2026-10-06: Finder double-click works. Dock drop, `open(1)` and cold-vs-warm launch were not reported separately. Windows parts of S4-A are not implemented. |
| Export | S6-B/C merged in `bd4e552` (`export_ui.rs`; export and ⌘S run on the background I/O worker). Owner-seen 2026-10-06: File ▸ Export ▸ PDF works. Export Cancel and Show in Finder are deferred; the 2.38 s large save was not re-measured off-thread. SVG/PNG interchange is pending. |
| UI system | [UI System v3](../specs/UI_SYSTEM.md) is on `main` as a document (`420b153`); its header still says "proposed". Merged pieces: P2 field law (`fa6bd10`), P3 split/registry/ratchets (`d882454`; `ui.rs` now 931 lines), top bar 4b with `mac_titlebar.rs` (`19611ae`), polish pass (`7adee37`, includes the grey-segment half of P4 and `T_MICRO = 10.5`), icon stage 1 (`4be3208`), Inter + JetBrains Mono fonts (`33aa767`). Owner-seen 2026-10-06: 4b + polish pass. Open: QW6 panel icon size 18 (not verified closed), rest of P4, P5–P8. Band Search removal landed in `736feba`. Icon stage 2 is parked. |

## Local worktree — SVG export (2026-10-06, uncommitted)

`varos-core::svg` now plans and writes standalone SVG 1.1 files for visible artboards or whole-board artwork bounds, without new production dependencies. Exact cubics, even-odd fills/clips, round strokes, names/ids, visibility and canvas opacity isolation/knockout have headless test coverage. [Work order](work_orders/SVG_EXPORT.md) records the API and validation. No app command/menu/FileJob wiring: the PDF-only sheet has no format choice, and multiple SVG destinations need design first. No GUI or owner hand test, installation, push or merge. This local implementation does not change the main baseline described above.

## Next steps

The single execution map is [PLAN](../PLAN.md). Start there for completed work, the queue and unresolved decisions.

1. Owner checks: recovery card / Review, frame-0 notice after Force Quit, and trackpad feel on real hardware.
2. Resume a separately gated stage from PLAN: QW6, remaining P4, P5/P7, measurements and Arabic §8. VIEW_ROTATION remains parked.
3. Keep the open product and platform items explicit; do not restart completed Search/Custom… or recovery-design work.

## Verification

| Check | Latest evidence |
|---|---|
| Workspace tests | **1087 passed, 0 failed, 8 ignored** — fresh cycle-close cleanup run, 2026-10-06 ([GATE_LOG](GATE_LOG.md)); baseline at `07ff058`: 1086/0/8. |
| Build, clippy and format | macOS and `x86_64-pc-windows-msvc` all-target clippy with `-D warnings`, and `cargo fmt --all --check`: PASS before and after cleanup, 2026-10-06. |
| Explicit old-reader checks | Run in the default suite; L2 added two v2-era gate tests for format 3 (GATE_LOG 2026-10-04 L2). Old binary not tested. |
| Architecture checker | Portable gate and 7 checker tests PASS, 2026-09-27; not re-run since. |
| Independent review | The landed code merges had independent reviews before landing; the cycle-close cleanup (Codex Sol 6.1, dead-code removal only) was gated and read by the moderator rather than a second model; rounds and verdicts per entry in GATE_LOG and in the merge commit messages. Exception: the UI System v3 document (`420b153`, docs only) — its independent review is still pending (GATE_LOG 2026-10-04 "UI System v3"). |
| CI configuration | Full macOS gates and Windows compile-only checks configured. [Run 36300014830](https://github.com/thisisgomaa/varos/actions/runs/36300014830) failed before either job started: account locked due to a billing issue; no test steps executed. No later run was inspected for this update (unverified whether the lock persists). |
| Source size | `varos-app/src/ui.rs`: 931 lines; `varos-core/src/editor.rs`: 5,174 lines, measured 2026-10-06 in the cycle-close tree. Ratchets measured 16/18/31/20; colour/type/radius ceilings unchanged; ui.rs ceiling tightened from 946 to 931. |
| GPU/window interaction | Owner hand test 2026-10-06 (top of this page). Not owner-seen and therefore tests-only: recovery card / frame-0 timing / trackpad, Start v2 page, K3 field law by hand, P12/P21 behaviour, Dock/`open(1)` opening, recovery cancel/failure paths, S5 refusal messages, export sheet edge cases. |

Fresh implementation validation is recorded in [GATE_LOG](GATE_LOG.md). Historical measurements are not current results.

## Open items and operating rules

- P21 (4) — moving an artboard moves clip content without its hidden mask — remains open in [PAINS_LOG](../PAINS_LOG.md).
- Mac screen eyedropper remains pending. Window geometry persistence is implemented provisionally in lane D (uncommitted worktree); merge/install and owner relaunch acceptance remain pending.
- Every submitted branch passes tests, clippy and format checks. Merge to `main` still requires independent review; local implementation verification is not merge approval.
- Owner decisions: Mac-first; Windows compile-only; visible user-controlled updates; protect the pure-core boundary; no UI motion **except the box glide**, which stays (owner, 2026-10-06: do not spend effort removing it). Icon stage 2, MCP and web work are parked (2026-10-06). Accepted ADRs remain authoritative.
- GitHub's billing hold is confirmed by the run annotations on 2026-09-27. The annotations do not specify whether a missing payment method is the cause; account settings were not inspected or changed. The owner's [2026-07-11 decision](../history/STATUS_THROUGH_2026-09-26.md#external-action-items-outside-the-repo) defers billing verification indefinitely: local gates remain authoritative, with independent review before merge. Hosted success is not a new merge prerequisite. Triggers stay enabled; rerun CI on the current tip when the hold clears. Branch protection remains deferred.
- Dependency advisories were last triaged on 2026-09-23. This is historical evidence, not a fresh audit: [triage](../audits/2026-09-23-CARGO_AUDIT_TRIAGE.md).
- The four charter trigger flags remain `false` (last owner setting: 2026-07-11). No flag is changed by this work.

## History

The full earlier status page, owner decisions, hand-test lists and old measurements are preserved in [the snapshot through 2026-09-26](../history/STATUS_THROUGH_2026-09-26.md). Completed gate evidence stays in [GATE_LOG](GATE_LOG.md).
