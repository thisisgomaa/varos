#![windows_subsystem = "windows"] // no console window alongside the app
//! Varos desktop shell: a winit window + wgpu canvas, with the entire UI chrome painted natively in
//! egui on the same wgpu surface (top bar, left tool rail + Fill/Stroke swatch, right inspector
//! dock). Canvas pointer input stays native (winit → Editor) so the pen feel is untouched; the egui
//! panels float over a full-bleed board. Dark skin + tokens from UI_FIGMA_SPEC
//! (#141313 / #1f1f22 / #262627 / #0c8ce9).

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};
use varos_core::editor::{Drag, Editor, Mods, PenHint, TfHit, ToolKind, ZOrder};
use varos_core::geom::{Pt, View};
use varos_core::scene::build_scene;
use varos_render_wgpu::Renderer;

mod cursors;
mod ui;
use cursors::CK;

/// Which native cursor the current effective tool wants (Pen reports its contextual state; the
/// Selection tool reports transform/copy states using the Illustrator cursor set).
fn desired_ck(ed: &Editor, world: Pt) -> CK {
    if let Drag::Scale { handle, angle, .. } = ed.drag { return resize_ck(handle, angle); }
    if ed.mods.alt && matches!(ed.drag, Drag::Object { .. } | Drag::DupPending { .. }) { return CK::Copy; }
    match ed.eff_tool() {
        ToolKind::Object => {
            match ed.transform_hit(world) {
                Some(TfHit::Scale(i)) => resize_ck(i, ed.obj_angle),
                Some(TfHit::Rotate(i)) => rotate_ck(i, ed.obj_angle),
                None if ed.mods.alt && ed.path_under(world).is_some() => CK::Copy,
                None => CK::Select,
            }
        }
        ToolKind::Direct => CK::Direct,
        ToolKind::Convert => CK::Convert,
        ToolKind::Eyedropper => CK::Eye,
        ToolKind::Pen => match ed.pen_hint(world) {
            PenHint::New => CK::PenNew, PenHint::Add => CK::PenAdd, PenHint::Delete => CK::PenDel,
            PenHint::Close => CK::PenClose, PenHint::Connect => CK::PenConnect, PenHint::Draw => CK::Pen,
        },
        _ => CK::Cross,
    }
}

/// Pick the resize double-arrow for a transform handle, accounting for the frame's rotation.
fn resize_ck(handle: u8, angle: f32) -> CK {
    use std::f32::consts::{PI, FRAC_PI_2, FRAC_PI_4};
    let base = match handle {
        5 => 0.0, 7 => PI,
        4 => -FRAC_PI_2, 6 => FRAC_PI_2,
        1 => -FRAC_PI_4, 3 => 3.0 * FRAC_PI_4,
        0 => -3.0 * FRAC_PI_4, 2 => FRAC_PI_4,
        _ => 0.0,
    };
    let a = (base + angle).rem_euclid(PI);
    match ((a / FRAC_PI_4).round() as i32) % 4 {
        0 => CK::ResizeH, 1 => CK::ResizeNW, 2 => CK::ResizeV, 3 => CK::ResizeNE, _ => CK::ResizeH,
    }
}

/// Pick the rotate cursor for a corner (0=TL,1=TR,2=BR,3=BL), accounting for frame rotation.
fn rotate_ck(corner: u8, angle: f32) -> CK {
    use std::f32::consts::{PI, FRAC_PI_4};
    let base = match corner {
        0 => 1.25 * PI, 1 => 1.75 * PI, 2 => 0.25 * PI, 3 => 0.75 * PI, _ => 0.25 * PI,
    };
    let a = (base + angle).rem_euclid(2.0 * PI);
    match ((a / FRAC_PI_4).round() as i32) % 8 {
        0 => CK::RotateE, 1 => CK::RotateSE, 2 => CK::RotateS, 3 => CK::RotateSW,
        4 => CK::RotateW, 5 => CK::RotateNW, 6 => CK::RotateN, 7 => CK::RotateNE, _ => CK::RotateE,
    }
}

// ============================ helpers ============================

fn tool_name(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Pen => "Pen (P)", ToolKind::Direct => "Direct Select (A)", ToolKind::Object => "Select (V)",
        ToolKind::Rect => "Rectangle (M)", ToolKind::Ellipse => "Ellipse (L)", ToolKind::Triangle => "Triangle",
        ToolKind::Polygon => "Polygon", ToolKind::Convert => "Convert", ToolKind::Eyedropper => "Eyedropper (I)",
    }
}
fn full_title(t: ToolKind) -> String { format!("Varos \u{3b1} \u{b7} pre-alpha \u{2014} {}", tool_name(t)) }

/// Apply a keyboard shortcut. `code` is a W3C key code; shared by canvas focus + forwarded keys.
fn apply_key(ed: &mut Editor, view: &mut View, code: &str, ctrl: bool, shift: bool, _alt: bool) {
    if ctrl {
        match code {
            "Digit1" => view.zoom = 1.0,
            "KeyZ" => if shift { ed.redo() } else { ed.undo() },
            "KeyY" => ed.redo(),
            "BracketRight" => ed.arrange(if shift { ZOrder::Front } else { ZOrder::Forward }),
            "BracketLeft" => ed.arrange(if shift { ZOrder::Back } else { ZOrder::Backward }),
            "KeyG" => if shift { ed.ungroup_selection() } else { ed.group_selection() },
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
        "KeyI" => ed.set_tool(ToolKind::Eyedropper),
        "KeyX" => if shift { ed.swap_colors() } else { ed.swap_paint() },
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
    let names = ["select","direct","pen","pennew","penadd","pendel","penclose","penconnect","convert","cross","eye"];
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
                    Some((rgba, w, h)) => { save_gray_png(&rgba, w, h, 1, &format!("{dir}/ai-{stem}.png"));
                        report.push_str(&format!("OK   {stem}  {w}x{h}\n")); }
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
    for px in out.chunks_mut(4) { px[3] = 255; }
    for y in 0..h { for x in 0..w {
        let i = ((y * w + x) * 4) as usize;
        let a = rgba[i+3] as u32;
        if a == 0 { continue; }
        let mix = |c: u32| ((c * a + 128 * (255 - a)) / 255) as u8;
        let (cr, cg, cb) = (mix(rgba[i] as u32), mix(rgba[i+1] as u32), mix(rgba[i+2] as u32));
        for dy in 0..scale { for dx in 0..scale {
            let oi = (((y * scale + dy) * ow + (x * scale + dx)) * 4) as usize;
            out[oi] = cr; out[oi+1] = cg; out[oi+2] = cb; out[oi+3] = 255;
        }}
    }}
    if let Some(img) = image::RgbaImage::from_raw(ow, oh, out) { let _ = img.save(path); }
}

fn preview_svgs(dir: &str) {
    let out = format!("{dir}/png");
    let _ = std::fs::create_dir_all(&out);
    let Ok(entries) = std::fs::read_dir(dir) else { return; };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("svg") { continue; }
        let Ok(svg) = std::fs::read_to_string(&p) else { continue; };
        let name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("x").to_string();
        if let Some((rgba, w, h)) = cursors::render_svg(&svg, 64, true) {
            save_gray_png(&rgba, w, h, 4, &format!("{out}/{name}.png"));
        }
    }
}

fn main() {
    // crash breadcrumb: any panic is written to target/panic.txt (the app has no console window).
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::create_dir_all("target");
        let _ = std::fs::write("target/panic.txt", format!("VAROS PANIC\n{info}\n"));
    }));
    if std::env::args().any(|a| a == "--dump-cursors") { dump_cursors(); return; }
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
            if let Some(dir) = args.get(i + 1) { preview_svgs(dir); }
            return;
        }
    }
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().with_title(full_title(ToolKind::Object))
        .with_window_icon(load_icon()).with_visible(false) // created hidden — no visible flash at all
        .with_transparent(true)                            // lets the startup splash card float over the desktop
        .with_decorations(false)                           // borderless during the splash → no window shadow; the
        .with_inner_size(winit::dpi::LogicalSize::new(1460.0, 860.0)).build(&event_loop).unwrap());
        // editor frame (decorations + shadow + snap) is applied once the splash finishes, below.
    // centre the window on the primary monitor (so the splash card lands in the middle of the screen)
    if let Some(mon) = event_loop.primary_monitor() {
        let (ms, mp, ws) = (mon.size(), mon.position(), window.outer_size());
        window.set_outer_position(PhysicalPosition::new(
            mp.x + (ms.width as i32 - ws.width as i32) / 2,
            mp.y + (ms.height as i32 - ws.height as i32) / 2));
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
    let mut renderer = pollster::block_on(Renderer::new(window.clone(), size.width, size.height));

    let scale = window.scale_factor();

    let mut gui = ui::Ui::new(&window); // native egui UI (spike) — paints on our surface via render_ui
    let mut ed = Editor::new();

    let installed = cursors::install(hwnd); // subclass live; custom_frame is deferred until the splash ends
    cursors::set_dark_class_brush(hwnd);    // any OS background fill is now #141313, never white
    let hcur: HashMap<CK, isize> = cursors::ALL_CURSORS.iter().map(|&ck| {
        let h = match cursors::ai_svg(ck) {
            Some((stem, hx, hy)) => cursors::hcursor_svg_file(stem, hx, hy).unwrap_or_else(|| cursors::hcursor(ck)),
            None => cursors::hcursor(ck),
        };
        (ck, h)
    }).collect();
    cursors::set(hcur[&CK::Select]);
    {
        let zeros = cursors::ALL_CURSORS.iter().filter(|c| hcur[c] == 0).count();
        let _ = std::fs::write("target/cursor-debug-startup.txt",
            format!("hwnd={hwnd}\ninstalled={installed}\nhcursors_total={}\nhcursors_zero={zeros}\n", cursors::ALL_CURSORS.len()));
    }
    let mut last_ck: Option<CK> = None;
    let mut last_click: Option<(Instant, Pt)> = None;
    let mut view = {
        // open zoomed-out so the artboard reads as a DEFINED page sitting on the larger board
        // (lots of dotted board visible around it). Ctrl+0 later does a tight Fit-in-Window.
        let a = &ed.doc.artboard;
        let sz = window.inner_size();
        View::fit(a.x, a.y, a.w, a.h, sz.width as f32, sz.height as f32, 0.45)
    };
    let mut screen_cursor: Pt = [0.0, 0.0];
    let mut panning = false;
    let mut pan_last: Pt = [0.0, 0.0];
    let mut space_down = false;

    // Paint frame 0 imperatively while cloaked, then reveal — the first pixels on screen are our dark
    // UI + splash (never a white flash, never the native caption).
    gui.start_splash();
    {
        // custom_frame stripped the caption → the client area grew; sync the surface before rendering.
        let sz0 = window.inner_size();
        renderer.resize(sz0.width, sz0.height);
        ed.ppu = view.zoom;
        let (jobs, tdelta, screen) = gui.run(&window, &mut ed, scale as f32);
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
    event_loop.run(move |event, elwt| {
        match event {
        Event::WindowEvent { event, window_id } => {
            if window_id != window.id() { return; }
            // Feed egui first. `over_panel` = pointer is over a native panel → the canvas must NOT
            // get the event (gate #3: panels don't swallow canvas strokes; canvas input stays native).
            let egui_consumed = gui.on_event(&window, &event);
            let over_panel = gui.wants_pointer();
            if egui_consumed { window.request_redraw(); }
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => {
                    renderer.resize(size.width, size.height);
                    window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let PhysicalPosition { x, y } = position; screen_cursor = [x as f32, y as f32];
                    if panning {
                        view.pan = [view.pan[0] + screen_cursor[0]-pan_last[0], view.pan[1] + screen_cursor[1]-pan_last[1]];
                        pan_last = screen_cursor;
                    } else if !over_panel { ed.ppu = view.zoom; ed.pointer_move(view.s2w(screen_cursor)); }
                    window.request_redraw();
                }
                WindowEvent::ModifiersChanged(m) => { ed.mods = Mods { shift: m.state().shift_key(), alt: m.state().alt_key(), ctrl: m.state().control_key() || m.state().super_key() }; }
                WindowEvent::MouseInput { state, button, .. } => {
                    match button {
                        MouseButton::Left => match state {
                            ElementState::Pressed => {
                                if space_down {
                                    if ed.mods.ctrl {
                                        let f = if ed.mods.alt { 1.0 / 1.5 } else { 1.5 };
                                        let wc = view.s2w(screen_cursor);
                                        view.zoom = (view.zoom * f).clamp(0.05, 40.0);
                                        view.pan = [screen_cursor[0]-wc[0]*view.zoom, screen_cursor[1]-wc[1]*view.zoom];
                                    } else { panning = true; pan_last = screen_cursor; }
                                    window.request_redraw(); return;
                                }
                                if over_panel { window.request_redraw(); return; } // egui handles the click
                                let now = Instant::now();
                                let dbl = last_click.map_or(false, |(t, p)| now.duration_since(t).as_millis() < 350 && ((p[0]-screen_cursor[0]).powi(2)+(p[1]-screen_cursor[1]).powi(2)).sqrt() < 6.0);
                                last_click = Some((now, screen_cursor));
                                ed.ppu = view.zoom;
                                let wp = view.s2w(screen_cursor);
                                if dbl && matches!(ed.tool, ToolKind::Object | ToolKind::Direct) { ed.double_click(wp); }
                                else { ed.pointer_down(wp); }
                            }
                            ElementState::Released => { if panning { panning = false; } else { ed.pointer_up(); } }
                        },
                        MouseButton::Middle => match state {
                            ElementState::Pressed => { panning = true; pan_last = screen_cursor; }
                            ElementState::Released => panning = false,
                        },
                        _ => {}
                    }
                    window.set_title(&full_title(ed.tool));
                    window.request_redraw();
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let (dx, dy) = match delta { MouseScrollDelta::LineDelta(x, y) => (x, y), MouseScrollDelta::PixelDelta(p) => (p.x as f32 / 40.0, p.y as f32 / 40.0) };
                    if ed.mods.alt {
                        let f = (1.0 + dy * 0.12).clamp(0.2, 5.0);
                        let wc = view.s2w(screen_cursor);
                        view.zoom = (view.zoom * f).clamp(0.05, 40.0);
                        view.pan = [screen_cursor[0]-wc[0]*view.zoom, screen_cursor[1]-wc[1]*view.zoom];
                    } else if ed.mods.shift { view.pan[0] += (dy + dx) * 30.0; }
                    else { view.pan[1] += dy * 30.0; view.pan[0] += dx * 30.0; }
                    window.request_redraw();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if egui_consumed { /* typing into an egui field — don't trigger canvas shortcuts */ }
                    else if let PhysicalKey::Code(code) = event.physical_key {
                        if code == KeyCode::Space {
                            space_down = event.state == ElementState::Pressed;
                            if !space_down { panning = false; }
                            window.request_redraw();
                        } else if event.state == ElementState::Pressed {
                            let (mc, ms, ma) = (ed.mods.ctrl, ed.mods.shift, ed.mods.alt);
                            let cs = format!("{:?}", code);
                            if mc && cs == "Digit0" {           // Ctrl+0 = Fit Artboard in Window (Illustrator)
                                let a = &ed.doc.artboard;
                                let sz = window.inner_size();
                                view = View::fit(a.x, a.y, a.w, a.h, sz.width as f32, sz.height as f32, 0.9);
                            } else {
                                apply_key(&mut ed, &mut view, &cs, mc, ms, ma);
                            }
                            window.set_title(&full_title(ed.tool));
                                    window.request_redraw();
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    // While minimized the window is 0×0 — rendering into a 0-size surface/egui panics
                    // (that was closing the app on minimize). Skip the frame until it's restored.
                    let psz = window.inner_size();
                    if psz.width == 0 || psz.height == 0 { return; }
                    ed.ppu = view.zoom;
                    // Native UI runs FIRST (the rail may switch the tool), THEN we build the scene from
                    // the updated editor so the change shows this same frame.
                    let prev_tool = ed.tool;
                    let (jobs, tdelta, screen) = gui.run(&window, &mut ed, scale as f32);
                    if ed.tool != prev_tool { window.set_title(&full_title(ed.tool)); }
                    // custom title-bar window controls
                    if let Some(act) = gui.win_action.take() {
                        match act {
                            ui::WinAction::Minimize => window.set_minimized(true),
                            ui::WinAction::ToggleMaximize => window.set_maximized(!window.is_maximized()),
                            ui::WinAction::Close => elwt.exit(),
                        }
                    }
                    // Cursor: over a panel show a UI cursor (↔ on a number field, arrow elsewhere) — NOT
                    // the tool/pen cursor; over the canvas show the tool's cursor.
                    let ck = if panning { CK::Grab }
                        else if space_down { CK::Hand }
                        else if gui.wants_pointer() { if gui.scrub_hover() { CK::ResizeH } else { CK::Select } }
                        else { desired_ck(&ed, view.s2w(screen_cursor)) };
                    if Some(ck) != last_ck {
                        cursors::set(hcur[&ck]); last_ck = Some(ck);
                        let (hw, ins, hits, cur) = cursors::dbg();
                        let _ = std::fs::write("target/cursor-debug.txt",
                            format!("hwnd={hw}\ninstalled={ins}\nsetcursor_hits={hits}\ncurrent_hcursor={cur}\n"));
                    }
                    if gui.splashing() {
                        renderer.render_splash(&jobs, &tdelta, &screen); // floating card on a transparent surface
                    } else {
                        let world = build_scene(&ed, view.zoom);
                        // Solid panel (no glass) + one light GPU drop shadow per panel rect.
                        let panels: Vec<[f32; 4]> = gui.rects.iter().map(|r| [r.min.x, r.min.y, r.width(), r.height()]).collect();
                        renderer.render_ui(&world, view, &jobs, &tdelta, &screen, &panels, gui.frosted);
                    }
                    // AFTER rendering this frame (so no mid-frame size change), switch the borderless splash
                    // window into the framed editor; the resulting Resized event syncs the surface next frame.
                    if !gui.splashing() && !editor_framed {
                        window.set_decorations(true);
                        cursors::custom_frame(hwnd);
                        editor_framed = true;
                        window.request_redraw();
                    }
                    if gui.repaint { window.request_redraw(); }
                }
                _ => {}
            }
        }
        _ => {}
        }
    }).unwrap();
}
