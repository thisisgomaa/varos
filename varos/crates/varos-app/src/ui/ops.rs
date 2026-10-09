use super::*;

pub(crate) enum Op {
    View(varos_core::editor::view_commands::ViewAction),
    Zoom(f32),
    DocumentSetup(EditCommand),
    DocumentSetupLive(EditCommand, bool),
    DocumentSetupFinish,
    Tool(ToolKind),
    NewLayer(bool),
    DistributeMode(AlignMode),
    DistributeSpacing(DistAxis),
    DistributeGap(f32),
    SetBBox(Option<f32>, Option<f32>, Option<f32>, Option<f32>, f32, f32), // nx,ny,nw,nh + ref ax,ay
    SetRot(f32),
    SetOpacity(f32),
    SetStrokeW(f32),
    SetClipExempt(bool), // A30: release the selection from artboard clip (true) / re-clip it (false)
    Paint(PaintTarget, Option<Rgba>),
    PaintFocus(PaintTarget), // rail fill/stroke control: focus the target (X toggles)
    SwapColors,              // Shift+X
    DefaultPaint,            // D — existing tool default fill / stroke
    OpenMini(u32, egui::Rect),
    OpenPicker(MTarget), // double-click a swatch → open the Colour picker panel for it (a click only focuses)
    // Colour picker v3: transactions exist only during an active gesture.
    PickerBegin,
    PickerLive(MTarget, Rgba),
    PickerCommit(MTarget, Rgba),
    PickerSet(MTarget, Rgba), // K3 fields and swatches: one atomic gesture
    PickerFinish,
    PickerCancel,
    PickerClose, // UI notification; never reverts paint
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
    Align(AlignMode, AlignTarget),     // A4: carries the target the align resolves against
    Bool(varos_core::boolean::BoolOp), // Pathfinder home + the Properties "Shape" mirror
    // ---- artboard ops (i = artboard index) ----
    AbActive(usize),
    AbRect(usize, Option<f32>, Option<f32>, Option<f32>, Option<f32>), // x,y,w,h (each optional)
    AbName(usize, String),
    AbColorId(u32, Option<Rgba>),
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
    Units(varos_core::units::Unit), // one dropdown choice = one undoable document edit
    FitArtboard(usize),
    ToggleGuidesLock,
    ToggleSmartGuides,
    ToggleSnapPoint,
    ToggleSnapGrid, // mirror of the existing View-menu grid flag
    ToggleSnapping, // doc.snap.enabled master switch (a non-undoable mode flag, like the magnet menu)
    ToggleGuides,   // show/hide ruler guides — the guides-visibility view pref (mirrors Ctrl+;)
    ToggleRulers,   // show/hide rulers — the rulers view pref (mirrors Ctrl+R)
    // ---- K3 field law (`ui/fields.rs`) ----
    Field(Box<Op>),                  // a field's commit: applied before the frame's other ops
    FieldPending(egui::Id, Box<Op>), // what the open field would commit now (kept by `Ui`, never applied)
}
/// The end of a frame (`Ui::run`): the snapping flags the band edited this frame (the Windows
/// burger's View rows) are written back FIRST, then the panels' ops — so a panel's `ToggleSnapping`
/// in the same frame wins over the band's snapshot. A non-undoable mode flag (`SetSnapConfig`).
pub(crate) fn apply_frame(ed: &mut Editor, snap: varos_core::model::SnapConfig, ops: Vec<Op>) {
    ed.execute_ui(EditCommand::SetSnapConfig(snap));
    apply_ops(ed, ops);
}

/// Field commits are applied before opening/reseeding the picker transaction.
pub(crate) fn apply_picker_frame(
    ed: &mut Editor,
    snap: varos_core::model::SnapConfig,
    mut ops: Vec<Op>,
    modal: &mut Option<ColorPanel>,
) {
    // A release/close may coincide with a field blur. Finish the older live snapshot before
    // the field's atomic begin; a new gesture begun in this same frame instead follows the field.
    if ed.transaction_open()
        && !ops.iter().any(|op| matches!(op, Op::PickerBegin))
        && ops.iter().any(|op| matches!(op, Op::PickerCommit(..) | Op::PickerFinish | Op::PickerCancel))
    {
        let (mut endings, mut rest): (Vec<_>, Vec<_>) = ops.into_iter().partition(|op| {
            matches!(op, Op::PickerLive(..) | Op::PickerCommit(..) | Op::PickerFinish | Op::PickerCancel)
        });
        endings.append(&mut rest);
        ops = endings;
    }
    // An independent control/field action ends the active gesture before opening another step.
    if !ops.iter().any(|op| matches!(op, Op::PickerBegin))
        && ops.iter().any(|op| {
            !matches!(
                op,
                Op::PickerBegin
                    | Op::PickerLive(..)
                    | Op::PickerCommit(..)
                    | Op::PickerFinish
                    | Op::PickerCancel
                    | Op::PickerClose
                    | Op::FieldPending(..)
            )
        })
    {
        if let Some(m) = modal {
            let mut finish = vec![];
            m.finish(&mut finish);
            finish.append(&mut ops);
            ops = finish;
        }
    }
    let mut request = None;
    let mut mini_anchor = None;
    ops.retain(|op| match op {
        Op::OpenMini(id, anchor) => {
            request = Some(MTarget::Ab(*id));
            mini_anchor = Some(*anchor);
            false
        }
        Op::OpenPicker(t) => {
            mini_anchor = None;
            request = Some(*t);
            false
        }
        Op::PaintFocus(t) if modal.is_some() => {
            request = Some(MTarget::Paint(*t));
            true
        }
        _ => true,
    });
    apply_frame(ed, snap, ops);
    if let Some(target) = request {
        open_picker(modal, target, ed);
        if let Some(anchor) = mini_anchor {
            modal.as_mut().unwrap().config = picker::Config::Mini(anchor);
        }
    }
}

pub(crate) fn apply_ops(ed: &mut Editor, ops: Vec<Op>) {
    for op in ops {
        match op {
            Op::View(action) => ed.execute_ui(EditCommand::View(action)),
            Op::Zoom(value) => ed.execute_ui(EditCommand::ZoomPercent(value)),
            Op::Tool(t) => ed.set_tool(t),
            Op::NewLayer(sub) => ed.execute_ui(EditCommand::Object(if sub {
                varos_core::editor::wave::ObjectAction::NewSublayer
            } else {
                varos_core::editor::wave::ObjectAction::NewLayer
            })),
            Op::DistributeMode(mode) => ed.execute_ui(EditCommand::DistributeMode(mode)),
            Op::DistributeSpacing(axis) => {
                ed.execute_ui(EditCommand::DistributeSpacing { axis, gap: ed.distribute_gap })
            }
            Op::DistributeGap(gap) => ed.execute_ui(EditCommand::SetDistributeGap(gap)),
            Op::SetBBox(x, y, width, height, anchor_x, anchor_y) => {
                ed.execute_ui(EditCommand::SetObjectBounds { x, y, width, height, anchor_x, anchor_y })
            }
            Op::SetRot(degrees) => ed.execute_ui(EditCommand::SetObjectRotation(degrees)),
            Op::SetOpacity(opacity) => ed.execute_ui(EditCommand::SetOpacity(opacity)),
            Op::SetClipExempt(exempt) => ed.execute_ui(EditCommand::SetClipExempt(exempt)),
            Op::SetStrokeW(width) => ed.execute_ui(EditCommand::SetStrokeWidth(width)),
            Op::Paint(target, color) => ed.execute_ui(EditCommand::ApplyPaint { target, color }),
            Op::PaintFocus(target) => ed.set_paint_target(target),
            Op::SwapColors => ed.execute_ui(EditCommand::SwapColors),
            Op::DefaultPaint => ed.execute_ui(EditCommand::DefaultPaint),
            Op::OpenMini(..) | Op::OpenPicker(_) => {} // UI-only: taken out by apply_picker_frame (opens the panel after the frame); never reaches here
            Op::PickerLive(..) if !ed.transaction_open() => {}
            Op::PickerLive(target, color) => match target {
                MTarget::Paint(target) => ed.execute_ui(EditCommand::PickerLivePaint { target, color }),
                MTarget::Ab(id) => {
                    if let Some(index) = ed.doc.artboard_index(id) {
                        ed.execute_ui(EditCommand::PickerLiveArtboard { index, color });
                    }
                }
            },
            Op::PickerBegin => ed.picker_begin(),
            Op::PickerCommit(target, color) => {
                ed.execute_ui(EditCommand::PickerCommit {
                    current: match target {
                        MTarget::Paint(t) => Some(t),
                        MTarget::Ab(_) => None,
                    },
                    color,
                });
            }
            Op::PickerSet(target, color) => {
                let snap = Snap::read(ed);
                let (old, mixed) = match target {
                    MTarget::Paint(t) => (snap_target_color(&snap, t), snap.target_mixed(t)),
                    MTarget::Ab(id) => (ed.doc.artboards.iter().find(|a| a.id == id).and_then(|a| a.page_color), false),
                };
                if mixed || old.is_none_or(|c| c.iter().zip(color).any(|(a, b)| (a - b).abs() > 1e-6)) {
                    ed.picker_begin();
                    match target {
                        MTarget::Paint(t) => ed.paint_live(t, Some(color)),
                        MTarget::Ab(id) => {
                            if let Some(i) = ed.doc.artboard_index(id) {
                                ed.ab_color_live(i, Some(color));
                            }
                        }
                    }
                    ed.execute_ui(EditCommand::PickerCommit {
                        current: match target {
                            MTarget::Paint(t) => Some(t),
                            MTarget::Ab(_) => None,
                        },
                        color,
                    });
                }
            }
            Op::PickerFinish => ed.commit(),
            Op::PickerCancel => ed.execute_ui(EditCommand::PickerCancel),
            Op::PickerClose => {}
            Op::LayerSelectSet(nids) => ed.layer_select_set(&nids),
            Op::LayerToggle(n) => ed.layer_toggle(n),
            Op::LayerEye(node) => ed.execute_ui(EditCommand::ToggleNodeHidden(node)),
            Op::LayerLock(node) => ed.execute_ui(EditCommand::ToggleNodeLocked(node)),
            // a `<Path>` row shows `Path::name`, not its leaf node's name — rename what the row reads
            Op::LayerRename(node, name) => match ed.doc.node(node).map(|n| n.kind) {
                Some(varos_core::model::NodeKind::Path(path)) => ed.execute_ui(EditCommand::RenamePath { path, name }),
                _ => ed.execute_ui(EditCommand::RenameNode { node, name }),
            },
            Op::LayerGroup => ed.execute_ui(EditCommand::GroupSelection),
            Op::LayerDeleteSel => ed.execute_ui(EditCommand::DeleteLayerSelection),
            Op::LayerMove(srcs, target, zone) => {
                let position = match zone {
                    0 => varos_core::model::DropPos::Before,
                    1 => varos_core::model::DropPos::Into,
                    _ => varos_core::model::DropPos::After,
                };
                ed.execute_ui(EditCommand::MoveLayer { sources: srcs, target, position });
            }
            Op::LayerMoveBoard(sources, source_board, target_board) => {
                ed.execute_ui(EditCommand::MoveLayerToBoard { sources, source_board, target_board })
            }
            Op::AbEye(index) => ed.execute_ui(EditCommand::ToggleArtboardHidden(index)),
            Op::AbLock(index) => ed.execute_ui(EditCommand::ToggleArtboardLocked(index)),
            Op::LayerDupMove(srcs, target, zone) => {
                let position = match zone {
                    0 => varos_core::model::DropPos::Before,
                    1 => varos_core::model::DropPos::Into,
                    _ => varos_core::model::DropPos::After,
                };
                ed.execute_ui(EditCommand::DuplicateMoveLayer { sources: srcs, target, position });
            }
            Op::Flip(horizontal) => ed.execute_ui(EditCommand::Flip(horizontal)),
            Op::Align(mode, target) => ed.execute_ui(EditCommand::Align { mode, target }),
            Op::Bool(operation) => ed.execute_ui(EditCommand::Boolean(operation)),
            Op::AbActive(index) => ed.execute_ui(EditCommand::SetActiveArtboard(index)),
            Op::AbRect(index, x, y, width, height) => {
                ed.execute_ui(EditCommand::SetArtboardRect { index, x, y, width, height })
            }
            Op::AbName(index, name) => ed.execute_ui(EditCommand::RenameArtboard { index, name }),
            Op::AbColorId(id, color) => {
                if let Some(index) = ed.doc.artboard_index(id) {
                    ed.execute_ui(EditCommand::SetArtboardColor { index, color });
                }
            }
            Op::AbColor(index, color) => ed.execute_ui(EditCommand::SetArtboardColor { index, color }),
            Op::AbClip(index) => ed.execute_ui(EditCommand::ToggleArtboardClip(index)),
            Op::AbOrient(index) => ed.execute_ui(EditCommand::OrientArtboard(index)),
            Op::AbAdd => ed.execute_ui(EditCommand::AddArtboard),
            Op::AbDup(index) => ed.execute_ui(EditCommand::DuplicateArtboard(index)),
            Op::AbDel(index) => ed.execute_ui(EditCommand::DeleteArtboard(index)),
            Op::AbCount(count) => ed.execute_ui(EditCommand::SetArtboardCount(count)),
            Op::AbMoveArt(enabled) => ed.execute_ui(EditCommand::SetMoveArtWithArtboard(enabled)),
            Op::RulerOrigin(Some(point)) => ed.execute_ui(EditCommand::SetRulerOrigin(point)),
            Op::RulerOrigin(None) => ed.clear_ruler_origin_preview(),
            Op::GuidePreview(vertical, p) => ed.set_guide_preview(vertical, p),
            Op::GuideCommit => ed.execute_ui(EditCommand::CommitGuide),
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
            Op::DocumentSetupLive(command, begin) => {
                if begin {
                    ed.begin();
                }
                ed.execute_ui(command);
            }
            Op::DocumentSetupFinish => ed.finish_document_setup(),
            Op::DocumentSetup(command) => ed.execute_ui(command),
            Op::Units(unit) => ed.execute_ui(EditCommand::SetUnits(unit)),
            Op::FitArtboard(_) => {} // UI-only, intercepted by run
            Op::ToggleGuidesLock => ed.execute_ui(EditCommand::ToggleGuidesLocked),
            Op::ToggleSmartGuides => ed.execute_ui(EditCommand::ToggleSmartGuides),
            Op::ToggleSnapPoint | Op::ToggleSnapGrid => {
                let mut snap = ed.doc.snap;
                if matches!(op, Op::ToggleSnapPoint) {
                    snap.key_points = !snap.key_points;
                } else {
                    snap.grid = !snap.grid;
                }
                ed.execute_ui(EditCommand::SetSnapConfig(snap));
            }
            // Applied after SetSnapConfig so the panel toggle is not clobbered by the frame snapshot.
            Op::ToggleSnapping => ed.execute_ui(EditCommand::ToggleSnapping),
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
