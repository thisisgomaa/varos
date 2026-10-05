use super::*;

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

/// A 34×30 top-bar icon button (menu/search/layout/panels). Returns its Response.
pub(crate) fn topbtn(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    tex: &Option<egui::TextureHandle>,
    key: &str,
    active: bool,
) -> egui::Response {
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click());
    let rr = CornerRadius::same(3); // §3.5: control radius r
    if active {
        p.rect_filled(rect, rr, BG_SURFACE);
    } else if resp.hovered() {
        p.rect_filled(rect, rr, HOVER);
    }
    let col = if active || resp.hovered() { TEXT } else { MUTED };
    if let Some(t) = tex {
        p.image(t.id(), egui::Rect::from_center_size(rect.center(), egui::vec2(17.0, 17.0)), UV01(), col);
    }
    resp
}

/// §3.5 app-bar text button (pad 5 12, radius r): solid = surface fill + line2 border;
/// ghost = bare muted text that lights on hover. Uses the shared top-bar layout rectangle.
pub(crate) fn bar_btn(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    label: &str,
    ghost: bool,
) -> egui::Response {
    let f = FontId::proportional(12.0);
    let resp = ui.interact(rect, ui.id().with(("bar-btn", label)), egui::Sense::click());
    let rr = CornerRadius::same(3);
    if ghost {
        if resp.hovered() {
            p.rect_filled(rect, rr, HOVER);
        }
    } else {
        p.rect_filled(rect, rr, if resp.hovered() { HOVER } else { BG_SURFACE });
        p.rect_stroke(rect, rr, Stroke::new(1.0, BORDER_2), StrokeKind::Middle);
    }
    let col = if ghost && !resp.hovered() { MUTED } else { TEXT };
    p.text(rect.center(), Align2::CENTER_CENTER, label, f, col);
    resp
}

/// A bar button that ISN'T available yet (Export / Share, DFS S1 §3.6): DISABLED text, no hover fill,
/// `Sense::hover` only — it can never register a click, so it cannot become an "enabled dead button"
/// (spec §2 forbids those). A tooltip carries the reason.
pub(crate) fn bar_btn_disabled(ui: &mut egui::Ui, p: &egui::Painter, rect: egui::Rect, label: &str, tip: &str) {
    let f = FontId::proportional(12.0);
    let resp = ui.interact(rect, ui.id().with(("bar-btn-disabled", label)), egui::Sense::hover());
    p.text(rect.center(), Align2::CENTER_CENTER, label, f, DISABLED);
    resp.on_hover_text(tip);
}

/// Width measurement for the shared top-bar layout (visual mirror; search has no home yet).
/// QW7: no "⌘ K" badge — the pill must not advertise a shortcut it doesn't run.
pub(crate) fn search_pill_width(p: &egui::Painter) -> f32 {
    let sw = p.layout_no_wrap("Search".into(), FontId::proportional(11.5), MUTED).size().x;
    9.0 + 13.0 + 6.0 + sw + 9.0
}

/// Home's body under the top bar: THE Start page plus the bar's live "Search boards" field. Filter
/// actions (tag / search / view) change the Start model; every other action becomes its `AppCommand`
/// through the one adapter (`host::start_command`).
pub(crate) fn home_body(
    root: &mut egui::Ui,
    page: &mut varos_app::start_page::StartPage,
    model: &mut varos_app::start::StartModel,
    warning: Option<&str>,
    cmds: &mut Vec<AppCommand>,
) {
    let mut actions = page.draw(root, model, warning);
    if let Some(rect) = root.ctx().data(|d| d.get_temp::<egui::Rect>(home_search_rect_id())) {
        actions.extend(page.search_box(root, rect, model));
    }
    for action in actions {
        if !model.apply(&action) {
            cmds.extend(crate::host::start_command(action));
        }
    }
}

/// Where the top bar left Home's "Search boards" field this frame (egui temp data, written by
/// `build_topbar` on Home, read by `run_home`).
pub(crate) fn home_search_rect_id() -> egui::Id {
    egui::Id::new("varos-home-search-rect")
}

/// Paint the search pill inside its shared top-bar layout rectangle. QW7: just "Search", muted — the
/// pill has no function yet, so it must not claim a ⌘K it doesn't run.
pub(crate) fn search_pill(ui: &mut egui::Ui, p: &egui::Painter, rect: egui::Rect, icon: &Option<egui::TextureHandle>) {
    let cy = rect.center().y;
    let f = FontId::proportional(11.5);
    // `Sense::hover` only (never clickable) — honest about having no function yet, with the reason
    // in the tooltip (spec §2 forbids an "enabled dead button"; UI audit finding 1).
    ui.interact(rect, ui.id().with("tb-kpill"), egui::Sense::hover()).on_hover_text("Search isn't available yet.");
    let rr = CornerRadius::same(3);
    p.rect_filled(rect, rr, BG_SURFACE);
    p.rect_stroke(rect, rr, Stroke::new(1.0, BORDER), StrokeKind::Middle);
    let mut x = rect.left() + 9.0;
    if let Some(t) = icon {
        p.image(t.id(), egui::Rect::from_center_size(egui::pos2(x + 6.5, cy), egui::vec2(13.0, 13.0)), UV01(), MUTED);
    }
    x += 13.0 + 6.0;
    p.text(egui::pos2(x, cy), Align2::LEFT_CENTER, "Search", f, MUTED);
}

/// One document tab: dirty dot, name, tooltip, × on hover, painted at `rect` (its resting slot, or
/// its lifted / reflowed rect during a drag — P16). `Sense::click_and_drag` so `tab_drag_update` can
/// read this chip's drag start / stop from egui. Returns `(response, close_clicked)` — the caller
/// reads `response.clicked()` / `.clicked_by(PointerButton::Middle)`.
pub(crate) fn tab_item(
    ui: &mut egui::Ui,
    p: &egui::Painter,
    rect: egui::Rect,
    tab: &TabView,
    active: bool,
    tex_x: &Option<egui::TextureHandle>,
    key: &str,
) -> (egui::Response, bool) {
    // Brave chip in the void (§3.5): active = filled panel block; inactive = bare muted text,
    // hover = a whisper of white. No accent — azure is a scalpel, not a tab decoration.
    let resp = ui.interact(rect, ui.id().with(key), egui::Sense::click_and_drag());
    let rr = CornerRadius::same(RBOX);
    if active {
        p.rect_filled(rect, rr, SOLID_PANEL);
    } else if resp.hovered() {
        p.rect_filled(rect, rr, VOID_HOVER);
    }
    let mut text_x = rect.left() + 12.0;
    if tab.dirty {
        // the neutral unsaved-changes dot — MUTED, never azure (azure is a scalpel: active/selection/
        // focus only, spec's visual constitution).
        p.circle_filled(egui::pos2(text_x + 2.0, rect.center().y), 2.5, MUTED);
        text_x += 10.0;
    }
    p.text(
        egui::pos2(text_x, rect.center().y),
        Align2::LEFT_CENTER,
        &tab.label,
        FontId::proportional(12.0),
        if active { TEXT } else { MUTED },
    );
    let x_r = crate::chrome::tab_close_rect(rect);
    let xr = ui.interact(x_r, ui.id().with((key, "x")), egui::Sense::click());
    if xr.hovered() {
        p.rect_filled(x_r, CornerRadius::same(4), HOVER);
    }
    if let Some(t) = tex_x {
        p.image(
            t.id(),
            egui::Rect::from_center_size(x_r.center(), egui::vec2(11.0, 11.0)),
            UV01(),
            if xr.hovered() { TEXT } else { MUTED },
        );
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

/// Custom top bar (the native caption is stripped in WM_NCCALCSIZE): menu · tabs · drag · right tools ·
/// window controls. Interactive rects are published as exclusions so the OS hit-test makes them HTCLIENT
/// (egui handles them) while the empty band is HTCAPTION (the OS drags/snaps the window).
#[allow(clippy::too_many_arguments)] // hand-painted panel builder: each arg is live UI state, split deferred with ui.rs
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
    maximized: bool,
    home: bool,
    native_home: bool,
    export_anchor: &mut Option<egui::Rect>,
) {
    let h = crate::chrome::TOPBAR.height;
    // DFS S6: Export (button, burger row, File ▸ Export ▸ PDF…) is ONE command through the one mapper
    let export_cmd = crate::host::to_app_command(crate::chrome::FileCmd::Export, active);
    // Stage 1 (BOX_SYSTEM_PLAN §3.5): the app bar IS the void — seam fill, no hairline; the doc tabs
    // are Brave-style chips floating in it and the window caps are flush 42px void cells.
    let frame = egui::Frame { fill: SEAM, inner_margin: Margin::ZERO, ..Default::default() };
    // no separator line — the bar melts into the void below it (Ahmed 07-07 "في خط لسا موجود")
    egui::Panel::top("topbar").exact_size(h).frame(frame).show_separator_line(false).show(root, |ui| {
        let bar = ui.max_rect();
        let p = ui.painter().clone();
        let text_width = |text: &str| p.layout_no_wrap(text.to_owned(), FontId::proportional(12.0), TEXT).size().x;
        let tab_widths: Vec<f32> = tabs.iter().map(|t| text_width(&t.label)).collect();
        let active_index = active.and_then(|id| tabs.iter().position(|t| t.id == id));
        let button_widths = [text_width("Window"), text_width("Share"), text_width("Export")];
        // Home: the search slot is Start's live "Search boards" field (200 wide, the mockup), placed by
        // `run_home` at the rect left in `HOME_SEARCH_RECT`; in a document it is today's pill
        let search_width = if home { varos_app::shell::tokens::SB_SEARCH_W } else { search_pill_width(&p) };
        let layout_for = |widths: &[f32], active: Option<usize>| {
            crate::chrome::topbar_layout(bar, crate::chrome::TOPBAR, button_widths, search_width, widths, active)
        };
        let layout = layout_for(&tab_widths, active_index);
        if home {
            ui.ctx().data_mut(|d| d.insert_temp(home_search_rect_id(), layout.search));
        }

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

        // right cluster (§3.5), right→left: window caps · [snapping] · Window · Share · Export · search pill
        let window_id = ui.make_persistent_id("window_menu");
        let menu_id = ui.make_persistent_id("app_menu");
        // magnet = the Snapping quick-menu (Illustrator layout)
        let magnet_id = ui.make_persistent_id("snap_menu");
        let magnet_r = layout.magnet;
        let (magr, winb) = if !home {
            let magnet_active = menu_open(ui, magnet_id) || snap.smart || snap.grid;
            let magr = topbtn(ui, &p, magnet_r, &top.magnet, "tb-magnet", magnet_active);
            if magr.clicked() {
                menu_toggle(ui, magnet_id);
            }
            // Window — every panel one click away, landing in an AUTOMATIC spot (Ahmed 07-07; replaces
            // the old layout/panels buttons)
            let winb = bar_btn(ui, &p, layout.window, "Window", true);
            if winb.clicked() {
                menu_toggle(ui, window_id);
            }
            // Share honesty (DFS S1 §3.6, review nit F15/P3-15): no home yet, so it looks and behaves
            // disabled instead of being an "enabled dead button" (spec §2 forbids those).
            bar_btn_disabled(ui, &p, layout.share, "Share", "Share isn't available yet.\nSave keeps an editable .vrs.");
            // Export (DFS S6): opens the Export PDF sheet, which hangs from this button
            *export_anchor = Some(layout.export);
            match &export_cmd {
                Some(cmd) => {
                    if bar_btn(ui, &p, layout.export, "Export", true).on_hover_text("Export PDF").clicked() {
                        cmds.push(cmd.clone());
                    }
                }
                None => bar_btn_disabled(ui, &p, layout.export, "Export", "Open a document to export it."),
            }
            // search pill: 🔍 Search — a surface capsule on the void (visual mirror; no function yet, QW7)
            let kpill_r = layout.search;
            search_pill(ui, &p, kpill_r, &top.search);
            (magr, winb)
        } else {
            (
                ui.interact(layout.magnet, ui.id().with("hidden-magnet"), egui::Sense::hover()),
                ui.interact(layout.window, ui.id().with("hidden-window-menu"), egui::Sense::hover()),
            )
        };

        // burger — a flush 36×40 void cell at the far left (§3.5)
        let menu_r = layout.menu;
        let mr = ui.interact(menu_r, ui.id().with("file-menu-anchor"), egui::Sense::hover());
        if native_home || home {
            ui.scope_builder(egui::UiBuilder::new().max_rect(menu_r.shrink(2.0)), |ui| {
                use varos_app::shell::kit::{self, Control, Icon};
                let mut c = Control::new(ui.id().with("home-chip"), "Home");
                c.icon = Some(Icon::Home);
                c.selected = home;
                c.pointer_only = home;
                c.help = "Home";
                if kit::action(ui, c, true).activated {
                    cmds.push(AppCommand::Home);
                }
            });
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
        }

        // doc tabs — Brave chips floating in the void: h28, gap 4, width fits the name (§3.5).
        // `layout.tabs` carries each chip's ORIGINAL tab index — not always a 0..n prefix once the
        // active tab has displaced the greedy fit's last slot on overflow (F7).
        // P16 — drag-to-reorder: past egui's drag threshold the chip is LIFTED (painted under the
        // pointer, above the rest) and the other chips reflow live around a gap where it would land.
        // All geometry is `chrome::tab_drag_frame`; this block only keeps the drag state and paints.
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
            // others first, the lifted chip last — drawn (and hit-tested) on top
            Some((i, f)) => paint.extend(f.others.iter().copied().chain([(*i, f.lifted)])),
            None => paint.extend(layout.tabs.iter().copied()),
        }
        for (i, trect) in paint {
            let tab = &tabs[i];
            let (resp, close) = tab_item(ui, &p, trect, tab, Some(tab.id) == active, &top.x, &chip_key(i));
            if close || resp.clicked_by(egui::PointerButton::Middle) {
                cmds.push(AppCommand::CloseDocument(tab.id));
            } else if resp.clicked() {
                cmds.push(AppCommand::ActivateDocument(tab.id));
            }
        }
        if let Some(plus_r) = layout.plus.filter(|_| !home) {
            if topbtn(ui, &p, plus_r, &top.plus, "tb-plus", false).clicked() {
                cmds.push(AppCommand::NewBoard);
            }
        }

        // dropdowns — the app-bar menus are FLUSH seam extensions of the bar (Ahmed 07-07): same
        // colour, no separating line, hanging straight off its bottom edge. FIXED widths — measured,
        // never elastic (the intrinsic-width try read the whole screen and blew the menus wide open).
        let flush = Some(bar.bottom());
        menu_below(ui, menu_id, &mr, flush, |ui| {
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
            }
            menu_sep(ui);
            match &export_cmd {
                Some(cmd) => {
                    if menu_row(ui, "Export\u{2026}", "") {
                        cmds.push(cmd.clone());
                        hit = true;
                    }
                }
                None => menu_row_disabled(ui, "Export\u{2026}", "Open a document to export it."),
            }
            if menu_row(ui, "Home", "") {
                cmds.push(AppCommand::Home);
                hit = true;
            }
            if hit {
                menu_set(ui, menu_id, false);
            }
        });
        // the Window menu: chrome toggles up top, then EVERY dockable panel — ✓ = it's in the
        // layout; click = open in an automatic spot / surface its tab / close (boxtree::toggle_panel)
        if !home {
            menu_below(ui, window_id, &winb, flush, |ui| {
                ui.set_width(200.0);
                let mut hit = false; // a chosen item closes the menu (Illustrator; P7)
                if check_row(ui, "Tool rail", *show_rail) {
                    *show_rail = !*show_rail;
                    hit = true;
                }
                if check_row(ui, "Control bar", *show_dock) {
                    *show_dock = !*show_dock;
                    hit = true;
                }
                menu_sep(ui);
                for pnl in varos_app::shell::PanelId::DOCKABLE {
                    if check_row(ui, pnl.title(), shell.is_open(pnl)) {
                        shell.toggle_panel(pnl);
                        hit = true;
                    }
                }
                if hit {
                    menu_set(ui, window_id, false);
                }
            });
            // Snapping quick-menu (Illustrator "Snapping" popover)
            menu_below(ui, magnet_id, &magr, flush, |ui| {
                ui.set_width(216.0);
                let mut hit = false; // a chosen item closes the menu (Illustrator; P7)
                if check_row(ui, "Snap to Grid", snap.grid) {
                    snap.grid = !snap.grid;
                    hit = true;
                }
                if check_row(ui, "Snap to Point", snap.key_points) {
                    snap.key_points = !snap.key_points;
                    hit = true;
                }
                menu_sep(ui);
                if check_row(ui, &format!("Smart Guides  ({})", shortcut_label("U")), snap.smart) {
                    toggle_smart_guides(snap);
                    hit = true;
                }
                if check_row(ui, "    Alignment Guides", snap.alignment_guides) {
                    snap.alignment_guides = !snap.alignment_guides;
                    hit = true;
                }
                if check_row(ui, "    Geometric Guides", snap.object_geometry) {
                    snap.object_geometry = !snap.object_geometry;
                    hit = true;
                }
                if hit {
                    menu_set(ui, magnet_id, false);
                }
            });
        }
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

pub(crate) fn toggle_smart_guides(snap: &mut varos_core::model::SnapConfig) {
    snap.smart = !snap.smart;
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

/// Recovery choices live on Start; these neutral strips lead there or to Save As.
pub(crate) fn build_recovery_strip(
    root: &mut egui::Ui,
    recovery: &crate::recovery_host::RecoveryUi,
    commands: &mut Vec<AppCommand>,
) {
    if !recovery.banner && recovery.recovered_notice.is_none() {
        return;
    }
    use varos_app::shell::{
        kit::{self, Control},
        tokens as t,
    };
    egui::Panel::top("recovery-strip").frame(egui::Frame::NONE.fill(t::SEAM).inner_margin(t::KIT_PAD)).show(
        root,
        |ui| {
            if recovery.banner {
                kit::notice(ui, "Varos closed unexpectedly. Recovery copies are available.");
                kit::notice(ui, "Review copies from your last session before continuing.");
                ui.horizontal_wrapped(|ui| {
                    if kit::action(ui, Control::new(egui::Id::new("review-recovery"), "Review Recovery"), false)
                        .activated
                    {
                        commands.push(AppCommand::ReviewRecovery);
                    }
                    if kit::action(ui, Control::new(egui::Id::new("defer-recovery"), "Later"), false).activated {
                        commands.push(AppCommand::DeferRecovery);
                    }
                });
            }
            if let Some(notice) = &recovery.recovered_notice {
                kit::notice(ui, notice);
                if let Some(id) = recovery.sid {
                    if kit::action(ui, Control::new(egui::Id::new("save-recovered"), "Save As…"), false).activated {
                        commands.push(AppCommand::SaveAs(id));
                    }
                }
            }
        },
    );
}

/// Status mirror: recovery state on the left; artboard, Fit and zoom on the right.
pub(crate) fn build_statusbar(
    root: &mut egui::Ui,
    ab_active: usize,
    ab_count: usize,
    zoom: f32,
    fit_icon: &Option<egui::TextureHandle>,
    fit_request: &mut Option<usize>,
    recovery_status: &str,
) {
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
        p.with_clip_rect(status_rect).text(
            egui::pos2(status_rect.left(), cy),
            Align2::LEFT_CENTER,
            recovery_status,
            f11.clone(),
            MUTED,
        );
        ui.interact(status_rect, ui.id().with("recovery-status"), egui::Sense::hover()).on_hover_text(recovery_status);
        // right, laid right→left: zoom % · Fit · Artboard i/n (gap 14)
        let zr = p.text(
            egui::pos2(bar.right() - 10.0, cy),
            Align2::RIGHT_CENTER,
            format!("{:.0}%", zoom * 100.0),
            m11.clone(),
            MUTED,
        );
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
