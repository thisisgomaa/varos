use super::*;
use varos_app::shell::tokens::{FIELD_H, STATUS_ZOOM_W};

pub(crate) fn with_a(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a * 255.0).clamp(0.0, 255.0) as u8)
}

// ───────────────────────────── custom title bar ─────────────────────────────

/// One window-control button (min/max/close): 46×40, no rounding, hover fill + icon.
#[derive(Clone, Copy)]
pub(crate) enum Cap {
    Min,
    Max,
    Restore,
    Close,
}

/// A window caption button. The glyph is PAINTED directly (crisp 1px lines like Windows 11 / Chrome),
/// not an SVG texture — the Lucide minus rendered with round caps looked like a fat pill, not a clean dash.
pub(crate) fn winctl(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    cap: Cap,
    key: &str,
    hover_bg: Color32,
    white_on_hover: bool,
) -> bool {
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    let hov = resp.hovered();
    if hov {
        p.rect_filled(rect, CornerRadius::ZERO, hover_bg);
    }
    let col = if white_on_hover && hov { Color32::WHITE } else { TEXT };
    let s = Stroke::new(1.0, col);
    let c = rect.center();
    match cap {
        Cap::Min => {
            let y = c.y.round() + 0.5;
            p.line_segment([egui::pos2(c.x - 5.0, y), egui::pos2(c.x + 5.0, y)], s);
        }
        Cap::Max => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)),
                CornerRadius::ZERO,
                s,
                StrokeKind::Middle,
            );
        }
        Cap::Restore => {
            // two overlapping windows — the "restore down" glyph shown WHILE maximized
            p.rect_stroke(
                egui::Rect::from_min_size(egui::pos2(c.x - 5.0, c.y - 2.0), egui::vec2(7.0, 7.0)),
                CornerRadius::ZERO,
                s,
                StrokeKind::Middle,
            ); // front
            p.line_segment([egui::pos2(c.x - 2.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y - 5.0)], s); // back top edge
            p.line_segment([egui::pos2(c.x + 5.0, c.y - 5.0), egui::pos2(c.x + 5.0, c.y + 2.0)], s);
            // back right edge
        }
        Cap::Close => {
            p.line_segment([c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)], s);
            p.line_segment([c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)], s);
        }
    }
    resp.clicked()
}

/// One 4b band button (Home, `+`, "+N", the V mark): no fill at rest, HOVER r3 on hover / press,
/// SURFACE when `on`; the 2-px azure ring only while it holds keyboard focus. The caller paints the
/// glyph (TEXT when hovered / on, else MUTED — `band_ink`).
pub(crate) fn band_button(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    key: &str,
    sense: egui::Sense,
    on: bool,
) -> egui::Response {
    let resp = ui.interact(rect, ui.id().with(key), sense);
    if on {
        p.rect_filled(rect, CornerRadius::same(R), BG_SURFACE);
    } else if resp.hovered() || resp.is_pointer_button_down_on() {
        p.rect_filled(rect, CornerRadius::same(R), HOVER);
    }
    if resp.has_focus() {
        let ring = Stroke::new(varos_app::shell::tokens::KIT_FOCUS_STROKE, ACCENT);
        p.rect_stroke(rect, CornerRadius::same(R), ring, StrokeKind::Inside);
    }
    resp
}

/// A band glyph's ink: TEXT when hovered or on, MUTED at rest.
pub(crate) fn band_ink(resp: &egui::Response, on: bool) -> Color32 {
    if on || resp.hovered() {
        TEXT
    } else {
        MUTED
    }
}

/// The V mark (4b): hand-painted, no texture — its three squares are FILLED with the colour under
/// them (`under`: the backdrop at rest, HOVER on hover), so the polyline reads as passing behind
/// them. 18×18 design units centred in `rect` (src.html `mark`).
pub(crate) fn paint_brand(p: &egui::Painter, rect: egui::Rect, under: Color32) {
    use varos_app::shell::tokens as t;
    let o = rect.center() - egui::Vec2::splat(t::BAND_BRAND_MARK / 2.0);
    let at = |x: f32, y: f32| o + egui::vec2(x, y);
    let ink = Stroke::new(1.5, MUTED);
    p.add(egui::Shape::line(vec![at(3.5, 4.0), at(9.0, 14.0), at(14.5, 4.0)], ink));
    for (x, y) in [(2.0, 2.5), (13.0, 2.5), (7.5, 12.5)] {
        let sq = egui::Rect::from_min_size(at(x, y), egui::Vec2::splat(3.0));
        p.rect(sq, CornerRadius::ZERO, under, ink, StrokeKind::Middle);
    }
}

/// CPU-only painting pass shared by run_home and its frame tests.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_home_frame(
    root: &mut egui::Ui,
    top: &TopIcons,
    shell: &mut varos_app::shell::ShellState,
    win: &mut Option<WinAction>,
    tabs: &[TabView],
    commands: &mut Vec<AppCommand>,
    rail: &mut bool,
    dock: &mut bool,
    page: &mut varos_app::start_page::StartPage,
    model: &mut varos_app::start::StartModel,
    warning: Option<&str>,
    maximized: bool,
) {
    build_topbar(
        root,
        top,
        shell,
        win,
        tabs,
        None,
        commands,
        rail,
        dock,
        &mut Default::default(),
        false,
        None,
        maximized,
        true,
        cfg!(target_os = "macos"),
    );
    home_body(root, page, model, warning, commands);
}

/// Home's body under the top bar: THE Start page. Filter actions (tag / view) change the Start
/// model; every other action becomes its `AppCommand` through the one adapter (`host::start_command`).
pub(crate) fn home_body(
    root: &mut egui::Ui,
    page: &mut varos_app::start_page::StartPage,
    model: &mut varos_app::start::StartModel,
    warning: Option<&str>,
    cmds: &mut Vec<AppCommand>,
) {
    for action in page.draw(root, model, warning) {
        if !model.apply(&action) {
            cmds.extend(crate::host::start_command(action));
        }
    }
}

/// One document tab (4b), painted at `rect` (its resting slot, or its lifted / reflowed rect during
/// a drag — P16): active = SURFACE chip with its name in TEXT 12/500; inactive = bare MUTED 12/400,
/// HOVER on hover; `lifted` = SURFACE + a LINE2 border. The name starts `TAB_PAD_L` in, in a box
/// `TAB_NAME_CLIP` narrower than the chip. On the right, centred `TAB_MARK_INSET` in: the dirty dot
/// (TEXT on the active tab, MUTED elsewhere) — replaced by the × on hover; the × also shows on the
/// active and the lifted chip. `Sense::click_and_drag` so `tab_drag_update` can read this chip's drag
/// start / stop. Returns `(response, close_clicked)`.
pub(crate) fn tab_item(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    tab: &TabView,
    active: bool,
    lifted: bool,
    key: &str,
) -> (egui::Response, bool) {
    use varos_app::shell::tokens as t;
    // No accent — azure is a scalpel, not a tab decoration.
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click_and_drag());
    let x_r = crate::chrome::tab_close_rect(rect);
    let xr = ui.interact(x_r, ui.id().with((key, "x")), egui::Sense::click());
    let hov = resp.hovered() || xr.hovered();
    let rr = CornerRadius::same(R);
    if lifted {
        p.rect(rect, rr, BG_SURFACE, Stroke::new(t::KIT_STROKE, BORDER_2), StrokeKind::Inside);
    } else if active {
        p.rect_filled(rect, rr, BG_SURFACE);
    } else if hov {
        p.rect_filled(rect, rr, HOVER);
    }
    let lit = active || lifted;
    let name_right = rect.left() + t::TAB_PAD_L + (rect.width() - t::TAB_NAME_CLIP).max(0.0);
    let name_clip = egui::Rect::from_min_max(rect.min, egui::pos2(name_right, rect.bottom()));
    p.with_clip_rect(name_clip.intersect(p.clip_rect())).text(
        egui::pos2(rect.left() + t::TAB_PAD_L, rect.center().y),
        Align2::LEFT_CENTER,
        &tab.label,
        if active { t::small_medium() } else { t::small() },
        if lit { TEXT } else { MUTED },
    );
    if tab.dirty && !hov {
        // the neutral unsaved-changes dot — never azure (azure is a scalpel)
        p.circle_filled(x_r.center(), t::TAB_DOT / 2.0, if lit { TEXT } else { MUTED });
    } else if lit || hov {
        if xr.hovered() {
            p.rect_filled(egui::Rect::from_center_size(x_r.center(), egui::Vec2::splat(t::TAB_CLOSE_HIT)), rr, HOVER);
        }
        Icon::Remove.paint(p, x_r.center(), t::TAB_CLOSE_ICON, if xr.hovered() { TEXT } else { MUTED });
    }
    let resp = resp.on_hover_text(tab.tooltip.clone());
    (resp, xr.clicked())
}

/// egui's window-focus flag for this frame, seeded by the host (P16 owner re-test, 2026-09-26).
/// egui-winit starts `RawInput::focused` at `false` and on macOS only updates it on a winit `Focused`
/// event — a bundle launched via `open` ran a whole session without one, so egui believed the window
/// unfocused: every text field lost its typed buffer and caret (`Response::has_focus` reads the flag)
/// and the tab drag cancelled itself. `window_key` = winit's `window.has_focus()` (AppKit's
/// `isKeyWindow`) read this frame: a key window is focused. It only ever RAISES the flag — losing
/// focus stays winit's own `Focused(false)` path.
pub(crate) fn egui_focus_seed(window_key: bool, egui_focused: bool) -> bool {
    egui_focused || window_key
}

/// The live tab drag (P16). Kept in egui temp memory under ONE fixed id (`TAB_DRAG_KEY`) so the host
/// can ask `Gui::tab_drag_active` — Esc then cancels the drag instead of also reaching the canvas.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TabDrag {
    /// The dragged tab.
    pub(crate) id: SessionId,
    /// Pointer x − chip left at the press: the grabbed point stays under the pointer.
    pub(crate) grab_dx: f32,
    /// The last known pointer x (kept for a frame where egui has no pointer position).
    pub(crate) last_x: f32,
    /// Last frame's gap slot (`TabDragFrame::landing`) — the hysteresis in `tab_drag_frame` reads it.
    pub(crate) landing: usize,
    /// The tab order and the active tab when the drag started. Any change under the drag (a tab
    /// closed by ⌘W, a new / opened document, Ctrl+Tab) cancels it: the gap was measured against a
    /// strip that no longer exists (P16 review).
    pub(crate) order: Vec<SessionId>,
    pub(crate) active: Option<SessionId>,
}

pub(crate) const TAB_DRAG_KEY: &str = "varos.tab-drag";

/// This frame of the tab drag (P16): start it (the frame egui passes its drag threshold on a chip),
/// follow the pointer, commit on release (`ReorderDocument` with `chrome::visible_drop_slot` — the
/// Workspace still owns the order), or cancel on Esc / window focus loss / the tab list changing
/// underneath (nothing committed, every chip back at rest). Returns the lifted chip's tab index and
/// the geometry to paint, or `None` when no chip is lifted (paint at rest). The geometry is all
/// `chrome::tab_drag_frame`; `drawn_after` is the real layout's "which tabs are drawn for this order".
pub(crate) fn tab_drag_update(
    ui: &egui::Ui,
    layout: &crate::chrome::TopbarLayout,
    tabs: &[TabView],
    active: Option<SessionId>,
    chip_key: impl Fn(usize) -> String,
    drawn_after: impl Fn(&[usize]) -> Vec<usize>,
    cmds: &mut Vec<AppCommand>,
) -> Option<(usize, crate::chrome::TabDragFrame)> {
    let ctx = ui.ctx();
    let key = egui::Id::new(TAB_DRAG_KEY);
    // the same id `tab_item` gives chip `i` (`ui.id().with(key)`)
    let chip_id = |i: usize| ui.id().with(chip_key(i).as_str());
    let clear = || {
        ctx.stop_dragging();
        ctx.data_mut(|d| d.remove::<TabDrag>(key));
    };
    let mut state: Option<TabDrag> = ctx.data(|d| d.get_temp(key));
    // Focus loss = the window-focus EVENT arriving this frame, never the `InputState::focused` level
    // (egui-winit's flag was stuck `false` on macOS bundle launches and cancelled every drag — P16
    // owner re-test, 2026-09-26). Checked for the WHOLE frame: a frame that loses focus never starts
    // a drag either, even when the threshold is crossed in that same frame (Codex review).
    let focus_lost = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::WindowFocused(false))));
    let chip_started = ctx.drag_started_id().is_some_and(|d| layout.tabs.iter().any(|&(i, _)| chip_id(i) == d));
    if focus_lost && (state.is_some() || chip_started) {
        clear(); // cancel: nothing is committed, the chips paint at rest this very frame
        return None;
    }
    if let Some(s) = &state {
        let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let changed = s.active != active || !s.order.iter().copied().eq(tabs.iter().map(|t| t.id));
        if esc || changed {
            clear(); // cancel: nothing is committed, the chips paint at rest this very frame
            return None;
        }
    }
    // egui resolves this frame's drag start before any widget runs, so the chip lifts in the SAME
    // frame the threshold is passed — no frame painted at rest in between.
    if state.is_none() {
        let started = ctx.drag_started_id()?;
        let (k, &(i, r)) = layout.tabs.iter().enumerate().find(|&(_, &(i, _))| chip_id(i) == started)?;
        let x = ctx.input(|inp| inp.pointer.press_origin().or(inp.pointer.interact_pos())).map_or(r.left(), |p| p.x);
        let order = tabs.iter().map(|t| t.id).collect();
        state = Some(TabDrag { id: tabs[i].id, grab_dx: x - r.left(), last_x: x, landing: k, order, active });
    }
    let mut s = state?;
    let found = layout.tabs.iter().position(|&(i, _)| tabs.get(i).is_some_and(|t| t.id == s.id));
    let (Some(dragged), Some(strip)) = (found, layout.tab_strip()) else {
        clear(); // the dragged tab is not drawn any more — drop the drag
        return None;
    };
    if let Some(p) = ctx.input(|i| i.pointer.interact_pos()) {
        s.last_x = p.x;
    }
    let frame = crate::chrome::tab_drag_frame(&layout.tabs, dragged, s.grab_dx, s.last_x, strip, s.landing)?;
    s.landing = frame.landing;
    let i = layout.tabs[dragged].0;
    if ctx.drag_stopped_id() == Some(chip_id(i)) {
        // release: the chip lands in the gap — the tab strip only REQUESTS the move
        let others: Vec<usize> = frame.others.iter().map(|&(j, _)| j).collect();
        let slot = crate::chrome::visible_drop_slot(tabs.len(), i, &others, frame.landing, drawn_after);
        cmds.push(AppCommand::ReorderDocument(s.id, slot));
        ctx.data_mut(|d| d.remove::<TabDrag>(key));
    } else if ctx.dragged_id() == Some(chip_id(i)) {
        ctx.data_mut(|d| d.insert_temp(key, s));
    } else {
        clear(); // the drag ended some other way — never leave a chip lifted
        return None;
    }
    Some((i, frame))
}

/// The top band (4b, MAC_CHROME.md §A′): Home · tabs · "+N ⌄" · `+` · drag space · V, all
/// on one centre line, on the one black backdrop (Windows: burger · … · caps). Interactive rects are
/// published as the caption exclusions so the OS / macOS caption hit-test makes them egui's while the
/// empty band drags the window. `right_zone` = the panel column's x-span (last frame), `None` on Home.
/// Why the burger's Revert row is disabled.
const REVERT_WHY: &str = "Revert needs a saved document with unsaved changes.";

/// Slice 0.6: one burger File row — the native row's command (`host::to_app_command`) when
/// `menus::file_row_enabled` allows it, else a disabled row saying `why`. `true` = chosen.
#[allow(clippy::too_many_arguments)] // one row's label, key, command and the state it reads
fn file_menu_row(
    ui: &mut egui::Ui,
    label: &str,
    shortcut: &str,
    f: crate::chrome::FileCmd,
    state: crate::menus::DocMenuState,
    active: Option<SessionId>,
    why: &str,
    cmds: &mut Vec<AppCommand>,
) -> bool {
    match crate::host::to_app_command(f, active).filter(|_| crate::menus::file_row_enabled(f, state)) {
        Some(cmd) if menu_row(ui, label, shortcut) => {
            cmds.push(cmd);
            true
        }
        Some(_) => false,
        None => {
            menu_row_disabled(ui, label, why);
            false
        }
    }
}

#[allow(clippy::too_many_arguments)] // hand-painted panel builder: each arg is live UI state
pub(crate) fn build_topbar(
    root: &mut egui::Ui,
    top: &TopIcons,
    shell: &mut varos_app::shell::ShellState,
    win_action: &mut Option<WinAction>,
    tabs: &[TabView],
    active: Option<SessionId>,
    cmds: &mut Vec<AppCommand>,
    show_rail: &mut bool,
    show_dock: &mut bool,
    snap: &mut varos_core::model::SnapConfig,
    has_selection: bool,
    right_zone: Option<egui::Rangef>,
    maximized: bool,
    home: bool,
    native_home: bool,
) {
    use varos_app::shell::tokens as t;
    let h = crate::chrome::TOPBAR.height;
    // DFS S6: Export (burger row, File ▸ Export ▸ PDF…) is ONE command through the one mapper
    let export_cmd = crate::host::to_app_command(crate::chrome::FileCmd::Export, active);
    // slice 0.6: the burger's Save a Copy / Revert / Close All / Export Selection rows read the same
    // enable rules as the native File menu (`menus::file_row_enabled`)
    let tab = active.and_then(|id| tabs.iter().find(|t| t.id == id));
    let file_state = crate::menus::DocMenuState {
        active: active.is_some(),
        can_revert: tab.is_some_and(|t| t.dirty && t.file),
        has_selection,
    };
    // the band IS the void — backdrop fill, no hairline, no step (4b: one flat #000)
    let frame = egui::Frame { fill: SEAM, inner_margin: Margin::ZERO, ..Default::default() };
    egui::Panel::top("topbar").exact_size(h).frame(frame).show_separator_line(false).show(root, |ui| {
        let bar = ui.max_rect();
        let p = ui.painter().clone();
        // every tab measured at 500 so a chip never changes width when it becomes active
        let text_width = |text: &str| p.layout_no_wrap(text.to_owned(), t::small_medium(), TEXT).size().x;
        let tab_widths: Vec<f32> = tabs.iter().map(|t| text_width(&t.label)).collect();
        let active_index = active.and_then(|id| tabs.iter().position(|t| t.id == id));
        let layout_for = |widths: &[f32], active: Option<usize>| {
            crate::chrome::topbar_layout(bar, crate::chrome::TOPBAR, right_zone, widths, active)
        };
        let layout = band_layout(layout_for(&tab_widths, active_index), home);

        // window controls (min · max · close), absent on macOS
        if let Some([min_r, max_r, close_r]) = layout.caps {
            if winctl(ui, &p, min_r, Cap::Min, "wc-min", HOVER, false) {
                *win_action = Some(WinAction::Minimize);
            }
            if winctl(ui, &p, max_r, if maximized { Cap::Restore } else { Cap::Max }, "wc-max", HOVER, false) {
                *win_action = Some(WinAction::ToggleMaximize);
            }
            if winctl(ui, &p, close_r, Cap::Close, "wc-close", CLOSE_RED, true) {
                *win_action = Some(WinAction::Close);
            }
        }

        // right zone: empty band over the panel column (it drags the window — no Search since
        // 2026-10-06), the V mark at its right edge.
        // V: the native About panel on macOS; Windows has no About panel to open, so hover-only
        let mac = cfg!(target_os = "macos");
        let sense = if mac { egui::Sense::click() } else { egui::Sense::hover() };
        let vr = band_button(ui, &p, layout.brand, "tb-brand", sense, false);
        paint_brand(&p, layout.brand, if vr.hovered() { HOVER } else { SEAM });
        if vr.on_hover_text("Varos").clicked() && mac {
            *win_action = Some(WinAction::About);
        }

        // Home (macOS, and Windows on Home) — or Windows' burger cell
        let menu_id = ui.make_persistent_id("app_menu");
        let menu_r = layout.menu;
        let mr = if native_home || home {
            let chip = home_chip(menu_r);
            // on Home Start owns Enter / Space: the chip is pointer-only there
            let sense = if home { egui::Sense::CLICK } else { egui::Sense::click() };
            let hr = band_button(ui, &p, chip, "home-chip", sense, home);
            Icon::Home.paint(&p, chip.center(), ICON_MD, band_ink(&hr, home));
            let hr = hr.on_hover_text("Home");
            if hr.clicked() {
                cmds.push(AppCommand::Home);
            }
            hr
        } else {
            let mr = ui.interact(menu_r, ui.id().with("tb-menu"), egui::Sense::click());
            let mopen = menu_open(ui, menu_id);
            if mopen || mr.hovered() {
                p.rect_filled(menu_r, CornerRadius::ZERO, HOVER);
            }
            if let Some(t) = &top.menu {
                let col = if mopen || mr.hovered() { TEXT } else { MUTED };
                p.image(t.id(), egui::Rect::from_center_size(menu_r.center(), egui::vec2(17.0, 17.0)), UV01(), col);
            }
            if mr.clicked() {
                menu_toggle(ui, menu_id);
            }
            mr
        };

        // doc tabs — chips in the void: 28 tall, gap 2, 88–176 wide (4b). `layout.tabs` carries each
        // chip's ORIGINAL tab index — not always a 0..n prefix once the active tab has displaced the
        // greedy fit's last slot on overflow (F7).
        // P16 — drag-to-reorder: past egui's drag threshold the chip is LIFTED (painted under the
        // pointer, above the rest) and the other chips reflow live around a gap where it would land,
        // outlined dashed. All geometry is `chrome::tab_drag_frame`; this block keeps state and paints.
        let chip_key = |i: usize| format!("tab{i}");
        // which tabs this SAME layout would draw for a reordered full order (original indices) — the
        // release uses it to keep the dropped tab visible (`chrome::visible_drop_slot`)
        let drawn_after = |order: &[usize]| {
            let widths: Vec<f32> = order.iter().map(|&i| tab_widths[i]).collect();
            let act = active_index.and_then(|a| order.iter().position(|&i| i == a));
            layout_for(&widths, act).tabs.iter().map(|&(k, _)| order[k]).collect::<Vec<usize>>()
        };
        let drag = tab_drag_update(ui, &layout, tabs, active, chip_key, drawn_after, cmds);
        let mut paint: Vec<(usize, egui::Rect)> = Vec::with_capacity(layout.tabs.len());
        match &drag {
            // the landing gap: a dashed LINE2 outline; others first, the lifted chip last (on top)
            Some((i, f)) => {
                let g = f.gap.shrink(0.5);
                let ring = vec![g.left_top(), g.right_top(), g.right_bottom(), g.left_bottom(), g.left_top()];
                p.extend(egui::Shape::dashed_line(
                    &ring,
                    Stroke::new(t::KIT_STROKE, BORDER_2),
                    t::BAND_DASH,
                    t::BAND_DASH_GAP,
                ));
                paint.extend(f.others.iter().copied().chain([(*i, f.lifted)]));
            }
            None => paint.extend(layout.tabs.iter().copied()),
        }
        let lifted = drag.as_ref().map(|&(i, _)| i);
        for (i, trect) in paint {
            let tab = &tabs[i];
            let (resp, close) = tab_item(ui, &p, trect, tab, Some(tab.id) == active, lifted == Some(i), &chip_key(i));
            if close || resp.clicked_by(egui::PointerButton::Middle) {
                cmds.push(AppCommand::CloseDocument(tab.id));
            } else if resp.clicked() {
                cmds.push(AppCommand::ActivateDocument(tab.id));
            }
        }
        // "+N ⌄": every hidden tab in one list; a row brings its tab forward (the active swap draws it)
        if let Some(ov) = layout.overflow {
            let owner = ui.id().with("tb-overflow");
            let open = kit::is_menu_open(ui.ctx(), owner);
            let orr = band_button(ui, &p, ov, "tb-overflow", egui::Sense::click(), open);
            let ink = band_ink(&orr, open);
            let count = format!("+{}", layout.hidden.len());
            let font = egui::FontId::new(
                t::BAND_OVERFLOW_TEXT,
                egui::FontFamily::Name(varos_app::shell::fonts::UI_400.into()),
            );
            p.text(
                egui::pos2(ov.left() + t::BAND_OVERFLOW_TEXT_X, ov.center().y),
                Align2::LEFT_CENTER,
                count,
                font,
                ink,
            );
            let chev = egui::pos2(ov.left() + t::BAND_OVERFLOW_CHEV_X + t::BAND_OVERFLOW_CHEV / 2.0, ov.center().y);
            Icon::ChevronDown.paint(&p, chev, t::BAND_OVERFLOW_CHEV, ink);
            if orr.on_hover_text("Hidden tabs").clicked() {
                if open {
                    kit::close_menu(ui.ctx());
                } else {
                    let at = egui::pos2(ov.left(), ov.bottom() + t::KIT_MENU_GAP);
                    kit::open_menu(ui.ctx(), owner, at, Some(ov));
                }
            }
            let rows: Vec<String> = layout.hidden.iter().map(|&i| overflow_row_label(&tabs[i])).collect();
            let entries: Vec<kit::MenuEntry<'_>> = rows.iter().map(|r| kit::MenuEntry::Item(r)).collect();
            if let Some(k) = kit::menu(ui.ctx(), owner, &entries) {
                cmds.push(AppCommand::ActivateDocument(tabs[layout.hidden[k]].id));
            }
        }
        if let Some(plus_r) = layout.plus {
            let pr = band_button(ui, &p, plus_r, "tb-plus", egui::Sense::click(), false);
            Icon::Plus.paint(&p, plus_r.center(), ICON_MD, band_ink(&pr, false));
            if pr.on_hover_text("New board").clicked() {
                cmds.push(AppCommand::NewBoard);
            }
        }

        // Windows' burger — the only flush dropdown left (no native menu bar there): the File rows,
        // then the Window rows (4b moved them out of the band; this keeps panel toggles reachable).
        // FIXED width — measured, never elastic.
        menu_below(ui, menu_id, &mr, Some(bar.bottom()), |ui| {
            ui.set_width(210.0);
            let mut hit = false; // a chosen item closes the menu (Illustrator; P7)
            if menu_row(ui, "New", &shortcut_label("N")) {
                cmds.push(AppCommand::NewBoard);
                hit = true;
            }
            if menu_row(ui, "Open\u{2026}", &shortcut_label("O")) {
                cmds.push(AppCommand::OpenDialog);
                hit = true;
            }
            if let Some(id) = active {
                if menu_row(ui, "Save", &shortcut_label("S")) {
                    cmds.push(AppCommand::Save(id));
                    hit = true;
                }
                let save_as = if cfg!(target_os = "macos") {
                    format!("\u{21e7}{}", shortcut_label("S"))
                } else {
                    "Ctrl+Shift+S".into()
                };
                if menu_row(ui, "Save As\u{2026}", &save_as) {
                    cmds.push(AppCommand::SaveAs(id));
                    hit = true;
                }
                // slice 0.6, Illustrator's keys (Revert is the bare F12, as in the native menu)
                use crate::chrome::FileCmd as F;
                let st = file_state;
                let alt = |k: &str| {
                    if cfg!(target_os = "macos") {
                        format!("\u{2325}{}", shortcut_label(k))
                    } else {
                        format!("Ctrl+Alt+{k}")
                    }
                };
                hit |= file_menu_row(ui, "Save a Copy\u{2026}", &alt("S"), F::SaveCopy, st, active, "", cmds);
                hit |= file_menu_row(ui, "Revert", "F12", F::Revert, st, active, REVERT_WHY, cmds);
                hit |= file_menu_row(ui, "Close All", &alt("W"), F::CloseAll, st, active, "", cmds);
            }
            menu_sep(ui);
            match &export_cmd {
                Some(cmd) => {
                    if menu_row(ui, "Export\u{2026}", "Alt+Ctrl+E") {
                        cmds.push(cmd.clone());
                        hit = true;
                    }
                }
                None => menu_row_disabled(ui, "Export\u{2026}", "Open a document to export it."),
            }
            hit |= file_menu_row(
                ui,
                "Export Selection\u{2026}",
                "",
                crate::chrome::FileCmd::ExportSelection,
                file_state,
                active,
                "Select something to export it.",
                cmds,
            );
            hit |= file_menu_row(
                ui,
                "Print…",
                &shortcut_label("P"),
                crate::chrome::FileCmd::Print,
                file_state,
                active,
                "Printing is currently available on macOS only.",
                cmds,
            );
            if menu_row(ui, "Home", "") {
                cmds.push(AppCommand::Home);
                hit = true;
            }
            hit |= super::clipping::rows(ui, active, cmds);
            // the View ▸ snapping rows (macOS has them in its native View menu): the same flags,
            // the same transitions — Smart Guides is exactly what Ctrl+U does
            menu_sep(ui);
            if check_row(ui, &format!("Smart Guides  ({})", shortcut_label("U")), snap.smart) {
                toggle_smart_guides(snap);
                hit = true;
            }
            for (label, on) in
                [("Alignment Guides", &mut snap.alignment_guides), ("Geometric Guides", &mut snap.object_geometry)]
            {
                if check_row(ui, label, *on) {
                    *on = !*on;
                    hit = true;
                }
            }
            menu_sep(ui);
            for (label, on) in [("Snap to Grid", &mut snap.grid), ("Snap to Point", &mut snap.key_points)] {
                if check_row(ui, label, *on) {
                    *on = !*on;
                    hit = true;
                }
            }
            // the Window rows: chrome toggles, then EVERY dockable panel — ✓ = it's in the layout;
            // click = open in an automatic spot / surface its tab / close (boxtree::toggle_panel)
            menu_sep(ui);
            if check_row(ui, "Tool rail", *show_rail) {
                *show_rail = !*show_rail;
                hit = true;
            }
            if check_row(ui, "Control bar", *show_dock) {
                *show_dock = !*show_dock;
                hit = true;
            }
            if check_row(
                ui,
                "Colour",
                ui.ctx().data(|d| d.get_temp::<bool>(egui::Id::new("picker-open"))).unwrap_or(false),
            ) {
                cmds.push(AppCommand::Window(crate::app_command::WindowCmd::TogglePicker));
                hit = true;
            }
            for pnl in
                varos_app::shell::PanelId::DOCKABLE.into_iter().filter(|p| *p != varos_app::shell::PanelId::History)
            {
                if check_row(ui, pnl.title(), shell.is_open(pnl)) {
                    shell.toggle_panel(pnl);
                    hit = true;
                }
            }
            menu_sep(ui);
            if menu_row(ui, "Reset layout", "") {
                cmds.push(AppCommand::Window(crate::app_command::WindowCmd::ResetLayout));
                hit = true;
            }
            // ---- Lane F ----
            for (label, action) in [
                ("Preferences…", crate::phase9::DesktopAction::Preferences),
                ("Keyboard Shortcuts…", crate::phase9::DesktopAction::Shortcuts),
                ("Actions…", crate::phase9::DesktopAction::Actions),
                ("Varos Help", crate::phase9::DesktopAction::Help),
                ("Report a problem", crate::phase9::DesktopAction::ReportProblem),
            ] {
                if menu_row(ui, label, "") {
                    cmds.push(AppCommand::Phase9(action));
                    hit = true;
                }
            }

            if check_row(ui, "History", shell.is_open(varos_app::shell::PanelId::History)) {
                shell.toggle_panel(varos_app::shell::PanelId::History);
                hit = true;
            }
            for (label, key, command) in [
                ("Document Setup…", "Ctrl+Alt+P", crate::chrome::FileCmd::DocumentSetup),
                ("Document Info", "", crate::chrome::FileCmd::DocumentInfo),
                ("Save as Template…", "", crate::chrome::FileCmd::SaveTemplate),
                ("New from Template…", "", crate::chrome::FileCmd::NewTemplate),
            ] {
                hit |= file_menu_row(ui, label, key, command, file_state, active, "Open a document first.", cmds);
            }
            if let Some(id) = active {
                hit |= command_rows(ui, id, cmds);
            }
            if hit {
                menu_set(ui, menu_id, false);
            }
        });
        // publish caption height + interactive (non-drag) rects, in physical px — ONE list, the
        // layout's own `interactive_rects` (every control and FULL tab slot drawn above), so the OS /
        // macOS caption band can never disagree with the strip about what a press belongs to (P15).
        // While a chip is lifted: the rects actually painted + its resting slot (P16 review).
        let ppp = ui.ctx().pixels_per_point();
        let rects = match &drag {
            Some((_, f)) => layout.interactive_rects_with(f.chip_rects()),
            None => layout.interactive_rects(),
        };
        let px = crate::chrome::caption_exclusions(&rects, ppp);
        crate::cursors::set_caption((h * ppp) as i32, &px);
    });
}

/// The band as `build_topbar` paints AND publishes it: on Home there is no `+` (it is not painted
/// there), so it is not published either — `interactive_rects` never lists a spot that neither acts
/// nor drags the window.
///
/// Home's chip is the 28×28 square centred in the menu cell (macOS: the cell already is that square;
/// Windows: the cell is the full-height 36-wide burger slot), so that square — not the cell — is
/// what is published: the strips around it are empty band and drag the window.
pub(crate) fn band_layout(mut layout: crate::chrome::TopbarLayout, home: bool) -> crate::chrome::TopbarLayout {
    if home {
        layout.plus = None;
        layout.menu = home_chip(layout.menu);
    }
    layout
}

/// The Home chip inside the menu cell: a `BAND_CHIP_H` square on the cell's centre.
pub(crate) fn home_chip(menu_cell: egui::Rect) -> egui::Rect {
    egui::Rect::from_center_size(menu_cell.center(), egui::Vec2::splat(varos_app::shell::tokens::BAND_CHIP_H))
}

/// Smart Guides on / off — the same transition as the Ctrl / ⌘ U key (test
/// `smart_guides_menu_and_shortcut_have_the_same_state_transition`).
pub(crate) fn toggle_smart_guides(snap: &mut varos_core::model::SnapConfig) {
    snap.smart = !snap.smart;
}

/// A "+N" list row: the hidden tab's name, with the unsaved-changes dot after it when dirty.
pub(crate) fn overflow_row_label(tab: &TabView) -> String {
    if tab.dirty {
        format!("{}  \u{2022}", tab.label)
    } else {
        tab.label.clone()
    }
}

/// The box tree's rect inside `.mid` (the region between the band and the status line). The boxes
/// FLOAT in the void (Ahmed 07-07): an outer breath of HALF the box-to-box seam on the sides; on top
/// the 4b band already holds the 12 (boxes start at its bottom); the bottom breath lives INSIDE the
/// taller status line so its text centres in the strip the eye sees.
pub(crate) fn editor_tree_rect(mid: egui::Rect) -> egui::Rect {
    let g = varos_app::shell::tokens::SEAM_GAP * 0.5;
    egui::Rect::from_min_max(mid.min + egui::vec2(g, 0.0), egui::pos2(mid.right() - g, mid.bottom()))
}

/// Seam-fill the `.mid` region EXCEPT the canvas hole (the Board pane's interior, where the wgpu
/// scene shows through). The void between boxes is exactly this underlay showing in the gaps.
pub(crate) fn paint_void_underlay(p: &egui::Painter, mid: egui::Rect, hole: Option<egui::Rect>) {
    match hole.map(|h| h.intersect(mid)).filter(|h| h.is_positive()) {
        None => {
            p.rect_filled(mid, CornerRadius::ZERO, SEAM);
        }
        Some(h) => {
            let (l, r) = (mid.left(), mid.right());
            p.rect_filled(egui::Rect::from_min_max(mid.min, egui::pos2(r, h.top())), CornerRadius::ZERO, SEAM);
            p.rect_filled(egui::Rect::from_min_max(egui::pos2(l, h.bottom()), mid.max), CornerRadius::ZERO, SEAM);
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(l, h.top()), egui::pos2(h.left(), h.bottom())),
                CornerRadius::ZERO,
                SEAM,
            );
            p.rect_filled(
                egui::Rect::from_min_max(egui::pos2(h.right(), h.top()), egui::pos2(r, h.bottom())),
                CornerRadius::ZERO,
                SEAM,
            );
        }
    }
}

/// Patch the four corners of the Board box with seam-coloured "square minus quarter-arc" wedges, so
/// the raw wgpu scene never pokes past the box's rounded silhouette.
pub(crate) fn corner_voids(p: &egui::Painter, rect: egui::Rect) {
    let r = RBOX as f32;
    let n = 8; // arc samples — plenty at 8px
    let corners = [
        (rect.left_top(), egui::vec2(1.0, 1.0)),
        (rect.right_top(), egui::vec2(-1.0, 1.0)),
        (rect.right_bottom(), egui::vec2(-1.0, -1.0)),
        (rect.left_bottom(), egui::vec2(1.0, -1.0)),
    ];
    for (c, dir) in corners {
        let centre = c + egui::vec2(r * dir.x, r * dir.y);
        let mut pts = vec![c];
        for i in 0..=n {
            let a = std::f32::consts::FRAC_PI_2 * i as f32 / n as f32;
            let (s, co) = a.sin_cos();
            // sweep the quarter arc between the two edge tangent points
            pts.push(centre - egui::vec2(co * r * dir.x, s * r * dir.y));
        }
        p.add(egui::Shape::convex_polygon(pts, SEAM, Stroke::NONE));
    }
}

/// The recovery card (owner decision 2026-10-06, direction B), floating in the Board box `board` on a
/// document tab — Home never draws it (Start lists the copies itself). Review opens the copies in
/// place; its clicks are Start's actions, through the one adapter (`host::start_command`): Restore =
/// `Recover`, Discard = `DiscardRecovery` (the host asks first), Later / × = `DeferRecovery`.
pub(crate) fn build_recovery_card(
    ctx: &egui::Context,
    board: egui::Rect,
    recovery: &crate::recovery_host::RecoveryUi,
    commands: &mut Vec<AppCommand>,
) {
    let rows = if recovery.banner { recovery.rows.as_slice() } else { &[] };
    for action in varos_app::recovery_card::show(ctx, board, rows, &recovery.footer) {
        commands.extend(crate::host::start_command(action));
    }
    if rows.is_empty() {
        if let (Some(sid), Some(notice)) = (recovery.sid, recovery.recovered_notice.as_deref()) {
            if varos_app::recovery_card::show_restored(ctx, board, sid.0, notice) {
                commands.push(AppCommand::SaveAs(sid));
            }
        }
    }
}

/// Status mirror: recovery state on the left; artboard, Fit and zoom on the right.
pub(crate) fn build_statusbar(
    root: &mut egui::Ui,
    artboards: (usize, usize),
    zoom: f32,
    fit_icon: &Option<egui::TextureHandle>,
    fit_request: &mut Option<usize>,
    recovery_status: &str,
    ops: &mut Vec<Op>,
) {
    let (ab_active, ab_count) = artboards;
    let frame = egui::Frame { fill: SEAM, inner_margin: Margin::ZERO, ..Default::default() };
    // 31 = 25 of bar + the 6pt float-gap under the boxes, folded IN so the text centres in the
    // strip the eye actually sees (Ahmed 07-07: "مش متوسطنة في الارتفاع")
    egui::Panel::bottom("statusbar").exact_size(31.0).frame(frame).show_separator_line(false).show(root, |ui| {
        let bar = ui.max_rect();
        let p = ui.painter().clone();
        let cy = bar.center().y;
        let f11 = FontId::proportional(11.0);
        let m11 = numeric_value(11.0);
        let status_rect = egui::Rect::from_min_max(
            bar.min + egui::vec2(10.0, 0.0),
            egui::pos2((bar.right() - 240.0).max(bar.left() + 10.0), bar.bottom()),
        );
        status::hint(ui, status_rect, recovery_status);
        // right, laid right→left: zoom % · Fit · Artboard i/n (gap 14)
        let zr = egui::Rect::from_center_size(
            egui::pos2(bar.right() - STATUS_ZOOM_W * 0.5 - 10.0, cy),
            egui::vec2(STATUS_ZOOM_W, FIELD_H),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(zr), |ui| {
            fields::num(
                ui,
                STATUS_ZOOM_W,
                Lab::Letter("%"),
                "Zoom percent",
                zoom * 100.0,
                0,
                1.0,
                5.0..=4000.0,
                ops,
                Op::Zoom,
            );
        });
        let fw = 13.0 + 4.0 + p.layout_no_wrap("Fit".into(), f11.clone(), MUTED).size().x;
        let fit_r = egui::Rect::from_min_size(egui::pos2(zr.left() - 14.0 - fw, cy - 9.0), egui::vec2(fw, 18.0));
        let fresp = ui.interact(fit_r, ui.id().with("st-fit"), egui::Sense::click());
        let fcol = if fresp.hovered() { TEXT } else { MUTED };
        if let Some(t) = fit_icon {
            p.image(
                t.id(),
                egui::Rect::from_center_size(egui::pos2(fit_r.left() + 6.5, cy), egui::Vec2::splat(ICON_SM)),
                UV01(),
                fcol,
            );
        }
        p.text(egui::pos2(fit_r.left() + 17.0, cy), Align2::LEFT_CENTER, "Fit", f11.clone(), fcol);
        if fresp.clicked() {
            *fit_request = Some(ab_active);
        }
        if ab_count > 0 {
            let nr = p.text(
                egui::pos2(fit_r.left() - 14.0, cy),
                Align2::RIGHT_CENTER,
                format!("{} / {}", ab_active + 1, ab_count),
                m11,
                MUTED,
            );
            p.text(egui::pos2(nr.left() - 4.0, cy), Align2::RIGHT_CENTER, "Artboard", f11, MUTED);
        }
    });
}

/// Windows mirrors the same command table as the native menu, including nested row labels.
fn command_rows(ui: &mut egui::Ui, id: SessionId, cmds: &mut Vec<AppCommand>) -> bool {
    use crate::menus::{Entry, MenuCmd};
    use varos_core::editor::wave::{ObjectAction as O, Selection as S};
    use winit::keyboard::KeyCode as K;
    fn walk(ui: &mut egui::Ui, id: SessionId, rows: &[Entry], cmds: &mut Vec<AppCommand>, prefix: &str) -> bool {
        let mut hit = false;
        for row in rows {
            match row {
                Entry::Sub { label, items } => {
                    hit |= walk(ui, id, items, cmds, &format!("{prefix}{label} / "));
                }
                Entry::Item { label, cmd, .. } => {
                    let action = match *cmd {
                        MenuCmd::TogglePasteRemembersLayers => Some(AppCommand::TogglePasteRemembersLayers),
                        MenuCmd::View(s) => Some(AppCommand::View(id, s)),
                        MenuCmd::Selection(s) => Some(AppCommand::Selection(id, s)),
                        MenuCmd::Object(s) => Some(AppCommand::Object(id, s)),
                        MenuCmd::Key(k) => match (k.code, k.shift, k.alt) {
                            (K::KeyA, false, false) => Some(AppCommand::Selection(id, S::All)),
                            (K::KeyA, true, false) => Some(AppCommand::Selection(id, S::Deselect)),
                            (K::KeyA, false, true) => Some(AppCommand::Selection(id, S::Artboard)),
                            (K::Digit0, false, true) => Some(AppCommand::FitAll(id)),
                            (K::Digit5, false, alt) => Some(AppCommand::View(
                                id,
                                if alt {
                                    varos_core::editor::view_commands::ViewAction::ReleaseGuides
                                } else {
                                    varos_core::editor::view_commands::ViewAction::MakeGuides
                                },
                            )),
                            (K::Quote, false, false) => {
                                Some(AppCommand::View(id, varos_core::editor::view_commands::ViewAction::ToggleGrid))
                            }
                            (K::Digit6, false, false) => Some(AppCommand::Selection(id, S::Reselect)),
                            (K::BracketRight, false, true) => Some(AppCommand::Selection(id, S::Above)),
                            (K::BracketLeft, false, true) => Some(AppCommand::Selection(id, S::Below)),
                            (K::Digit2, false, alt) => {
                                Some(AppCommand::Object(id, if alt { O::UnlockAll } else { O::Lock }))
                            }
                            (K::Digit3, false, alt) => {
                                Some(AppCommand::Object(id, if alt { O::ShowAll } else { O::Hide }))
                            }
                            (K::KeyJ, false, alt) => {
                                Some(AppCommand::Object(id, if alt { O::Average } else { O::Join }))
                            }
                            (K::Digit8, false, false) => Some(AppCommand::Object(id, O::CompoundMake)),
                            (K::Digit8, true, true) => Some(AppCommand::Object(id, O::CompoundRelease)),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(action) = action {
                        if menu_row(ui, &format!("{prefix}{label}"), "") {
                            cmds.push(action);
                            hit = true;
                        }
                    }
                }
                Entry::Sep => {}
                Entry::Native(_) => {}
            }
        }
        hit
    }
    let mut hit = false;
    for (title, rows) in crate::menus::menus() {
        if matches!(title, "Select" | "Object" | "View") {
            let popup_id = doc_id(ui, ("burger-commands", title));
            let rect =
                egui::Rect::from_min_size(ui.next_widget_position(), egui::vec2(ui.available_width(), MENU_ROW_H));
            if menu_row(ui, title, "") {
                menu_toggle(ui, popup_id);
            }
            let anchor = ui.interact(rect, popup_id.with("anchor"), egui::Sense::hover());
            menu_below(ui, popup_id, &anchor, None, |ui| {
                ui.set_width(varos_app::shell::tokens::KIT_COMMAND_MENU_W);
                hit |= walk(ui, id, &rows, cmds, "");
            });
        }
    }
    hit
}
#[path = "status.rs"]
mod status;
