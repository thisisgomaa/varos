#![windows_subsystem = "windows"] // no console window alongside the app
#![allow(deprecated)] // winit 0.30: the closure EventLoop::run + create_window are deprecated-but-present — the zero-drift path
//! Varos desktop shell: a winit window + wgpu canvas, with the entire UI chrome painted natively in
//! egui on the same wgpu surface (top bar, left tool rail + Fill/Stroke swatch, right inspector
//! dock). Canvas pointer input stays native (winit → Editor) so the pen feel is untouched; the egui
//! panels float over a full-bleed board. Dark skin + tokens from UI_FIGMA_SPEC
//! (#141313 / #1f1f22 / #262627 / #0c8ce9).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use varos_core::editor::{AbDrag, AbHit, Drag, Editor, Mods, PenHint, TfHit, ToolKind, ZOrder};
use varos_core::geom::{Pt, View};
use varos_core::scene::build_scene;
use varos_render_wgpu::Renderer;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

mod cursors;
mod ui;
use cursors::CK;

/// Which native cursor the current effective tool wants (Pen reports its contextual state; the
/// Selection tool reports transform/copy states using the Illustrator cursor set).
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
/// Display name of the open document ("Untitled-1" until it lives on disk).
fn doc_stem(file: Option<&std::path::Path>) -> String {
    file.and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled-1".into())
}
/// Window title: "name* · Varos α — Tool" (the * = unsaved changes, like every desktop editor).
fn full_title(t: ToolKind, file: Option<&std::path::Path>, unsaved: bool) -> String {
    format!("{}{} \u{b7} Varos \u{3b1} \u{2014} {}", doc_stem(file), if unsaved { "*" } else { "" }, tool_name(t))
}

/// Apply a keyboard shortcut. `code` is a W3C key code; shared by canvas focus + forwarded keys.
fn apply_key(ed: &mut Editor, view: &mut View, code: &str, ctrl: bool, shift: bool, alt: bool) {
    if ctrl {
        match code {
            "Semicolon" => {
                if alt {
                    ed.doc.guides_locked = !ed.doc.guides_locked
                }
                // Lock Guides (Alt+Ctrl+;)
                else {
                    ed.guides_hidden = !ed.guides_hidden
                }
            } // Hide/Show Guides (Ctrl+;)
            "Digit1" => view.zoom = 1.0,
            "KeyZ" => {
                if shift {
                    ed.redo()
                } else {
                    ed.undo()
                }
            }
            "KeyY" => ed.redo(),
            "BracketRight" => ed.arrange(if shift { ZOrder::Front } else { ZOrder::Forward }),
            "BracketLeft" => ed.arrange(if shift { ZOrder::Back } else { ZOrder::Backward }),
            "KeyG" => {
                if shift {
                    ed.ungroup_selection()
                } else {
                    ed.group_selection()
                }
            }
            "KeyU" => ed.doc.snap.smart = !ed.doc.snap.smart, // Smart Guides toggle (Illustrator Ctrl+U)
            "KeyD" => ed.transform_again(),                   // Transform Again / step-and-repeat (Illustrator Ctrl+D)
            "KeyR" => ed.show_rulers = !ed.show_rulers,       // Show/Hide Rulers (Illustrator Ctrl+R)
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
                ed.swap_colors()
            } else {
                ed.swap_paint()
            }
        }
        "KeyD" => ed.default_paint(),
        "Slash" => ed.apply_paint(None),
        "Escape" | "Enter" => ed.escape(),
        "Delete" | "Backspace" => ed.delete_selected(),
        "ArrowLeft" => ed.nudge(-s, 0.0),
        "ArrowRight" => ed.nudge(s, 0.0),
        "ArrowUp" => ed.nudge(0.0, -s),
        "ArrowDown" => ed.nudge(0.0, s),
        _ => {}
    }
}

/// Fit an artboard into the CANVAS area — the Board box's interior when the shell reports one
/// (Stage 4; physical px), else the whole window. Pan shifts so the page centres in the BOX.
fn fit_to_board(gui: &ui::Ui, window: &Window, x: f32, y: f32, w: f32, h: f32, k: f32) -> View {
    let sz = window.inner_size();
    let b = gui
        .board_px
        .unwrap_or(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(sz.width as f32, sz.height as f32)));
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

/// Dev-only: render every tool cursor to an 8× PNG (over neutral gray) for eyeballing.
fn dump_cursors() {
    let dir = "target/cursor-preview";
    let _ = std::fs::create_dir_all(dir);
    let names =
        ["select", "direct", "pen", "pennew", "penadd", "pendel", "penclose", "penconnect", "convert", "cross", "eye"];
    let scale = 8u32;
    for (ck, name) in cursors::ALL.iter().zip(names) {
        let (rgba, w, h, _hx, _hy) = cursors::rgba(*ck);
        save_gray_png(&rgba, w as u32, h as u32, scale, &format!("{dir}/{name}.png"));
    }
    let svg_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/cursors-ai/svg/");
    let mut report = String::new();
    for ck in cursors::ALL_CURSORS {
        if let Some((stem, _, _)) = cursors::ai_svg(ck) {
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

/// Where the remembered window geometry lives (`%APPDATA%\Varos\window.txt`).
fn win_state_path() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("Varos").join("window.txt"))
}
/// Crash-log home (`%APPDATA%\Varos\crash.txt`) — same folder as window.txt.
fn crash_log_path() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|a| std::path::PathBuf::from(a).join("Varos").join("crash.txt"))
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
    let event_loop = match EventLoop::new() {
        Ok(el) => el,
        Err(e) => fatal("Varos couldn't connect to the Windows desktop.", &e.to_string()),
    };
    let saved = load_win_state(); // remembered geometry from last session (None on first run)
                                  // winit 0.30 removed WindowBuilder — WindowAttributes carries the identical with_* methods
    let mut attrs = Window::default_attributes()
        .with_title(full_title(ToolKind::Object, None, false))
        .with_window_icon(load_icon())
        .with_visible(false) // created hidden — no visible flash at all
        .with_transparent(true) // lets the startup splash card float over the desktop
        .with_decorations(false) // borderless during the splash → no window shadow; the
        // editor frame (decorations + shadow + snap) is applied once the splash finishes, below.
        .with_min_inner_size(winit::dpi::LogicalSize::new(800.0, 560.0)); // floor: never a degenerate layout
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
    let mut ed = Editor::new();

    let installed = cursors::install(hwnd); // subclass live; custom_frame is deferred until the splash ends
    cursors::set_dark_class_brush(hwnd); // any OS background fill is now #141313, never white
    let hcur: HashMap<CK, isize> = cursors::ALL_CURSORS
        .iter()
        .map(|&ck| {
            let h = match cursors::ai_svg(ck) {
                Some((stem, hx, hy)) => cursors::hcursor_svg_file(stem, hx, hy).unwrap_or_else(|| cursors::hcursor(ck)),
                None => cursors::hcursor(ck),
            };
            (ck, h)
        })
        .collect();
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
    let mut view = {
        // open zoomed-out so the artboard reads as a DEFINED page sitting on the larger board
        // (lots of dotted board visible around it). Ctrl+0 later does a tight Fit-in-Window.
        let a = ed.doc.active_artboard().cloned().unwrap_or_default();
        let sz = window.inner_size();
        View::fit(a.x, a.y, a.w, a.h, sz.width as f32, sz.height as f32, 0.45)
    };
    let mut screen_cursor: Pt = [0.0, 0.0];
    let mut panning = false;
    let mut pan_last: Pt = [0.0, 0.0];
    let mut space_down = false;
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
    // Stage 4: refit into the Board box on the first real frame (the factor = how tight); the
    // pre-shell fits centre on the whole window, so one box-aware pass corrects them.
    let mut board_fit_pending: Option<f32> = Some(0.45);

    // ---- the 🔖 slice: the open .vrs + unsaved-changes tracking (drives the title/tab "*") ----
    let mut cur_file: Option<std::path::PathBuf> = None;
    let mut saved_rev: u64 = ed.rev;
    let mut last_title = String::new();

    // Paint frame 0 imperatively while cloaked, then reveal — the first pixels on screen are our dark
    // UI + splash (never a white flash, never the native caption).
    gui.start_splash();
    {
        // custom_frame stripped the caption → the client area grew; sync the surface before rendering.
        let sz0 = window.inner_size();
        renderer.resize(sz0.width, sz0.height);
        ed.ppu = view.zoom;
        let (jobs, tdelta, screen) = gui.run(&window, &mut ed, scale as f32, view, cursors::is_maximized(hwnd));
        if gui.splashing() {
            renderer.render_splash(&jobs, &tdelta, &screen);
        } else {
            let world = build_scene(&ed, view.zoom);
            let panels: Vec<[f32; 4]> = gui.rects.iter().map(|r| [r.min.x, r.min.y, r.width(), r.height()]).collect();
            renderer.render_ui(&world, view, &jobs, &tdelta, &screen, &panels, gui.frosted);
        }
    }
    cursors::set_cloaked(hwnd, false);

    let mut editor_framed = false; // becomes true when we switch the splash → the framed editor window
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop
        .run(move |event, elwt: &winit::event_loop::ActiveEventLoop| {
            if let Event::WindowEvent { event, window_id } = event {
                if window_id != window.id() {
                    return;
                }
                // Feed egui first. `over_panel` = pointer is over a native panel → the canvas must NOT
                // get the event (gate #3: panels don't swallow canvas strokes; canvas input stays native).
                let egui_consumed = gui.on_event(&window, &event);
                let over_panel = gui.wants_pointer();
                if egui_consumed {
                    window.request_redraw();
                }
                match event {
                    WindowEvent::CloseRequested => {
                        save_win_state(cursors::is_maximized(hwnd), win_norm.0, win_norm.1, win_norm.2, win_norm.3);
                        elwt.exit();
                    }
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
                            // restored a maximized window → fit the page to the (now large) view ONCE
                            let a = ed.doc.active_artboard().cloned().unwrap_or_default();
                            view = fit_to_board(&gui, &window, a.x, a.y, a.w, a.h, 0.9);
                            refit_pending = false;
                        }
                        window.request_redraw();
                    }
                    WindowEvent::Moved(pos) => {
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
                    WindowEvent::ModifiersChanged(m) => {
                        ed.mods = Mods {
                            shift: m.state().shift_key(),
                            alt: m.state().alt_key(),
                            ctrl: m.state().control_key() || m.state().super_key(),
                        };
                    }
                    WindowEvent::MouseInput { state, button, .. } => {
                        match button {
                            MouseButton::Left => match state {
                                ElementState::Pressed => {
                                    if space_down {
                                        if ed.mods.ctrl {
                                            let f = if ed.mods.alt { 1.0 / 1.5 } else { 1.5 };
                                            let wc = view.s2w(screen_cursor);
                                            view.zoom = (view.zoom * f).clamp(0.05, 40.0);
                                            view.pan = [
                                                screen_cursor[0] - wc[0] * view.zoom,
                                                screen_cursor[1] - wc[1] * view.zoom,
                                            ];
                                        } else {
                                            panning = true;
                                            pan_last = screen_cursor;
                                        }
                                        window.request_redraw();
                                        return;
                                    }
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
                                    canvas_gesture = false;
                                    if panning {
                                        panning = false;
                                    } else if over_panel && ed.delete_dragged_guide() {
                                        window.request_redraw();
                                    }
                                    // dropped a guide onto a ruler → delete
                                    else {
                                        ed.pointer_up();
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
                            // exponential per notch: winit 0.30 coalesces fast wheel notches into ONE event
                            // with a bigger dy — 1.12^dy makes that identical to the old one-event-per-notch
                            // stream (a linear 1+0.12·dy overshoots in a single visible jump)
                            let f = 1.12f32.powf(dy).clamp(0.2, 5.0);
                            let wc = view.s2w(screen_cursor);
                            view.zoom = (view.zoom * f).clamp(0.05, 40.0);
                            view.pan = [screen_cursor[0] - wc[0] * view.zoom, screen_cursor[1] - wc[1] * view.zoom];
                        } else if ed.mods.shift {
                            view.pan[0] += (dy + dx) * 30.0;
                        } else {
                            view.pan[1] += dy * 30.0;
                            view.pan[0] += dx * 30.0;
                        }
                        window.request_redraw();
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        // Only skip canvas shortcuts when a text field is actually focused — NOT on egui's
                        // generic "consumed" (which is true for an Arabic-layout char, swallowing V/A/P/…).
                        // The Color Picker is a floating palette: the canvas stays fully usable beside it,
                        // but Esc/Enter belong to the dialog while it is open (Cancel / OK).
                        if gui.wants_keyboard() { /* typing into a field — keys go to egui */
                        } else if let PhysicalKey::Code(code) = event.physical_key {
                            if gui.modal_open()
                                && matches!(code, KeyCode::Escape | KeyCode::Enter | KeyCode::NumpadEnter)
                            {
                                /* the dialog owns these */
                            } else if code == KeyCode::Space {
                                space_down = event.state == ElementState::Pressed;
                                if !space_down {
                                    panning = false;
                                }
                                window.request_redraw();
                            } else if event.state == ElementState::Pressed {
                                let (mc, ms, ma) = (ed.mods.ctrl, ed.mods.shift, ed.mods.alt);
                                let cs = format!("{:?}", code);
                                if mc && matches!(code, KeyCode::Digit0 | KeyCode::Numpad0) {
                                    // Ctrl+0 = Fit Artboard in the Board box
                                    let a = ed.doc.active_artboard().cloned().unwrap_or_default();
                                    view = fit_to_board(&gui, &window, a.x, a.y, a.w, a.h, 0.9);
                                } else if mc && code == KeyCode::KeyS {
                                    // Ctrl+S = Save · Ctrl+Shift+S = Save As (Illustrator-exact)
                                    let dest = if ms { None } else { cur_file.clone() }.or_else(|| {
                                        rfd::FileDialog::new()
                                            .add_filter("Varos document (PDF-compatible)", &["vrs"])
                                            .add_filter("PDF", &["pdf"]) // same bytes — a valid PDF either way
                                            .set_file_name(format!("{}.vrs", doc_stem(cur_file.as_deref())))
                                            .save_file()
                                    });
                                    if let Some(mut p) = dest {
                                        if p.extension().is_none_or(|e| {
                                            !(e.eq_ignore_ascii_case("vrs") || e.eq_ignore_ascii_case("pdf"))
                                        }) {
                                            p.set_extension("vrs");
                                        }
                                        match varos_pdf::save_vrs(&ed.doc, &p) {
                                            Ok(()) => {
                                                cur_file = Some(p);
                                                saved_rev = ed.rev;
                                            }
                                            Err(e) => {
                                                rfd::MessageDialog::new()
                                                    .set_level(rfd::MessageLevel::Error)
                                                    .set_title("Varos")
                                                    .set_description(format!("Save failed: {e}"))
                                                    .show();
                                            }
                                        }
                                    }
                                    ed.mods = Default::default(); // the native dialog eats the key releases
                                } else if mc && code == KeyCode::KeyO {
                                    // Ctrl+O = Open — guard unsaved changes first
                                    let proceed = ed.rev == saved_rev
                                        || rfd::MessageDialog::new()
                                            .set_level(rfd::MessageLevel::Warning)
                                            .set_title("Varos")
                                            .set_description(
                                                "You have unsaved changes.\nDiscard them and open another file?",
                                            )
                                            .set_buttons(rfd::MessageButtons::YesNo)
                                            .show()
                                            == rfd::MessageDialogResult::Yes;
                                    if proceed {
                                        if let Some(p) = rfd::FileDialog::new()
                                            .add_filter("Varos document", &["vrs", "pdf"])
                                            .pick_file()
                                        {
                                            match varos_pdf::load_vrs(&p) {
                                                Ok(doc) => {
                                                    ed.replace_doc(doc);
                                                    cur_file = Some(p);
                                                    saved_rev = ed.rev;
                                                    let a = ed.doc.active_artboard().cloned().unwrap_or_default();
                                                    view = fit_to_board(&gui, &window, a.x, a.y, a.w, a.h, 0.9);
                                                }
                                                Err(e) => {
                                                    rfd::MessageDialog::new()
                                                        .set_level(rfd::MessageLevel::Error)
                                                        .set_title("Varos")
                                                        .set_description(format!("Open failed: {e}"))
                                                        .show();
                                                }
                                            }
                                        }
                                    }
                                    ed.mods = Default::default();
                                } else {
                                    apply_key(&mut ed, &mut view, &cs, mc, ms, ma);
                                }
                                window.request_redraw();
                            }
                        }
                    }
                    WindowEvent::RedrawRequested => {
                        // While minimized the window is 0×0 — rendering into a 0-size surface/egui panics
                        // (that was closing the app on minimize). Skip the frame until it's restored.
                        let psz = window.inner_size();
                        if psz.width == 0 || psz.height == 0 {
                            return;
                        }
                        ed.ppu = view.zoom;
                        // Native UI runs FIRST (the rail may switch the tool), THEN we build the scene from
                        // the updated editor so the change shows this same frame.
                        let (jobs, tdelta, screen) =
                            gui.run(&window, &mut ed, scale as f32, view, cursors::is_maximized(hwnd));
                        // title + tab track the document (name, unsaved *) and the active tool
                        let unsaved = ed.rev != saved_rev;
                        let title = full_title(ed.tool, cur_file.as_deref(), unsaved);
                        if title != last_title {
                            window.set_title(&title);
                            gui.set_doc_tab(format!(
                                "{}{}",
                                doc_stem(cur_file.as_deref()),
                                if unsaved { " *" } else { "" }
                            ));
                            last_title = title;
                            window.request_redraw(); // repaint once more so the tab text shows this change
                        }
                        // a "Fit in window" request from the artboard panel / status Fit / ⋮ menu
                        if let Some(i) = gui.fit_request.take() {
                            if let Some(a) = ed.doc.artboards.get(i).cloned() {
                                view = fit_to_board(&gui, &window, a.x, a.y, a.w, a.h, 0.9);
                            }
                        }
                        // Stage 4: the first non-splash frame knows the Board box — refit the startup
                        // view INTO it once (the pre-shell fit centred on the whole window).
                        if let Some(k) = board_fit_pending {
                            if !gui.splashing() && gui.board_px.is_some() {
                                let a = ed.doc.active_artboard().cloned().unwrap_or_default();
                                view = fit_to_board(&gui, &window, a.x, a.y, a.w, a.h, k);
                                board_fit_pending = None;
                                window.request_redraw();
                            }
                        }
                        // custom title-bar window controls
                        if let Some(act) = gui.win_action.take() {
                            match act {
                                ui::WinAction::Minimize => window.set_minimized(true),
                                ui::WinAction::ToggleMaximize => window.set_maximized(!cursors::is_maximized(hwnd)),
                                ui::WinAction::Close => {
                                    save_win_state(
                                        cursors::is_maximized(hwnd),
                                        win_norm.0,
                                        win_norm.1,
                                        win_norm.2,
                                        win_norm.3,
                                    );
                                    elwt.exit();
                                }
                            }
                        }
                        // Cursor: over chrome show the UI's OWN cursor (egui's icon mapped to the
                        // Win32 set — seam-resize arrows on box splitters, ↔ on a scrubbed field,
                        // arrow elsewhere); over the canvas show the tool's cursor. It was hardwired
                        // to Select here, which broke the new box seams' arrows (Ahmed 07-07).
                        let ck = if panning {
                            CK::Grab
                        } else if space_down {
                            CK::Hand
                        } else if gui.wants_pointer() {
                            gui.chrome_ck()
                        } else {
                            desired_ck(&ed, view.s2w(screen_cursor))
                        };
                        if Some(ck) != last_ck {
                            cursors::set(hcur[&ck]);
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
                            let world = build_scene(&ed, view.zoom);
                            // Solid panel (no glass) + one light GPU drop shadow per panel rect.
                            let panels: Vec<[f32; 4]> =
                                gui.rects.iter().map(|r| [r.min.x, r.min.y, r.width(), r.height()]).collect();
                            renderer.render_ui(&world, view, &jobs, &tdelta, &screen, &panels, gui.frosted);
                        }
                        // AFTER rendering this frame (so no mid-frame size change), switch the borderless splash
                        // window into the framed editor; the resulting Resized event syncs the surface next frame.
                        if !gui.splashing() && !editor_framed {
                            window.set_decorations(true);
                            cursors::custom_frame(hwnd);
                            if saved.is_some_and(|(m, ..)| m) {
                                // re-open maximized if it was last time
                                cursors::maximize(hwnd);
                                // sync the surface + view to the NEW (maximized) size NOW, before the next render —
                                // otherwise a frame draws a maximized viewport into the still-small target (wgpu panic).
                                let sz = window.inner_size();
                                renderer.resize(sz.width, sz.height);
                                let a = ed.doc.active_artboard().cloned().unwrap_or_default();
                                view = View::fit(a.x, a.y, a.w, a.h, sz.width as f32, sz.height as f32, 0.9);
                                board_fit_pending = Some(0.9); // the box re-lays-out at the new size — refit into it
                                refit_pending = false;
                            }
                            editor_framed = true;
                            window.request_redraw();
                        }
                        if gui.repaint {
                            window.request_redraw();
                        }
                    }
                    _ => {}
                }
            }
        })
        .unwrap_or_else(|e| fatal("The Windows event loop stopped unexpectedly.", &e.to_string()));
}
