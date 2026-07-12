> **Status:** current — F4.1 implementation design, governed by ADR-0002 and `docs/foundation/FOUNDATION_CHARTER.md` sections 3-6.
# F4.1 EditCommand Design

## Boundary

`EditCommand` is the closed, internal command boundary for deterministic edits owned by `varos-core`. Any panel or keyboard action that mutates `Document` or participates in history enters core through this enum. Tool choice, selection, previews, and view preferences remain explicit `Editor` interfaces. Pointer/gesture methods remain core input interfaces: the app supplies input coordinates and modifiers, while core alone decides document mutation and history.

The enum is not a plugin or AI API. ADR-0002 explicitly defers adapters, versioning, query contracts, permissions, and compatibility.

## Proposed API

```rust
pub enum EditCommand {
    SetObjectBounds { x: Option<f32>, y: Option<f32>, width: Option<f32>, height: Option<f32>, anchor_x: f32, anchor_y: f32 },
    SetObjectRotation(f32),
    SetOpacity(f32),
    SetStrokeWidth(f32),
    SetClipExempt(bool),
    ApplyPaint { target: PaintTarget, color: Option<Rgba> },
    SwapColors,
    DefaultPaint,
    PickerBegin,
    PickerLivePaint { target: PaintTarget, color: Rgba },
    PickerLiveArtboard { index: usize, color: Rgba },
    PickerCommit { current: Option<PaintTarget>, color: Rgba },
    PickerCancel,
    ToggleNodeHidden(u32),
    ToggleNodeLocked(u32),
    RenameNode { node: u32, name: String },
    GroupSelection,
    UngroupSelection,
    DeleteLayerSelection,
    MoveLayer { sources: Vec<u32>, target: u32, position: DropPos },
    DuplicateMoveLayer { sources: Vec<u32>, target: u32, position: DropPos },
    MoveLayerToBoard { sources: Vec<u32>, source_board: Option<usize>, target_board: usize },
    Flip(bool),
    Align { mode: AlignMode, target: AlignTarget },
    Distribute(DistAxis),
    Boolean(BoolOp),
    Arrange(ZOrder),
    TransformAgain,
    DeleteSelected,
    Nudge { x: f32, y: f32 },
    SetActiveArtboard(usize),
    SetArtboardRect { index: usize, x: Option<f32>, y: Option<f32>, width: Option<f32>, height: Option<f32> },
    RenameArtboard { index: usize, name: String },
    SetArtboardColor { index: usize, color: Option<Rgba> },
    ToggleArtboardClip(usize),
    ToggleArtboardHidden(usize),
    ToggleArtboardLocked(usize),
    OrientArtboard(usize),
    AddArtboard,
    DuplicateArtboard(usize),
    DeleteArtboard(usize),
    SetArtboardCount(usize),
    SetMoveArtWithArtboard(bool),
    SetRulerOrigin(Pt),
    CommitGuide,
    CycleUnits,
    SetSnapConfig(SnapConfig),
    ToggleSnapping,
    ToggleGuidesLocked,
    ToggleSmartGuides,
    Undo,
    Redo,
}

impl Editor {
    pub fn execute(&mut self, command: EditCommand);

    // Declared transient/view interfaces used by the app translator.
    pub fn set_paint_target(&mut self, target: PaintTarget);
    pub fn set_constrain_wh(&mut self, locked: bool);
    pub fn clear_ruler_origin_preview(&mut self);
    pub fn toggle_guides_visibility(&mut self);
    pub fn toggle_rulers_visibility(&mut self);
}
```

`execute` does not add a blanket `begin/commit` wrapper. Each variant delegates to the existing core semantic method, preserving its exact history policy. This is required for picker begin/live/commit, undo/redo, and the currently non-undoable serialized mode flags. `SetStrokeWidth`, ruler origin, and snap assignment move into core with their current behavior unchanged.

## Complete Op migration

Source inventory: `varos/crates/varos-app/src/ui.rs:91-144` contains 48 variants; dispatch is at `ui.rs:5376-5453` after F3.

| Current `Op` | F4.1 destination | Reason / gray-state ruling |
|---|---|---|
| `Tool` | `Editor::set_tool` | Transient active-tool state; declared core input interface. |
| `SetBBox` | `EditCommand::SetObjectBounds` | Document geometry plus history. |
| `SetRot` | `EditCommand::SetObjectRotation` | Document transform plus history. |
| `SetOpacity` | `EditCommand::SetOpacity` | Document paint property plus history. |
| `SetStrokeW` | `EditCommand::SetStrokeWidth` | Empty selection updates current style; non-empty selection mutates Document and history. The whole semantic action stays one command. |
| `SetClipExempt` | `EditCommand::SetClipExempt` | Document node property plus history. |
| `Paint` | `EditCommand::ApplyPaint` | Sets current target/style and may mutate selected Document paths with history. |
| `PaintFocus` | `Editor::set_paint_target` | Transient current-target focus only. |
| `SwapColors` | `EditCommand::SwapColors` | Updates current style and may mutate selected paths with history. |
| `DefaultPaint` | `EditCommand::DefaultPaint` | Updates current style and may mutate selected paths with history. |
| `OpenPicker` | Remains UI presentation; emits `EditCommand::PickerBegin` when intercepted | Opening a modal is presentation, but opening its history session belongs to core command dispatch. |
| `PickerLive` | `PickerLivePaint` or `PickerLiveArtboard` | Live Document mutation inside the already-open history session. |
| `PickerCommit` | `EditCommand::PickerCommit` | Commits the one-step picker history contract. |
| `PickerCancel` | `EditCommand::PickerCancel` | Restores the pending Document snapshot. |
| `LayerSelectSet` | `Editor::layer_select_set` | Transient selection only. |
| `LayerToggle` | `Editor::layer_toggle` | Transient selection only. |
| `LayerEye` | `EditCommand::ToggleNodeHidden` | Document tree visibility plus history. |
| `LayerLock` | `EditCommand::ToggleNodeLocked` | Document tree lock plus history. |
| `LayerRename` | `EditCommand::RenameNode` | Document tree name plus history. |
| `LayerGroup` | `EditCommand::GroupSelection` | Document structure plus history. |
| `LayerDeleteSel` | `EditCommand::DeleteLayerSelection` | Document deletion plus history. |
| `LayerMove` | `EditCommand::MoveLayer` | UI translates drop zone to core `DropPos`; core owns structure/history. |
| `LayerDupMove` | `EditCommand::DuplicateMoveLayer` | Document duplication/structure plus history. |
| `LayerMoveBoard` | `EditCommand::MoveLayerToBoard` | Document artboard membership plus history. |
| `Flip` | `EditCommand::Flip` | Document geometry plus history. |
| `Align` | `EditCommand::Align` | Document geometry plus history. |
| `Distribute` | `EditCommand::Distribute` | Document geometry plus history. |
| `Bool` | `EditCommand::Boolean` | Document path structure plus history. |
| `AbActive` | `EditCommand::SetActiveArtboard` | Mutates serialized `Document.active`, even though it is not currently undoable. |
| `AbRect` | `EditCommand::SetArtboardRect` | Document geometry plus history. |
| `AbName` | `EditCommand::RenameArtboard` | Document name plus history. |
| `AbColor` | `EditCommand::SetArtboardColor` | Document page color plus history. |
| `AbClip` | `EditCommand::ToggleArtboardClip` | Document clipping state plus history. |
| `AbEye` | `EditCommand::ToggleArtboardHidden` | Document visibility plus history. |
| `AbLock` | `EditCommand::ToggleArtboardLocked` | Document lock plus history. |
| `AbOrient` | `EditCommand::OrientArtboard` | Document geometry plus history. |
| `AbAdd` | `EditCommand::AddArtboard` | Document structure plus history. |
| `AbDup` | `EditCommand::DuplicateArtboard` | Document structure plus history. |
| `AbDel` | `EditCommand::DeleteArtboard` | Document structure plus history. |
| `AbCount` | `EditCommand::SetArtboardCount` | Document structure plus history. |
| `AbMoveArt` | `EditCommand::SetMoveArtWithArtboard` | Mutates serialized document setting using existing core semantics. |
| `RulerOrigin(Some)` | `EditCommand::SetRulerOrigin` | Direct Document write at `ui.rs:5441` moves into core; snap and preview behavior stay together. |
| `RulerOrigin(None)` | `Editor::clear_ruler_origin_preview` | Ends transient preview without changing Document. |
| `GuidePreview` | `Editor::set_guide_preview` | Transient preview only. |
| `GuideCommit` | `EditCommand::CommitGuide` | Document guide insertion plus history. |
| `CycleUnits` | `EditCommand::CycleUnits` | Serialized units plus history. |
| `ToggleSnapping` | `EditCommand::ToggleSnapping` | Direct Document write at `ui.rs:5450`; preserves current non-undoable mode-flag behavior. |
| `ToggleGuides` | `Editor::toggle_guides_visibility` | View preference only. |
| `ToggleRulers` | `Editor::toggle_rulers_visibility` | View preference only. |

## Additional call sites in F4.1

- The magnet-menu write-back `ed.doc.snap = snap_cfg` at `ui.rs:1184` becomes `EditCommand::SetSnapConfig`; it is a fourth direct Document write outside `apply_ops` and must not survive the boundary.
- `set_stroke_width` at `ui.rs:5458-5472`, including the direct path write at line 5467, moves completely into `EditCommand::SetStrokeWidth`.
- Discrete keyboard edits in `main.rs:153-218` use the same command boundary: undo/redo, arrange, group/ungroup, guides lock, Smart Guides, transform-again, color application, delete, and nudge. Tool switching, paint-target switching, escape, and view-only toggles remain declared `Editor` interfaces.
- File lifecycle (`replace_doc`, New/Open/Save/Export), dialogs, panels, and window actions are F4.2 `AppCommand` work. File menu rows remain visual-only.
- Pointer, double-click, and guide-drag input methods stay `Editor` interfaces. They already cross into core before edit semantics are chosen; wrapping raw pointer events in `EditCommand` would mislabel input as a deterministic edit and duplicate the gesture state machine.

## Acceptance checks

1. The six F3 tests pass unchanged.
2. `ui.rs` contains no direct write to `ed.doc`; the three named direct writes and the magnet-menu write-back are gone.
3. `apply_ops` contains translation only: `EditCommand` construction/execution or declared transient `Editor` calls.
4. Headless core tests execute representative commands and verify existing revision/undo behavior.
5. The closed enum is documented only as an internal boundary, never as a plugin protocol.

## Implementation evidence

- The implemented enum starts at `varos/crates/varos-core/src/command.rs:12`; `Editor::execute` is at `command.rs:191`, and the migrated stroke-width/history behavior is at `command.rs:216`.
- The frame-level snap write-back now executes `SetSnapConfig` at `varos/crates/varos-app/src/ui.rs:1185`; picker history begins through `PickerBegin` at `ui.rs:1199`; `apply_ops` is the translator at `ui.rs:5377`.
- Keyboard edit dispatch enters the same boundary from `varos/crates/varos-app/src/main.rs:153`.
- A production-only scan of `ui.rs` lines 1-5517 (everything before the first `#[cfg(test)]`) found 0 direct `ed.doc` assignments or collection mutations. The same direct-assignment scan found 0 in `main.rs`.
- The F3 core files `golden.rs` and `history_lifecycle.rs` have no diff from `main`. The normalized SHA-256 of the F3 `ui.rs::characterization_tests` module is identical before and after F4.1: `D11ADD82FF976BA31694B5FE8385BF07DB502973A6D714AF1FCE3FD2FF9F5FE7`.

### New headless tests and red proof

| Test | Evidence | Temporary semantic mutation | Observed red result |
|---|---|---|---|
| Stroke/history/undo/redo | `varos/crates/varos-core/tests/edit_command.rs:28` | Change the migrated clamp from `max(0)` to `max(1)`. | Stroke assertion failed: 1 vs 0. |
| Non-undoable serialized settings | `varos/crates/varos-core/tests/edit_command.rs:45` | Make `ToggleSnapping` a no-op. | `snap.enabled` assertion failed. |
| Paint target + selected path | `varos/crates/varos-core/tests/edit_command.rs:63` | Ignore the command's target before applying paint. | `PaintTarget::Stroke` assertion failed. |

Every mutation was restored immediately; `rg 'F4 red-proof mutation' varos --glob '!target/**'` returns no matches.

### Branch-tip gates

- `tools/check_links.ps1`: PASS (73 first-party docs, 81 relative links, 58 heading anchors).
- `tools/check_dep_directions.ps1`: PASS.
- `cargo fmt --all -- --check`: PASS.
- `cargo test --workspace -j 4`: PASS (232 tests, 0 failed).
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: PASS.
- `git diff --check main..HEAD`: PASS.
