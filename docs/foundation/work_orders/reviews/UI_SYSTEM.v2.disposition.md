> **Status:** proposed — implementer's response to the five v1 reviews, 2026-09-27; not a new independent review or implementation sign-off.
# UI v2 — review disposition

Baseline: `601fd7c`. Contract keys K1–K6 and piece IDs refer to [UI System v2](../../../specs/UI_SYSTEM.md). Original reviews remain unchanged. **Design** means the proposal now states a remedy; **Deferred** names the later owner; **Existing** identifies current protection, not complete closure. All remedies still need their piece's code/tests; owner choices and independent acceptance remain explicit.

## Architecture

Source: [architecture review](UI_SYSTEM.architecture.review.md).

| Finding | Disposition and evidence required |
|---|---|
| 1 | Design K1: action-agnostic lib kit; U1 types before consumers. Compile lib + both binaries. |
| 2 | Design K3 / U3-T: core-owned live span; release-safe nesting and inner commits tested before migration. |
| 3 | Design K4 / U2-P: measure present baseline; selection/live invalidation and idle counters. Old cost is not a new measurement. |
| 4 | Design K1 / U2-O: mechanical Op removal before panels; Edit/App only. Gray setters follow current F4 until explicit ruling amendment. |
| 5 | Design K1 / U1-A: owned CmdCtx, native-menu/host resolution outside UI frame. |
| 6 | Existing SessionId-salted field IDs; remaining K4 / U2-D separates set_tabs identity, Save invalidation and same-id replacement generation. |
| 7 | Owner decision deferred to U3-T: current discard vs proposed confirm-valid/block-invalid. E2 keeps S1; no silent behavior change. |
| 8 | Design K3 / U3-A/B: core constrain_wh and existing reference point arguments; no duplicate lock. |
| 9 | Design K6 / U3: single registry + match, no Panel trait. |
| 10 | Design piece table: Op/cache/DocUi/core transaction split; E2/F1/F2 precede broad migration, one owner per shared file. |
| 11 | Design K2/K5/U6: precise grep, host actions outside frame, explicit Platform fixture/repaint, explicit widget-key hook. |

## Input

Source: [input review](UI_SYSTEM.input.review.md).

| Finding | Disposition and evidence required |
|---|---|
| P1-1 | Design K2/U1-B: text responder first for keyboard and native menu; retain native event adapter until tested. |
| P1-2 | Design K2: StartModel owns Start focus. Canvas Tab policy waits for U5 owner choice; no box focus engine/Ctrl-F6. |
| P1-3 | Deferred K3/U3-T: host-readable field buffer and settle policy; S1 remains until explicit behavior change. |
| P2-4 | Design K2/U5-A: simple owned InputEvent, no private Winit constructors or arbitrary adapter line cap. |
| P2-5 | Design K2/U1: physical keys, distinct modifiers, platform adapter, collision/reserved/native-accelerator tests. |
| P2-6 | Existing host FIFO/S1 protections; E2/S6 extend AppCommand. No second dispatcher or table prerequisite. |
| P2-7 | Design K2/U1: fresh validation at drain for every source; pointer and modal gates separate. |
| P2-8 | Design K2/U5: physical canvas vs egui ppp including UI zoom; one conversion boundary. |
| P2-9 | Design piece table: types → menus → keyboard, then router extraction → behavior fixes; host/core slots explicit. |
| P2-10 | Deferred optional U5 pinch: finite delta, clamp, chrome yield; no smart-zoom behavior implied. |
| P2-11 | Design K6/U5: shared OS interval and egui InputOptions; custom helper only canvas/caption. |
| P3-12 | Design K2: preserve title, modifier order and one chord/hint/native-equivalent source. |
| P3-13 | Design K2/U1: repeat policy explicit; verify nudge/zoom against incumbent behavior. |
| P3-14 | Design K3/U3-T: release-safe owner guard, no nested pending overwrite. |

## Visual

Source: [visual review](UI_SYSTEM.visual.review.md).

| Finding | Disposition and evidence required |
|---|---|
| F1 | Design K1/U0-B: lib kit contains no binary types. |
| F2 | Recorded owner on-states preserved in K5; disabled/on/hover/focus specified. HUD/switch/caret/drop reviewed with their later consumer. |
| F3 | Recorded FAINT→MUTED and QW6 sizes; U0-A measures actual contexts. S2 legacy micro-label values superseded explicitly. |
| F4 | Design K5/U0-A: versioned licensed fonts, glyph fallback, actual pinned-egui bidi/caret/clipboard spike; no blanket Arabic claim. |
| F5 | Owner direction preserved; K6/U4-M separates ghost/glide/glow/direction, superseding ADR; scroll and wheel residual distinguished. |
| F6 | U0 minimum buttons/list/section/Home; text presentation now, edit/switch controls before later consumers. Document-tab migration remains separate. |
| F7 | K5/U0-C after fonts/kit: warm-up, key hook, alpha, scripted states, ppp 1/2, real-window gallery. |
| F8 | K5: keyboard focus-visible overlay tested on azure/selected; disabled help reachable. |
| F9 | K5: composite-background contrast; TEXT on selected/hover where required. |
| F10 | K5/U6: runtime tokens only, no Markdown/HTML synchronization parser. |
| F11 | U0-A inventory keeps semantic colors/weights/spacing and derives dimensions; no unapproved palette replacement. |
| F12 | Deferred font cleanup after U0 coverage and measured binary size; default-font removal is not a blind prerequisite. |
| F13 | K5/U3/U6: Mono values only, overflow/weight tests, meaningful single events, casing at renderer; icon standard migrates incrementally. |

## Panels

Source: [panels review](UI_SYSTEM.panels.review.md).

| Finding | Disposition and evidence required |
|---|---|
| F1 | K6/U4-S: logical-point size model/Fill, resizable Properties, monitor fallback; proposed 274/800×560 are not accepted constants. |
| F2 | Deferred Workspaces: app schema with conversion only inside boxtree, no fork serde contract. |
| F3 | U4-M requires superseding ADR before glide code; accepted ADR-0006 remains unchanged now. |
| F4 | Piece table: controls before panels, core before picker, sequential ownership for Properties/ui.rs. |
| F5 | K6/U3: one registry, no trait metadata duplication. |
| F6 | K6/U3: home jump opens container, reveals and expands target section. |
| F7 | K2/K6/E2: Start surface owns focus and hides document controls without violating never-empty Workspace. |
| F8 | Deferred Workspaces: debounce/worker/flush and corrupt/newer-file protection. |
| F9 | Recorded owner decision: Reset in Window only, UI-only state. |
| F10 | Deferred Accessibility: no Ctrl-F6 default. |
| F11 | Piece table names host/app_command/ui/core ownership; E2 owns Start drawing, not a blanket board extraction. |
| F12 | U3-B slot table; U4-P placement/Fit only. |
| F13 | Size before motion/placement; persistence moved out to Workspaces after size model. |
| F14 | E2 → F1 → F2 before U1; no command-framework delay to recovery. |
| F15 | U4-M adds patch/hash evidence before fork mutation; current confinement gate is not drift detection. |
| F16 | U2-P measurement/cache separate from U2-D frame/DocUi extraction. |
| F17 | U0-A/U4-S inventory actual heights; board rect excludes rulers. No stale values copied as tokens. |
| F18 | K6/U3-B overflow keeps each domain's home target. |
| F19 | K6: placement/direction changes update UI_DIRECTION only with the relevant approved decision. |
| F20 | K1: retain ToggleDock. |
| F21 | K6/U4-S: normalize tree on mutation, not per frame. |
| F22 | K6/U4-M: ghost disabled input and discarded scratch output. |
| F23 | K6/U3: visible/frontmost menu ticks and show/focus/toggle rules; Board excluded. |
| F24 | K6: Board exactly once, never closable/tabbable. |
| F25 | Deferred full RTL to Accessibility; U0 still tests mixed user names. |
| F26 | Deferred Workspaces naming; no second layout file. |
| F27 | U4-P owns Fit occlusion from actual bar/rail geometry; no duplicate QW8 band constants. |

## Economy

Source: [economy review](UI_SYSTEM.economy.review.md).

| Finding | Disposition and evidence required |
|---|---|
| P1-1 | Piece table names existing hot files including tokens/main/host/core; U0 is not “new files only.” |
| P1-2 | K2/K5: physical keys, shared labels and semantics from the first new control; no unsupported AccessKit claim. |
| P1-3 | Scope/owner table: planning-only now; later font changes visibly affect UI, tests/RTL/accessibility not claimed done. |
| P2-4 | No agent-day promises; pieces have explicit dependencies and acceptance. |
| P2-5 | K4/U2-P: reproducible Mac baseline, p95/rebuilds, no idle recompute/regression; numeric budgets only with measurement. |
| P2-6 | K3/K5: empty/busy/error, truthful Cancel, single-commit scrub/picker, no-history view/tool, existing nudge/checkpoint policy. |
| P2-7 | K5 contextual contrast plus explicit S2 label amendment. |
| P2-8 | K5 focus overlay; Ctrl-F6 deferred. |
| P2-9 | Deferred vendor-neutral standard until V1 evidence exists. |
| P2-10 | Deferred Workspaces with app-owned schema. |
| P2-11 | K5: limited kit geometry goldens; panels test semantic events, screen reader still unverified. |
| P2-12 | Charter remains authoritative: p6-header and OWNERSHIP_MAP before F5. Reviewer suggestion to supersede charter is not adopted. |
| P3-13 | Drop doc/token parsers; values in Rust only. |
| P3-14 | Drop fake-value grep as standing law; preserve safety/debug gates and flags. |
| P3-15 | Replace overlapping law list with six contracts plus gates; no numerical law-count target. |
| P3-16 | U6: word-boundary checks or type boundary, no false “ed.”/Option matches. |
| P3-17 | Retain ToggleDock. |
| P3-18 | K6 resizable Properties, no fixed-width promise. |
| P3-19 | Deferred F7 local Copy Diagnostics design; no blanket perf-flag removal or network telemetry. |
| P3-20 | K5 24pt hit rect or documented spacing exception. |
| P3-21 | Optional later pinch piece, not a Start prerequisite. |

## Remaining decisions and validation

Owner decisions needed only before their consumer: typed-field settlement and canvas Tab policy; visual details not covered by recorded on-states; any smart zoom behavior. Workspaces, full RTL/accessibility, broad standard extraction and diagnostics remain named deferred work. Technical proposals still require implementation tests and an independent review before merge. This document does not change the five original REQUEST CHANGES verdicts.
