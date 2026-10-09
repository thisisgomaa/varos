# Crash safety and clipping boundary — 2026-10-09
Slice 0.2 evidence: reference/gap/GAP_2_TOOLS_PAINT_LAYERS_APP.md:151.
Slice 0.9 evidence: reference/gap/GAP_2_TOOLS_PAINT_LAYERS_APP.md:128.
Native command execution catches unwind panics and restores the entire Editor snapshot, including
selection, undo/redo, pending transactions and revision. `execute` returns `EngineError::Internal`;
the interactive `execute_ui` facade retains the error for host notices. Abort-mode panics cannot unwind.
Bridge staging catches adapter panics before publication; the request boundary also restores the live
editor for mutating requests and returns an internal structured failure. Read-only requests take no
rollback checkpoint. API 1.0/1.1 fixtures remain frozen.
GPU callbacks record the first fault. Nonblocking poll catches a lost-device panic. Stopped devices
skip resize and render work. The first fault wakes the idle event loop once; it shows one notice and stops retrying frames and keeps file
commands available. No CPU fallback is introduced. ADR-0001's readable failure rule extends to runtime.
Clipping commands use the existing group container and frontmost selected path as the mask, discard
its fill/stroke, select the resulting group, and publish one history step. Release preserves the
container under the existing model semantics. Partial-group or group-mask selections
are refused rather than silently clipping a subset.

No-panic baselines measured on the original HEAD sources: core 37, Bridge 106. The AST scan excludes
#[cfg(test)] modules, tests/ files and test-only statements/arms, but includes method/macro calls
inside production macro token trees. Comments and string literals do not count. These caps never rise.
Rollback snapshots include history and transient tool state; rebuilding a disposable flatten cache
after recovery is safe. Immutable undo/redo/pending documents use shared Arc snapshots: a checkpoint
copies the live document/transient state and bounded history handles, never retained document contents.
Clipping menu mirrors share per-editor enablement keyed by revision and selection; transactions bypass
the cache and command/begin invalidate it. On cache misses, indexed lookups and deduplicated units
avoid repeating subtree walks once per selected descendant. Direct command checks remain uncached.
API 1.2 discovery is opt-in through capabilities and tools/list's api parameter; default tool tables
and API 1.0/1.1 receipts remain unchanged. Headless CLI apply supports ClipMake/ClipRelease after
SelectPaths, including release after reopening a clipped file.
