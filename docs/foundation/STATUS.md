> **Status:** current — single current-state page, governed by [FOUNDATION_CHARTER](FOUNDATION_CHARTER.md) §3.
# Varos — current state

Updated 2026-10-06. Code baseline: `main` at `df1c26e` (last code merge `7adee37`). Since the 2026-10-04 evening update, these code changes landed on `main`, each with Mac gates and an independent review: UI System P2 field commit law (`fa6bd10`); Start v2 — Boards lanes L1 fonts (`33aa767`), L2 board metadata + `.vrs` format 3 (`380bf05`), L3 thumbnails (`c97be28`) and the page itself (`6405dc9`); UI P3 `ui.rs` split + icon registry + ratchets (`d882454`); top bar 4b (`19611ae`); the 12-item editor polish pass (`7adee37`). Earlier on 2026-10-04: the Codex branch batch (`2cb072a`), DFS S4 on macOS (`221310f`), DFS S6-B/C (`bd4e552`), UI System v3 docs (`420b153`, review pending), icon library stage 1 (`4be3208`), P12 + P21 (1–3) (`abf8ec1`). Gates on `main` (moderator-run on the merged tree): **1056 passed / 0 failed / 8 ignored**, clippy Mac + Windows target clean, fmt clean ([GATE_LOG](GATE_LOG.md) 2026-10-06). Owner hand test in the real window, 2026-10-06: top bar 4b and the polish pass — "كله شغال"; recovery after Force Quit works, but its banner appeared about 30 s late (fix in flight, not merged) and the owner dislikes its look (design pass pending); Finder double-click on a `.vrs` works; File ▸ Export ▸ PDF works. Everything else below is tests-only unless it says otherwise.

## Product and platform

Varos is an early native vector editor in Rust. macOS is the primary and only official build for now. Windows receives compile checks only (owner decision, reconfirmed 2026-10-06); Windows runtime testing is deferred. Arabic typography is the long-term product focus; editable text is not implemented yet, and the Arabic UI gate ([UI_SYSTEM §8](../specs/UI_SYSTEM.md#8-arabic--rtl-gate-owner-piece)) has not passed.

Keep the four-crate architecture: pure `varos-core`, scene renderer `varos-render-wgpu`, PDF container `varos-pdf`, desktop `varos-app`. No rewrite or additional framework is needed for this work.

## What is actually available

| Area | Current state |
|---|---|
| Drawing | Shapes, Pen/Direct editing, boolean operations, layers, transforms, snapping and artboard clipping are implemented. P12 (Pen delete reconnects neighbours) and P21 (1–3) merged in `abf8ec1`; P21 (4) is open. Mask model/render/PDF foundations exist; creation/release gestures are pending. Edge cases remain in the pain log. |
| Documents | Independent tabs, dirty tracking, undo/redo, save/close/quit guards and `.vrs` save/open are integrated. A document is a Board with name, description and tags (`380bf05`). |
| File format | `.vrs` remains a PDF containing editable document data. `FORMAT_VERSION = 3` ([format/mod.rs](../../varos/crates/varos-core/src/format/mod.rs)) with v2→v3 migration and frozen v1/v2/v3 fixtures; ADR-0008 accepted and amended 2026-10-04 for format 3 (the amendment's own independent review is noted as pending in the ADR). S5-C semantic validation, S5-D bounded PDF reading and S5-E refusal corpus merged in `2cb072a`. A real `.vrs` opened from Finder was owner-seen 2026-10-06; refusal messages and the v1 broken-mask notice are tests-only. |
| Start and recent files | E2 merged in `2cb072a`; its page was replaced by [Start v2 — Boards](work_orders/START_V2_BOARDS.md) (`6405dc9`: board cards with thumbnails, tag filters, list view, presets, Board section in Properties). Owner acceptance of the Start v2 page is not recorded (GATE_LOG 2026-10-05: not run in a real window). Owner decision 2026-10-06: the "Custom…" size preset is removed and a new board opens empty — not yet implemented. |
| Recovery | [F1](work_orders/DFS_S3_F1_INTEGRATION.md) and [F2](work_orders/DFS_S3_F2_INTEGRATION.md) merged in `2cb072a` (`recovery_host.rs`). Owner-seen 2026-10-06: recovery after Force Quit works. Open: the banner appeared ~30 s late (fix in flight) and needs a design pass. Cancel/failure paths are tests-only. |
| File association | S4 on macOS merged in `221310f` (`mac_open.rs` + `os_open.rs` → one `OpenPaths`). Owner-seen 2026-10-06: Finder double-click works. Dock drop, `open(1)` and cold-vs-warm launch were not reported separately. Windows parts of S4-A are not implemented. |
| Export | S6-B/C merged in `bd4e552` (`export_ui.rs`; export and ⌘S run on the background I/O worker). Owner-seen 2026-10-06: File ▸ Export ▸ PDF works. Export Cancel and Show in Finder are deferred; the 2.38 s large save was not re-measured off-thread. SVG/PNG interchange is pending. |
| UI system | [UI System v3](../specs/UI_SYSTEM.md) is on `main` as a document (`420b153`); its header still says "proposed". Merged pieces: P2 field law (`fa6bd10`), P3 split/registry/ratchets (`d882454`; `ui.rs` now 957 lines), top bar 4b with `mac_titlebar.rs` (`19611ae`), polish pass (`7adee37`, includes the grey-segment half of P4 and `T_MICRO = 10.5`), icon stage 1 (`4be3208`), Inter + JetBrains Mono fonts (`33aa767`). Owner-seen 2026-10-06: 4b + polish pass. Open: QW6 panel icon size 18 (not verified closed), rest of P4, P5–P8, Search field removal from the band (owner decision 2026-10-06, not yet implemented). Icon stage 2 is parked. |

## Next steps

The single execution map is [PLAN](../PLAN.md). Start there for completed work, the queue and unresolved decisions.

1. Recovery banner: land the ~30 s delay fix, then a mockup-first design pass for its look.
2. Owner decisions of 2026-10-06 still to implement: remove the Search field from the band; remove the "Custom…" board preset.
3. UI pieces: QW6 icon size, rest of P4, P5–P8 (P8 = the Arabic UI gate, owner chooses path A/B), then the remaining UI/Editor extraction in behaviour-preserving pieces; F7/F8 measurements after that. Deferred product features remain recorded in PLAN.

## Verification

| Check | Latest evidence |
|---|---|
| Workspace tests | **1056 passed, 0 failed, 8 ignored** — moderator-run on the merged tree for the polish pass (`7adee37`), 2026-10-06 ([GATE_LOG](GATE_LOG.md)). `df1c26e` after it is docs-only. Not re-run for this page. |
| Build, clippy and format | macOS and `x86_64-pc-windows-msvc` all-target clippy with `-D warnings`, and `cargo fmt --all --check`: PASS, same run, 2026-10-06. |
| Explicit old-reader checks | Run in the default suite; L2 added two v2-era gate tests for format 3 (GATE_LOG 2026-10-04 L2). Old binary not tested. |
| Architecture checker | Portable gate and 7 checker tests PASS, 2026-09-27; not re-run since. |
| Independent review | Every code merge listed at the top had an independent review before landing (Opus reviews Codex's code, Codex Sol reviews Opus's); rounds and verdicts per entry in GATE_LOG and in the merge commit messages. Exception: the UI System v3 document (`420b153`, docs only) — its independent review is still pending (GATE_LOG 2026-10-04 "UI System v3"). |
| CI configuration | Full macOS gates and Windows compile-only checks configured. [Run 36300014830](https://github.com/thisisgomaa/varos/actions/runs/36300014830) failed before either job started: account locked due to a billing issue; no test steps executed. No later run was inspected for this update (unverified whether the lock persists). |
| Source size | `varos-app/src/ui.rs`: 957 lines; `varos-core/src/editor.rs`: 5,174 lines, measured 2026-10-06 at `df1c26e`. |
| GPU/window interaction | Owner hand test 2026-10-06 (top of this page). Not owner-seen and therefore tests-only: Start v2 page, K3 field law by hand, P12/P21 behaviour, Dock/`open(1)` opening, recovery cancel/failure paths, S5 refusal messages, export sheet edge cases. |

Fresh implementation validation is recorded in [GATE_LOG](GATE_LOG.md). Historical measurements are not current results.

## Open items and operating rules

- P21 (4) — moving an artboard moves clip content without its hidden mask — remains open in [PAINS_LOG](../PAINS_LOG.md).
- Mac screen eyedropper and window geometry persistence remain pending.
- Every submitted branch passes tests, clippy and format checks. Merge to `main` still requires independent review; local implementation verification is not merge approval.
- Owner decisions: Mac-first; Windows compile-only; visible user-controlled updates; protect the pure-core boundary; no UI motion **except the box glide**, which stays (owner, 2026-10-06: do not spend effort removing it). Icon stage 2, MCP and web work are parked (2026-10-06). Accepted ADRs remain authoritative.
- GitHub's billing hold is confirmed by the run annotations on 2026-09-27. The annotations do not specify whether a missing payment method is the cause; account settings were not inspected or changed. The owner's [2026-07-11 decision](../history/STATUS_THROUGH_2026-09-26.md#external-action-items-outside-the-repo) defers billing verification indefinitely: local gates remain authoritative, with independent review before merge. Hosted success is not a new merge prerequisite. Triggers stay enabled; rerun CI on the current tip when the hold clears. Branch protection remains deferred.
- Dependency advisories were last triaged on 2026-09-23. This is historical evidence, not a fresh audit: [triage](../audits/2026-09-23-CARGO_AUDIT_TRIAGE.md).
- The four charter trigger flags remain `false` (last owner setting: 2026-07-11). No flag is changed by this work.

## History

The full earlier status page, owner decisions, hand-test lists and old measurements are preserved in [the snapshot through 2026-09-26](../history/STATUS_THROUGH_2026-09-26.md). Completed gate evidence stays in [GATE_LOG](GATE_LOG.md).
