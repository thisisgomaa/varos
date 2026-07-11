> **Status:** current — F3 evidence record, governed by `docs/foundation/FOUNDATION_CHARTER.md` sections 3-5.
# F3 Characterization Evidence

## Pinned behavior

| Seam | Test evidence | Contract pinned |
|---|---|---|
| Golden file compatibility | `varos/crates/varos-core/tests/golden.rs:26` | Each of the three frozen fixtures follows load, save A, reload A with `Document` equality, save B, and `A == B`. Original fixture bytes are never compared with current bytes. |
| Completed gesture history | `varos/crates/varos-core/tests/history_lifecycle.rs:29` | Pointer-down begins one snapshot, pointer-up commits one revision, undo restores the original document, and redo restores the moved document. |
| Known mid-gesture defect | `varos/crates/varos-core/tests/history_lifecycle.rs:51` | The current stale-`pending` behavior, redo loss, and newer-state-on-next-undo defect are frozen without being fixed. Risk register: `docs/audits/2026-07-11-CLAUDE-COUNTER-REVIEW.md` section 4.6. |
| Private `Op` dispatch | `varos/crates/varos-app/src/ui.rs:5600` | Representative editor delegation, opacity clamping, direct stroke-width mutation, paint focus, and separate history revisions. |
| Direct UI writes | `varos/crates/varos-app/src/ui.rs:5621` | Ruler-origin document write and preview, snap-master write, guides visibility, rulers visibility, and preview clear. Direct production writes are at `ui.rs:5441`, `ui.rs:5450`, and `ui.rs:5467`. |
| Menu/shortcut semantics | `varos/crates/varos-app/src/ui.rs:5643` | The Smart Guides menu callback and Ctrl+U produce the same `SnapConfig` transition. |

## Test scaffolding

- `Op` and `apply_ops` remain private inside `ui.rs`; their tests use a local `#[cfg(test)]` module, following the existing color-test precedent.
- The Smart Guides assignment was mechanically extracted into private `toggle_smart_guides`; the menu still calls the same one-line state transition.
- Core history coverage is integration-test-only. No production history API or visibility changed.
- No dependency was added and no public surface was widened.

## Red/green proof

Each new test was run once against a temporary behavior mutation, then the mutation was restored and the targeted test rerun green.

| Test | Temporary mutation | Red evidence |
|---|---|---|
| Golden round-trip | Increment `Document.ids` in `doc_from_blob` after every load. | Failed fixture `ancient_pre_artboards.vrs` at reloaded-`Document` equality (`ids` 13 vs 12). |
| Completed drag | Clear `dirty` immediately before pointer-up `commit`. | Failed revision assertion (0 vs 1). |
| Mid-gesture undo | Clear `pending` and `dirty` inside `undo`, modeling the parked fix. | Failed current redo-loss assertion (revision 3 vs 2). |
| `Op` delegation/clamps | Change the direct stroke-width clamp from `max(0)` to `max(1)`. | Failed stroke-width assertion (1 vs 0). |
| Direct UI writes | Make `Op::ToggleSnapping` a no-op. | Failed `snap.enabled` assertion. |
| Smart Guides parity | Make the extracted menu callback a no-op. | Failed menu/shortcut equality (`true` vs `false`). |

The restored targeted commands were:

```powershell
cargo test -p varos-core --test golden every_golden_fixture_obeys_the_full_round_trip_law -j 4
cargo test -p varos-core --test history_lifecycle -j 4
cargo test -p varos-app --bin varos characterization_tests -j 4
```

## Honest coverage boundary

- The File menu rows currently discard `menu_row` click results, so New/Open/Save/Export are visual rows, not callable menu actions. An action-parity test would be false coverage until F4 supplies `AppCommand` wiring (or the parked release-milestone work wires them).
- Ctrl+S and Ctrl+O are handled inside the winit event loop and invoke native `rfd` dialogs. A headless unit test cannot truthfully drive those OS interactions or compare them with the unwired File menu.
- The Smart Guides test starts at the extracted semantic callback. It proves menu/shortcut state-transition parity, not egui pointer hit-testing, popup placement, or click delivery; those require UI automation outside the current harness.

## Branch-tip gates

- `tools/check_links.ps1`: PASS (72 first-party docs, 81 relative links, 58 heading anchors).
- `tools/check_dep_directions.ps1`: PASS (the four-crate edge policy and `egui_tiles` confinement).
- `cargo fmt --all -- --check`: PASS.
- `cargo test --workspace -j 4`: PASS (229 tests, 0 failed; baseline 223 plus the 6 F3 tests).
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: PASS.
