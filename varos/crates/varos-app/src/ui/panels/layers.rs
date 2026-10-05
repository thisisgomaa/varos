use super::super::*;

pub(crate) struct TopIcons {
    /// Windows' burger. min/max/close are painted glyphs (`winctl`); the 4b band's Home, `+`, "+N",
    /// ×, Search glyphs come from the kit icon registry (`Icon`).
    pub(crate) menu: Option<egui::TextureHandle>,
}

// ───────────────────────────── layers panel snapshot ─────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum LKind {
    Board, // an ARTBOARD section header (derived from doc.artboards, not a scene node)
    Layer,
    Group,
    Path,
}

/// One drawable shape in a row's thumbnail — its outline rings (normalised into the row's COMBINED
/// bbox, 0..1, Y down) plus its own paint. A leaf has one; a Layer/Group stacks all its art (Ahmed:
/// the container thumbnail is a real mini-preview of everything inside, like Illustrator).
#[derive(Clone)]
pub(crate) struct ThumbShape {
    pub(crate) rings: Vec<Vec<Pt>>,
    pub(crate) fill: Option<Rgba>,
    pub(crate) stroke: Option<Rgba>,
}

/// One rendered row of the Layers panel (a flattened, display-ordered view of the scene tree,
/// SECTIONED by artboard — Ahmed 07-06: the panel splits by page, like Figma; membership is derived
/// from geometry via `node_boards`, mirror rows for straddlers, floaters loose at the bottom).
#[derive(Clone)]
pub(crate) struct LRow {
    pub(crate) id: u32, // node id; Board headers use u32::MAX - board index (never collides with node ids)
    pub(crate) depth: u16,
    pub(crate) kind: LKind,
    pub(crate) sec: u32, // section: the artboard index, u32::MAX = the floater strip (salts egui ids for mirrors)
    pub(crate) name: String,
    pub(crate) hidden: bool,
    pub(crate) locked: bool, // OWN flags (drive the toggle icons)
    pub(crate) eff_hidden: bool,
    pub(crate) eff_locked: bool, // cascaded (drive dimming + "forced" look)
    pub(crate) has_children: bool,
    pub(crate) collapsed: bool,
    pub(crate) selected: bool,         // any of the row's art is selected on canvas
    pub(crate) full_sel: bool,         // ALL of the row's art is selected (children read this off their parent)
    pub(crate) drag_sel: bool,         // top-most fully-selected row — the unit a multi-row drag picks up
    pub(crate) active: bool,           // the active (target) layer / the active artboard on Board headers
    pub(crate) thumb: Vec<ThumbShape>, // real mini-preview, back→front; empty = no art (blank box)
}

pub(crate) struct LayerRowsCache {
    pub(crate) key: u64,
    pub(crate) rows: Vec<LRow>,
}

pub(crate) struct ThumbCacheEntry {
    pub(crate) key: u64,
    pub(crate) shapes: Vec<ThumbShape>,
}

pub(crate) fn hash_f32(value: f32, state: &mut impl Hasher) {
    value.to_bits().hash(state);
}

pub(crate) fn layer_rows_key(ed: &Editor, collapsed: &std::collections::HashSet<u32>, search: &str) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    ed.rev.hash(&mut state);
    ed.dirty.hash(&mut state);
    ed.doc.active.hash(&mut state);
    ed.doc.active_layer.hash(&mut state);
    for node in &ed.doc.nodes {
        node.id.hash(&mut state);
        std::mem::discriminant(&node.kind).hash(&mut state);
        node.name.hash(&mut state);
        node.parent.hash(&mut state);
        node.children.hash(&mut state);
        node.hidden.hash(&mut state);
        node.locked.hash(&mut state);
    }
    for board in &ed.doc.artboards {
        board.name.hash(&mut state);
        for value in [board.x, board.y, board.w, board.h] {
            hash_f32(value, &mut state);
        }
        board.hidden.hash(&mut state);
        board.locked.hash(&mut state);
    }
    let mut ids: Vec<u32> = ed.objsel.iter().copied().collect();
    ids.sort_unstable();
    ids.hash(&mut state);
    ids.clear();
    ids.extend(ed.selected.iter().copied());
    ids.sort_unstable();
    ids.hash(&mut state);
    let mut folded: Vec<u32> = collapsed.iter().copied().collect();
    folded.sort_unstable();
    folded.hash(&mut state);
    search.hash(&mut state);
    if ed.dirty {
        hash_f32(ed.cursor[0], &mut state);
        hash_f32(ed.cursor[1], &mut state);
    }
    state.finish()
}

pub(crate) fn thumb_key(ed: &Editor, pids_zorder: &[u32]) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    for &pid in pids_zorder {
        pid.hash(&mut state);
        let Some(pi) = ed.doc.pidx(pid) else { continue };
        let path = &ed.doc.paths[pi];
        path.closed.hash(&mut state);
        for paint in [path.fill.solid(), path.stroke.solid()] {
            paint.is_some().hash(&mut state);
            if let Some(color) = paint {
                color.into_iter().for_each(|channel| hash_f32(channel, &mut state));
            }
        }
        hash_f32(path.stroke_width, &mut state);
        hash_f32(path.opacity, &mut state);
        for anchor in path.anchors.iter().chain(path.holes.iter().flatten()) {
            anchor.id.hash(&mut state);
            hash_f32(anchor.p[0], &mut state);
            hash_f32(anchor.p[1], &mut state);
            for handle in [anchor.hin, anchor.hout] {
                handle.is_some().hash(&mut state);
                if let Some(handle) = handle {
                    hash_f32(handle[0], &mut state);
                    hash_f32(handle[1], &mut state);
                }
            }
        }
    }
    state.finish()
}

/// Auto-name for a leaf path (Illustrator angle-bracket style) unless the user renamed it.
pub(crate) fn path_auto_name(p: &varos_core::model::Path) -> String {
    p.name.clone().unwrap_or_else(|| "<Path>".into())
}

/// Flatten the scene tree into display rows (roots front-first, pre-order; collapsed subtrees skipped).
/// `search` (lowercased) keeps only matching rows + their ancestors. Thumbs are unit-square outlines.
/// Board-header sentinel row id (u32::MAX - board index) — node ids are small sequential, never near MAX.
pub(crate) fn board_row_id(bi: usize) -> u32 {
    u32::MAX - bi as u32
}
pub(crate) fn build_layer_rows(
    ed: &Editor,
    collapsed: &std::collections::HashSet<u32>,
    search: &str,
    thumb_cache: &mut std::collections::HashMap<u32, ThumbCacheEntry>,
) -> Vec<LRow> {
    use varos_core::model::NodeKind;
    let q = search.trim().to_lowercase();
    let mut rows: Vec<LRow> = Vec::new();
    let mut parent: Vec<Option<usize>> = Vec::new(); // parallel: each row's parent ROW index

    #[allow(clippy::too_many_arguments)] // recursive row emitter: tree cursor + section + two out-params
    fn walk(
        ed: &Editor,
        nid: u32,
        depth: u16,
        sec: u32,
        sf: (bool, bool), // the SECTION's board (hidden, locked) — dims/forces this instance's rows
        par: Option<usize>,
        collapsed: &std::collections::HashSet<u32>,
        thumb_cache: &mut std::collections::HashMap<u32, ThumbCacheEntry>,
        rows: &mut Vec<LRow>,
        parent: &mut Vec<Option<usize>>,
    ) {
        let Some(n) = ed.doc.node(nid) else { return };
        let anc_hidden = || {
            let mut c = n.parent;
            while let Some(i) = c {
                let Some(x) = ed.doc.node(i) else { break };
                if x.hidden {
                    return true;
                }
                c = x.parent;
            }
            false
        };
        let anc_locked = || {
            let mut c = n.parent;
            while let Some(i) = c {
                let Some(x) = ed.doc.node(i) else { break };
                if x.locked {
                    return true;
                }
                c = x.parent;
            }
            false
        };
        let (kind, name) = match n.kind {
            NodeKind::Layer => (LKind::Layer, n.name.clone()),
            NodeKind::Group => (LKind::Group, if n.name.is_empty() { "<Group>".into() } else { n.name.clone() }),
            NodeKind::Path(pid) => (
                LKind::Path,
                ed.doc.pidx(pid).map(|pi| path_auto_name(&ed.doc.paths[pi])).unwrap_or_else(|| "<Path>".into()),
            ),
        };
        // thumbnail = every path under this node, composited in z-order (back→front) into one bbox —
        // a leaf shows itself; a Layer/Group shows a true preview of its contents.
        let mut paths = ed.doc.node_paths(nid);
        paths.sort_by_key(|pid| ed.doc.pidx(*pid).unwrap_or(usize::MAX)); // back→front z
        let key = thumb_key(ed, &paths);
        let thumb = match thumb_cache.get(&nid) {
            Some(entry) if entry.key == key => entry.shapes.clone(),
            _ => {
                let shapes = thumb_shapes(ed, &paths);
                thumb_cache.insert(nid, ThumbCacheEntry { key, shapes: shapes.clone() });
                shapes
            }
        };
        let full_sel = !paths.is_empty() && paths.iter().all(|p| ed.objsel.contains(p));
        // the top-most fully-selected row is the multi-drag unit (its parent isn't fully selected)
        let drag_sel = full_sel && !par.map(|pi| rows[pi].full_sel).unwrap_or(false);
        rows.push(LRow {
            id: nid,
            depth,
            kind,
            sec,
            name,
            hidden: n.hidden,
            locked: n.locked,
            eff_hidden: n.hidden || anc_hidden() || sf.0,
            eff_locked: n.locked || anc_locked() || sf.1,
            has_children: !n.children.is_empty(),
            collapsed: collapsed.contains(&nid),
            selected: paths.iter().any(|p| ed.objsel.contains(p)),
            full_sel,
            drag_sel,
            active: nid == ed.doc.active_layer,
            thumb,
        });
        parent.push(par);
        let me = rows.len() - 1;
        if !collapsed.contains(&nid) {
            for &c in &n.children {
                walk(ed, c, depth + 1, sec, sf, Some(me), collapsed, thumb_cache, rows, parent);
            }
        }
    }
    // The TOP-LEVEL items (the implicit root Layer stays a model-only container — 07-03 pivot; a
    // non-Layer legacy root counts as an item itself).
    let mut top: Vec<u32> = Vec::new();
    for &r in &ed.doc.roots {
        match ed.doc.node(r) {
            Some(n) if matches!(n.kind, NodeKind::Layer) => top.extend(n.children.iter().copied()),
            _ => top.push(r),
        }
    }
    // SECTIONS BY ARTBOARD (Ahmed 07-06, Figma-style): one header per board; a top-level item lists
    // under every board its subtree stands on (mirror rows for straddlers — same object, same state);
    // items on no board float LOOSE at the bottom, under no header — visibly outside every page and
    // outside export.
    let memb: Vec<(u32, Vec<usize>)> = top.iter().map(|&nid| (nid, ed.doc.node_boards(nid))).collect();
    for (bi, ab) in ed.doc.artboards.iter().enumerate() {
        let hid = board_row_id(bi);
        let members: Vec<u32> = memb.iter().filter(|(_, bs)| bs.contains(&bi)).map(|(n, _)| *n).collect();
        rows.push(LRow {
            id: hid,
            depth: 0,
            kind: LKind::Board,
            sec: bi as u32,
            name: ab.name.clone(),
            hidden: ab.hidden,
            locked: ab.locked,
            eff_hidden: ab.hidden,
            eff_locked: ab.locked,
            has_children: !members.is_empty(),
            collapsed: collapsed.contains(&hid),
            selected: false,
            full_sel: false,
            drag_sel: false,
            active: bi == ed.doc.active,
            thumb: vec![],
        });
        parent.push(None);
        let me = rows.len() - 1;
        if !collapsed.contains(&hid) {
            for nid in members {
                walk(
                    ed,
                    nid,
                    1,
                    bi as u32,
                    (ab.hidden, ab.locked),
                    Some(me),
                    collapsed,
                    thumb_cache,
                    &mut rows,
                    &mut parent,
                );
            }
        }
    }
    for (nid, bs) in &memb {
        if bs.is_empty() {
            walk(ed, *nid, 0, u32::MAX, (false, false), None, collapsed, thumb_cache, &mut rows, &mut parent);
        }
    }

    thumb_cache.retain(|nid, _| ed.doc.node(*nid).is_some());

    if q.is_empty() {
        return rows;
    }
    // keep matches + all their ancestors (so hierarchy stays readable)
    let mut keep = vec![false; rows.len()];
    for i in 0..rows.len() {
        if rows[i].name.to_lowercase().contains(&q) {
            keep[i] = true;
            let mut p = parent[i];
            while let Some(pi) = p {
                keep[pi] = true;
                p = parent[pi];
            }
        }
    }
    rows.into_iter().zip(keep).filter(|(_, k)| *k).map(|(r, _)| r).collect()
}
/// One path's raw thumbnail ingredients before bbox-fitting: `(rings, fill, stroke)`.
type RawThumb = (Vec<Vec<Pt>>, Option<Rgba>, Option<Rgba>);
/// Build a row's thumbnail: gather every path (already in back→front z order), collect its outline
/// rings + paint in pixel space, then fit the ONE combined bbox to the unit square (Y down, shorter
/// axis centred) so the composite preview keeps each shape's real position, size and colour.
pub(crate) fn thumb_shapes(ed: &Editor, pids_zorder: &[u32]) -> Vec<ThumbShape> {
    let mut raw: Vec<RawThumb> = Vec::new();
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for &pid in pids_zorder {
        let Some(pi) = ed.doc.pidx(pid) else { continue };
        let p = &ed.doc.paths[pi];
        let mut rings = vec![ed.doc.outline_px(pi, 1.0)];
        for h in &p.holes {
            rings.push(varos_core::model::Document::ring_px(h, true, 1.0));
        }
        for r in &rings {
            for q in r {
                x0 = x0.min(q[0]);
                y0 = y0.min(q[1]);
                x1 = x1.max(q[0]);
                y1 = y1.max(q[1]);
            }
        }
        raw.push((rings, p.fill.solid(), p.stroke.solid())); // Paint → the UI snapshot's Option<Rgba>
    }
    if raw.is_empty() {
        return vec![];
    }
    let (w, h) = ((x1 - x0).max(1e-3), (y1 - y0).max(1e-3));
    let s = 1.0 / w.max(h);
    let (ox, oy) = ((1.0 - w * s) * 0.5, (1.0 - h * s) * 0.5); // centre the shorter axis
    raw.into_iter()
        .map(|(rings, fill, stroke)| ThumbShape {
            rings: rings
                .into_iter()
                .map(|r| r.into_iter().map(|q| [ox + (q[0] - x0) * s, oy + (q[1] - y0) * s]).collect())
                .collect(),
            fill,
            stroke,
        })
        .collect()
}

// ───────────────────────────── inspector dock ─────────────────────────────

pub(crate) struct DockIcons<'a> {
    pub(crate) rotate: &'a Option<egui::TextureHandle>,
    pub(crate) opacity: &'a Option<egui::TextureHandle>,
    pub(crate) strokew: &'a Option<egui::TextureHandle>,
    pub(crate) align: &'a [Option<egui::TextureHandle>; 8],
}

/// A reveal-on-hover column toggle (eye / lock). `marked` = the persistent state that always shows its
/// glyph (hidden / locked); otherwise the glyph appears only when the row is hovered. `forced` = the
/// state is inherited from an ancestor (drawn dim, the "you can't change it here" cue). Returns clicked.
#[allow(clippy::too_many_arguments)] // hand-painted widget: geometry + behaviour knobs, split deferred with ui.rs
pub(crate) fn col_toggle(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    row_hovered: bool,
    marked: bool,
    forced: bool,
    marked_tex: &Option<egui::TextureHandle>,
    hint_tex: &Option<egui::TextureHandle>,
    tip: &str,
) -> bool {
    let resp = ui.interact(rect, ui.id().with(("col", rect.left() as i32, rect.top() as i32)), egui::Sense::click());
    if marked || forced || row_hovered {
        let (tex, col) = if marked || forced {
            (marked_tex, if forced && !marked { Color32::from_gray(74) } else { TEXT })
        } else {
            (hint_tex, MUTED)
        };
        if let Some(t) = tex {
            ui.painter().image(
                t.id(),
                egui::Rect::from_center_size(rect.center(), egui::Vec2::splat(ICON_SM)),
                UV01(),
                col,
            );
        }
    }
    resp.on_hover_text(tip).clicked()
}

/// The Layers panel — the SIMPLE (Photoshop/Affinity) VIEW of the scene tree (07-03 pivot), docked UNDER
/// the inspector (`dock_below`) and growing downward. Row = eye · lock · disclosure · thumbnail · name.
/// Click=select · Ctrl=toggle · Shift=range · dbl or right-click ▸ Rename=rename · drag=reorder/nest ·
/// Alt+drag=duplicate.
/// Header: title + search. Footer: Group · Delete.
#[allow(clippy::too_many_arguments)] // hand-painted panel builder: each arg is live UI state, split deferred with ui.rs
pub(crate) fn panel_layers(
    ui: &mut egui::Ui,
    rows: &[LRow],
    ic: &LayerIcons,
    search: &mut String,
    rename: &mut Option<(u32, String)>,
    collapsed: &mut std::collections::HashSet<u32>,
    drag: &mut Option<(u32, u32)>,
    anchor: &mut Option<(u32, u32)>,
    ops: &mut Vec<Op>,
) {
    // intersect with the clip rect: the shell box may hand us a ui taller than what is actually
    // VISIBLE — sizing the list from max_rect alone pushed the footer below the box edge and the
    // Group/Delete icons drew clipped (Ahmed 2026-07-11, twice).
    let pane = ui.max_rect().intersect(ui.clip_rect());
    let w = pane.width();
    // columns: eye · lock · [disclosure · thumb · name]. No identity bar, no target/select gutter.
    let (eye_w, lock_w) = (26.0, 22.0);
    let body_x0 = eye_w + lock_w + 8.0;
    {
        {
            let hairline = |ui: &mut egui::Ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
                ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, BORDER));
            };
            // ── header: search only (the box header already carries the "Layers" title) ──
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(11.0);
                let (sr, _) = ui.allocate_exact_size(egui::vec2(w - 22.0, 26.0), egui::Sense::hover());
                ui.painter().rect(sr, CornerRadius::same(R), BG_SURFACE, Stroke::new(1.0, BORDER), StrokeKind::Middle);
                if let Some(t) = &ic.search {
                    ui.painter().image(
                        t.id(),
                        egui::Rect::from_center_size(
                            egui::pos2(sr.left() + 14.0, sr.center().y),
                            egui::Vec2::splat(ICON_SM),
                        ),
                        UV01(),
                        MUTED,
                    );
                }
                let at = egui::Rect::from_min_max(egui::pos2(sr.left() + 28.0, sr.top()), sr.max);
                fields::search(ui, at.shrink2(egui::vec2(2.0, 3.0)), search);
            });
            ui.add_space(8.0);
            hairline(ui);

            // ── the row list ──
            // measure the space that ACTUALLY remains below the header (never assume its height),
            // and reserve the footer from the VISIBLE bottom: hairline 1 + space 3 + row 24 + breath 6.
            let list_h = (pane.bottom() - ui.cursor().top() - 34.0).max(60.0);
            let row_h = 26.0;
            // drag bookkeeping — ROW drag only (reorder / nest). Grabbing a row that is part of the
            // (fully-selected) multi-selection lifts the WHOLE selection: its top-most fully-selected
            // rows travel together in panel order (Photoshop). An unselected/partial row lifts alone.
            // forbidden = every payload node + its whole subtree; the model re-guards. Alt = duplicate.
            let ptr = ui.input(|i| i.pointer.interact_pos());
            let payload: Vec<u32> = drag
                .map(|(s, _)| {
                    if rows.iter().any(|r| r.id == s && r.drag_sel) {
                        let mut seen = std::collections::HashSet::new();
                        rows.iter().filter(|r| r.drag_sel && seen.insert(r.id)).map(|r| r.id).collect()
                    } else {
                        vec![s]
                    }
                })
                .unwrap_or_default();
            let src_is_layer = payload.iter().any(|&s| rows.iter().any(|r| r.id == s && r.kind == LKind::Layer));
            let forbidden: std::collections::HashSet<u32> = {
                let mut set = std::collections::HashSet::new();
                for &s in &payload {
                    if let Some(si) = rows.iter().position(|r| r.id == s) {
                        set.insert(s);
                        let sd = rows[si].depth;
                        for r in &rows[si + 1..] {
                            if r.depth > sd {
                                set.insert(r.id);
                            } else {
                                break;
                            }
                        }
                    }
                }
                set
            };
            let mut drop_ind: Option<(u32, u8, egui::Rect, u16)> = None; // target id, zone, rect, depth
                                                                         // scroll by wheel + bar only — NEVER by dragging content (that would fight our row drag-drop)
            let scroll_src = egui::scroll_area::ScrollSource {
                scroll_bar: true,
                drag: egui::scroll_area::DragScroll::Never,
                mouse_wheel: true,
            };
            egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, false]).scroll_source(scroll_src).show(
                ui,
                |ui| {
                    if rows.is_empty() {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::hover());
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            if search.trim().is_empty() { "No layers yet" } else { "No matching layers" },
                            FontId::proportional(12.0),
                            MUTED,
                        );
                    }
                    // last top-level row + last drawn rect — "drop below the list = send to the bottom"
                    let (mut last_top, mut last_rect) = (None::<u32>, None::<egui::Rect>);
                    let mut prev_sec = 0u32;
                    let mut rename_shown = false; // mirror rows: only the first instance opens the editor
                    for (ri, row) in rows.iter().enumerate() {
                        // the floater strip (art on NO board — outside export) separates with a hairline
                        if ri > 0 && row.sec == u32::MAX && prev_sec != u32::MAX {
                            ui.add_space(4.0);
                            let (hr, _) = ui.allocate_exact_size(egui::vec2(w, 1.0), egui::Sense::hover());
                            ui.painter().hline(hr.x_range(), hr.center().y, Stroke::new(1.0, BORDER));
                            ui.add_space(4.0);
                        }
                        prev_sec = row.sec;
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, row_h), egui::Sense::click_and_drag());
                        if resp.drag_started() && row.kind != LKind::Board {
                            *drag = Some((row.id, row.sec));
                            // a drag is not a click — drop any half-built manual double-click
                            let dc_id = doc_id(ui, "lay-last-click");
                            ui.data_mut(|d| d.remove::<(u32, u32, f64)>(dc_id));
                        }
                        // decide the drop zone. SAME section: top third = Before, bottom third = After,
                        // middle = Into (containers only; a Layer can't nest into a Group); leaves halve.
                        // ANOTHER board's section (its header or any row) = zone 3: move the art onto
                        // that page spatially — drop_ind.0 then holds the TARGET SECTION, not a node id.
                        if let (Some((_, ssec)), Some(pp)) = (*drag, ptr) {
                            if !payload.contains(&row.id) && rect.contains(pp) {
                                if row.sec != ssec && row.sec != u32::MAX {
                                    drop_ind = Some((row.sec, 3, rect, row.depth));
                                } else if row.sec == ssec && row.kind != LKind::Board && !forbidden.contains(&row.id) {
                                    let f = ((pp.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                                    let into_ok =
                                        row.kind != LKind::Path && !(src_is_layer && row.kind == LKind::Group);
                                    let zone = if into_ok {
                                        if f < 0.30 {
                                            0
                                        } else if f > 0.70 {
                                            2
                                        } else {
                                            1
                                        }
                                    } else if f < 0.5 {
                                        0
                                    } else {
                                        2
                                    };
                                    drop_ind = Some((row.id, zone, rect, row.depth));
                                }
                            }
                        }
                        let p = ui.painter_at(rect);
                        let dim = if row.eff_hidden { 0.42 } else { 1.0 };
                        let hov = resp.hovered();
                        // state: SELECTED (its art is in the canvas selection) is the strong highlight; the
                        // active layer keeps a subtle accent edge even when nothing on it is selected.
                        // Board headers read as a SECTION strip; the active board keeps the accent edge.
                        if row.kind == LKind::Board {
                            p.rect_filled(rect, CornerRadius::ZERO, if hov { ROW_HOVER } else { BG_SURFACE });
                            if row.active {
                                p.rect_filled(
                                    egui::Rect::from_min_size(rect.min, egui::vec2(2.0, row_h)),
                                    CornerRadius::ZERO,
                                    ACCENT,
                                );
                            }
                        } else if row.selected {
                            p.rect_filled(rect, CornerRadius::ZERO, ACCENT_TINT);
                            p.rect_filled(
                                egui::Rect::from_min_size(rect.min, egui::vec2(2.0, row_h)),
                                CornerRadius::ZERO,
                                ACCENT,
                            );
                        } else if hov {
                            p.rect_filled(rect, CornerRadius::ZERO, ROW_HOVER);
                        } else if row.active {
                            p.rect_filled(
                                egui::Rect::from_min_size(rect.min, egui::vec2(2.0, row_h)),
                                CornerRadius::ZERO,
                                with_a(ACCENT, 0.5),
                            );
                        }
                        // eye + lock (reveal on hover; hidden/locked persist). On a Board header they
                        // act on the whole PAGE (piece C): hide the board + its art / lock its art.
                        {
                            let eye = egui::Rect::from_min_size(rect.min, egui::vec2(eye_w, row_h));
                            let lok =
                                egui::Rect::from_min_size(rect.min + egui::vec2(eye_w, 0.0), egui::vec2(lock_w, row_h));
                            let board = row.kind == LKind::Board;
                            if col_toggle(
                                ui,
                                eye,
                                hov,
                                row.hidden,
                                row.eff_hidden && !row.hidden,
                                &ic.eye_off,
                                &ic.eye,
                                if board { "Show/Hide board" } else { "Show/Hide" },
                            ) {
                                ops.push(if board { Op::AbEye(row.sec as usize) } else { Op::LayerEye(row.id) });
                            }
                            if col_toggle(
                                ui,
                                lok,
                                hov,
                                row.locked,
                                row.eff_locked && !row.locked,
                                &ic.lock,
                                &ic.unlock,
                                if board { "Lock/Unlock board" } else { "Lock/Unlock" },
                            ) {
                                ops.push(if board { Op::AbLock(row.sec as usize) } else { Op::LayerLock(row.id) });
                            }
                        }
                        // indent guides (~6% white) + disclosure
                        for lvl in 0..row.depth {
                            p.vline(
                                rect.left() + body_x0 + lvl as f32 * 14.0 + 2.0,
                                rect.y_range(),
                                Stroke::new(1.0, Color32::from_white_alpha(16)),
                            );
                        }
                        let mut x = rect.left() + body_x0 + row.depth as f32 * 14.0;
                        if row.has_children {
                            let c = egui::pos2(x + 3.0, rect.center().y);
                            let pts = if row.collapsed {
                                vec![c + egui::vec2(-3.0, -4.0), c + egui::vec2(3.0, 0.0), c + egui::vec2(-3.0, 4.0)]
                            } else {
                                vec![c + egui::vec2(-4.0, -2.5), c + egui::vec2(4.0, -2.5), c + egui::vec2(0.0, 3.5)]
                            };
                            p.add(egui::Shape::convex_polygon(pts, with_a(MUTED, dim), Stroke::NONE));
                            if ui
                                .interact(
                                    egui::Rect::from_center_size(c, egui::vec2(16.0, row_h)),
                                    ui.id().with(("disc", row.id)),
                                    egui::Sense::click(),
                                )
                                .clicked()
                            {
                                if row.collapsed {
                                    collapsed.remove(&row.id);
                                } else {
                                    collapsed.insert(row.id);
                                }
                            }
                        }
                        x += 14.0;
                        // thumbnail — one path or a whole container, composited as a real mini-preview
                        // (Ahmed: no more folder chip; the thin identity bar carries "which layer"). Empty
                        // containers/paths draw nothing — the bar alone identifies them.
                        let thumb =
                            egui::Rect::from_min_size(egui::pos2(x, rect.center().y - 9.0), egui::vec2(18.0, 18.0));
                        if !row.thumb.is_empty() {
                            let translucent = row.thumb.iter().any(|s| s.fill.is_none_or(|c| c[3] < 0.999));
                            if translucent {
                                checker(&p, thumb, 4.5);
                            }
                            p.rect(
                                thumb,
                                CornerRadius::same(2),
                                if translucent { Color32::TRANSPARENT } else { Color32::from_gray(24) },
                                Stroke::new(1.0, with_a(BORDER_2, 0.8)),
                                StrokeKind::Middle,
                            );
                            for sh in &row.thumb {
                                let fill = sh.fill.map(|c| with_a(rgba_c32a(c), dim)).unwrap_or(Color32::TRANSPARENT);
                                let stroke = sh
                                    .stroke
                                    .map(|c| with_a(rgba_c32a(c), dim))
                                    .unwrap_or(Color32::from_gray((160.0 * dim) as u8));
                                for ring in &sh.rings {
                                    if ring.len() >= 3 {
                                        let pts: Vec<egui::Pos2> =
                                            ring.iter().map(|q| thumb.min + egui::vec2(q[0], q[1]) * 18.0).collect();
                                        p.add(egui::Shape::convex_polygon(pts, fill, Stroke::new(1.0, stroke)));
                                    }
                                }
                            }
                        }
                        x += if row.kind == LKind::Board { 6.0 } else { 24.0 };
                        // name (auto-names muted; user/layer names bright) — or inline rename
                        let name_rect = egui::Rect::from_min_max(
                            egui::pos2(x, rect.top()),
                            egui::pos2(rect.right() - 10.0, rect.bottom()),
                        );
                        let renaming = !rename_shown && rename.as_ref().is_some_and(|(id, _)| *id == row.id);
                        if renaming {
                            rename_shown = true;
                            // Illustrator's inline rename (QW3) under the K3 law (`fields::rename`). The field's
                            // id is explicit (never an auto id that shifts with what the rows above allocate).
                            let te_id = doc_id(ui, ("lay-rename", row.id));
                            let (board, id) = (row.kind == LKind::Board, row.id);
                            let mk = |v| if board { Op::AbName(row.sec as usize, v) } else { Op::LayerRename(id, v) };
                            if fields::rename(
                                ui,
                                te_id,
                                name_rect.shrink2(egui::vec2(2.0, 4.0)),
                                &row.name,
                                12.5,
                                false,
                                ops,
                                mk,
                            ) {
                                *rename = None;
                            }
                        } else {
                            let auto = row.name.starts_with('<');
                            let (size, base) = match row.kind {
                                LKind::Board => (12.5, TEXT),
                                LKind::Layer => (12.5, TEXT),
                                _ if auto => (12.0, MUTED),
                                _ => (12.0, Color32::from_gray(208)),
                            };
                            let s = elide(&row.name, name_rect.width(), size);
                            p.text(
                                egui::pos2(name_rect.left(), rect.center().y),
                                Align2::LEFT_CENTER,
                                s,
                                FontId::proportional(size),
                                with_a(base, dim),
                            );
                        }
                        // selection — click / Ctrl-toggle / Shift-range act on the ROW (the 07-03 bug fix).
                        // A Board header click makes that board ACTIVE instead (new art lands there).
                        if resp.clicked() && !renaming {
                            if row.kind == LKind::Board {
                                ops.push(Op::AbActive(row.sec as usize));
                            } else {
                                let (ctrl, shift) =
                                    ui.input(|i| (i.modifiers.command || i.modifiers.ctrl, i.modifiers.shift));
                                if ctrl {
                                    ops.push(Op::LayerToggle(row.id));
                                    *anchor = Some((row.id, row.sec));
                                } else if shift {
                                    // resolve the anchor to its OWN section's appearance — a mirror
                                    // row's id exists in several sections and the first-by-id match
                                    // ranges from the wrong one (07-06 review fix #2). Fallback: the
                                    // first appearance (membership changed), else the clicked row.
                                    let a = anchor
                                        .and_then(|(aid, asec)| {
                                            rows.iter()
                                                .position(|r| r.id == aid && r.sec == asec)
                                                .or_else(|| rows.iter().position(|r| r.id == aid))
                                        })
                                        .unwrap_or(ri);
                                    let (lo, hi) = if a <= ri { (a, ri) } else { (ri, a) };
                                    ops.push(Op::LayerSelectSet(
                                        rows[lo..=hi].iter().filter(|r| r.kind != LKind::Board).map(|r| r.id).collect(),
                                    ));
                                } else {
                                    ops.push(Op::LayerSelectSet(vec![row.id]));
                                    *anchor = Some((row.id, row.sec));
                                }
                            }
                        }
                        // dbl = rename. egui's own double_clicked() can miss on click_and_drag rows
                        // (the first press may classify as a drag start) — so we ALSO detect it by
                        // hand: two clicks on the SAME row within 0.4s (P4, Ahmed 2026-07-11).
                        // keyed on (id, SEC): a straddler's mirror rows share the id across board
                        // sections — two single clicks on two mirrors must not read as a double-click.
                        let manual_dbl = resp.clicked() && !renaming && {
                            let dc_id = doc_id(ui, "lay-last-click");
                            let now = ui.input(|i| i.time);
                            let last: Option<(u32, u32, f64)> = ui.data(|d| d.get_temp(dc_id));
                            ui.data_mut(|d| d.insert_temp(dc_id, (row.id, row.sec, now)));
                            last.is_some_and(|(id, sec, t)| id == row.id && sec == row.sec && now - t < 0.4)
                        };
                        if (resp.double_clicked() && !renaming) || manual_dbl {
                            *rename = Some((row.id, row.name.clone()));
                        }
                        // the name cell says how to rename it; right-click offers the same editor (Astra
                        // F10: nothing on the row hinted at the double-click, right-click did nothing)
                        if !renaming && resp.hovered() && ptr.is_some_and(|pp| name_rect.contains(pp)) {
                            resp.clone().on_hover_text("Double-click to rename");
                        }
                        let menu_id = ui.id().with(("lay-menu", row.id, row.sec));
                        if resp.secondary_clicked() && !renaming {
                            menu_set(ui, menu_id, true);
                        }
                        menu_below(ui, menu_id, &resp, None, |ui| {
                            ui.set_width(160.0);
                            if menu_row(ui, "Rename", "") {
                                *rename = Some((row.id, row.name.clone()));
                                menu_set(ui, menu_id, false);
                            }
                        });
                        // the lifted rows read as "picked up" — the whole payload dims while dragged
                        // (a mirror dims on BOTH appearances — it IS the same object)
                        if drag.is_some() && payload.contains(&row.id) {
                            p.rect_filled(rect, CornerRadius::ZERO, Color32::from_black_alpha(120));
                        }
                        // a droppable "top-level item": a member row directly under a header (depth 1)
                        // or a loose floater (depth 0) — same-section rule as everywhere else
                        let is_top_item = row.kind != LKind::Board
                            && ((row.sec == u32::MAX && row.depth == 0) || (row.sec != u32::MAX && row.depth == 1));
                        if is_top_item && drag.is_some_and(|(_, ssec)| ssec == row.sec) {
                            last_top = Some(row.id);
                        }
                        last_rect = Some(rect);
                    }
                    // below the last row = drop at the very bottom of the stack (the Photoshop feel)
                    if drag.is_some() && drop_ind.is_none() {
                        if let (Some(pp), Some(tid), Some(lr)) = (ptr, last_top, last_rect) {
                            if !forbidden.contains(&tid) && pp.y > lr.bottom() && ui.clip_rect().contains(pp) {
                                drop_ind = Some((tid, 2, lr, 0));
                            }
                        }
                    }
                    // ── drop indicator: a nest box for Into, an indented ACCENT line for Before/After ──
                    if drag.is_some() {
                        if let Some((_, zone, trect, depth)) = drop_ind {
                            let dp = ui.painter();
                            if zone == 1 || zone == 3 {
                                // Into a container / onto ANOTHER board's section — the same "lands
                                // inside this" ring (zone 3 rows read as "move to that page")
                                dp.rect(
                                    trect.shrink(1.5),
                                    CornerRadius::same(3),
                                    Color32::TRANSPARENT,
                                    Stroke::new(2.0, ACCENT),
                                    StrokeKind::Inside,
                                );
                            } else {
                                let y = if zone == 0 { trect.top() + 1.0 } else { trect.bottom() - 1.0 };
                                let ix = trect.left() + body_x0 + depth as f32 * 13.0;
                                dp.hline(ix..=(trect.right() - 10.0), y, Stroke::new(2.0, ACCENT));
                                dp.circle_filled(egui::pos2(ix, y), 3.0, ACCENT);
                            }
                        }
                    }
                },
            );
            // release: zone 3 = move the art onto the target BOARD (spatial; drop_ind.0 = section);
            // otherwise Alt = duplicate into the target row, else reorder / nest. The whole payload
            // (the multi-selection) travels in one undoable op.
            if let Some((_, src_sec)) = *drag {
                if ui.input(|i| i.pointer.any_released()) {
                    if let Some((tid, zone, _, _)) = drop_ind {
                        if zone == 3 {
                            let src_board = (src_sec != u32::MAX).then_some(src_sec as usize);
                            ops.push(Op::LayerMoveBoard(payload.clone(), src_board, tid as usize));
                        } else if ui.input(|i| i.modifiers.alt) {
                            ops.push(Op::LayerDupMove(payload.clone(), tid, zone));
                        } else {
                            ops.push(Op::LayerMove(payload.clone(), tid, zone));
                        }
                    }
                    *drag = None;
                }
            }
            if drag.is_some() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            hairline(ui);
            // ── footer: Group · Delete ──
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.add_space(11.0);
                if IA_LAYER_GROUP.show(ui, kit::IconState::Action) {
                    ops.push(Op::LayerGroup);
                }
                if IA_LAYER_DELETE.show(ui, kit::IconState::Action) {
                    ops.push(Op::LayerDeleteSel);
                }
            });
        }
    }
}

/// Truncate a name with a trailing "…" so it fits `avail` px at `size` (rough per-glyph estimate).
pub(crate) fn elide(name: &str, avail: f32, size: f32) -> String {
    let per = size * 0.55;
    let max = (avail / per).floor() as usize;
    if name.chars().count() <= max || max < 2 {
        return name.to_string();
    }
    let mut s: String = name.chars().take(max.saturating_sub(1)).collect();
    s.push('\u{2026}');
    s
}
