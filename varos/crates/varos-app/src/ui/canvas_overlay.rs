use super::*;

// ───────────────────────────── rulers ─────────────────────────────

pub(crate) const RULER: f32 = 18.0; // ruler strip thickness in points

/// Decimals needed to print multiples of `grid` cleanly (grid is a power of 5: 25→0, 0.2→1, 0.04→2).
pub(crate) fn ruler_dec(grid: f32) -> usize {
    (-grid.log10()).ceil().max(0.0) as usize
}

pub(crate) fn fmt_ruler(v: f32, grid: f32, dec: usize) -> String {
    let v = if v.abs() < grid * 0.001 { 0.0 } else { v }; // kill -0
    format!("{:.*}", dec, v)
}

/// Top + left rulers (Ctrl+R), drawn INSIDE the Board box (Stage 4): two 18px strips hugging its top
/// and left edges (§3.5: bg = ruler_bg). Ticks sit on the SAME base-5 lattice as the dot grid; every
/// big tick is labeled. Numbers read relative to `origin`; a live tick tracks the pointer; the corner
/// box drag-sets the origin (snapped) and double-click resets it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn board_rulers(
    ui: &mut egui::Ui,
    board: egui::Rect,
    view: View,
    ppp: f32,
    grid: f32,
    origin: [f32; 2],
    reset: [f32; 2],
    ops: &mut Vec<Op>,
) {
    let num_font = numeric_value(9.5);
    let dec = ruler_dec(grid);
    // label every Nth grid-tick so numbers stay ~70 pts apart at ANY zoom (dense, never a vast gap). N is a
    // grid multiple, so labels still land on dots; small N → rounder numbers (×2 = 250s, not ×5 = 625s).
    let ms_pts = grid * view.zoom / ppp.max(1e-6); // one grid-tick in screen points
    let label_every =
        [1i64, 2, 5, 10, 20, 50, 100, 200].into_iter().find(|&n| n as f32 * ms_pts >= 70.0).unwrap_or(200);
    let pointer = ui.ctx().pointer_latest_pos();

    // top (horizontal) ruler — ticks at WORLD multiples of `grid` (land on the dots), label every 5th
    {
        let r = egui::Rect::from_min_max(board.left_top(), egui::pos2(board.right(), board.top() + RULER));
        let p = ui.painter_at(r);
        p.rect_filled(r, CornerRadius::ZERO, RULER_BG);
        p.hline(r.x_range(), r.bottom() - 0.5, Stroke::new(1.0, BORDER));
        let x_lo = r.left() + RULER; // ticks start after the corner box
        let v_lo = view.s2w([x_lo * ppp, 0.0])[0] - origin[0];
        let v_hi = view.s2w([r.right() * ppp, 0.0])[0] - origin[0];
        let m1 = (v_hi / grid).ceil() as i64;
        let mut m = (v_lo / grid).floor() as i64;
        while m <= m1 {
            let val = m as f32 * grid; // value shown — relative to the origin, so m==0 is ZERO
            let sx = view.w2s([origin[0] + val, 0.0])[0] / ppp;
            let (big, zero) = (m.rem_euclid(label_every) == 0, m == 0);
            m += 1;
            if sx < x_lo - 0.5 || sx > r.right() + 0.5 {
                continue;
            }
            let h = if zero {
                12.0
            } else if big {
                9.0
            } else {
                5.0
            };
            p.vline(
                sx,
                (r.bottom() - h)..=r.bottom(),
                Stroke::new(
                    1.0,
                    if zero {
                        TEXT
                    } else if big {
                        MUTED
                    } else {
                        BORDER_2
                    },
                ),
            );
            if big {
                p.text(
                    egui::pos2(sx + 2.5, r.top() + 1.0),
                    Align2::LEFT_TOP,
                    fmt_ruler(val, grid, dec),
                    num_font.clone(),
                    if zero { TEXT } else { MUTED },
                );
            }
        }
        if let Some(pt) = pointer {
            p.vline(pt.x, r.y_range(), Stroke::new(1.0, ACCENT));
        }
        // drag off the strip body → pull out a HORIZONTAL guide (follows the pointer onto the canvas)
        let body = egui::Rect::from_min_max(egui::pos2(r.left() + RULER, r.top()), r.right_bottom());
        let dr = ui.interact(body, ui.id().with("ruler-h-body"), egui::Sense::drag());
        if dr.dragged() {
            if let Some(pp) = ui.ctx().pointer_latest_pos() {
                ops.push(Op::GuidePreview(false, view.s2w([pp.x * ppp, pp.y * ppp])));
            }
        }
        if dr.drag_stopped() {
            ops.push(Op::GuideCommit);
        }
        // corner box: drag sets the origin (snapped), double-click resets it
        let corner = egui::Rect::from_min_size(r.left_top(), egui::vec2(RULER, RULER));
        let resp = ui.interact(corner, ui.id().with("ruler-corner"), egui::Sense::click_and_drag());
        p.rect_filled(corner, CornerRadius::ZERO, RULER_BG);
        p.vline(corner.right() - 0.5, r.y_range(), Stroke::new(1.0, BORDER));
        let c = corner.center();
        p.line_segment([egui::pos2(c.x - 3.0, c.y), egui::pos2(c.x + 3.0, c.y)], Stroke::new(1.0, MUTED));
        p.line_segment([egui::pos2(c.x, c.y - 3.0), egui::pos2(c.x, c.y + 3.0)], Stroke::new(1.0, MUTED));
        if resp.double_clicked() {
            ops.push(Op::RulerOrigin(Some(reset)));
            ops.push(Op::RulerOrigin(None));
        } else if resp.dragged() {
            if let Some(pp) = ui.ctx().pointer_latest_pos() {
                ops.push(Op::RulerOrigin(Some(view.s2w([pp.x * ppp, pp.y * ppp]))));
            }
        }
        if resp.drag_stopped() {
            ops.push(Op::RulerOrigin(None));
        }
    }

    // left (vertical) ruler — numbers rotated 90° (read upward), like Illustrator
    {
        let r = egui::Rect::from_min_max(
            egui::pos2(board.left(), board.top() + RULER),
            egui::pos2(board.left() + RULER, board.bottom()),
        );
        let p = ui.painter_at(r);
        p.rect_filled(r, CornerRadius::ZERO, RULER_BG);
        p.vline(r.right() - 0.5, r.y_range(), Stroke::new(1.0, BORDER));
        let v_lo = view.s2w([0.0, r.top() * ppp])[1] - origin[1];
        let v_hi = view.s2w([0.0, r.bottom() * ppp])[1] - origin[1];
        let m1 = (v_hi / grid).ceil() as i64;
        let mut m = (v_lo / grid).floor() as i64;
        while m <= m1 {
            let val = m as f32 * grid; // value shown — relative to the origin, so m==0 is ZERO
            let sy = view.w2s([0.0, origin[1] + val])[1] / ppp;
            let (big, zero) = (m.rem_euclid(label_every) == 0, m == 0);
            m += 1;
            if sy < r.top() - 0.5 || sy > r.bottom() + 0.5 {
                continue;
            }
            let w = if zero {
                12.0
            } else if big {
                9.0
            } else {
                5.0
            };
            p.hline(
                (r.right() - w)..=r.right(),
                sy,
                Stroke::new(
                    1.0,
                    if zero {
                        TEXT
                    } else if big {
                        MUTED
                    } else {
                        BORDER_2
                    },
                ),
            );
            if big {
                let col = if zero { TEXT } else { MUTED };
                let galley = p.layout_no_wrap(fmt_ruler(val, grid, dec), num_font.clone(), col);
                let mut ts =
                    egui::epaint::TextShape::new(egui::pos2(r.left() + 2.0, sy + galley.size().x / 2.0), galley, col);
                ts.angle = -std::f32::consts::FRAC_PI_2;
                p.add(ts);
            }
        }
        if let Some(pt) = pointer {
            p.hline(r.x_range(), pt.y, Stroke::new(1.0, ACCENT));
        }
        // drag off the strip → pull out a VERTICAL guide (follows the pointer onto the canvas)
        let dr = ui.interact(r, ui.id().with("ruler-v-body"), egui::Sense::drag());
        if dr.dragged() {
            if let Some(pp) = ui.ctx().pointer_latest_pos() {
                ops.push(Op::GuidePreview(true, view.s2w([pp.x * ppp, pp.y * ppp])));
            }
        }
        if dr.drag_stopped() {
            ops.push(Op::GuideCommit);
        }
    }
}

/// While the ruler zero-point is being dragged, a full-canvas DASHED crosshair (vertical = X, horizontal
/// = Y) marks where the new origin will land — drawn at the SNAPPED position, so the snap to a corner /
/// anchor / grid dot is visible and you can see exactly where (0,0) is going.
pub(crate) fn build_origin_crosshair(
    ctx: &egui::Context,
    view: View,
    ppp: f32,
    hole: egui::Rect,
    preview: Option<varos_core::geom::Pt>,
) {
    let Some(w) = preview else {
        return;
    };
    let s = view.w2s(w);
    let (sx, sy) = (s[0] / ppp, s[1] / ppp);
    let scr = hole; // Stage 4: the crosshair belongs to the canvas — never over the boxes
    let p = ctx
        .layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("origin-cross")))
        .with_clip_rect(hole);
    let stroke = Stroke::new(1.0, ACCENT);
    for seg in egui::Shape::dashed_line(&[egui::pos2(sx, scr.top()), egui::pos2(sx, scr.bottom())], stroke, 5.0, 4.0) {
        p.add(seg);
    }
    for seg in egui::Shape::dashed_line(&[egui::pos2(scr.left(), sy), egui::pos2(scr.right(), sy)], stroke, 5.0, 4.0) {
        p.add(seg);
    }
}

/// The live measurement HUD — a small pill near the cursor showing the drag readout (X/Y position now;
/// W×H / angle later). Pure feedback on a foreground layer; no interaction, never blocks the canvas.
pub(crate) fn build_snap_hud(
    ctx: &egui::Context,
    view: View,
    ppp: f32,
    hole: egui::Rect,
    hud: &Option<(varos_core::geom::Pt, String)>,
) {
    let (wp, text) = match hud {
        Some(h) => h,
        None => return,
    };
    let sp = view.w2s(*wp);
    let anchor = egui::pos2(sp[0] / ppp + 15.0, sp[1] / ppp - 26.0);
    // Stage 4: the HUD rides the canvas — clip so it never floats over the boxes
    let p =
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("snap-hud"))).with_clip_rect(hole);
    let font = FontId::proportional(12.0);
    let galley = p.layout_no_wrap(text.clone(), font.clone(), TEXT);
    let rect = egui::Rect::from_min_size(anchor, galley.size() + egui::vec2(14.0, 7.0));
    p.rect_filled(rect, CornerRadius::same(R), SOLID_PANEL);
    p.rect_stroke(rect, CornerRadius::same(R), Stroke::new(1.0, BORDER), StrokeKind::Middle);
    p.text(rect.center(), Align2::CENTER_CENTER, text, font, TEXT);
}

// on-canvas overlays are CONFINED to the Board hole (Ahmed 07-07)
/// Background paints above the wgpu canvas/root, below the floating control bar, tool rail,
/// page chrome and recovery card. Painter only: no hit-testing.
pub(crate) fn paint_agent_presence(
    ctx: &egui::Context,
    view: View,
    ppp: f32,
    hole: egui::Rect,
    frame: &crate::agent_presence::Frame,
) {
    use varos_app::shell::tokens::{
        AGENT, AGENT_LABEL_GAP, AGENT_LABEL_H, AGENT_LABEL_MIN_PAGE_W, AGENT_LABEL_PAD, AGENT_LABEL_TITLE_TRAIL,
        AGENT_OBJECT_STROKE, AGENT_PAGE_STROKE, KIT_STROKE,
    };
    if frame.pages.is_empty() && frame.objects.is_empty() && frame.repaint_after.is_none() {
        return;
    }
    let painter = ctx
        .layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("agent-presence")))
        .with_clip_rect(hole);
    let rect = |b: [f32; 4]| {
        let tl = view.w2s([b[0], b[1]]);
        let br = view.w2s([b[2], b[3]]);
        egui::Rect::from_min_max(egui::pos2(tl[0] / ppp, tl[1] / ppp), egui::pos2(br[0] / ppp, br[1] / ppp))
    };
    for page in &frame.pages {
        let r = rect(page.bounds);
        if hole.intersects(r.expand(AGENT_LABEL_H * (page.labels.len() + 1) as f32)) {
            painter.rect_stroke(r, CornerRadius::ZERO, Stroke::new(AGENT_PAGE_STROKE, AGENT), StrokeKind::Outside);
            if r.intersect(hole).width() < AGENT_LABEL_MIN_PAGE_W {
                continue;
            }
            let title = painter.layout_no_wrap(page.name.clone(), varos_app::shell::tokens::micro(), TEXT);
            let title_end = r.left() + title.size().x + AGENT_LABEL_GAP;
            for (i, label) in page.labels.iter().enumerate() {
                // Title-style labels are pinned above the top-right edge and stack upward.
                // Leave the existing page settings dots at the corner clear.
                let galley = painter.layout_no_wrap(label.clone(), varos_app::shell::tokens::micro(), TEXT);
                let size = egui::vec2(galley.size().x + AGENT_LABEL_PAD * 2.0, AGENT_LABEL_H);
                let chip = egui::Rect::from_min_size(
                    egui::pos2(
                        (r.right() - size.x - AGENT_LABEL_TITLE_TRAIL).max(title_end),
                        r.top() - AGENT_LABEL_GAP - AGENT_LABEL_H - i as f32 * (AGENT_LABEL_H + AGENT_LABEL_GAP),
                    ),
                    size,
                );
                painter.rect_filled(chip, CornerRadius::same(R), SOLID_PANEL);
                painter.rect_stroke(chip, CornerRadius::same(R), Stroke::new(KIT_STROKE, AGENT), StrokeKind::Inside);
                painter.galley(
                    egui::pos2(chip.left() + AGENT_LABEL_PAD, chip.center().y - galley.size().y / 2.0),
                    galley,
                    TEXT,
                );
            }
        }
    }
    for object in &frame.objects {
        let r = rect(object.bounds);
        if hole.intersects(r) {
            painter.rect_stroke(
                r,
                CornerRadius::ZERO,
                Stroke::new(AGENT_OBJECT_STROKE, AGENT.gamma_multiply(object.alpha)),
                StrokeKind::Outside,
            );
        }
    }
    let delay = crate::agent_presence::repaint_after(frame, |bounds| hole.intersects(rect(bounds)));
    if let Some(delay) = delay {
        ctx.request_repaint_after(delay);
    }
}
