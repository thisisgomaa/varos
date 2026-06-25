#![windows_subsystem = "windows"] // no console window alongside the app
//! Varos desktop shell: winit window + glue. Translates input → varos-core Editor,
//! builds the scene (+ a toolbar), and hands it to the wgpu renderer.

use std::sync::Arc;
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};
use wry::{dpi::{LogicalPosition as WPos, LogicalSize as WSize}, Rect as WryRect, WebView, WebViewBuilder};
use varos_core::editor::{AlignMode, DistAxis, Editor, Mods, PaintTarget, ToolKind, ZOrder};
use varos_core::BoolOp;
use varos_core::geom::{Pt, Rgba, View};
use varos_core::scene::{build_scene, Prim, WHITE};
use varos_render_wgpu::Renderer;

const BTN_BG: [f32; 4] = [0.18, 0.18, 0.18, 1.0];

// Right-side web (wry) panel — the real UI shell. Canvas renders full-window; the panel's child
// HWND composites on top of the right strip. See memory varos-ui-shell-decision.
const PANEL_W: f64 = 280.0; // logical px

#[derive(Debug, Clone)]
enum UserEvent { Ipc(String) }

const PANEL_HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<style>
  :root{
    --bg-panel:#202024;--bg-hover:#2c2c33;--bg-active:#34343a;--border:#2a2a30;
    --text:#f0f0f2;--muted:#a0a0a8;--faint:#6b6b72;--accent:#0c8ce9;
    --ui:'Inter','Segoe UI Variable','Segoe UI',system-ui,sans-serif;
    --mono:'JetBrains Mono','Cascadia Code',Consolas,monospace;
  }
  *{box-sizing:border-box;margin:0;padding:0}
  html,body{height:100%;background:var(--bg-panel);color:var(--text);font:13px/1.5 var(--ui);overflow:hidden;user-select:none;-webkit-font-smoothing:antialiased}
  .tabs{display:flex;gap:2px;padding:10px 12px 0}
  .tab{padding:7px 12px 9px;font-size:13px;font-weight:500;color:var(--muted);cursor:pointer;position:relative}
  .tab.on{color:var(--text)}
  .tab.on::after{content:'';position:absolute;left:8px;right:8px;bottom:0;height:2px;border-radius:2px;background:var(--accent)}
  .tab:hover:not(.on){color:var(--text)}
  .scroll{height:calc(100% - 39px);overflow:auto;border-top:1px solid var(--border)}
  .sec{padding:14px 16px;border-bottom:1px solid var(--border)}
  .sec-h{font-size:11px;letter-spacing:.5px;text-transform:uppercase;color:var(--muted);font-weight:600;margin-bottom:10px;display:flex;align-items:center;justify-content:space-between}
  .count{font-family:var(--mono);color:var(--faint);font-size:11px;font-weight:400}
  .empty{padding:22px 8px;color:var(--faint);font-size:12px;line-height:1.7;text-align:center}
  .list{display:flex;flex-direction:column;gap:1px}
  .layer{display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:8px;cursor:pointer;color:var(--muted);font-size:12.5px}
  .layer:hover{background:var(--bg-hover);color:var(--text)}
  .layer.sel{background:var(--accent);color:#fff}
  .dot{width:11px;height:11px;border-radius:3px;background:#4a4a54;flex:0 0 auto}
  .layer.grp .dot{border-radius:50%;background:#c9a23a}
  .layer.sel .dot{background:#fff}
  .nm{flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
</style></head><body>
  <div class="tabs">
    <div class="tab on" data-t="layers">Layers</div>
    <div class="tab" data-t="design">Design</div>
  </div>
  <div class="scroll">
    <div id="layers">
      <div class="sec"><div class="sec-h"><span>Layers</span><span class="count" id="lc">0</span></div>
        <div class="list" id="list"><div class="empty">No objects yet</div></div></div>
    </div>
    <div id="design" style="display:none">
      <div class="sec"><div class="sec-h">Properties</div>
        <div class="empty">Select an object to edit<br>its size, fill &amp; stroke.</div></div>
    </div>
  </div>
<script>
  const tabs=document.querySelectorAll('.tab');
  function show(t){ for(const el of tabs) el.classList.toggle('on', el.dataset.t===t);
    document.getElementById('layers').style.display = t==='layers'?'':'none';
    document.getElementById('design').style.display = t==='design'?'':'none'; }
  for(const el of tabs) el.addEventListener('click',()=>show(el.dataset.t));
  window.varosLayers=(rows)=>{
    document.getElementById('lc').textContent=rows.length;
    const list=document.getElementById('list');
    if(!rows.length){ list.innerHTML='<div class="empty">No objects yet</div>'; return; }
    list.innerHTML='';
    for(const r of rows){
      const d=document.createElement('div');
      d.className='layer'+(r.sel?' sel':'')+(r.group?' grp':'');
      d.style.paddingLeft=(9+r.depth*16)+'px';
      d.innerHTML='<span class="dot"></span><span class="nm"></span>';
      d.querySelector('.nm').textContent=r.name;
      d.addEventListener('click',()=>window.ipc.postMessage(String(r.pid)));
      list.appendChild(d);
    }
  };
  window.addEventListener('DOMContentLoaded',()=>window.ipc.postMessage('ready'));
</script></body></html>"#;

// Left vertical tools rail (web). Replaces the in-canvas tool buttons.
const LEFT_W: f64 = 52.0; // logical px
const TOOLS_HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><style>
  :root{--bg-app:#1c1c1e;--bg-hover:#2c2c33;--border:#2a2a30;--muted:#a0a0a8;--text:#f0f0f2;--accent:#0c8ce9;
    --ui:'Inter','Segoe UI Variable','Segoe UI',system-ui,sans-serif;}
  *{box-sizing:border-box;margin:0;padding:0}
  html,body{height:100%;background:var(--bg-app);overflow:hidden;user-select:none;font-family:var(--ui)}
  .rail{display:flex;flex-direction:column;align-items:center;height:100%;padding:8px 0;gap:2px}
  .logo{width:30px;height:30px;border-radius:8px;background:var(--accent);color:#fff;display:flex;align-items:center;justify-content:center;font-weight:700;font-size:16px;margin-bottom:8px}
  .tb{width:38px;height:38px;border-radius:9px;display:flex;align-items:center;justify-content:center;cursor:pointer;color:var(--muted);transition:background .12s,color .12s}
  .tb:hover{background:var(--bg-hover);color:var(--text)}
  .tb.on{background:var(--accent);color:#fff}
  .tb svg{width:20px;height:20px}
  .sep{width:22px;height:1px;background:var(--border);margin:5px 0}
</style></head><body><div class="rail" id="rail"></div>
<script>
  const A='<svg viewBox="0 0 24 24" fill="currentColor"><path d="M6 3 L6 19 L10 15 L13 21 L15 20 L12 14 L18 14 Z"/></svg>';
  const Ao='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M6 3 L6 19 L10 15 L13 21 L15 20 L12 14 L18 14 Z"/></svg>';
  const PEN='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M4 20 L6 14 L15 5 L19 9 L10 18 Z"/><path d="M6 14 L10 18"/></svg>';
  const RECT='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><rect x="5" y="6" width="14" height="12" rx="1"/></svg>';
  const ELL='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6"><ellipse cx="12" cy="12" rx="8" ry="6"/></svg>';
  const TRI='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="M12 5 L19 19 L5 19 Z"/></svg>';
  const EYE='<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d="M4 20 L11 13"/><path d="M13 11 L17 7 a2.1 2.1 0 1 0 -3 -3 L10 8 Z"/></svg>';
  // [id, svg] or null = separator
  const TOOLS=[['object',A],['direct',Ao],null,['pen',PEN],null,['rect',RECT],['ellipse',ELL],['triangle',TRI],null,['eyedropper',EYE]];
  const rail=document.getElementById('rail'); let active='pen';
  const logo=document.createElement('div'); logo.className='logo'; logo.textContent='V'; rail.appendChild(logo);
  function render(){
    [...rail.querySelectorAll('.tb,.sep')].forEach(e=>e.remove());
    for(const t of TOOLS){
      if(!t){ const s=document.createElement('div'); s.className='sep'; rail.appendChild(s); continue; }
      const [id,svg]=t; const d=document.createElement('div');
      d.className='tb'+(id===active?' on':''); d.innerHTML=svg;
      d.onclick=()=>window.ipc.postMessage('tool:'+id); rail.appendChild(d);
    }
  }
  window.varosUI=(s)=>{ if(s&&s.tool){ active=s.tool; render(); } };
  render();
  window.addEventListener('DOMContentLoaded',()=>window.ipc.postMessage('ready'));
</script></body></html>"#;

// temporary paint palette (real Color/Stroke panels arrive with Tauri)
const PALETTE: [Option<Rgba>; 8] = [
    Some([0.95,0.95,0.97,1.0]), Some([0.10,0.10,0.12,1.0]), Some([0.90,0.26,0.24,1.0]), Some([0.30,0.75,0.40,1.0]),
    Some([0.20,0.55,0.95,1.0]), Some([0.97,0.80,0.25,1.0]), Some([0.60,0.42,0.85,1.0]), None,
];
fn in_rect(p: Pt, r: (f32,f32,f32,f32)) -> bool { p[0] >= r.0 && p[0] <= r.0 + r.2 && p[1] >= r.1 && p[1] <= r.1 + r.3 }
// top temp bar starts right of the 52px left tools rail (tools moved to the web rail)
fn fill_sw() -> (f32,f32,f32,f32) { (62.0, 12.0, 28.0, 28.0) }
fn stroke_sw() -> (f32,f32,f32,f32) { (92.0, 12.0, 28.0, 28.0) }
fn pal_sw(j: usize) -> (f32,f32,f32,f32) { (136.0 + j as f32 * 30.0, 14.0, 24.0, 24.0) }
// align / distribute buttons: 0-2 align H (L/Ch/R), 3-5 align V (T/M/B), 6-7 distribute (H/V)
fn align_sw(k: usize) -> (f32,f32,f32,f32) {
    let gap = if k >= 6 { 16.0 } else if k >= 3 { 8.0 } else { 0.0 };
    (396.0 + k as f32 * 30.0 + gap, 12.0, 26.0, 26.0)
}
// Pathfinder buttons: Unite / Minus Front / Intersect / Exclude
fn pf_sw(k: usize) -> (f32,f32,f32,f32) { (682.0 + k as f32 * 30.0, 12.0, 26.0, 26.0) }

/// Handle a click on the toolbar UI (tools / fill+stroke target / palette). Returns true if consumed.
fn ui_click(ed: &mut Editor, pos: Pt) -> bool {
    // tools now live in the left web rail; this temp top bar keeps colors / align / pathfinder
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

/// Serialize the document into Layers-panel rows (top of z-order first). Groups appear as a header
/// row (depth 0) followed by their members (depth 1). Hand-built JSON (data is simple + safe).
fn layers_json(ed: &Editor) -> String {
    let d = &ed.doc;
    let mut rows: Vec<String> = Vec::new();
    let mut cur_group: Option<u32> = None;
    for p in d.paths.iter().rev() {
        let sel = ed.objsel.contains(&p.id);
        match d.top_group_of_path(p.id) {
            None => {
                cur_group = None;
                rows.push(format!("{{\"pid\":{},\"name\":\"Object\",\"depth\":0,\"group\":false,\"sel\":{}}}", p.id, sel));
            }
            Some(g) => {
                if cur_group != Some(g) {
                    cur_group = Some(g);
                    rows.push(format!("{{\"pid\":{},\"name\":\"Group\",\"depth\":0,\"group\":true,\"sel\":{}}}", p.id, sel));
                }
                rows.push(format!("{{\"pid\":{},\"name\":\"Object\",\"depth\":1,\"group\":false,\"sel\":{}}}", p.id, sel));
            }
        }
    }
    format!("[{}]", rows.join(","))
}
fn push_layers(panel: &WebView, ed: &Editor) {
    let _ = panel.evaluate_script(&format!("window.varosLayers && window.varosLayers({});", layers_json(ed)));
}
fn tool_id(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Object => "object", ToolKind::Direct => "direct", ToolKind::Pen => "pen",
        ToolKind::Rect => "rect", ToolKind::Ellipse => "ellipse", ToolKind::Triangle => "triangle",
        ToolKind::Eyedropper => "eyedropper", ToolKind::Polygon => "polygon", ToolKind::Convert => "convert",
    }
}
fn push_ui(tools: &WebView, ed: &Editor) {
    let _ = tools.evaluate_script(&format!("window.varosUI && window.varosUI({{\"tool\":\"{}\"}});", tool_id(ed.tool)));
}

fn load_icon() -> Option<winit::window::Icon> {
    let img = image::load_from_memory(include_bytes!("../icon.png")).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    winit::window::Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn main() {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let window = Arc::new(WindowBuilder::new().with_title(full_title(ToolKind::Pen))
        .with_window_icon(load_icon())
        .with_inner_size(winit::dpi::LogicalSize::new(1460.0, 800.0)).build(&event_loop).unwrap());
    let size = window.inner_size();
    let mut renderer = pollster::block_on(Renderer::new(window.clone(), size.width, size.height));

    // right-side web panel (wry) — sibling child HWND; the canvas event loop stays untouched
    let scale = window.scale_factor();
    let lsz = window.inner_size().to_logical::<f64>(scale);
    let panel = WebViewBuilder::new()
        .with_bounds(WryRect {
            position: WPos::new((lsz.width - PANEL_W).max(0.0), 0.0).into(),
            size: WSize::new(PANEL_W, lsz.height).into(),
        })
        .with_background_color((30, 30, 34, 255))
        .with_html(PANEL_HTML)
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window)
        .unwrap();
    // left vertical tools rail
    let tools_panel = WebViewBuilder::new()
        .with_bounds(WryRect {
            position: WPos::new(0.0, 0.0).into(),
            size: WSize::new(LEFT_W, lsz.height).into(),
        })
        .with_background_color((34, 34, 39, 255))
        .with_html(TOOLS_HTML)
        .with_ipc_handler(move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); })
        .build_as_child(&*window)
        .unwrap();

    let mut ed = Editor::new();
    let mut last_click: Option<(Instant, Pt)> = None;
    let mut view = View::identity();
    let mut screen_cursor: Pt = [0.0, 0.0];
    let mut panning = false;
    let mut pan_last: Pt = [0.0, 0.0];
    let mut space_down = false;

    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run(move |event, elwt| {
        match event {
        Event::UserEvent(UserEvent::Ipc(msg)) => {
            if msg == "ready" {
                push_layers(&panel, &ed); push_ui(&tools_panel, &ed); // a panel loaded → send state
            } else if let Some(t) = msg.strip_prefix("tool:") {
                let tk = match t {
                    "object" => Some(ToolKind::Object), "direct" => Some(ToolKind::Direct), "pen" => Some(ToolKind::Pen),
                    "rect" => Some(ToolKind::Rect), "ellipse" => Some(ToolKind::Ellipse), "triangle" => Some(ToolKind::Triangle),
                    "eyedropper" => Some(ToolKind::Eyedropper), _ => None,
                };
                if let Some(tk) = tk {
                    ed.set_tool(tk);
                    push_ui(&tools_panel, &ed); push_layers(&panel, &ed);
                    window.set_title(&full_title(ed.tool)); window.request_redraw();
                }
            } else if let Ok(id) = msg.parse::<u32>() {
                // clicked a Layers row → select that object's whole (top) group on the canvas
                if ed.doc.pidx(id).is_some() {
                    ed.set_tool(ToolKind::Object);
                    ed.objsel = ed.doc.group_members(id).into_iter().collect();
                    ed.selected.clear();
                    ed.obj_angle = 0.0;
                    push_layers(&panel, &ed); push_ui(&tools_panel, &ed);
                    window.set_title(&full_title(ed.tool));
                    window.request_redraw();
                }
            }
        }
        Event::WindowEvent { event, window_id } => {
            if window_id != window.id() { return; }
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => {
                    renderer.resize(size.width, size.height);
                    let (lw, lh) = (size.width as f64 / scale, size.height as f64 / scale);
                    let _ = panel.set_bounds(WryRect { position: WPos::new((lw - PANEL_W).max(0.0), 0.0).into(), size: WSize::new(PANEL_W, lh).into() });
                    let _ = tools_panel.set_bounds(WryRect { position: WPos::new(0.0, 0.0).into(), size: WSize::new(LEFT_W, lh).into() });
                    window.request_redraw();
                }
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
                    push_layers(&panel, &ed); push_ui(&tools_panel, &ed);
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
                            else if ed.mods.ctrl && code == KeyCode::KeyG { if ed.mods.shift { ed.ungroup_selection(); } else { ed.group_selection(); } }
                            else if !ed.mods.ctrl { handle_key(&mut ed, code); }
                            window.set_title(&full_title(ed.tool));
                            push_layers(&panel, &ed); push_ui(&tools_panel, &ed);
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
        _ => {}
        }
    }).unwrap();
}
