#![windows_subsystem = "windows"] // no console window alongside the app
//! Varos desktop shell: winit window + glue. Translates input → varos-core Editor,
//! builds the scene (+ a toolbar), and hands it to the wgpu renderer.

use std::sync::Arc;
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};
use varos_core::editor::{AlignMode, DistAxis, Editor, Mods, PaintTarget, ToolKind, ZOrder};
use varos_core::BoolOp;
use varos_core::geom::{Pt, Rgba, View};
use varos_core::scene::{build_scene, Prim, ACCENT, WHITE};
use varos_render_wgpu::Renderer;

const BTN_BG: [f32; 4] = [0.18, 0.18, 0.18, 1.0];
const TOOLBAR: [ToolKind; 6] = [ToolKind::Object, ToolKind::Direct, ToolKind::Pen, ToolKind::Rect, ToolKind::Ellipse, ToolKind::Triangle];

fn btn_x(i: usize) -> f32 { 10.0 + i as f32 * 42.0 }

// temporary paint palette (real Color/Stroke panels arrive with Tauri)
const PALETTE: [Option<Rgba>; 8] = [
    Some([0.95,0.95,0.97,1.0]), Some([0.10,0.10,0.12,1.0]), Some([0.90,0.26,0.24,1.0]), Some([0.30,0.75,0.40,1.0]),
    Some([0.20,0.55,0.95,1.0]), Some([0.97,0.80,0.25,1.0]), Some([0.60,0.42,0.85,1.0]), None,
];
fn in_rect(p: Pt, r: (f32,f32,f32,f32)) -> bool { p[0] >= r.0 && p[0] <= r.0 + r.2 && p[1] >= r.1 && p[1] <= r.1 + r.3 }
fn fill_sw() -> (f32,f32,f32,f32) { (272.0, 12.0, 28.0, 28.0) }
fn stroke_sw() -> (f32,f32,f32,f32) { (302.0, 12.0, 28.0, 28.0) }
fn pal_sw(j: usize) -> (f32,f32,f32,f32) { (346.0 + j as f32 * 30.0, 14.0, 24.0, 24.0) }
// align / distribute buttons: 0-2 align H (L/Ch/R), 3-5 align V (T/M/B), 6-7 distribute (H/V)
fn align_sw(k: usize) -> (f32,f32,f32,f32) {
    let gap = if k >= 6 { 16.0 } else if k >= 3 { 8.0 } else { 0.0 };
    (606.0 + k as f32 * 30.0 + gap, 12.0, 26.0, 26.0)
}
// Pathfinder buttons: Unite / Minus Front / Intersect / Exclude
fn pf_sw(k: usize) -> (f32,f32,f32,f32) { (892.0 + k as f32 * 30.0, 12.0, 26.0, 26.0) }

/// Handle a click on the toolbar UI (tools / fill+stroke target / palette). Returns true if consumed.
fn ui_click(ed: &mut Editor, pos: Pt) -> bool {
    for (i, t) in TOOLBAR.iter().enumerate() {
        let bx = btn_x(i);
        if pos[0] >= bx && pos[0] <= bx + 34.0 && pos[1] >= 10.0 && pos[1] <= 44.0 { ed.set_tool(*t); return true; }
    }
    if in_rect(pos, fill_sw()) { ed.paint = PaintTarget::Fill; return true; }
    if in_rect(pos, stroke_sw()) { ed.paint = PaintTarget::Stroke; return true; }
    for (j, c) in PALETTE.iter().enumerate() { if in_rect(pos, pal_sw(j)) { ed.apply_paint(*c); return true; } }
    for k in 0..8 { if in_rect(pos, align_sw(k)) {
        match k {
            0 => ed.align(AlignMode::Left), 1 => ed.align(AlignMode::CenterH), 2 => ed.align(AlignMode::Right),
            3 => ed.align(AlignMode::Top),  4 => ed.align(AlignMode::Middle),  5 => ed.align(AlignMode::Bottom),
            6 => ed.distribute(DistAxis::Horizontal), _ => ed.distribute(DistAxis::Vertical),
        }
        return true;
    }}
    for k in 0..4 { if in_rect(pos, pf_sw(k)) {
        match k { 0 => ed.pathfinder(BoolOp::Unite), 1 => ed.pathfinder(BoolOp::MinusFront), 2 => ed.pathfinder(BoolOp::Intersect), _ => ed.pathfinder(BoolOp::Exclude) }
        return true;
    }}
    false
}

fn swatch(s: &mut Vec<Prim>, r: (f32,f32,f32,f32), color: Option<Rgba>, active: bool) {
    let c = [r.0 + r.2/2.0, r.1 + r.3/2.0]; let half = r.2/2.0;
    if active { s.push(Prim::Square { c, half: half + 2.0, color: WHITE }); }
    s.push(Prim::Square { c, half, color: [0.10, 0.10, 0.12, 1.0] });
    if let Some(col) = color { s.push(Prim::Square { c, half: half - 1.5, color: col }); }
    else { s.push(Prim::Stroke { pts: vec![[r.0+4.0, r.1+r.3-4.0], [r.0+r.2-4.0, r.1+4.0]], width: 2.0, color: [0.9,0.2,0.2,1.0] }); }
}

fn ln(s: &mut Vec<Prim>, a: Pt, b: Pt, w: f32, col: Rgba) { s.push(Prim::Stroke { pts: vec![a, b], width: w, color: col }); }
fn sq_ring(s: &mut Vec<Prim>, c: Pt, half: f32, w: f32, col: Rgba) {
    s.push(Prim::Stroke { pts: vec![[c[0]-half,c[1]-half],[c[0]+half,c[1]-half],[c[0]+half,c[1]+half],[c[0]-half,c[1]+half],[c[0]-half,c[1]-half]], width: w, color: col });
}

/// Pathfinder icon (k: 0 Unite, 1 Minus Front, 2 Intersect, 3 Exclude) — two overlapping squares.
fn pf_icon(s: &mut Vec<Prim>, k: usize, r: (f32,f32,f32,f32), ic: Rgba) {
    let (bx, by) = (r.0, r.1);
    let ca = [bx+10.0, by+11.0]; let cb = [bx+16.0, by+17.0]; let h = 6.0; let ov = [bx+13.0, by+14.0];
    match k {
        0 => { s.push(Prim::Square { c: ca, half: h, color: ic }); s.push(Prim::Square { c: cb, half: h, color: ic }); }
        1 => { s.push(Prim::Square { c: ca, half: h, color: ic }); s.push(Prim::Square { c: cb, half: h, color: BTN_BG }); sq_ring(s, cb, h, 1.2, ic); }
        2 => { sq_ring(s, ca, h, 1.2, ic); sq_ring(s, cb, h, 1.2, ic); s.push(Prim::Square { c: ov, half: 3.0, color: ic }); }
        _ => { s.push(Prim::Square { c: ca, half: h, color: ic }); s.push(Prim::Square { c: cb, half: h, color: ic }); s.push(Prim::Square { c: ov, half: 3.0, color: BTN_BG }); }
    }
}

/// Draw an align/distribute icon (k = button index) inside button rect r, in colour `col`.
fn align_icon(s: &mut Vec<Prim>, k: usize, r: (f32,f32,f32,f32), col: Rgba) {
    let (bx, by) = (r.0, r.1);
    match k {
        0 => { ln(s,[bx+5.0,by+5.0],[bx+5.0,by+21.0],2.0,col); ln(s,[bx+5.0,by+10.0],[bx+20.0,by+10.0],3.5,col); ln(s,[bx+5.0,by+16.0],[bx+14.0,by+16.0],3.5,col); }
        1 => { ln(s,[bx+13.0,by+4.0],[bx+13.0,by+22.0],1.6,col); ln(s,[bx+5.0,by+10.0],[bx+21.0,by+10.0],3.5,col); ln(s,[bx+9.0,by+16.0],[bx+17.0,by+16.0],3.5,col); }
        2 => { ln(s,[bx+21.0,by+5.0],[bx+21.0,by+21.0],2.0,col); ln(s,[bx+6.0,by+10.0],[bx+21.0,by+10.0],3.5,col); ln(s,[bx+12.0,by+16.0],[bx+21.0,by+16.0],3.5,col); }
        3 => { ln(s,[bx+5.0,by+5.0],[bx+21.0,by+5.0],2.0,col); ln(s,[bx+10.0,by+5.0],[bx+10.0,by+20.0],3.5,col); ln(s,[bx+16.0,by+5.0],[bx+16.0,by+13.0],3.5,col); }
        4 => { ln(s,[bx+4.0,by+13.0],[bx+22.0,by+13.0],1.6,col); ln(s,[bx+10.0,by+5.0],[bx+10.0,by+21.0],3.5,col); ln(s,[bx+16.0,by+9.0],[bx+16.0,by+17.0],3.5,col); }
        5 => { ln(s,[bx+5.0,by+21.0],[bx+21.0,by+21.0],2.0,col); ln(s,[bx+10.0,by+6.0],[bx+10.0,by+21.0],3.5,col); ln(s,[bx+16.0,by+13.0],[bx+16.0,by+21.0],3.5,col); }
        6 => { ln(s,[bx+6.0,by+6.0],[bx+6.0,by+20.0],3.0,col); ln(s,[bx+13.0,by+6.0],[bx+13.0,by+20.0],3.0,col); ln(s,[bx+20.0,by+6.0],[bx+20.0,by+20.0],3.0,col); }
        _ => { ln(s,[bx+6.0,by+6.0],[bx+20.0,by+6.0],3.0,col); ln(s,[bx+6.0,by+13.0],[bx+20.0,by+13.0],3.0,col); ln(s,[bx+6.0,by+20.0],[bx+20.0,by+20.0],3.0,col); }
    }
}

fn toolbar(ed: &Editor, s: &mut Vec<Prim>) {
    for (i, t) in TOOLBAR.iter().enumerate() {
        let bx = btn_x(i); let by = 10.0; let c = [bx + 17.0, by + 17.0];
        let active = ed.tool == *t;
        let bg = if active { ACCENT } else { BTN_BG };
        let ic = if active { WHITE } else { ACCENT };
        s.push(Prim::Square { c, half: 17.0, color: bg });
        match t {
            ToolKind::Object => s.push(Prim::Tri { a: [bx+11.0,by+9.0], b: [bx+11.0,by+25.0], c: [bx+24.0,by+19.0], color: ic }),
            ToolKind::Direct => s.push(Prim::Stroke { pts: vec![[bx+11.0,by+9.0],[bx+11.0,by+25.0],[bx+24.0,by+19.0],[bx+11.0,by+9.0]], width: 1.4, color: ic }),
            ToolKind::Pen => { s.push(Prim::Stroke { pts: vec![[bx+10.0,by+25.0],[bx+24.0,by+11.0]], width: 2.5, color: ic }); s.push(Prim::Square { c: [bx+11.0,by+24.0], half: 2.0, color: ic }); }
            ToolKind::Rect => { s.push(Prim::Square { c, half: 8.0, color: ic }); s.push(Prim::Square { c, half: 5.5, color: bg }); }
            ToolKind::Ellipse => { s.push(Prim::Disc { c, r: 8.0, color: ic }); s.push(Prim::Disc { c, r: 5.5, color: bg }); }
            ToolKind::Triangle => s.push(Prim::Stroke { pts: vec![[bx+17.0,by+9.0],[bx+25.0,by+25.0],[bx+9.0,by+25.0],[bx+17.0,by+9.0]], width: 1.4, color: ic }),
            _ => {}
        }
    }
    // fill / stroke target swatches + palette
    swatch(s, fill_sw(), ed.cur_fill, ed.paint == PaintTarget::Fill);
    swatch(s, stroke_sw(), ed.cur_stroke, ed.paint == PaintTarget::Stroke);
    for (j, c) in PALETTE.iter().enumerate() { swatch(s, pal_sw(j), *c, false); }
    // align / distribute buttons (greyed unless enough objects are selected)
    let n = ed.objsel.len();
    for k in 0..8 {
        let r = align_sw(k);
        s.push(Prim::Square { c: [r.0 + 13.0, r.1 + 13.0], half: 13.0, color: BTN_BG });
        let enabled = if k >= 6 { n >= 3 } else { n >= 2 };
        align_icon(s, k, r, if enabled { [0.82,0.86,0.92,1.0] } else { [0.34,0.34,0.38,1.0] });
    }
    // Pathfinder buttons (need >=2 objects)
    for k in 0..4 {
        let r = pf_sw(k);
        s.push(Prim::Square { c: [r.0 + 13.0, r.1 + 13.0], half: 13.0, color: BTN_BG });
        pf_icon(s, k, r, if n >= 2 { [0.82,0.86,0.92,1.0] } else { [0.34,0.34,0.38,1.0] });
    }
}

fn tool_name(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Pen => "Pen (P)", ToolKind::Direct => "White arrow (A)", ToolKind::Object => "Black arrow (V)",
        ToolKind::Rect => "Rectangle (M)", ToolKind::Ellipse => "Ellipse (L)", ToolKind::Triangle => "Triangle",
        ToolKind::Polygon => "Polygon", ToolKind::Convert => "Convert", ToolKind::Eyedropper => "Eyedropper (I)",
    }
}
fn full_title(t: ToolKind) -> String { format!("Varos \u{3b1} \u{b7} pre-alpha (\u{644}\u{633}\u{647} \u{628}\u{64a}\u{633}\u{62d}\u{641} \u{1f41b}) \u{2014} {}", tool_name(t)) }

// a tiny caterpillar in the bottom-left corner: "still crawling" :)
fn easter_egg(s: &mut Vec<Prim>, h: f32) {
    let y = h - 20.0;
    let col = [0.32, 0.52, 0.72, 0.65];
    for (x, r) in [(20.0, 5.0), (30.0, 4.6), (39.0, 4.2), (47.0, 3.8), (54.0, 3.4)] {
        s.push(Prim::Disc { c: [x, y], r, color: col });
    }
    s.push(Prim::Disc { c: [18.0, y - 2.5], r: 1.1, color: [0.92, 0.92, 0.92, 0.85] }); // eye
}

fn handle_key(ed: &mut Editor, code: KeyCode) {
    let s = if ed.mods.shift { 10.0 } else { 1.0 };
    match code {
        KeyCode::KeyV => ed.set_tool(ToolKind::Object),
        KeyCode::KeyA => ed.set_tool(ToolKind::Direct),
        KeyCode::KeyP => ed.set_tool(ToolKind::Pen),
        KeyCode::KeyM => ed.set_tool(ToolKind::Rect),
        KeyCode::KeyL => ed.set_tool(ToolKind::Ellipse),
        KeyCode::KeyI => ed.set_tool(ToolKind::Eyedropper),
        KeyCode::KeyX => { if ed.mods.shift { ed.swap_colors(); } else { ed.swap_paint(); } }
        KeyCode::KeyD => ed.default_paint(),
        KeyCode::Escape | KeyCode::Enter => ed.escape(),
        KeyCode::Delete | KeyCode::Backspace => ed.delete_selected(),
        KeyCode::ArrowLeft => ed.nudge(-s, 0.0),
        KeyCode::ArrowRight => ed.nudge(s, 0.0),
        KeyCode::ArrowUp => ed.nudge(0.0, -s),
        KeyCode::ArrowDown => ed.nudge(0.0, s),
        _ => {}
    }
}

fn load_icon() -> Option<winit::window::Icon> {
    let img = image::load_from_memory(include_bytes!("../icon.png")).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(WindowBuilder::new().with_title(full_title(ToolKind::Pen))
        .with_window_icon(load_icon())
        .with_inner_size(winit::dpi::LogicalSize::new(1180.0, 800.0)).build(&event_loop).unwrap());
    let size = window.inner_size();
    let mut renderer = pollster::block_on(Renderer::new(window.clone(), size.width, size.height));
    let mut ed = Editor::new();
    let mut last_click: Option<(Instant, Pt)> = None;
    let mut view = View::identity();
    let mut screen_cursor: Pt = [0.0, 0.0];
    let mut panning = false;
    let mut pan_last: Pt = [0.0, 0.0];
    let mut space_down = false;

    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, window_id } = event {
            if window_id != window.id() { return; }
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => { renderer.resize(size.width, size.height); window.request_redraw(); }
                WindowEvent::CursorMoved { position, .. } => {
                    let PhysicalPosition { x, y } = position; screen_cursor = [x as f32, y as f32];
                    if panning {
                        view.pan = [view.pan[0] + screen_cursor[0]-pan_last[0], view.pan[1] + screen_cursor[1]-pan_last[1]];
                        pan_last = screen_cursor;
                    } else { ed.ppu = view.zoom; ed.pointer_move(view.s2w(screen_cursor)); }
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
                                let now = Instant::now();
                                let dbl = last_click.map_or(false, |(t, p)| now.duration_since(t).as_millis() < 350 && ((p[0]-screen_cursor[0]).powi(2)+(p[1]-screen_cursor[1]).powi(2)).sqrt() < 6.0);
                                last_click = Some((now, screen_cursor));
                                if ui_click(&mut ed, screen_cursor) { }
                                else {
                                    ed.ppu = view.zoom;
                                    let wp = view.s2w(screen_cursor);
                                    if dbl && matches!(ed.tool, ToolKind::Object | ToolKind::Direct) { ed.double_click(wp); }
                                    else { ed.pointer_down(wp); }
                                }
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
                    if ed.mods.alt {                                   // Alt + wheel = zoom (around cursor) — like Illustrator
                        let f = (1.0 + dy * 0.12).clamp(0.2, 5.0);
                        let wc = view.s2w(screen_cursor);
                        view.zoom = (view.zoom * f).clamp(0.05, 40.0);
                        view.pan = [screen_cursor[0]-wc[0]*view.zoom, screen_cursor[1]-wc[1]*view.zoom];
                    } else if ed.mods.shift {                          // Shift + wheel = horizontal scroll
                        view.pan[0] += (dy + dx) * 30.0;
                    } else {                                           // plain wheel = vertical scroll
                        view.pan[1] += dy * 30.0; view.pan[0] += dx * 30.0;
                    }
                    window.request_redraw();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        if code == KeyCode::Space { space_down = event.state == ElementState::Pressed; if !space_down { panning = false; } window.request_redraw(); }
                        else if event.state == ElementState::Pressed {
                            if ed.mods.ctrl && code == KeyCode::Digit0 { view = View::identity(); }
                            else if ed.mods.ctrl && code == KeyCode::Digit1 { let wc = view.s2w(screen_cursor); view.zoom = 1.0; view.pan = [screen_cursor[0]-wc[0], screen_cursor[1]-wc[1]]; }
                            else if ed.mods.ctrl && code == KeyCode::KeyZ { if ed.mods.shift { ed.redo(); } else { ed.undo(); } }
                            else if ed.mods.ctrl && code == KeyCode::KeyY { ed.redo(); }
                            else if ed.mods.ctrl && code == KeyCode::BracketRight { ed.arrange(if ed.mods.shift { ZOrder::Front } else { ZOrder::Forward }); }
                            else if ed.mods.ctrl && code == KeyCode::BracketLeft { ed.arrange(if ed.mods.shift { ZOrder::Back } else { ZOrder::Backward }); }
                            else if !ed.mods.ctrl { handle_key(&mut ed, code); }
                            window.set_title(&full_title(ed.tool));
                            window.request_redraw();
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let world = build_scene(&ed, view.zoom);
                    let mut ui: Vec<Prim> = Vec::new();
                    toolbar(&ed, &mut ui);
                    easter_egg(&mut ui, window.inner_size().height as f32);
                    renderer.render(&world, &ui, view);
                }
                _ => {}
            }
        }
    }).unwrap();
}
