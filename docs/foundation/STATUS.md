> **Status:** current — single current-state page, governed by [FOUNDATION_CHARTER](FOUNDATION_CHARTER.md) §3.
# Varos — current state

Updated 2026-09-27. Code baseline: `601fd7c` on `main`, merging [PR #1](https://github.com/thisisgomaa/varos/pull/1). Current work: `codex/ui-system-plan` now includes UI planning, S5-C semantic validation S5-D bounded PDF reading and S5-E frozen fixtures/harnesses and U0-A bundled Latin/numeric fonts plus minimum U0-B/C controls and E2 Start/Home/Recent integration; independent review batched at the owner’s request, not merged.

## Product and platform

Varos is an early native vector editor in Rust. macOS is the primary and only official build for now. Windows receives compile checks; Windows runtime testing is deferred. Arabic typography is the long-term product focus; editable text is not implemented yet.

Keep the four-crate architecture: pure `varos-core`, scene renderer `varos-render-wgpu`, PDF container `varos-pdf`, desktop `varos-app`. No rewrite or additional framework is needed for this work.

## What is actually available

| Area | Current state |
|---|---|
| Drawing | Shapes, Pen/Direct editing, boolean operations, layers, transforms, snapping and artboard clipping are implemented. Mask model/render/PDF foundations exist; creation/release gestures are pending. Edge cases remain in the pain log. |
| Documents | Independent tabs, dirty tracking, undo/redo, save/close/quit guards and `.vrs` v2 save/open are integrated. |
| File format | `.vrs` remains a PDF containing editable document data. Core v2 version/structure checks, migration and v1 fixtures are implemented; ADR-0008 is accepted. S5-C semantic validation and v1 broken-mask notices are implemented on the work branch, not merged. S5-D bounded PDF reading is also implemented on this branch; S5-E frozen v2/refusal fixtures and old-gate checks are implemented; owner reports no personal files and the scoped read-only scan found only fixtures; real-application acceptance remains pending. |
| Start and recent files | [E2](work_orders/DFS_S2_E2_START_INTEGRATION.md) is integrated on the work branch: immediate Start, Home retaining tabs, successful-only Recent persistence, Locate/Remove/Clear and native Open Recent. Independent review and merge pending. |
| Recovery | Durable writer, snapshot store, scheduler and I/O worker are tested modules. Automatic recovery writing and recovery UI (F1/F2) are not wired into the running app. Existing Save still uses `varos_pdf::save_vrs`. |
| Export | Pure-PDF export exists in the library; the complete app export flow is pending. SVG/PNG interchange is pending. |
| UI system | Current panels work. [UI System v2](../specs/UI_SYSTEM.md) is a bounded planning proposal with a finding-by-finding response; independent acceptance is pending. U0-A embeds Plex Sans/Mono and symbol fallbacks; Arabic remains diagnostic-only after a failed RTL/caret probe. [Minimum Start controls](work_orders/UI_U0_BC_KIT.md) are library-ready with CPU interaction tests and a native gallery; E2 consumes these controls in the running app. |

## Next steps

The single execution map is [PLAN](../PLAN.md). Start there for completed work, the queue and unresolved decisions.

1. Project-health/planning changes merged in PR #1 at `601fd7c`, after independent approval. UI v2 planning is now drafted separately; its scope is in the [UI planning work order](work_orders/UI_SYSTEM_V2_PLAN_2026-09-27.md).
2. Batch-review UI v2 and S5-C/D/E at session end; no personal corpus exists in the scanned scope; application acceptance remains open. Minimum U0-B/C and E2 are built; next F1 durable Save/recovery writing → F2 recovery UI, reusing the modules already built.
3. Complete S4/S6 association/export, then the remaining UI/Editor extraction in behavior-preserving pieces. Deferred product features remain recorded in PLAN.

## Verification

| Check | Latest evidence |
|---|---|
| Workspace tests | 772 passed, 0 failed, 5 intentionally ignored; fresh E2 work-branch run, 2026-09-27. |
| Build, clippy and format | macOS build, macOS and Windows-target clippy with `-D warnings`, and `fmt --check`: PASS, 2026-09-27 implementation checks. |
| Explicit old-reader checks | 3 passed in the default suite, 2026-09-27; fresh/frozen v2 refused and frozen v1 gate control accepted. Old binary not tested. |
| Architecture checker | Portable gate and 7 checker tests PASS; real-source negative probe rejected as expected. |
| Independent review | PR #1 only: APPROVE after one documentation-policy correction; separate Codex reviewer, 2026-09-27. Full scope and limitations in GATE_LOG. |
| CI configuration | Full macOS gates and Windows compile-only checks configured. [Run 36300014830](https://github.com/thisisgomaa/varos/actions/runs/36300014830) failed before either job started: account locked due to a billing issue; no test steps executed. |
| Source size | `ui.rs`: 8,061 lines; `editor.rs`: 5,057 lines, measured 2026-09-27. |
| GPU/window interaction | U0-A native Mac candidate/final font windows checked, 2026-09-27; Arabic candidate failed ordering/caret readiness. U0-B/C native Retina gallery checked for layout and keyboard activation. E2 Start/Open/Home/native Recent checked with isolated data and a copied fixture before final splash removal; final-build presentation recheck pending because Metal reported the window occluded. No general UI or S5 acceptance claimed; owner batch testing remains pending. |

Fresh implementation validation is recorded in [GATE_LOG](GATE_LOG.md). Historical measurements are not current results.

## Open items and operating rules

- P21 selection/mask edge decisions and name-field click-away behavior remain in [PAINS_LOG](../PAINS_LOG.md).
- Mac screen eyedropper and window geometry persistence remain pending.
- Every submitted branch passes tests, clippy and format checks. Merge to `main` still requires independent review; local implementation verification is not merge approval.
- Owner decisions: Mac-first; Windows compile-only; visible user-controlled updates; no UI motion; protect the pure-core boundary. Accepted ADRs remain authoritative.
- GitHub's billing hold is confirmed by the run annotations on 2026-09-27. The annotations do not specify whether a missing payment method is the cause; account settings were not inspected or changed. The owner's [2026-07-11 decision](../history/STATUS_THROUGH_2026-09-26.md#external-action-items-outside-the-repo) defers billing verification indefinitely: local gates remain authoritative, with independent review before merge. Hosted success is not a new merge prerequisite. Triggers stay enabled; rerun CI on the current PR tip when the hold clears. Branch protection remains deferred.
- Dependency advisories were last triaged on 2026-09-23. This is historical evidence, not a fresh audit: [triage](../audits/2026-09-23-CARGO_AUDIT_TRIAGE.md).
- The four charter trigger flags remain `false` (last owner setting: 2026-07-11). No flag is changed by this work.

## History

The full earlier status page, owner decisions, hand-test lists and old measurements are preserved in [the snapshot through 2026-09-26](../history/STATUS_THROUGH_2026-09-26.md). Completed gate evidence stays in [GATE_LOG](GATE_LOG.md).
