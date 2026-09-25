#![windows_subsystem = "windows"] // no console window alongside the app
#![allow(deprecated)] // winit 0.30: the closure EventLoop::run + create_window are deprecated-but-present — the zero-drift path
//! Varos desktop shell: a winit window + wgpu canvas, with the entire UI chrome painted natively in
//! egui on the same wgpu surface (top bar, left tool rail + Fill/Stroke swatch, right inspector
//! dock). Canvas pointer input stays native (winit → Editor) so the pen feel is untouched; the egui
//! panels float over a full-bleed board. Dark skin + tokens from UI_FIGMA_SPEC
//! (#141313 / #1f1f22 / #262627 / #0c8ce9).

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use varos_core::editor::{AbDrag, AbHit, Drag, Editor, Mods, PenHint, TfHit, ToolKind, ZOrder};
use varos_core::geom::{self, Pt, View};
use varos_core::scene::{build_scene_in_view, scene_signature};
use varos_core::EditCommand;
use varos_render_wgpu::Renderer;
#[cfg(windows)]
use winit::platform::windows::WindowAttributesExtWindows;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

mod app_command;
mod chrome;
mod cursors;
mod file_ports;
mod host;
mod lifecycle;
#[cfg(target_os = "macos")]
mod mac_caption;
#[cfg(target_os = "macos")]
mod mac_menu;
mod single_instance;
mod ui;
mod workspace;
use app_command::{AppCommand, OpenOrigin, SessionId, WindowCmd};
use cursors::CK;

/// The one cursor this frame wants: a pan in progress beats the Space hand, which beats the chrome's
/// own cursor (`chrome` = Some while the pointer is over a panel), which beats the tool's (lazy). Pure.
fn resolve_ck(panning: bool, space_down: bool, chrome: Option<CK>, tool: impl FnOnce() -> CK) -> CK {
    if panning {
        CK::Grab
    } else if space_down {
        CK::Hand
    } else if let Some(c) = chrome {
        c
    } else {
        tool()
    }
}

/// Windows: our WM_SETCURSOR subclass owns the OS cursor, so it is set only when it changes. Elsewhere
/// the OS can put its own cursor up between frames (egui-winit's writes off macOS; on macOS AppKit's
/// cursor rect when the pointer enters the view), so the resolved cursor is re-asserted every frame
/// after egui's platform output — otherwise e.g. Space-hand → over a splitter → back leaves the
/// arrow up while `CK` never changed (docs/foundation/MAC_SHELL_PORT.md).
const REASSERT_CURSOR_EACH_FRAME: bool = cfg!(not(windows));

/// Should `ck` be (re)applied this frame? Pure; unit-tested for both platform policies.
fn cursor_apply_needed(last: Option<CK>, ck: CK, reassert_each_frame: bool) -> bool {
    reassert_each_frame || last != Some(ck)
}

/// AppKit owns the cursor outside our focused content view.
/// Keep physical containment separate from focus so reactivation can restore an unchanged cursor
/// without waiting for a mouse move. Focus loss always relinquishes effective ownership.
#[cfg(any(target_os = "macos", test))]
fn native_cursor_apply_needed(pointer_inside: bool, focused: bool) -> bool {
    pointer_inside && focused
}

/// Which native cursor the current effective tool wants (Pen reports its contextual state; the
/// Selection tool reports its transform/copy states).
fn desired_ck(ed: &Editor, world: Pt) -> CK {
    if let Drag::Scale { handle, angle, .. } = ed.drag {
        return resize_ck(handle, angle);
    }
    if let AbDrag::Resize { handle, .. } = ed.ab_drag {
        return resize_ck(handle, 0.0);
    }
    if matches!(ed.ab_drag, AbDrag::Move { .. }) {
        return CK::Select;
    }
    if matches!(ed.ab_drag, AbDrag::Create { .. }) {
        return CK::Cross;
    }
    if ed.mods.alt && matches!(ed.drag, Drag::Object { .. } | Drag::DupPending { .. }) {
        return CK::Copy;
    }
    match ed.eff_tool() {
        ToolKind::Object => match ed.transform_hit(world) {
            Some(TfHit::Scale(i)) => resize_ck(i, ed.obj_angle),
            Some(TfHit::Rotate(i)) => rotate_ck(i, ed.obj_angle),
            None if ed.mods.alt && ed.path_under(world).is_some() => CK::Copy,
            None => CK::Select,
        },
        ToolKind::Direct => CK::Direct,
        ToolKind::Convert => CK::Convert,
        ToolKind::Eyedropper => CK::Eye,
        ToolKind::Pen => match ed.pen_hint(world) {
            PenHint::New => CK::PenNew,
            PenHint::Add => CK::PenAdd,
            PenHint::Delete => CK::PenDel,
            PenHint::Close => CK::PenClose,
            PenHint::Connect => CK::PenConnect,
            PenHint::Draw => CK::Pen,
        },
        ToolKind::Artboard => match ed.ab_hit(world) {
            Some(AbHit::Handle(i)) => resize_ck(i, 0.0), // ↔ on a page resize handle
            Some(AbHit::Body(_)) => CK::Select,          // arrow over a page (click to select / move)
            None => CK::Cross,                           // empty board (drag to create a page)
        },
        _ => CK::Cross,
    }
}

/// Pick the resize double-arrow for a transform handle, accounting for the frame's rotation.
fn resize_ck(handle: u8, angle: f32) -> CK {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
    let base = match handle {
        5 => 0.0,
        7 => PI,
        4 => -FRAC_PI_2,
        6 => FRAC_PI_2,
        1 => -FRAC_PI_4,
        3 => 3.0 * FRAC_PI_4,
        0 => -3.0 * FRAC_PI_4,
        2 => FRAC_PI_4,
        _ => 0.0,
    };
    let a = (base + angle).rem_euclid(PI);
    match ((a / FRAC_PI_4).round() as i32) % 4 {
        0 => CK::ResizeH,
        1 => CK::ResizeNW,
        2 => CK::ResizeV,
        3 => CK::ResizeNE,
        _ => CK::ResizeH,
    }
}

/// Pick the rotate cursor for a corner (0=TL,1=TR,2=BR,3=BL), accounting for frame rotation.
fn rotate_ck(corner: u8, angle: f32) -> CK {
    use std::f32::consts::{FRAC_PI_4, PI};
    let base = match corner {
        0 => 1.25 * PI,
        1 => 1.75 * PI,
        2 => 0.25 * PI,
        3 => 0.75 * PI,
        _ => 0.25 * PI,
    };
    let a = (base + angle).rem_euclid(2.0 * PI);
    match ((a / FRAC_PI_4).round() as i32) % 8 {
        0 => CK::RotateE,
        1 => CK::RotateSE,
        2 => CK::RotateS,
        3 => CK::RotateSW,
        4 => CK::RotateW,
        5 => CK::RotateNW,
        6 => CK::RotateN,
        7 => CK::RotateNE,
        _ => CK::RotateE,
    }
}

// ============================ helpers ============================

/// The control bar's idle label for the current tool (`ui.rs`).
fn tool_name(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Pen => "Pen (P)",
        ToolKind::Direct => "Direct Select (A)",
        ToolKind::Object => "Select (V)",
        ToolKind::Rect => "Rectangle (M)",
        ToolKind::Ellipse => "Ellipse (L)",
        ToolKind::Triangle => "Triangle",
        ToolKind::Polygon => "Polygon",
        ToolKind::Convert => "Convert",
        ToolKind::Eyedropper => "Eyedropper (I)",
        ToolKind::Artboard => "Artboard (Shift+O)",
        ToolKind::Rotate => "Rotate (R)",
        ToolKind::Scale => "Scale (S)",
    }
}

/// Apply a keyboard shortcut. `code` is a W3C key code; shared by canvas focus + forwarded keys.
/// `canvas_centre` = the centre of the visible drawing area (physical px): the point the keyboard
/// zooms (⌘= / ⌘− / ⌘1) keep fixed, so the view never jumps away from the work.
fn apply_key(ed: &mut Editor, view: &mut View, canvas_centre: Pt, code: &str, ctrl: bool, shift: bool, alt: bool) {
    if ctrl {
        match code {
            "Semicolon" => {
                if alt {
                    ed.execute(EditCommand::ToggleGuidesLocked)
                }
                // Lock Guides (Alt+Ctrl+;)
                else {
                    ed.toggle_guides_visibility()
                }
            } // Hide/Show Guides (Ctrl+;)
            // Actual Size (⌘1): 100% around the canvas centre — never a jump to empty space (Astra F06)
            "Digit1" => zoom_to(view, canvas_centre, 1.0),
            // Zoom In / Out (⌘= or ⌘+ · ⌘−): one key step, instantly, around the canvas centre (Astra F05)
            "Equal" | "NumpadAdd" => zoom_step(view, canvas_centre, ZOOM_KEY_STEP),
            "Minus" | "NumpadSubtract" => zoom_step(view, canvas_centre, 1.0 / ZOOM_KEY_STEP),
            "KeyZ" => {
                if shift {
                    ed.execute(EditCommand::Redo)
                } else {
                    ed.execute(EditCommand::Undo)
                }
            }
            "KeyY" => ed.execute(EditCommand::Redo),
            "BracketRight" => ed.execute(EditCommand::Arrange(if shift { ZOrder::Front } else { ZOrder::Forward })),
            "BracketLeft" => ed.execute(EditCommand::Arrange(if shift { ZOrder::Back } else { ZOrder::Backward })),
            "KeyG" => {
                if shift {
                    ed.execute(EditCommand::UngroupSelection)
                } else {
                    ed.execute(EditCommand::GroupSelection)
                }
            }
            "KeyU" => ed.execute(EditCommand::ToggleSmartGuides), // Smart Guides toggle (Illustrator Ctrl+U)
            "KeyD" => ed.execute(EditCommand::TransformAgain), // Transform Again / step-and-repeat (Illustrator Ctrl+D)
            "KeyR" => ed.toggle_rulers_visibility(),           // Show/Hide Rulers (Illustrator Ctrl+R)
            // Edit ▸ Copy / Cut (⌘C / ⌘X) — the in-app clipboard. ⌘V / ⇧⌘V (Paste / Paste in Place)
            // need the canvas rect for a view-centred paste, so `doc_key` owns them.
            "KeyC" if !shift && !alt => ed.execute(EditCommand::Copy),
            "KeyX" if !shift && !alt => ed.execute(EditCommand::Cut),
            // Edit ▸ Select All (⌘A) / Deselect (⇧⌘A — the Escape path). Never reached from a focused
            // text field: the keyboard path skips canvas shortcuts there and the menu hands ⌘A to egui.
            "KeyA" if !alt => {
                if shift {
                    ed.escape()
                } else {
                    ed.select_all()
                }
            }
            _ => {}
        }
        return;
    }
    let s = if shift { 10.0 } else { 1.0 };
    match code {
        "KeyV" => ed.set_tool(ToolKind::Object),
        "KeyA" => ed.set_tool(ToolKind::Direct),
        "KeyP" => ed.set_tool(ToolKind::Pen),
        "KeyM" => ed.set_tool(ToolKind::Rect),
        "KeyL" => ed.set_tool(ToolKind::Ellipse),
        "KeyR" => ed.set_tool(ToolKind::Rotate), // Rotate tool (Illustrator R)
        "KeyS" => ed.set_tool(ToolKind::Scale),  // Scale tool (Illustrator S)
        "KeyI" => ed.set_tool(ToolKind::Eyedropper),
        "KeyO" => {
            if shift {
                ed.set_tool(ToolKind::Artboard);
            }
        }
        "KeyX" => {
            if shift {
                ed.execute(EditCommand::SwapColors)
            } else {
                ed.swap_paint()
            }
        }
        "KeyD" => ed.execute(EditCommand::DefaultPaint),
        "Slash" => ed.execute(EditCommand::ApplyPaint { target: ed.paint, color: None }),
        "Escape" | "Enter" => ed.escape(),
        "Delete" | "Backspace" => ed.execute(EditCommand::DeleteSelected),
        "ArrowLeft" => ed.execute(EditCommand::Nudge { x: -s, y: 0.0 }),
        "ArrowRight" => ed.execute(EditCommand::Nudge { x: s, y: 0.0 }),
        "ArrowUp" => ed.execute(EditCommand::Nudge { x: 0.0, y: -s }),
        "ArrowDown" => ed.execute(EditCommand::Nudge { x: 0.0, y: s }),
        _ => {}
    }
}

/// The ✓ a native menu row shows, for the states the EDITOR owns (None = a UI-shell state the `Ui`
/// answers). The macOS menu sync and the tests read the same function (MAC_CHROME.md §C).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn editor_check(ed: &Editor, c: chrome::Check) -> Option<bool> {
    use chrome::Check as C;
    Some(match c {
        C::Rulers => ed.show_rulers,
        C::Guides => !ed.guides_hidden,
        C::GuidesLocked => ed.doc.guides_locked,
        C::SmartGuides => ed.doc.snap.smart,
        C::SnapGrid => ed.doc.snap.grid,
        C::SnapPoint => ed.doc.snap.key_points,
        C::Rail | C::Dock | C::Panel(_) => return None,
    })
}

/// View ▸ Snap to Grid / Snap to Point — the magnet quick-menu's rows: flip the flag and commit it
/// through the same non-undoable `SetSnapConfig` the magnet menu's frame commit uses.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn menu_snap_toggle(ed: &mut Editor, grid: bool) {
    let mut s = ed.doc.snap;
    if grid {
        s.grid = !s.grid;
    } else {
        s.key_points = !s.key_points;
    }
    ed.execute(EditCommand::SetSnapConfig(s));
}

/// A8a fallback region when there is nothing to frame (an empty free canvas): a default page-sized
/// square at the origin, so a brand-new boardless document still opens onto something sensible.
const FIT_FALLBACK: (f32, f32, f32, f32) = (0.0, 0.0, 1080.0, 1080.0);

/// The world rect a "Fit in window" should frame: the ACTIVE page if the document has one, else the
/// artwork's bounds (a free canvas that has content but no board — A8a fits to content), else the
/// default fallback region so an empty document opens onto a sensible view. Never assumes a board.
fn fit_rect(ed: &Editor) -> (f32, f32, f32, f32) {
    if let Some(a) = ed.doc.active_artboard() {
        return (a.x, a.y, a.w, a.h);
    }
    // union outline bbox of all real paths = the artwork extent (fit-to-content on a free canvas)
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut any = false;
    for pi in 0..ed.doc.paths.len() {
        if ed.doc.paths[pi].anchors.is_empty() {
            continue;
        }
        let (bx0, by0, bx1, by1) = ed.doc.outline_bbox(pi);
        x0 = x0.min(bx0);
        y0 = y0.min(by0);
        x1 = x1.max(bx1);
        y1 = y1.max(by1);
        any = true;
    }
    if any {
        (x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0))
    } else {
        FIT_FALLBACK
    }
}

/// The CANVAS area (the visible drawing region) in physical px — the Board box's interior when the
/// shell reports one (Stage 4), else the whole window.
fn canvas_px(gui: &ui::Ui, window: &Window) -> egui::Rect {
    let sz = window.inner_size();
    gui.board_px
        .unwrap_or(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(sz.width as f32, sz.height as f32)))
}

/// Fit an artboard into the CANVAS area (`canvas_px`). Pan shifts so the page centres in the BOX.
fn fit_to_board(gui: &ui::Ui, window: &Window, x: f32, y: f32, w: f32, h: f32, k: f32) -> View {
    fit_in(canvas_px(gui, window), x, y, w, h, k)
}
/// Fit a world rect into the canvas area `b` (physical px).
fn fit_in(b: egui::Rect, x: f32, y: f32, w: f32, h: f32, k: f32) -> View {
    let mut v = View::fit(x, y, w, h, b.width(), b.height(), k);
    v.pan[0] += b.left();
    v.pan[1] += b.top();
    v
}

/// Snapshot of the current selection for the native egui Properties panel (spike, read-only).
fn load_icon() -> Option<winit::window::Icon> {
    let img = image::load_from_memory(include_bytes!("../icon.png")).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

/// Dev-only: render every v1 tool cursor (1× at 8×, 2× at 4×) over neutral gray for eyeballing.
fn dump_cursors() {
    let dir = "target/cursor-preview";
    let _ = std::fs::create_dir_all(dir);
    for e in cursors::v1_table() {
        let name = format!("{:?}-{}", e.ck, e.file.trim_end_matches(".svg"));
        for (px, scale) in [(32u32, 8u32), (64, 4)] {
            if let Some((rgba, w, h, _hx, _hy)) = cursors::v1_rgba(e.ck, px) {
                save_gray_png(&rgba, w as u32, h as u32, scale, &format!("{dir}/{name}@{px}.png"));
            }
        }
    }
    if !cursors::ai_override_enabled() {
        return;
    }
    let svg_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/cursors-ai/svg/");
    let mut report = String::new();
    // the local reference set (VAROS_CURSORS_AI), when present on this machine
    for ck in cursors::ALL_CURSORS {
        let (stem, _, _) = cursors::ai_svg(ck);
        match std::fs::read_to_string(format!("{svg_dir}{stem}.svg")) {
            Ok(svg) => match cursors::render_svg(&svg, 96, false) {
                Some((rgba, w, h)) => {
                    save_gray_png(&rgba, w, h, 1, &format!("{dir}/ai-{stem}.png"));
                    report.push_str(&format!("OK   {stem}  {w}x{h}\n"));
                }
                None => report.push_str(&format!("RENDERFAIL {stem}\n")),
            },
            Err(_) => report.push_str(&format!("MISSING {stem}\n")),
        }
    }
    let _ = std::fs::write(format!("{dir}/ai-report.txt"), report);
}

fn save_gray_png(rgba: &[u8], w: u32, h: u32, scale: u32, path: &str) {
    let (ow, oh) = (w * scale, h * scale);
    let mut out = vec![128u8; (ow * oh * 4) as usize];
    for px in out.chunks_mut(4) {
        px[3] = 255;
    }
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let a = rgba[i + 3] as u32;
            if a == 0 {
                continue;
            }
            let mix = |c: u32| ((c * a + 128 * (255 - a)) / 255) as u8;
            let (cr, cg, cb) = (mix(rgba[i] as u32), mix(rgba[i + 1] as u32), mix(rgba[i + 2] as u32));
            for dy in 0..scale {
                for dx in 0..scale {
                    let oi = (((y * scale + dy) * ow + (x * scale + dx)) * 4) as usize;
                    out[oi] = cr;
                    out[oi + 1] = cg;
                    out[oi + 2] = cb;
                    out[oi + 3] = 255;
                }
            }
        }
    }
    if let Some(img) = image::RgbaImage::from_raw(ow, oh, out) {
        let _ = img.save(path);
    }
}

fn preview_svgs(dir: &str) {
    let out = format!("{dir}/png");
    let _ = std::fs::create_dir_all(&out);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("svg") {
            continue;
        }
        let Ok(svg) = std::fs::read_to_string(&p) else {
            continue;
        };
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("x").to_string();
        if let Some((rgba, w, h)) = cursors::render_svg(&svg, 64, true) {
            save_gray_png(&rgba, w, h, 4, &format!("{out}/{name}.png"));
        }
    }
}

/// Where the remembered window geometry lives (`<data root>/window.txt` — still `%APPDATA%\Varos\window.txt`
/// on Windows). Stays off on macOS: Mac window memory is separate Mac polish (DFS work order Q4).
fn win_state_path() -> Option<std::path::PathBuf> {
    if cfg!(target_os = "macos") {
        return None;
    }
    varos_app::storage::paths::AppLayout::current().map(|l| l.window_state())
}
/// Crash-log home (`<data root>/Logs/crash.txt`) — the one app-data resolver, so macOS gets a crash log too.
fn crash_log_path() -> Option<std::path::PathBuf> {
    varos_app::storage::paths::AppLayout::current().map(|l| l.crash_log())
}
fn write_crash_log(msg: &str) -> Option<std::path::PathBuf> {
    let p = crash_log_path()?;
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(&p, msg).ok()?;
    Some(p)
}
/// The respectful fatal path (ENGINEERING_REVIEW §3.3): unrecoverable startup/OS failures get a readable
/// dialog + a crash log — never a bare panic (the app has no console, so a panic is an invisible death).
fn fatal(context: &str, detail: &str) -> ! {
    let logged = write_crash_log(&format!("VAROS FATAL\n{context}\n{detail}\n"));
    let mut body = format!("{context}\n\n{detail}");
    if let Some(p) = logged {
        body.push_str(&format!("\n\nDetails were saved to:\n{}", p.display()));
    }
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Varos couldn't start")
        .set_description(body)
        .show();
    std::process::exit(1);
}

/// One wheel notch of zoom (Alt+wheel) — fine, because a wheel gives many notches per gesture.
const ZOOM_NOTCH: f32 = 1.12;
/// One ⌘= / ⌘− press — Illustrator-sized (100 → 150 → 225 %…), because a key press is one deliberate step.
const ZOOM_KEY_STEP: f32 = 1.5;

/// Edit ▸ Paste (⌘V): the world translation that puts the clipboard's centre on the world point under
/// the centre of the CANVAS (`canvas_centre`, physical px) — Illustrator pastes into the middle of the
/// view. `None` when the clipboard is empty.
fn view_centre_paste_offset(ed: &Editor, view: &View, canvas_centre: Pt) -> Option<Pt> {
    let c = ed.clipboard().center()?;
    let t = view.s2w(canvas_centre);
    Some([t[0] - c[0], t[1] - c[1]])
}

/// ⌘V = Paste centred in the canvas · ⇧⌘V = Paste in Place (the copied coordinates).
fn paste_key(ed: &mut Editor, view: &View, canvas_centre: Pt, in_place: bool) {
    let offset = if in_place { None } else { view_centre_paste_offset(ed, view, canvas_centre) };
    ed.execute(EditCommand::Paste { offset });
}

/// Apply a complete zoom step now, keeping the world point under the cursor fixed.
fn zoom_step(view: &mut View, screen: Pt, factor: f32) {
    zoom_to(view, screen, view.zoom * factor);
}

/// Jump to `zoom` now (clamped to the view limits), keeping the world point at `screen` fixed.
fn zoom_to(view: &mut View, screen: Pt, zoom: f32) {
    let anchor = view.s2w(screen);
    view.zoom = zoom.clamp(0.05, 40.0);
    view.pan = geom::pan_for_anchor(anchor, screen, view.zoom);
}

/// One DOCUMENT shortcut key (a lifecycle key never gets here — `host::key_command` takes those):
/// ⌘0 Fit (the active page, or the content on a free canvas — A8a — or the default region), ⌘V /
/// ⇧⌘V Paste centred in the canvas / Paste in Place, else `apply_key`. The keyboard AND the macOS
/// menu bar (MAC_CHROME.md §C) both land here, so a menu row can never drift from its key.
/// `canvas` = the visible drawing area in physical px (`canvas_px`).
fn doc_key(ed: &mut Editor, view: &mut View, canvas: egui::Rect, code: KeyCode, m: Mods) {
    let c = canvas.center();
    if m.ctrl && matches!(code, KeyCode::Digit0 | KeyCode::Numpad0) {
        let (x, y, w, h) = fit_rect(ed);
        *view = fit_in(canvas, x, y, w, h, 0.9);
    } else if m.ctrl && !m.alt && code == KeyCode::KeyV {
        paste_key(ed, view, [c.x, c.y], m.shift);
    } else {
        apply_key(ed, view, [c.x, c.y], &format!("{code:?}"), m.ctrl, m.shift, m.alt);
    }
}

/// Apply a tab's owed fit (`DocumentSession::fit_pending`: a new / opened / re-maximized document)
/// once the Board box is known. Returns `true` when it was applied.
fn take_pending_fit(fit: &mut Option<f32>, ed: &Editor, view: &mut View, gui: &ui::Ui, window: &Window) -> bool {
    if gui.splashing() || gui.board_px.is_none() {
        return false;
    }
    let Some(k) = fit.take() else {
        return false;
    };
    let (x, y, w, h) = fit_rect(ed);
    *view = fit_to_board(gui, window, x, y, w, h, k);
    true
}

/// THE dispatch (DFS S1 §3.5): every queued `HostAction` runs here, in queue order — the window /
/// panel effects on the window, everything else through `run_action`.
#[allow(clippy::too_many_arguments)] // the event loop's own state, passed as-is
fn dispatch(
    action: host::HostAction,
    ws: &mut workspace::Workspace,
    gui: &mut ui::Ui,
    window: &Window,
    hwnd: isize,
    canvas: egui::Rect,
    dialogs: &mut dyn lifecycle::Dialogs,
    store: &mut dyn lifecycle::DocStore,
    keys: &host::Keyboard,
) -> host::Ran {
    match action {
        host::HostAction::App(AppCommand::Window(w)) => {
            match w {
                WindowCmd::Minimize => window.set_minimized(true),
                WindowCmd::ToggleMaximize => window.set_maximized(!cursors::is_maximized(hwnd)),
                #[cfg(target_os = "macos")]
                WindowCmd::ToggleRail => gui.toggle_rail(),
                #[cfg(target_os = "macos")]
                WindowCmd::ToggleDock => gui.toggle_dock(),
                #[cfg(target_os = "macos")]
                WindowCmd::TogglePanel(p) => gui.toggle_panel(p),
                // raised only by the macOS menu bar's Window rows
                #[cfg(not(target_os = "macos"))]
                WindowCmd::ToggleRail | WindowCmd::ToggleDock | WindowCmd::TogglePanel(_) => {}
            }
            host::Ran::default()
        }
        action => run_action(action, ws, gui, canvas, dialogs, store, keys),
    }
}

/// One queued action that is not a window command: a lifecycle command (`host::run_lifecycle`:
/// settle the active tab, run the rules over the ports, reset what a dialog or a switch left stale),
/// or a document action on whichever tab is active by then. No window: the FIFO test drives it
/// headless. `canvas` = the visible drawing area (`canvas_px`); `keys` = the keys held right now
/// (`host::Keyboard`), mirrored into the tab active after a command.
fn run_action(
    action: host::HostAction,
    ws: &mut workspace::Workspace,
    ui: &mut dyn host::DocUi,
    canvas: egui::Rect,
    dialogs: &mut dyn lifecycle::Dialogs,
    store: &mut dyn lifecycle::DocStore,
    keys: &host::Keyboard,
) -> host::Ran {
    match action {
        host::HostAction::App(cmd) => host::run_lifecycle(cmd, ws, ui, dialogs, store, keys),
        host::HostAction::Doc(a) => {
            if let Some(s) = ws.active_mut() {
                run_doc_action(a, &mut s.editor, &mut s.view, canvas);
            }
            host::Ran::default()
        }
    }
}

/// A document action on one tab: a shortcut key's path, or a magnet quick-menu row.
fn run_doc_action(a: host::DocAction, ed: &mut Editor, view: &mut View, canvas: egui::Rect) {
    match a {
        host::DocAction::Key(code, m) => doc_key(ed, view, canvas, code, m),
        host::DocAction::Snap { grid } => menu_snap_toggle(ed, grid),
    }
}

/// A file / tab key (⌘N ⌘O ⌘S ⇧⌘S ⌘W ⌘Q, Ctrl+Tab) is a command, decided at its press on the
/// keyboard's held modifiers (`host::Keyboard::key`) — never on a tab's own copy, which a tab switch
/// resets while the key is still down (owner 2026-09-25: held-Ctrl Tab · Tab switched only once).
/// Its press queues the command; its repeats (no queue of dialogs) and its release follow the press,
/// whatever the modifiers did since. Returns whether the key event belongs to a command key: then it
/// goes nowhere else — not to egui (so Ctrl+Tab cannot move egui's keyboard focus, UI audit 04 B4),
/// not to the document.
fn command_key(
    pending: &mut host::ActionQueue,
    keyboard: &mut host::Keyboard,
    code: KeyCode,
    active: Option<SessionId>,
    pressed: bool,
    repeat: bool,
) -> bool {
    match keyboard.key(code, pressed, repeat, active) {
        host::KeyRoute::Queue(cmd) => {
            pending.push(host::HostAction::App(cmd));
            true
        }
        host::KeyRoute::Swallow => true,
        host::KeyRoute::Pass => false,
    }
}

/// Raise a document action (a key, a menu row) on the active tab: it runs at once when nothing raised
/// earlier is still waiting (`ActionQueue::doc_runs_now` — no command, no click whose command is still
/// to come), else it joins the queue behind it — so the queue order is the event order (review P1: a
/// ⌘Z after a queued ⌘S runs after the Save; a ⌘Z after a click on tab B runs on B).
fn raise_doc(
    pending: &mut host::ActionQueue,
    a: host::DocAction,
    ed: &mut Editor,
    view: &mut View,
    canvas: egui::Rect,
) {
    if pending.doc_runs_now() {
        run_doc_action(a, ed, view, canvas);
    } else {
        pending.push(host::HostAction::Doc(a));
    }
}

/// Restore the last window geometry: `(maximized, outer_x, outer_y, inner_w, inner_h)` in physical px.
fn load_win_state() -> Option<(bool, i32, i32, u32, u32)> {
    let s = std::fs::read_to_string(win_state_path()?).ok()?;
    let mut it = s.split_whitespace();
    let maxed = it.next()? == "1";
    let (x, y) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    let (w, h): (u32, u32) = (it.next()?.parse().ok()?, it.next()?.parse().ok()?);
    if w < 320 || h < 240 {
        return None;
    } // ignore absurd/degenerate saved sizes
    Some((maxed, x, y, w, h))
}
fn save_win_state(maxed: bool, x: i32, y: i32, w: u32, h: u32) {
    if let Some(p) = win_state_path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, format!("{} {} {} {} {}", maxed as u8, x, y, w, h));
    }
}

fn main() {
    // The user-facing safety net (ENGINEERING_REVIEW §3.3 #4): ANY panic — including paths no table
    // ever enumerates — writes a crash log and shows a readable dialog instead of dying silently.
    // target/panic.txt stays as the dev breadcrumb.
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("VAROS PANIC\n{info}\n");
        let _ = std::fs::create_dir_all("target");
        let _ = std::fs::write("target/panic.txt", &msg);
        let logged = write_crash_log(&msg);
        let mut body = String::from("Something went wrong and Varos has to close.\nYour last saved file is untouched.");
        if let Some(p) = logged {
            body.push_str(&format!("\n\nA crash log was saved to:\n{}", p.display()));
        }
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("Varos crashed")
            .set_description(body)
            .show();
    }));
    if std::env::args().any(|a| a == "--dump-cursors") {
        dump_cursors();
        return;
    }
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(i) = args.iter().position(|a| a == "--dump-tool-icons") {
            ui::dump_tool_icons(args.get(i + 1).map(|s| s.as_str()).unwrap_or("rail.png"));
            return;
        }
    }
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(i) = args.iter().position(|a| a == "--preview") {
            if let Some(dir) = args.get(i + 1) {
                preview_svgs(dir);
            }
            return;
        }
    }
    let file_arg = single_instance::file_arg_from_env();
    let _single_instance = match single_instance::acquire_or_forward(file_arg.as_deref()) {
        Some(guard) => guard,
        None => return,
    };
    // DFS S1: every tab is its own document (editor + view + file + saved checkpoint); the host
    // always works on the ACTIVE session (never empty in S1: it starts as a pristine Untitled-1).
    let mut ws = workspace::Workspace::new();
    // The ONE FIFO action queue (review P1): the lifecycle / window commands (keys, native menu, tab
    // strip, burger, window controls, OS close, files handed in) queue here and run through `dispatch`
    // once the loop is about to wait; a document action a key or a menu row raises runs at once only
    // when nothing is waiting, else it queues behind (`raise_doc`) — so they all run in event order.
    // Pointer input and panel edits act on the editor directly. The first instance opens its OWN file
    // argument, once, after the first framed frame (F13).
    let mut pending = host::ActionQueue::default();
    // the held modifiers: ONE truth for the whole window (a tab switch must not forget a held Control)
    let mut keyboard = host::Keyboard::default();
    let mut startup_open = host::open_paths_command(file_arg.into_iter().collect(), OpenOrigin::CommandLine);
    // the lifecycle's ports: native dialogs + the disk
    let (mut dialogs, mut store) = (file_ports::RfdDialogs, file_ports::DiskStore);
    #[cfg(not(target_os = "macos"))]
    let event_loop = EventLoop::new();
    // macOS: winit's own default menu (app name only) is replaced by our menu bar (MAC_CHROME.md §C).
    #[cfg(target_os = "macos")]
    let event_loop = {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        EventLoop::builder().with_default_menu(false).build()
    };
    let event_loop = match event_loop {
        Ok(el) => el,
        Err(e) => fatal("Varos couldn't connect to the Windows desktop.", &e.to_string()),
    };
    let saved = load_win_state(); // remembered geometry from last session (None on first run)
                                  // winit 0.30 removed WindowBuilder — WindowAttributes carries the identical with_* methods
    let mut attrs = Window::default_attributes()
        .with_title(ws.active().map_or_else(|| "Varos".into(), |s| host::window_title(&s.display_name(), false)))
        .with_window_icon(load_icon())
        .with_visible(false) // created hidden — no visible flash at all
        .with_transparent(true) // lets the startup splash card float over the desktop
        .with_decorations(false) // borderless during the splash → no window shadow; the
        // editor frame (decorations + shadow + snap) is applied once the splash finishes, below.
        .with_min_inner_size(winit::dpi::LogicalSize::new(800.0, 560.0)); // floor: never a degenerate layout
    #[cfg(windows)]
    {
        attrs = attrs.with_class_name(single_instance::WINDOW_CLASS_NAME);
    }
    // macOS (MAC_CHROME.md §A/§B): ONE bar — our egui bar fills the title-bar area with the native
    // traffic lights over it — and an OPAQUE window (a transparent one let the title strip show the
    // desktop). Decorated from the start: winit's later set_decorations(true) would drop the
    // full-size content view, so macOS never toggles decorations.
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        attrs = attrs
            .with_transparent(false)
            .with_decorations(true)
            .with_fullsize_content_view(true)
            .with_titlebar_transparent(true)
            .with_title_hidden(true);
    }
    attrs = match saved {
        Some((_, _, _, w, h)) => attrs.with_inner_size(winit::dpi::PhysicalSize::new(w, h)),
        None => attrs.with_inner_size(winit::dpi::LogicalSize::new(1460.0, 860.0)),
    };
    #[allow(deprecated)]
    let window = match event_loop.create_window(attrs) {
        Ok(w) => Arc::new(w),
        Err(e) => fatal("Varos couldn't create its window.", &e.to_string()),
    };
    match saved {
        // re-open exactly where it was last time …
        Some((_, x, y, _, _)) => window.set_outer_position(PhysicalPosition::new(x, y)),
        // … or, first run, centre on the primary monitor (the splash card lands mid-screen)
        None => {
            if let Some(mon) = window.primary_monitor() {
                let (ms, mp, ws) = (mon.size(), mon.position(), window.outer_size());
                window.set_outer_position(PhysicalPosition::new(
                    mp.x + (ms.width as i32 - ws.width as i32) / 2,
                    mp.y + (ms.height as i32 - ws.height as i32) / 2,
                ));
            }
        }
    }
    let size = window.inner_size();
    // Cloak the window the instant it exists (before the slow GPU/webview setup) so the OS never
    // composites it — kills the startup white flash and the native-caption flash. Un-cloaked after
    // frame 0 below.
    let hwnd: isize = {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(w)) => w.hwnd.get(),
            _ => 0,
        }
    };
    // Non-Windows: there is no HWND, so the cursor/maximize fallbacks talk to the winit window directly.
    #[cfg(not(windows))]
    cursors::bind_window(window.clone());
    // macOS: the NSWindow's own background is the warm black, so no frame (the one before the GPU's
    // first present, a live-resize edge) ever shows the desktop or a system gray (MAC_CHROME.md §B).
    #[cfg(target_os = "macos")]
    {
        let bg = varos_app::shell::tokens::BG;
        mac_menu::set_window_background(&window, [bg.r(), bg.g(), bg.b()]);
    }
    // macOS: the native menu bar; installed on the first NewEvents (after the app finished launching).
    #[cfg(target_os = "macos")]
    let mac_menu = match mac_menu::MacMenu::build(event_loop.create_proxy()) {
        Ok(m) => Some(m),
        Err(e) => {
            eprintln!("[varos] menu bar: could not be built ({e}) — running without it");
            None
        }
    };
    single_instance::install_file_open_handler(hwnd);
    cursors::set_cloaked(hwnd, true);
    window.set_visible(true); // now "shown" but cloaked → not composited (no flash), surface is presentable
    let mut renderer = match pollster::block_on(Renderer::new(window.clone(), size.width, size.height)) {
        Ok(r) => r,
        Err(e) => {
            fatal("Varos couldn't start its graphics engine.\nUpdating your graphics driver usually fixes this.", &e)
        }
    };

    let scale = window.scale_factor();

    let mut gui = ui::Ui::new(&window); // native egui UI (spike) — paints on our surface via render_ui
    gui.set_tabs(ws.tabs(), ws.active_id());

    let installed = cursors::install(hwnd); // subclass live; custom_frame is deferred until the splash ends
    cursors::set_dark_class_brush(hwnd); // any OS background fill is now #141313, never white

    // The Varos cursor set v1 (embedded), built once per platform: Win32 HCURSORs / macOS Retina
    // NSCursors / winit CustomCursors. Logs `[varos] cursors: 28 v1 (+ N reference overrides) …`.
    #[cfg(windows)]
    let hcur: HashMap<CK, isize> = cursors::create_cursors();
    #[cfg(not(windows))]
    let hcur: HashMap<CK, isize> = cursors::create_cursors(&event_loop);
    // On macOS wait for client-view pointer ownership before touching AppKit's global cursor.
    #[cfg(not(target_os = "macos"))]
    cursors::set(hcur[&CK::Select]);
    {
        let zeros = cursors::ALL_CURSORS.iter().filter(|c| hcur[c] == 0).count();
        let _ = std::fs::write(
            "target/cursor-debug-startup.txt",
            format!(
                "hwnd={hwnd}\ninstalled={installed}\nhcursors_total={}\nhcursors_zero={zeros}\n",
                cursors::ALL_CURSORS.len()
            ),
        );
    }
    let mut last_ck: Option<CK> = None;
    let mut last_click: Option<(Instant, Pt)> = None;
    #[cfg(target_os = "macos")]
    let mut caption_clicks = mac_caption::CaptionClicks::default();
    if let Some(s) = ws.active_mut() {
        // open zoomed-out so the artboard reads as a DEFINED page sitting on the larger board
        // (lots of dotted board visible around it). Ctrl+0 later does a tight Fit-in-Window.
        // A8a: a boardless new doc has no page — frame its content, else the default region.
        let (x, y, w, h) = fit_rect(&s.editor);
        let sz = window.inner_size();
        s.view = View::fit(x, y, w, h, sz.width as f32, sz.height as f32, 0.45);
    }
    let mut screen_cursor: Pt = [0.0, 0.0];
    #[cfg(target_os = "macos")]
    let mut pointer_inside = false;
    #[cfg(target_os = "macos")]
    let mut cursor_window_focused = window.has_focus();
    let mut panning = false;
    let mut pan_last: Pt = [0.0, 0.0];
    // a drag / marquee / pen gesture that STARTED on the canvas — keep feeding it moves even if the
    // cursor strays over a panel (C5), so it never freezes under chrome; cleared on button release.
    let mut canvas_gesture = false;
    // window-geometry persistence: track the NORMAL (un-maximized) bounds so we can save them on close,
    // and refit the view ONCE if we restored a maximized window (so the page isn't tiny in the corner).
    let mut win_norm: (i32, i32, u32, u32) = {
        let sz = window.inner_size();
        let p = window.outer_position().unwrap_or(PhysicalPosition::new(0, 0));
        saved.map(|(_, x, y, w, h)| (x, y, w, h)).unwrap_or((p.x, p.y, sz.width, sz.height))
    };
    let mut refit_pending = saved.is_some_and(|(m, ..)| m);
    // Stage 4: each tab refits into the Board box on its first real frame (`fit_pending`, 0.45 for the
    // startup Untitled-1); the pre-shell fit above centres on the whole window, so one box-aware pass
    // corrects it.
    let mut last_title = String::new();
    let mut drawn_tabs = ws.tabs();

    // Paint frame 0 imperatively while cloaked, then reveal — the first pixels on screen are our dark
    // UI + splash (never a white flash, never the native caption).
    gui.start_splash();
    {
        // custom_frame stripped the caption → the client area grew; sync the surface before rendering.
        let sz0 = window.inner_size();
        renderer.resize(sz0.width, sz0.height);
        if let Some(s) = ws.active_mut() {
            let view = s.view;
            let ed = &mut s.editor;
            ed.ppu = view.zoom;
            let (jobs, tdelta, screen) = gui.run(&window, ed, scale as f32, view, cursors::is_maximized(hwnd));
            if gui.splashing() {
                renderer.render_splash(&jobs, &tdelta, &screen);
            } else {
                let world = build_scene_in_view(ed, view, [sz0.width, sz0.height]);
                renderer.render_ui(&world, view, &jobs, &tdelta, &screen);
            }
        }
    }
    cursors::set_cloaked(hwnd, false);

    let mut editor_framed = false; // becomes true when we switch the splash → the framed editor window
    let mut last_scene_signature: Option<u64> = None;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run(move |event, elwt: &winit::event_loop::ActiveEventLoop| {
            #[cfg(target_os = "macos")]
            if let Some(menu) = &mac_menu {
                if matches!(&event, Event::NewEvents(winit::event::StartCause::Init)) {
                    menu.install();
                }
                // a menu row (clicked, or its ⌘ key): every row queues into the one FIFO queue — File /
                // Quit / Window rows as commands, the other rows as the SAME document action their key
                // queues — except a ⌘-row while typing, which goes to the focused field
                for cmd in menu.drain() {
                    use host::{DocAction as D, HostAction as A, MenuRoute as R};
                    let canvas = canvas_px(&gui, &window);
                    match host::menu_route(cmd, ws.active_id()) {
                        Some(R::App(c)) => pending.push(A::App(c)),
                        Some(R::Key(k)) => {
                            if gui.wants_keyboard() {
                                // typing in a field: the key belongs to egui, as on the keyboard path
                                if let Some(key) = chrome::egui_key(k.code) {
                                    gui.forward_shortcut(key, k.shift, k.alt);
                                }
                            } else if let Some(s) = ws.active_mut() {
                                let m = Mods { ctrl: true, shift: k.shift, alt: k.alt };
                                match host::key_action(k.code, m, Some(s.id)) {
                                    A::App(c) => pending.push(A::App(c)),
                                    A::Doc(d) => raise_doc(&mut pending, d, &mut s.editor, &mut s.view, canvas),
                                }
                            }
                        }
                        // a click-only row (Edit ▸ Delete): the plain key's path, never while typing
                        Some(R::Plain(code)) => {
                            if !gui.wants_keyboard() {
                                if let Some(s) = ws.active_mut() {
                                    let d = D::Key(code, Mods::default());
                                    raise_doc(&mut pending, d, &mut s.editor, &mut s.view, canvas);
                                }
                            }
                        }
                        Some(R::Snap { grid }) => {
                            if let Some(s) = ws.active_mut() {
                                raise_doc(&mut pending, D::Snap { grid }, &mut s.editor, &mut s.view, canvas);
                            }
                        }
                        None => {}
                    }
                    window.request_redraw();
                }
            }
            if matches!(&event, Event::AboutToWait) {
                // a second instance handed us files (Windows single-instance)
                pending.extend(
                    host::open_paths_command(single_instance::take_pending_file_paths(), OpenOrigin::OsHandoff)
                        .map(host::HostAction::App),
                );
                // THE one dispatch: every queued action, in the order it was raised (FIFO) — up to a
                // click the Ui frame has not turned into its command yet
                let ready = pending.take_ready();
                if !ready.is_empty() {
                    let canvas = canvas_px(&gui, &window);
                    for action in ready {
                        let (ds, keys) = (&mut dialogs, &keyboard);
                        let ran = dispatch(action, &mut ws, &mut gui, &window, hwnd, canvas, ds, &mut store, keys);
                        if ran.ran {
                            last_scene_signature = None; // the drawn document may be another one now
                        }
                        if ran.switched {
                            // a gesture / pan in flight belonged to the tab that was active before
                            canvas_gesture = false;
                            panning = false;
                        }
                        if ran.exit {
                            save_win_state(cursors::is_maximized(hwnd), win_norm.0, win_norm.1, win_norm.2, win_norm.3);
                            elwt.exit();
                            return;
                        }
                    }
                    drawn_tabs = ws.tabs();
                    gui.set_tabs(drawn_tabs.clone(), ws.active_id());
                    window.request_redraw();
                }
            }
            if let Event::WindowEvent { event, window_id } = event {
                if window_id != window.id() {
                    return;
                }
                // The held keys: the keyboard's one truth (`host::Keyboard`), mirrored into the active
                // tab's editor — kept even when no tab is active. Losing focus lets Space and the command
                // keys go (their key-ups go elsewhere now); winit releases the modifiers itself.
                if let WindowEvent::Focused(false) = &event {
                    if keyboard.space() {
                        panning = false; // as a Space key-up does: the Space pan ends
                    }
                    keyboard.focus_lost(ws.active_mut().map(|s| &mut s.editor));
                }
                if let WindowEvent::ModifiersChanged(m) = &event {
                    let m = Mods {
                        shift: m.state().shift_key(),
                        alt: m.state().alt_key(),
                        ctrl: m.state().control_key() || m.state().super_key(),
                    };
                    keyboard.modifiers_changed(m, ws.active_mut().map(|s| &mut s.editor));
                }
                // Feed egui first. `over_panel` = pointer is over a native panel → the canvas must NOT
                // get the event (gate #3: panels don't swallow canvas strokes; canvas input stays native).
                // A file / tab key (⌘S, Ctrl+Tab …) is a command, not text: egui never sees it — so
                // Ctrl+Tab cannot also move egui's keyboard focus onto a widget (UI audit 04 B4).
                let command_key_event = match &event {
                    WindowEvent::KeyboardInput { event: k, .. } => match k.physical_key {
                        PhysicalKey::Code(c) => {
                            let pressed = k.state == ElementState::Pressed;
                            command_key(&mut pending, &mut keyboard, c, ws.active_id(), pressed, k.repeat)
                        }
                        _ => false,
                    },
                    _ => false,
                };
                if command_key_event {
                    window.request_redraw();
                }
                let egui_consumed = !command_key_event && gui.on_event(&window, &event);
                // a pointer button's chrome command (a tab chip, a burger row) exists only after the next
                // Ui frame: each press / release leaves its mark, and what is raised after it waits
                if let WindowEvent::MouseInput { state, .. } = &event {
                    pending.pointer_button(*state == ElementState::Released);
                    window.request_redraw();
                }
                let over_panel = gui.wants_pointer();
                let Some(s) = ws.active_mut() else { return };
                let (ed, view) = (&mut s.editor, &mut s.view);
                if egui_consumed {
                    window.request_redraw();
                }
                match event {
                    // Winit's macOS tracking rect follows our full-size content view, including
                    // the custom caption. The chrome resolves its cursor inside that view.
                    #[cfg(target_os = "macos")]
                    WindowEvent::CursorEntered { .. } => {
                        pointer_inside = true;
                        window.request_redraw();
                    }
                    #[cfg(target_os = "macos")]
                    WindowEvent::CursorLeft { .. } => {
                        pointer_inside = false; // no cursor write: AppKit owns it now
                    }
                    #[cfg(target_os = "macos")]
                    WindowEvent::Focused(focused) => {
                        cursor_window_focused = focused;
                        // Unfocused means outside for cursor ownership, even if the pointer has
                        // not moved. Retain containment for immediate restoration on reactivation.
                        if focused {
                            window.request_redraw();
                        }
                    }
                    // red traffic light / OS close: the Quit transaction over every tab (Astra F01; S1
                    // has one window, so Close Window = Quit — work order §6 Q1)
                    WindowEvent::CloseRequested => pending.push(host::HostAction::App(AppCommand::Quit)),
                    WindowEvent::Resized(size) => {
                        if size.width == 0 || size.height == 0 {
                            return; // minimized / degenerate — don't reconfigure the surface or record garbage bounds
                        }
                        renderer.resize(size.width, size.height);
                        if !cursors::is_maximized(hwnd) {
                            // remember the normal bounds (so un-maximize/next-open restores them)
                            if let Ok(pos) = window.outer_position() {
                                win_norm = (pos.x, pos.y, size.width, size.height);
                            }
                        } else if refit_pending {
                            // restored a maximized window → fit the page (or content, A8a) to the view ONCE
                            let (x, y, w, h) = fit_rect(ed);
                            *view = fit_to_board(&gui, &window, x, y, w, h, 0.9);
                            refit_pending = false;
                        }
                        window.request_redraw();
                    }
                    // macOS: an opaque window that was covered gets no redraws (AppKit skips drawing an
                    // occluded view), so a splash that started behind another window would sit there
                    // until the next input event — repaint the moment it is uncovered.
                    #[cfg(target_os = "macos")]
                    WindowEvent::Occluded(false) => window.request_redraw(),
                    WindowEvent::Moved(pos) => {
                        // AppKit can consume mouse motion/releases during its native drag loop.
                        // Native move events also catch a drag that returns to its starting point.
                        #[cfg(target_os = "macos")]
                        caption_clicks.reset_after_drag();
                        // Windows parks a MINIMIZED window at (−32000,−32000) with a 0×0 client area —
                        // never persist that as the "normal" bounds, or it reopens the window invisible.
                        if !cursors::is_maximized(hwnd) {
                            let sz = window.inner_size();
                            if sz.width > 0 && sz.height > 0 {
                                win_norm = (pos.x, pos.y, sz.width, sz.height);
                            }
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        let PhysicalPosition { x, y } = position;
                        screen_cursor = [x as f32, y as f32];
                        if panning {
                            view.pan = [
                                view.pan[0] + screen_cursor[0] - pan_last[0],
                                view.pan[1] + screen_cursor[1] - pan_last[1],
                            ];
                            pan_last = screen_cursor;
                        } else if !over_panel || canvas_gesture {
                            // a gesture that began on the canvas keeps tracking even under a panel (C5)
                            ed.ppu = view.zoom;
                            ed.pointer_move(view.s2w(screen_cursor));
                        }
                        window.request_redraw();
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        // A5 — while the picker's system eyedropper is armed, the sample click is read
                        // globally (GetAsyncKeyState); swallow the in-window event so it doesn't also
                        // poke the canvas (select/deselect) under the floating picker.
                        if gui.picking_screen() {
                            window.request_redraw();
                            return;
                        }
                        match button {
                            MouseButton::Left => match state {
                                ElementState::Pressed => {
                                    if keyboard.space() {
                                        #[cfg(target_os = "macos")]
                                        caption_clicks.reset_after_drag();
                                        if ed.mods.ctrl {
                                            // Apply the entire step at the click point immediately.
                                            let f = if ed.mods.alt { 1.0 / 1.5 } else { 1.5 };
                                            zoom_step(view, screen_cursor, f);
                                        } else {
                                            panning = true;
                                            pan_last = screen_cursor;
                                        }
                                        window.request_redraw();
                                        return;
                                    }
                                    // macOS: an EMPTY spot on our bar is the title bar (Windows gets this from
                                    // the OS hit-test, HTCAPTION): press = move the window, double-click =
                                    // zoom, like the native title bar did (MAC_CHROME.md §A).
                                    #[cfg(target_os = "macos")]
                                    if let Some(pos) = gui.caption_drag_position(screen_cursor) {
                                        if caption_clicks.press(pos, Instant::now()) {
                                            window.set_maximized(!window.is_maximized());
                                        } else {
                                            let before = window.outer_position().ok();
                                            let dragged = window.drag_window();
                                            // The native drag call returns after mouse-up; clear immediately
                                            // as well as on Moved, before another press can trigger zoom.
                                            if dragged.is_err() || before != window.outer_position().ok() {
                                                caption_clicks.reset_after_drag();
                                            }
                                        }
                                        window.request_redraw();
                                        return;
                                    }
                                    // A press belonging to a widget/canvas breaks the caption sequence too.
                                    #[cfg(target_os = "macos")]
                                    caption_clicks.reset_after_drag();
                                    if over_panel {
                                        window.request_redraw();
                                        return;
                                    } // egui handles the click
                                    let now = Instant::now();
                                    let dbl = last_click.is_some_and(|(t, p)| {
                                        now.duration_since(t).as_millis() < 350
                                            && ((p[0] - screen_cursor[0]).powi(2) + (p[1] - screen_cursor[1]).powi(2))
                                                .sqrt()
                                                < 6.0
                                    });
                                    last_click = Some((now, screen_cursor));
                                    ed.ppu = view.zoom;
                                    let wp = view.s2w(screen_cursor);
                                    if dbl && matches!(ed.tool, ToolKind::Object | ToolKind::Direct) {
                                        ed.double_click(wp);
                                    } else {
                                        ed.pointer_down(wp);
                                    }
                                    canvas_gesture = true; // started on the canvas — track it through panels until release
                                }
                                ElementState::Released => {
                                    // the release goes where its press went (UI audit 04 A2/B1): a press
                                    // on chrome never ends an Editor transaction such as the colour
                                    // picker's open session
                                    let pressed_on_canvas = std::mem::replace(&mut canvas_gesture, false);
                                    match host::route_left_release(pressed_on_canvas, panning, over_panel) {
                                        host::LeftRelease::EndPan => panning = false,
                                        host::LeftRelease::Chrome => {}
                                        // dropped a guide onto a ruler → delete
                                        host::LeftRelease::Canvas { over_panel: true } if ed.delete_dragged_guide() => {
                                            window.request_redraw();
                                        }
                                        host::LeftRelease::Canvas { .. } => ed.pointer_up(),
                                    }
                                }
                            },
                            MouseButton::Middle => match state {
                                ElementState::Pressed => {
                                    panning = true;
                                    pan_last = screen_cursor;
                                }
                                ElementState::Released => panning = false,
                            },
                            _ => {}
                        }
                        window.request_redraw();
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        // a panel is a hard scroll boundary: if the pointer is over egui chrome (e.g. the
                        // Layers list), the wheel scrolls THAT — it must never leak to canvas pan/zoom.
                        if gui.wants_pointer() {
                            window.request_redraw();
                            return;
                        }
                        let (dx, dy) = match delta {
                            MouseScrollDelta::LineDelta(x, y) => (x, y),
                            MouseScrollDelta::PixelDelta(p) => (p.x as f32 / 40.0, p.y as f32 / 40.0),
                        };
                        if ed.mods.alt {
                            // Exponential per notch, including coalesced wheel events.
                            let f = ZOOM_NOTCH.powf(dy).clamp(0.2, 5.0);
                            zoom_step(view, screen_cursor, f);
                        } else if ed.mods.shift {
                            view.pan[0] += (dy + dx) * 30.0;
                        } else {
                            view.pan[1] += dy * 30.0;
                            view.pan[0] += dx * 30.0;
                        }
                        window.request_redraw();
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        let PhysicalKey::Code(code) = event.physical_key else { return };
                        // DFS S1 + UI audit 04: the file / tab keys (⌘N ⌘O ⌘S ⇧⌘S ⌘W ⌘Q, Ctrl+Tab) are
                        // commands, queued above (`command_key`) BEFORE the text-field check — so ⌘S
                        // inside a field saves on every platform, as the Mac menu's key equivalent
                        // already does. Such a key goes nowhere else.
                        if command_key_event {
                            return;
                        }
                        // Only skip canvas shortcuts when a text field is actually focused — NOT on egui's
                        // generic "consumed" (which is true for an Arabic-layout char, swallowing V/A/P/…).
                        // The Color Picker is a floating palette: the canvas stays fully usable beside it,
                        // but Esc/Enter belong to the dialog while it is open (Cancel / OK).
                        if gui.wants_keyboard() { /* typing into a field — keys go to egui */
                        } else if gui.modal_open()
                            && matches!(code, KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter)
                        {
                            /* the dialog owns these */
                        } else if code == KeyCode::Space {
                            // A9: `Editor::space` lets a live placement drag reposition on Space
                            let down = event.state == ElementState::Pressed;
                            keyboard.space_changed(down, Some(ed));
                            if !down {
                                panning = false;
                            }
                            window.request_redraw();
                        } else if event.state == ElementState::Pressed {
                            // runs now, or waits behind a command raised earlier in this batch (FIFO)
                            let d = host::DocAction::Key(code, keyboard.held());
                            raise_doc(&mut pending, d, ed, view, canvas_px(&gui, &window));
                            window.request_redraw();
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // While minimized the window is 0×0 — rendering into a 0-size surface/egui panics
                        // (that was closing the app on minimize). Skip the frame until it's restored.
                        let psz = window.inner_size();
                        if psz.width == 0 || psz.height == 0 {
                            pending.chrome_frame([]); // no Ui frame: a click's mark must not hold the queue
                            return;
                        }
                        let perf_start = Instant::now();
                        // a new / opened tab's owed fit, BEFORE its first frame is drawn (the Board box is
                        // known from the previous frame)
                        take_pending_fit(&mut s.fit_pending, ed, view, &gui, &window);
                        ed.ppu = view.zoom;
                        // Native UI runs FIRST (the rail may switch the tool), THEN we build the scene from
                        // the updated editor so the change shows this same frame.
                        let (jobs, tdelta, screen) =
                            gui.run(&window, ed, scale as f32, *view, cursors::is_maximized(hwnd));
                        // the tab strip / burger / window controls raised commands: the one dispatch runs
                        // them when the loop is about to wait (right after this frame)
                        // …in the place of the click that raised them (`ActionQueue::chrome_frame`)
                        let win = gui.win_action.take().map(host::win_action_command);
                        let raised = gui.take_app_commands().into_iter().chain(win);
                        // egui reports at most one click per frame, decided at the latest release
                        let at = pending.last_release();
                        pending.chrome_frame(raised.map(|c| (at, host::HostAction::App(c))));
                        if !pending.is_empty() {
                            window.request_redraw();
                        }
                        // macOS menu bar: every ✓ is read back from the real state (only changes are written)
                        #[cfg(target_os = "macos")]
                        if let Some(menu) = &mac_menu {
                            use chrome::Check as C;
                            menu.sync(|c| {
                                editor_check(ed, c).unwrap_or_else(|| match c {
                                    C::Rail => gui.rail_shown(),
                                    C::Dock => gui.dock_shown(),
                                    C::Panel(p) => gui.panel_open(p),
                                    _ => false,
                                })
                            });
                        }
                        // a "Fit in window" request from the artboard panel / status Fit / ⋮ menu
                        if let Some(i) = gui.fit_request.take() {
                            if let Some(a) = ed.doc.artboards.get(i).cloned() {
                                *view = fit_to_board(&gui, &window, a.x, a.y, a.w, a.h, 0.9);
                            }
                        }
                        // Stage 4: the first non-splash frame knows the Board box — refit the startup
                        // view INTO it once (the pre-shell fit centred on the whole window).
                        if take_pending_fit(&mut s.fit_pending, ed, view, &gui, &window) {
                            window.request_redraw();
                        }
                        // Cursor: over chrome show the UI's OWN cursor (egui's icon mapped to the
                        // Win32 set — seam-resize arrows on box splitters, ↔ on a scrubbed field,
                        // arrow elsewhere); over the canvas show the tool's cursor. It was hardwired
                        // to Select here, which broke the new box seams' arrows (Ahmed 07-07).
                        let ck =
                            resolve_ck(panning, keyboard.space(), gui.wants_pointer().then(|| gui.chrome_ck()), || {
                                desired_ck(ed, view.s2w(screen_cursor))
                            });
                        // Runs AFTER gui.run (egui's platform output is already applied), so on non-Windows
                        // the re-assert each frame wins over egui-winit's own cursor write.
                        if cursor_apply_needed(last_ck, ck, REASSERT_CURSOR_EACH_FRAME) {
                            #[cfg(target_os = "macos")]
                            if native_cursor_apply_needed(pointer_inside, cursor_window_focused) {
                                cursors::set(hcur[&ck]);
                            }
                            #[cfg(not(target_os = "macos"))]
                            cursors::set(hcur[&ck]);
                        }
                        if Some(ck) != last_ck {
                            last_ck = Some(ck);
                            let (hw, ins, hits, cur) = cursors::dbg();
                            let _ = std::fs::write(
                                "target/cursor-debug.txt",
                                format!("hwnd={hw}\ninstalled={ins}\nsetcursor_hits={hits}\ncurrent_hcursor={cur}\n"),
                            );
                        }
                        if gui.splashing() {
                            renderer.render_splash(&jobs, &tdelta, &screen);
                        // floating card on a transparent surface
                        } else {
                            // keyed by WHICH tab too: equal signatures of two tabs must never share art
                            let signature = host::scene_key(s.id, scene_signature(ed, *view, [psz.width, psz.height]));
                            let scene_start = Instant::now();
                            let cache_hit = last_scene_signature == Some(signature);
                            let rendered = if cache_hit {
                                renderer.render_ui_cached(&jobs, &tdelta, &screen)
                            } else {
                                let world = build_scene_in_view(ed, *view, [psz.width, psz.height]);
                                renderer.render_ui(&world, *view, &jobs, &tdelta, &screen)
                            };
                            last_scene_signature = rendered.then_some(signature);
                            let scene_elapsed = scene_start.elapsed();
                            if std::env::var_os("VAROS_PERF").is_some() {
                                let _ = writeln!(
                                    std::io::stderr(),
                                    "[varos-perf] scene_cache={} scene_path={:.3}ms full_frame={:.3}ms",
                                    if cache_hit { "hit" } else { "miss" },
                                    scene_elapsed.as_secs_f64() * 1_000.0,
                                    perf_start.elapsed().as_secs_f64() * 1_000.0
                                );
                            }
                        }
                        // AFTER rendering this frame (so no mid-frame size change), switch the borderless splash
                        // window into the framed editor; the resulting Resized event syncs the surface next frame.
                        if !gui.splashing() && !editor_framed {
                            // macOS is decorated from creation (set_decorations would drop the full-size
                            // content view — MAC_CHROME.md §A)
                            #[cfg(not(target_os = "macos"))]
                            window.set_decorations(true);
                            cursors::custom_frame(hwnd);
                            if saved.is_some_and(|(m, ..)| m) {
                                // re-open maximized if it was last time
                                cursors::maximize(hwnd);
                                // sync the surface + view to the NEW (maximized) size NOW, before the next render —
                                // otherwise a frame draws a maximized viewport into the still-small target (wgpu panic).
                                let sz = window.inner_size();
                                renderer.resize(sz.width, sz.height);
                                let (x, y, w, h) = fit_rect(ed);
                                *view = View::fit(x, y, w, h, sz.width as f32, sz.height as f32, 0.9);
                                s.fit_pending = Some(0.9); // the box re-lays-out at the new size — refit into it
                                refit_pending = false;
                            }
                            editor_framed = true;
                            // the first instance opens its OWN file argument now (F13), through the dispatch
                            pending.extend(startup_open.take().map(host::HostAction::App));
                            window.request_redraw();
                        }
                        // Zoom needs no follow-up frames; idle when egui has no work.
                        if gui.repaint {
                            window.request_redraw();
                        }
                        // the tab strip + window title follow the documents (name, unsaved dot / `*`)
                        let (name, dirty) = (s.display_name(), s.is_dirty());
                        let title = host::window_title(&name, dirty);
                        if title != last_title {
                            window.set_title(&title);
                            #[cfg(target_os = "macos")]
                            {
                                use winit::platform::macos::WindowExtMacOS;
                                window.set_document_edited(dirty);
                            }
                            last_title = title;
                        }
                        // the one per-frame snapshot; handed to the strip only when it changed (a
                        // dispatch in `AboutToWait` already handed over its own result)
                        let tabs = ws.tabs();
                        if tabs != drawn_tabs {
                            window.request_redraw(); // repaint once more so the strip shows the change
                            gui.set_tabs(tabs.clone(), ws.active_id());
                            drawn_tabs = tabs;
                        }
                    }
                    _ => {}
                }
            }
        })
        .unwrap_or_else(|e| fatal("The Windows event loop stopped unexpectedly.", &e.to_string()));
}

#[cfg(test)]
mod cursor_policy_tests {
    use super::{cursor_apply_needed, native_cursor_apply_needed, resolve_ck, CK};

    #[test]
    fn native_cursor_requires_both_client_containment_and_focus() {
        assert!(!native_cursor_apply_needed(false, false));
        assert!(!native_cursor_apply_needed(false, true)); // keyboard redraw outside/title bar
        assert!(!native_cursor_apply_needed(true, false)); // focus lost without pointer movement
        assert!(native_cursor_apply_needed(true, true));
    }

    #[test]
    fn native_cursor_restores_unchanged_tool_on_reentry_and_reactivation() {
        // Startup → enter → leave → re-enter → deactivate → reactivate → leave while inactive.
        let ownership = [
            (false, true, false),
            (true, true, true),
            (false, true, false),
            (true, true, true),
            (true, false, false),
            (true, true, true),
            (false, false, false),
            (false, true, false),
        ];
        for (inside, focused, expected) in ownership {
            let apply =
                cursor_apply_needed(Some(CK::Pen), CK::Pen, true) && native_cursor_apply_needed(inside, focused);
            assert_eq!(apply, expected, "inside={inside}, focused={focused}");
        }
    }

    #[test]
    fn resolve_ck_priority_pan_then_space_then_chrome_then_tool() {
        assert!(resolve_ck(true, true, Some(CK::ResizeH), || CK::Pen) == CK::Grab);
        assert!(resolve_ck(false, true, Some(CK::ResizeH), || CK::Pen) == CK::Hand);
        assert!(resolve_ck(false, false, Some(CK::ResizeH), || CK::Pen) == CK::ResizeH);
        assert!(resolve_ck(false, false, None, || CK::Pen) == CK::Pen);
        // the tool closure is not evaluated when something above it decides
        assert!(resolve_ck(false, true, None, || unreachable!()) == CK::Hand);
    }

    // Review P2: Space held (Hand) while crossing a splitter and back — CK never changes, but egui-winit
    // may have written its own cursor in between. The re-assert policy must still apply Hand every frame.
    #[test]
    fn reassert_policy_reapplies_an_unchanged_cursor_every_frame() {
        let frames = [None, Some(CK::ResizeH), None]; // canvas → splitter → canvas, Space held throughout
        let mut last = None;
        for chrome in frames {
            let ck = resolve_ck(false, true, chrome, || CK::Select);
            assert!(ck == CK::Hand);
            assert!(cursor_apply_needed(last, ck, true)); // non-Windows: every frame
            last = Some(ck);
        }
        // Windows policy (unchanged behaviour): only on change
        assert!(!cursor_apply_needed(Some(CK::Hand), CK::Hand, false));
        assert!(cursor_apply_needed(Some(CK::Hand), CK::Select, false));
        assert!(cursor_apply_needed(None, CK::Hand, false));
    }
}

#[cfg(test)]
mod scene_signature_tests {
    use super::*;
    use varos_core::model::{Anchor, Paint, Path};

    #[test]
    fn idle_pointer_motion_reuses_the_scene_but_hover_does_not() {
        let mut ed = Editor::new();
        let before = scene_signature(&ed, View::identity(), [800, 600]);
        ed.cursor = [320.0, 240.0];
        assert_eq!(before, scene_signature(&ed, View::identity(), [800, 600]));
        ed.hover_path = Some(7);
        assert_ne!(before, scene_signature(&ed, View::identity(), [800, 600]));
    }

    #[test]
    fn drag_cursor_motion_invalidates_the_scene() {
        let mut ed = Editor::new();
        ed.drag = Drag::Marquee { start: [0.0, 0.0], base: vec![] };
        let before = scene_signature(&ed, View::identity(), [800, 600]);
        ed.cursor = [12.0, 18.0];
        assert_ne!(before, scene_signature(&ed, View::identity(), [800, 600]));
    }

    #[test]
    fn live_selected_path_paint_invalidates_before_commit() {
        let mut ed = Editor::new();
        ed.doc.paths.push(Path::new(
            7,
            vec![
                Anchor { id: 8, p: [0.0, 0.0], hin: None, hout: None, smooth: false },
                Anchor { id: 9, p: [10.0, 0.0], hin: None, hout: None, smooth: false },
            ],
            false,
            Some([1.0, 0.0, 0.0, 1.0]),
            None,
            1.0,
        ));
        ed.doc.sync_tree();
        ed.objsel.insert(7);
        let before = scene_signature(&ed, View::identity(), [800, 600]);
        ed.doc.paths[0].fill = Paint::Solid([0.0, 1.0, 0.0, 1.0]);
        assert_ne!(before, scene_signature(&ed, View::identity(), [800, 600]));
    }
}

#[cfg(test)]
mod action_queue_tests {
    use super::*;
    use crate::app_command::SessionId;
    use crate::lifecycle::{Dialogs, DocStore, SaveDecision, SaveFailChoice};
    use crate::workspace::FileKey;
    use std::path::{Path, PathBuf};
    use varos_core::model::Document;

    #[derive(Default)]
    struct FakeUi;
    impl host::DocUi for FakeUi {
        fn settle(&mut self, _: &mut Editor) {}
        fn document_switched(&mut self) {}
    }

    struct NoDialogs;
    impl Dialogs for NoDialogs {
        fn pick_open(&mut self) -> Vec<PathBuf> {
            unreachable!()
        }
        fn pick_save(&mut self, _: &str, _: Option<&Path>) -> Option<PathBuf> {
            unreachable!()
        }
        fn ask_save_changes(&mut self, _: &str, _: Option<(usize, usize)>) -> SaveDecision {
            unreachable!()
        }
        fn save_failed(&mut self, _: &str, _: &str) -> SaveFailChoice {
            unreachable!()
        }
        fn open_failed(&mut self, _: &str, _: &str) {
            unreachable!()
        }
        fn confirm_replace(&mut self, _: &str) -> bool {
            unreachable!()
        }
        fn notice(&mut self, _: &str, _: &str) {
            unreachable!()
        }
    }

    #[derive(Default)]
    struct RecordingStore {
        saved: Option<Document>,
    }
    impl DocStore for RecordingStore {
        fn load(&mut self, _: &Path) -> Result<Document, String> {
            unreachable!()
        }
        fn save(&mut self, doc: &Document, _: &Path) -> Result<(), String> {
            self.saved = Some(doc.clone());
            Ok(())
        }
        fn key(&self, path: &Path) -> FileKey {
            FileKey { path: path.to_path_buf(), dev_ino: None }
        }
        fn exists(&self, _: &Path) -> bool {
            false
        }
    }

    const UNDO: KeyCode = KeyCode::KeyZ;
    const CMD: Mods = Mods { ctrl: true, shift: false, alt: false };

    /// A tab saved at 1 artboard, then edited to 2 (dirty).
    fn saved_then_edited() -> (workspace::Workspace, SessionId) {
        let mut ws = workspace::Workspace::new();
        let id = ws.active_id().unwrap();
        let path = PathBuf::from("ordered.vrs");
        let session = ws.active_mut().unwrap();
        session.editor.execute(EditCommand::AddArtboard);
        session.mark_saved(path.clone(), FileKey { path, dev_ino: None });
        session.editor.execute(EditCommand::AddArtboard);
        assert_eq!(session.editor.doc.artboards.len(), 2);
        assert!(session.is_dirty_exact());
        (ws, id)
    }

    /// What one event batch raises, in event order.
    enum Raised {
        Action(host::HostAction),
        /// A pointer button fed to egui: pressed (false) or released (true).
        Pointer(bool),
        /// The Ui frame: the commands the chrome raised from the buffered clicks.
        UiFrame(Vec<AppCommand>),
    }

    /// One event batch as the event loop runs it: a command is queued, a document action goes through
    /// `raise_doc`, a pointer button marks the queue, the Ui frame inserts its commands at the latest
    /// release mark (as the event loop does); then the queue drains at `AboutToWait`.
    fn run_batch(ws: &mut workspace::Workspace, raised: Vec<Raised>) -> RecordingStore {
        let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let mut pending = host::ActionQueue::default();
        for r in raised {
            match r {
                Raised::Action(host::HostAction::App(c)) => pending.push(host::HostAction::App(c)),
                Raised::Action(host::HostAction::Doc(d)) => {
                    let s = ws.active_mut().unwrap();
                    raise_doc(&mut pending, d, &mut s.editor, &mut s.view, canvas);
                }
                Raised::Pointer(release) => {
                    pending.pointer_button(release);
                }
                Raised::UiFrame(cmds) => {
                    let at = pending.last_release();
                    pending.chrome_frame(cmds.into_iter().map(|c| (at, host::HostAction::App(c))));
                }
            }
        }
        let (mut ui, mut dialogs, mut store) = (FakeUi, NoDialogs, RecordingStore::default());
        for action in pending.take_ready() {
            run_action(action, ws, &mut ui, canvas, &mut dialogs, &mut store, &host::Keyboard::default());
        }
        assert!(pending.is_empty(), "the batch drained completely");
        store
    }

    fn app(c: AppCommand) -> Raised {
        Raised::Action(host::HostAction::App(c))
    }

    fn undo() -> Raised {
        Raised::Action(host::HostAction::Doc(host::DocAction::Key(UNDO, CMD)))
    }

    #[test]
    fn save_then_undo_in_one_batch_saves_pre_undo_content_and_stays_dirty() {
        let (mut ws, id) = saved_then_edited();
        let store = run_batch(&mut ws, vec![app(AppCommand::Save(id)), undo()]);
        assert_eq!(store.saved.unwrap().artboards.len(), 2, "Save ran before the following Undo");
        let session = ws.active().unwrap();
        assert_eq!(session.editor.doc.artboards.len(), 1, "Undo still ran after Save");
        assert!(session.is_dirty_exact(), "the undone content differs from the saved checkpoint");
    }

    #[test]
    fn undo_then_save_in_one_batch_saves_post_undo_content_and_is_clean() {
        let (mut ws, id) = saved_then_edited();
        let store = run_batch(&mut ws, vec![undo(), app(AppCommand::Save(id))]);
        assert_eq!(store.saved.unwrap().artboards.len(), 1, "Undo ran before the following Save");
        let session = ws.active().unwrap();
        assert_eq!(session.editor.doc.artboards.len(), 1);
        assert!(!session.is_dirty_exact(), "what is on screen is what was saved");
    }

    /// Tab A (saved, then edited: 2 artboards) active; tab B beside it with 1 artboard of its own.
    fn two_tabs() -> (workspace::Workspace, SessionId, SessionId) {
        let (mut ws, a) = saved_then_edited();
        ws.new_untitled();
        let b = ws.active_id().unwrap();
        ws.active_mut().unwrap().editor.execute(EditCommand::AddArtboard);
        assert!(ws.activate(a));
        (ws, a, b)
    }

    #[test]
    fn undo_after_a_click_on_another_tab_undoes_that_tab() {
        // click B's chip (egui turns it into Activate(B) only at the Ui frame), then ⌘Z before that frame
        let (mut ws, a, b) = two_tabs();
        let frame = Raised::UiFrame(vec![AppCommand::ActivateDocument(b)]);
        run_batch(&mut ws, vec![Raised::Pointer(false), Raised::Pointer(true), undo(), frame]);
        assert_eq!(ws.active_id(), Some(b));
        assert_eq!(ws.get(b).unwrap().editor.doc.artboards.len(), 0, "the ⌘Z undid B's edit");
        assert_eq!(ws.get(a).unwrap().editor.doc.artboards.len(), 2, "A was not touched");
    }

    #[test]
    fn undo_between_press_and_release_undoes_the_tab_active_before_the_click() {
        // press B → ⌘Z → release B: the click completes after the key
        let (mut ws, a, b) = two_tabs();
        let frame = Raised::UiFrame(vec![AppCommand::ActivateDocument(b)]);
        run_batch(&mut ws, vec![Raised::Pointer(false), undo(), Raised::Pointer(true), frame]);
        assert_eq!(ws.active_id(), Some(b));
        assert_eq!(ws.get(a).unwrap().editor.doc.artboards.len(), 1, "the ⌘Z undid A's edit");
        assert_eq!(ws.get(b).unwrap().editor.doc.artboards.len(), 1, "B was not touched");
    }

    /// Owner hand-test 2026-09-25 (macOS): with Control HELD, Tab · Tab · Tab must cycle A → B → C → A,
    /// and ⇧Tab back. winit reports the modifiers only when they change (`ModifiersChanged` on the
    /// Control / ⇧ press), then only the Tab key events — the event stream modelled here through the
    /// event loop's own pieces (`host::Keyboard`, `command_key`, the queue's drain, `run_action`), one
    /// loop turn per key. It failed before the fix: the key path read the active tab's `Editor::mods`,
    /// which the first switch reset, so the second Tab was a plain Tab (fed to egui = a focus move).
    #[test]
    fn held_ctrl_tab_cycles_every_tab_and_wraps() {
        let mut ws = workspace::Workspace::new();
        let a = ws.active_id().unwrap();
        let b = ws.new_untitled();
        let c = ws.new_untitled();
        assert!(ws.activate(a));
        let canvas = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
        let (mut ui, mut dialogs, mut store) = (FakeUi, NoDialogs, RecordingStore::default());
        let mut pending = host::ActionQueue::default();
        let mut keyboard = host::Keyboard::default();
        // one turn of the event loop for a Tab press (+ its release): queue, drain at AboutToWait
        let mut tab = |ws: &mut workspace::Workspace, keyboard: &mut host::Keyboard| {
            let id = ws.active_id();
            assert!(command_key(&mut pending, keyboard, KeyCode::Tab, id, true, false), "a command, never egui's");
            for action in pending.take_ready() {
                run_action(action, ws, &mut ui, canvas, &mut dialogs, &mut store, keyboard);
            }
            assert!(command_key(&mut pending, keyboard, KeyCode::Tab, id, false, false), "its release too");
            assert!(pending.is_empty(), "a release queues nothing");
            ws.active_id().unwrap()
        };
        // Control goes down once…
        keyboard.modifiers_changed(CMD, ws.active_mut().map(|s| &mut s.editor));
        let forward: Vec<SessionId> = (0..3).map(|_| tab(&mut ws, &mut keyboard)).collect();
        assert_eq!(forward, [b, c, a], "held Ctrl + Tab ×3 cycles A → B → C → A");
        assert!(ws.active().unwrap().editor.mods.ctrl, "the active tab's gestures still see Control held");
        // …⇧ joins it: backwards, wrapping the other way
        let back = Mods { shift: true, ..CMD };
        keyboard.modifiers_changed(back, ws.active_mut().map(|s| &mut s.editor));
        let backward: Vec<SessionId> = (0..3).map(|_| tab(&mut ws, &mut keyboard)).collect();
        assert_eq!(backward, [c, b, a], "held Ctrl+⇧ + Tab ×3 cycles A → C → B → A");
        // both released: Tab is a plain Tab again (the UI's / document's key, not a switch)
        keyboard.modifiers_changed(Mods::default(), ws.active_mut().map(|s| &mut s.editor));
        assert!(!command_key(&mut pending, &mut keyboard, KeyCode::Tab, Some(a), true, false));
        assert!(!ws.active().unwrap().editor.mods.ctrl);
    }

    /// Codex review P2 (a): Ctrl+Tab down, Control released FIRST, then Tab up — the release belongs
    /// to the command its press was, so it must not leak to egui (whose Tab press it never saw).
    #[test]
    fn a_command_keys_release_follows_its_press_when_ctrl_goes_up_first() {
        let mut pending = host::ActionQueue::default();
        let mut keyboard = host::Keyboard::default();
        let id = Some(SessionId(1));
        keyboard.modifiers_changed(CMD, None);
        assert!(command_key(&mut pending, &mut keyboard, KeyCode::Tab, id, true, false), "Ctrl+Tab: a command");
        assert_eq!(pending.take_ready().len(), 1, "queued once");
        keyboard.modifiers_changed(Mods::default(), None);
        assert!(command_key(&mut pending, &mut keyboard, KeyCode::Tab, id, false, false), "its release: not egui's");
        assert!(pending.is_empty(), "a release queues nothing");
        // the next Tab is a fresh, plain press again
        assert!(!command_key(&mut pending, &mut keyboard, KeyCode::Tab, id, true, false));
    }

    /// Codex review P2 (b): a PLAIN Tab down (egui got it), then Control down, then Tab up (and a
    /// repeat in between) — the key is egui's from press to release, or egui's `keys_down` goes stale.
    #[test]
    fn a_plain_keys_release_follows_its_press_when_ctrl_goes_down_in_between() {
        let mut pending = host::ActionQueue::default();
        let mut keyboard = host::Keyboard::default();
        let id = Some(SessionId(1));
        assert!(!command_key(&mut pending, &mut keyboard, KeyCode::Tab, id, true, false), "plain Tab: egui's");
        keyboard.modifiers_changed(CMD, None);
        assert!(!command_key(&mut pending, &mut keyboard, KeyCode::Tab, id, true, true), "its repeat: egui's");
        assert!(!command_key(&mut pending, &mut keyboard, KeyCode::Tab, id, false, false), "its release: egui's");
        assert!(pending.is_empty(), "no tab switch was raised");
    }
}

#[cfg(test)]
mod menu_mirror_tests {
    //! The native menu (MAC_CHROME.md §C) must run the SAME path as the keyboard: every ⌘-row that
    //! carries a ✓ flips, through `apply_key`, exactly the state its ✓ reads back.
    use super::{apply_key, editor_check, menu_snap_toggle, Editor, ZOOM_KEY_STEP};
    use crate::chrome::{menus, Accel, Check, Entry, MenuCmd};
    use varos_core::geom::View;

    fn keyed_checks(entries: &[Entry], out: &mut Vec<(String, Accel, Check)>) {
        for e in entries {
            match e {
                Entry::Item { id, cmd: MenuCmd::Key(k), check: Some(c), .. } => out.push((id.clone(), *k, *c)),
                Entry::Sub { items, .. } => keyed_checks(items, out),
                _ => {}
            }
        }
    }

    #[test]
    fn every_checked_shortcut_row_flips_the_state_its_check_mark_reads() {
        let mut rows = Vec::new();
        for (_, m) in menus() {
            keyed_checks(&m, &mut rows);
        }
        assert_eq!(rows.len(), 4, "Rulers · Guides · Lock Guides · Smart Guides");
        for (id, k, c) in rows {
            let mut ed = Editor::new();
            let mut view = View::identity();
            let before = editor_check(&ed, c).expect("an editor-owned check");
            apply_key(&mut ed, &mut view, [400.0, 300.0], &format!("{:?}", k.code), true, k.shift, k.alt);
            assert_eq!(editor_check(&ed, c), Some(!before), "{id}: the ⌘ key did not flip its ✓ state");
        }
    }

    fn menu_key(id: &str) -> Accel {
        crate::chrome::flat_items(&menus())
            .into_iter()
            .find_map(|e| match e {
                Entry::Item { id: i, cmd: MenuCmd::Key(k), .. } if i == id => Some(k),
                _ => None,
            })
            .unwrap_or_else(|| panic!("{id} is a ⌘-row of the menu"))
    }

    fn assert_pinned(view: &View, anchor: [f32; 2], centre: [f32; 2], what: &str) {
        let p = view.w2s(anchor);
        assert!(
            (p[0] - centre[0]).abs() < 0.01 && (p[1] - centre[1]).abs() < 0.01,
            "{what}: the world point at the canvas centre moved to {p:?} (want {centre:?})"
        );
    }

    /// Astra F06: ⌘1 = 100% AND the place stays — the world point at the canvas centre is kept,
    /// even far from the origin (the old path set zoom only and jumped to empty space).
    #[test]
    fn actual_size_row_is_the_ctrl_1_path() {
        let k = menu_key("view.actual");
        let centre = [700.0, 420.0];
        for (zoom, pan) in [(0.3, [0.0, 0.0]), (1.77, [-90_000.0, 42_000.0]), (8.0, [15_000.0, -3_000.0])] {
            let mut ed = Editor::new();
            let mut view = View { zoom, pan };
            let anchor = view.s2w(centre);
            apply_key(&mut ed, &mut view, centre, &format!("{:?}", k.code), true, k.shift, k.alt);
            assert_eq!(view.zoom, 1.0);
            assert_pinned(&view, anchor, centre, "⌘1");
        }
    }

    /// Astra F05: the View menu's Zoom In / Zoom Out rows send exactly the keystroke the keyboard
    /// path handles, and that path zooms the CANVAS by one key step around the canvas centre.
    #[test]
    fn zoom_rows_are_the_ctrl_plus_minus_path() {
        let centre = [512.0, 384.0];
        for (id, factor) in [("view.zoomin", ZOOM_KEY_STEP), ("view.zoomout", 1.0 / ZOOM_KEY_STEP)] {
            let k = menu_key(id);
            let mut ed = Editor::new();
            let mut view = View { zoom: 1.77, pan: [-9_000.0, 4_000.0] };
            let anchor = view.s2w(centre);
            apply_key(&mut ed, &mut view, centre, &format!("{:?}", k.code), true, k.shift, k.alt);
            assert_eq!(view.zoom, 1.77 * factor, "{id}");
            assert_pinned(&view, anchor, centre, id);
        }
    }

    /// The other keys that mean zoom: ⌘+ typed as ⌘⇧= on a US keyboard, and the numeric keypad.
    /// Without ⌘ they do nothing to the view.
    #[test]
    fn plus_and_keypad_keys_zoom_the_canvas_too() {
        let centre = [300.0, 200.0];
        for (code, shift, factor) in [
            ("Equal", true, ZOOM_KEY_STEP),
            ("NumpadAdd", false, ZOOM_KEY_STEP),
            ("NumpadSubtract", false, 1.0 / ZOOM_KEY_STEP),
        ] {
            let mut ed = Editor::new();
            let mut view = View { zoom: 2.0, pan: [123.0, -456.0] };
            let anchor = view.s2w(centre);
            apply_key(&mut ed, &mut view, centre, code, true, shift, false);
            assert_eq!(view.zoom, 2.0 * factor, "{code}");
            assert_pinned(&view, anchor, centre, code);
            let before = view;
            apply_key(&mut ed, &mut view, centre, code, false, shift, false);
            assert_eq!((view.zoom, view.pan), (before.zoom, before.pan), "{code} without ⌘");
        }
    }

    #[test]
    fn snap_rows_flip_what_their_check_marks_read() {
        for (grid, c) in [(true, Check::SnapGrid), (false, Check::SnapPoint)] {
            let mut ed = Editor::new();
            let before = editor_check(&ed, c).unwrap();
            menu_snap_toggle(&mut ed, grid);
            assert_eq!(editor_check(&ed, c), Some(!before), "{c:?}");
            menu_snap_toggle(&mut ed, grid);
            assert_eq!(editor_check(&ed, c), Some(before), "{c:?} toggles back");
        }
    }
}

#[cfg(test)]
mod instant_zoom_tests {
    use super::*;

    #[test]
    fn one_zoom_step_reaches_final_zoom_immediately_at_cursor() {
        let screen = [341.0, 227.0];
        for factor in [1.12f32.powf(3.0), 1.5, 1.0 / 1.5] {
            let mut view = View { zoom: 2.0, pan: [17.0, -41.0] };
            let anchor = view.s2w(screen);
            zoom_step(&mut view, screen, factor);
            assert_eq!(view.zoom, 2.0 * factor);
            let pinned = view.w2s(anchor);
            assert!((pinned[0] - screen[0]).abs() < 0.0001);
            assert!((pinned[1] - screen[1]).abs() < 0.0001);
        }
    }

    #[test]
    fn instant_zoom_limits_keep_the_cursor_anchor() {
        for (zoom, factor, expected) in [(39.0, 1.5, 40.0), (0.06, 0.2, 0.05)] {
            let mut view = View { zoom, pan: [12.0, -8.0] };
            let screen = [200.0, 300.0];
            let anchor = view.s2w(screen);
            zoom_step(&mut view, screen, factor);
            assert_eq!(view.zoom, expected);
            let pinned = view.w2s(anchor);
            assert!((pinned[0] - screen[0]).abs() < 0.0001);
            assert!((pinned[1] - screen[1]).abs() < 0.0001);
        }
    }
}

#[cfg(test)]
mod clipboard_key_tests {
    //! Astra F04: ⌘C / ⌘X / ⌘V / ⇧⌘V reach the core clipboard commands; plain V / X keep their
    //! tool / colour meaning.
    use super::*;
    use varos_core::editor::PaintTarget;
    use varos_core::model::{Anchor, Path};

    fn one_square() -> Editor {
        let a = |i: u32, p: [f32; 2]| Anchor { id: i, p, hin: None, hout: None, smooth: false };
        let mut ed = Editor::new();
        ed.doc.artboards.clear();
        ed.doc.paths.push(Path::new(
            1,
            vec![a(2, [0.0, 0.0]), a(3, [40.0, 0.0]), a(4, [40.0, 20.0]), a(5, [0.0, 20.0])],
            true,
            Some([0.5, 0.5, 0.5, 1.0]),
            None,
            1.0,
        ));
        ed.doc.ids = 5;
        ed.doc.sync_tree();
        ed.objsel.insert(1);
        ed
    }

    #[test]
    fn cmd_c_copies_and_cmd_v_pastes_centred_in_the_canvas() {
        let mut ed = one_square();
        let mut view = View { zoom: 2.0, pan: [100.0, 50.0] };
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyC", true, false, false);
        assert_eq!(ed.rev, 0, "⌘C does not edit the document");
        assert_eq!(ed.clipboard().len(), 1);
        // canvas box centre (physical px) → the world point the paste must centre on
        let centre = [700.0, 450.0];
        paste_key(&mut ed, &view, centre, false);
        assert_eq!(ed.rev, 1);
        assert_eq!(ed.doc.paths.len(), 2);
        let (x0, y0, x1, y1) = ed.obj_bbox().unwrap();
        let want = view.s2w(centre);
        assert!(((x0 + x1) * 0.5 - want[0]).abs() < 1e-3 && ((y0 + y1) * 0.5 - want[1]).abs() < 1e-3);
    }

    #[test]
    fn shift_cmd_v_pastes_in_place() {
        let mut ed = one_square();
        let mut view = View { zoom: 3.0, pan: [-10.0, 5.0] };
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyC", true, false, false);
        paste_key(&mut ed, &view, [640.0, 400.0], true);
        let pid = *ed.objsel.iter().next().unwrap();
        assert_ne!(pid, 1);
        let copy = ed.doc.paths.iter().find(|p| p.id == pid).unwrap();
        assert_eq!(copy.anchors[0].p, [0.0, 0.0], "in place = the copied coordinates");
    }

    #[test]
    fn cmd_x_cuts_as_one_undo_step() {
        let mut ed = one_square();
        let mut view = View::identity();
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyX", true, false, false);
        assert!(ed.doc.paths.is_empty());
        assert_eq!(ed.rev, 1);
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyZ", true, false, false);
        assert_eq!(ed.doc.paths.len(), 1, "one ⌘Z brings the cut art back");
    }

    #[test]
    fn plain_v_and_x_keep_their_tool_and_colour_meaning() {
        let mut ed = one_square();
        let mut view = View::identity();
        ed.set_tool(ToolKind::Pen);
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyV", false, false, false);
        assert!(ed.tool == ToolKind::Object, "V = Selection tool");
        assert!(ed.paint == PaintTarget::Fill);
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyX", false, false, false);
        assert!(ed.paint == PaintTarget::Stroke, "X = fill/stroke focus swap");
        assert_eq!(ed.doc.paths.len(), 1, "plain X never cuts");
        assert!(ed.clipboard().is_empty(), "plain keys never copy");
    }

    #[test]
    fn paste_with_an_empty_clipboard_does_nothing() {
        let mut ed = one_square();
        paste_key(&mut ed, &View::identity(), [100.0, 100.0], false);
        paste_key(&mut ed, &View::identity(), [100.0, 100.0], true);
        assert_eq!(ed.rev, 0);
        assert_eq!(ed.doc.paths.len(), 1);
    }
}

#[cfg(test)]
mod select_all_key_tests {
    //! QW5: ⌘A / ⇧⌘A reach Select All / Deselect through `apply_key` (the path the Edit menu rows
    //! send), plain A keeps the Direct tool, and the click-only Edit ▸ Delete row's plain key runs the
    //! Delete/Backspace path.
    use super::*;
    use crate::chrome::{flat_items, menus, Entry, MenuCmd};
    use varos_core::model::{Anchor, Path};

    fn two_squares() -> Editor {
        let a = |i: u32, x: f32| Anchor { id: i, p: [x, 0.0], hin: None, hout: None, smooth: false };
        let mut ed = Editor::new();
        ed.doc.artboards.clear();
        for (id, x) in [(1u32, 0.0f32), (6, 100.0)] {
            ed.doc.paths.push(Path::new(
                id,
                vec![a(id + 1, x), a(id + 2, x + 40.0), a(id + 3, x + 40.0), a(id + 4, x)],
                true,
                Some([0.5, 0.5, 0.5, 1.0]),
                None,
                1.0,
            ));
        }
        ed.doc.ids = 10;
        ed.doc.sync_tree();
        ed
    }

    fn row(id: &str) -> MenuCmd {
        flat_items(&menus())
            .into_iter()
            .find_map(|e| match e {
                Entry::Item { id: i, cmd, .. } if i == id => Some(cmd),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the menu has {id}"))
    }

    #[test]
    fn cmd_a_selects_all_and_shift_cmd_a_deselects() {
        let mut ed = two_squares();
        let mut view = View::identity();
        for (id, want) in [("edit.selectall", 2), ("edit.deselect", 0)] {
            let MenuCmd::Key(k) = row(id) else { panic!("{id} is a ⌘-row") };
            apply_key(&mut ed, &mut view, [0.0, 0.0], &format!("{:?}", k.code), true, k.shift, k.alt);
            assert_eq!(ed.objsel.len(), want, "{id}");
            assert_eq!(ed.rev, 0, "{id}: selection is not a document edit");
        }
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyA", false, false, false);
        assert!(ed.tool == ToolKind::Direct, "plain A stays the Direct Selection tool");
        assert!(ed.objsel.is_empty() && ed.selected.is_empty(), "plain A selects nothing");
    }

    #[test]
    fn delete_row_runs_delete_selected() {
        let MenuCmd::Plain(code) = row("edit.delete") else { panic!("Edit ▸ Delete is a plain-key row") };
        let mut ed = two_squares();
        let mut view = View::identity();
        ed.objsel.insert(1);
        // the host's `M::Plain` arm: `shortcut(code, false, false, false)` → this `apply_key` call
        apply_key(&mut ed, &mut view, [0.0, 0.0], &format!("{code:?}"), false, false, false);
        assert_eq!(ed.doc.paths.iter().map(|p| p.id).collect::<Vec<_>>(), [6], "the selection is deleted");
        assert_eq!(ed.rev, 1, "one undoable edit, exactly as the Delete key");
        apply_key(&mut ed, &mut view, [0.0, 0.0], "KeyZ", true, false, false);
        assert_eq!(ed.doc.paths.len(), 2, "⌘Z brings it back");
    }
}
