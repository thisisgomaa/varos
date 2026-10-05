use super::*;

pub(crate) enum Op {
    Tool(ToolKind),
    SetBBox(Option<f32>, Option<f32>, Option<f32>, Option<f32>, f32, f32), // nx,ny,nw,nh + ref ax,ay
    SetRot(f32),
    SetOpacity(f32),
    SetStrokeW(f32),
    SetClipExempt(bool), // A30: release the selection from artboard clip (true) / re-clip it (false)
    Paint(PaintTarget, Option<Rgba>),
    PaintFocus(PaintTarget), // rail fill/stroke control: focus the target (X toggles)
    SwapColors,              // Shift+X
    DefaultPaint,            // D — white fill / black stroke
    OpenPicker(MTarget),     // double-click a swatch → open the Color Picker modal for it
    // ---- Color Picker live session (A6): one undo step per whole picker interaction ----
    PickerLive(MTarget, Rgba), // per-frame preview: apply the current colour, NO new history
    PickerCommit(Option<PaintTarget>, Rgba), // OK: fold the drag into ONE step + set current paint + MRU
    PickerCancel,              // Cancel/Esc: revert to the value captured on open
    // ---- layers panel (node ids) — the SIMPLE panel (07-03 pivot) ----
    LayerSelectSet(Vec<u32>), // plain click / Shift-range: select these rows' art (replace)
    LayerToggle(u32),         // Ctrl+click a row: toggle its art in/out of the selection
    LayerEye(u32),
    LayerLock(u32),
    LayerRename(u32, String),
    LayerGroup,
    LayerDeleteSel,                                 // footer: Group the selection · Delete the selection
    LayerMove(Vec<u32>, u32, u8), // drag-drop: srcs (a multi-selection travels together), target, zone
    LayerDupMove(Vec<u32>, u32, u8), // Alt+drag: duplicate the rows' art into the target
    LayerMoveBoard(Vec<u32>, Option<usize>, usize), // cross-section drop: srcs, source board, target board
    Flip(bool),
    Align(AlignMode, AlignTarget), // A4: carries the target the align resolves against
    Distribute(DistAxis),
    Bool(varos_core::boolean::BoolOp), // Pathfinder home + the Properties "Shape" mirror
    // ---- artboard ops (i = artboard index) ----
    AbActive(usize),
    AbRect(usize, Option<f32>, Option<f32>, Option<f32>, Option<f32>), // x,y,w,h (each optional)
    AbName(usize, String),
    AbColor(usize, Option<Rgba>), // None = transparent page
    AbClip(usize),                // toggle
    AbEye(usize),                 // board eye — the Layers section header (piece C)
    AbLock(usize),                // board padlock — the Layers section header (piece C)
    AbOrient(usize),              // swap w/h
    AbAdd,
    AbDup(usize),
    AbDel(usize),
    AbCount(usize),
    AbMoveArt(bool),
    RulerOrigin(Option<varos_core::geom::Pt>), // Some = set zero-point (snapped) + show crosshair; None = end drag
    GuidePreview(bool, varos_core::geom::Pt),  // ruler drag-out: (vertical, world) → live snapped guide preview
    GuideCommit,                               // drop the previewed guide into the document
    // ---- the Board section (Start v2 L5: name / description / tags; one undo step each, dirty) ----
    BoardName(String),
    BoardDescription(String),
    BoardTags(Vec<String>),
    // ---- document settings (Pain A15: Properties dock when nothing is selected) ----
    CycleUnits,     // step the document display unit (undoable — mutates the serialized doc.units)
    ToggleSnapping, // doc.snap.enabled master switch (a non-undoable mode flag, like the magnet menu)
    ToggleGuides,   // show/hide ruler guides — the guides-visibility view pref (mirrors Ctrl+;)
    ToggleRulers,   // show/hide rulers — the rulers view pref (mirrors Ctrl+R)
    // ---- K3 field law (`ui/fields.rs`) ----
    Field(Box<Op>),                  // a field's commit: applied before the frame's other ops
    FieldPending(egui::Id, Box<Op>), // what the open field would commit now (kept by `Ui`, never applied)
}
pub(crate) fn apply_ops(ed: &mut Editor, ops: Vec<Op>) {
    for op in ops {
        match op {
            Op::Tool(t) => ed.set_tool(t),
            Op::SetBBox(x, y, width, height, anchor_x, anchor_y) => {
                ed.execute(EditCommand::SetObjectBounds { x, y, width, height, anchor_x, anchor_y })
            }
            Op::SetRot(degrees) => ed.execute(EditCommand::SetObjectRotation(degrees)),
            Op::SetOpacity(opacity) => ed.execute(EditCommand::SetOpacity(opacity)),
            Op::SetClipExempt(exempt) => ed.execute(EditCommand::SetClipExempt(exempt)),
            Op::SetStrokeW(width) => ed.execute(EditCommand::SetStrokeWidth(width)),
            Op::Paint(target, color) => ed.execute(EditCommand::ApplyPaint { target, color }),
            Op::PaintFocus(target) => ed.set_paint_target(target),
            Op::SwapColors => ed.execute(EditCommand::SwapColors),
            Op::DefaultPaint => ed.execute(EditCommand::DefaultPaint),
            Op::OpenPicker(_) => {} // UI-only: intercepted in run() (opens the modal); never reaches here
            Op::PickerLive(target, color) => match target {
                MTarget::Paint(target) => ed.execute(EditCommand::PickerLivePaint { target, color }),
                MTarget::Ab(index) => ed.execute(EditCommand::PickerLiveArtboard { index, color }),
            },
            Op::PickerCommit(current, color) => ed.execute(EditCommand::PickerCommit { current, color }),
            Op::PickerCancel => ed.execute(EditCommand::PickerCancel),
            Op::LayerSelectSet(nids) => ed.layer_select_set(&nids),
            Op::LayerToggle(n) => ed.layer_toggle(n),
            Op::LayerEye(node) => ed.execute(EditCommand::ToggleNodeHidden(node)),
            Op::LayerLock(node) => ed.execute(EditCommand::ToggleNodeLocked(node)),
            // a `<Path>` row shows `Path::name`, not its leaf node's name — rename what the row reads
            Op::LayerRename(node, name) => match ed.doc.node(node).map(|n| n.kind) {
                Some(varos_core::model::NodeKind::Path(path)) => ed.execute(EditCommand::RenamePath { path, name }),
                _ => ed.execute(EditCommand::RenameNode { node, name }),
            },
            Op::LayerGroup => ed.execute(EditCommand::GroupSelection),
            Op::LayerDeleteSel => ed.execute(EditCommand::DeleteLayerSelection),
            Op::LayerMove(srcs, target, zone) => {
                let position = match zone {
                    0 => varos_core::model::DropPos::Before,
                    1 => varos_core::model::DropPos::Into,
                    _ => varos_core::model::DropPos::After,
                };
                ed.execute(EditCommand::MoveLayer { sources: srcs, target, position });
            }
            Op::LayerMoveBoard(sources, source_board, target_board) => {
                ed.execute(EditCommand::MoveLayerToBoard { sources, source_board, target_board })
            }
            Op::AbEye(index) => ed.execute(EditCommand::ToggleArtboardHidden(index)),
            Op::AbLock(index) => ed.execute(EditCommand::ToggleArtboardLocked(index)),
            Op::LayerDupMove(srcs, target, zone) => {
                let position = match zone {
                    0 => varos_core::model::DropPos::Before,
                    1 => varos_core::model::DropPos::Into,
                    _ => varos_core::model::DropPos::After,
                };
                ed.execute(EditCommand::DuplicateMoveLayer { sources: srcs, target, position });
            }
            Op::Flip(horizontal) => ed.execute(EditCommand::Flip(horizontal)),
            Op::Align(mode, target) => ed.execute(EditCommand::Align { mode, target }),
            Op::Distribute(axis) => ed.execute(EditCommand::Distribute(axis)),
            Op::Bool(operation) => ed.execute(EditCommand::Boolean(operation)),
            Op::AbActive(index) => ed.execute(EditCommand::SetActiveArtboard(index)),
            Op::AbRect(index, x, y, width, height) => {
                ed.execute(EditCommand::SetArtboardRect { index, x, y, width, height })
            }
            Op::AbName(index, name) => ed.execute(EditCommand::RenameArtboard { index, name }),
            Op::AbColor(index, color) => ed.execute(EditCommand::SetArtboardColor { index, color }),
            Op::AbClip(index) => ed.execute(EditCommand::ToggleArtboardClip(index)),
            Op::AbOrient(index) => ed.execute(EditCommand::OrientArtboard(index)),
            Op::AbAdd => ed.execute(EditCommand::AddArtboard),
            Op::AbDup(index) => ed.execute(EditCommand::DuplicateArtboard(index)),
            Op::AbDel(index) => ed.execute(EditCommand::DeleteArtboard(index)),
            Op::AbCount(count) => ed.execute(EditCommand::SetArtboardCount(count)),
            Op::AbMoveArt(enabled) => ed.execute(EditCommand::SetMoveArtWithArtboard(enabled)),
            Op::RulerOrigin(Some(point)) => ed.execute(EditCommand::SetRulerOrigin(point)),
            Op::RulerOrigin(None) => ed.clear_ruler_origin_preview(),
            Op::GuidePreview(vertical, p) => ed.set_guide_preview(vertical, p),
            Op::GuideCommit => ed.execute(EditCommand::CommitGuide),
            // the field parsed with the same core checks, so a refusal here cannot happen; if it did, the
            // checked setter changes nothing (no undo step, not dirty)
            Op::BoardName(name) => {
                let _ = ed.try_set_board_name(&name);
            }
            Op::BoardDescription(text) => {
                let _ = ed.try_set_board_description(&text);
            }
            Op::BoardTags(tags) => {
                let _ = ed.try_set_board_tags(tags);
            }
            Op::CycleUnits => ed.execute(EditCommand::CycleUnits),
            // Applied after SetSnapConfig so the panel toggle is not clobbered by the frame snapshot.
            Op::ToggleSnapping => ed.execute(EditCommand::ToggleSnapping),
            Op::ToggleGuides => ed.toggle_guides_visibility(),
            Op::ToggleRulers => ed.toggle_rulers_visibility(),
            Op::Field(op) => apply_ops(ed, vec![*op]),
            Op::FieldPending(..) => {} // intercepted by `fields::finish_frame`
        }
    }
}

/// Dev-only: composite the rail to a PNG so the icon rasterization can be eyeballed without the
/// native window. `varos.exe --dump-tool-icons <path>`.
pub fn dump_tool_icons(path: &str) {
    let icons = [
        LEGACY_SELECT,
        LEGACY_DIRECT,
        LEGACY_PEN,
        LEGACY_RECT,
        LEGACY_ELLIPSE,
        LEGACY_TRIANGLE,
        LEGACY_EYE,
        LEGACY_ROTATE,
        LEGACY_OPACITY,
        LEGACY_STROKEW,
    ];
    let n = icons.len() as u32;
    let (pad, btn, gap, icon) = (7u32, 40u32, 4u32, 24u32);
    let w = btn + pad * 2;
    let h = pad * 2 + btn * n + gap * (n - 1);
    let panel = [0x1fu8, 0x1f, 0x22, 255];
    let accent = [0x0cu8, 0x8c, 0xe9, 255];
    let mut img = vec![0u8; (w * h * 4) as usize];
    for px in img.chunks_mut(4) {
        px.copy_from_slice(&panel);
    }
    for (i, svg) in icons.iter().enumerate() {
        let by = pad + i as u32 * (btn + gap);
        if i == 2 {
            for yy in by..by + btn {
                for xx in pad..pad + btn {
                    let o = ((yy * w + xx) * 4) as usize;
                    img[o..o + 4].copy_from_slice(&accent);
                }
            }
        }
        if let Some((rgba, iw, ih)) =
            varos_app::shell::svg::render_svg(&varos_app::shell::kit::icons::legacy_svg(svg, false), icon, false)
        {
            let (ox, oy) = (pad + (btn - iw) / 2, by + (btn - ih) / 2);
            for yy in 0..ih {
                for xx in 0..iw {
                    let si = ((yy * iw + xx) * 4) as usize;
                    let a = rgba[si + 3] as u32;
                    if a == 0 {
                        continue;
                    }
                    let di = (((oy + yy) * w + (ox + xx)) * 4) as usize;
                    for c in 0..3 {
                        img[di + c] = ((rgba[si + c] as u32 * a + img[di + c] as u32 * (255 - a)) / 255) as u8;
                    }
                    img[di + 3] = 255;
                }
            }
        }
    }
    if let Some(im) = image::RgbaImage::from_raw(w, h, img) {
        let _ = im.save(path);
    }
}
