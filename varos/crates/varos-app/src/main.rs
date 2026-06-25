#![windows_subsystem = "windows"] // no console window alongside the app
//! Varos desktop shell: winit window + wgpu canvas, with the UI chrome as wry web panels
//! (top bar, left tool rail, right inspector/layers, zoom pill). The canvas event loop and
//! renderer are untouched — panels are sibling child HWNDs over the dark canvas.

use std::sync::Arc;
use std::time::Instant;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorIcon, WindowBuilder},
};
use wry::{dpi::{LogicalPosition as WPos, LogicalSize as WSize}, Rect as WryRect, WebView, WebViewBuilder};
use varos_core::editor::{AlignMode, DistAxis, Editor, Mods, PaintTarget, ToolKind, ZOrder};
use varos_core::BoolOp;
use varos_core::geom::{Pt, Rgba, View};
use varos_core::scene::build_scene;
use varos_render_wgpu::Renderer;

#[derive(Debug, Clone)]
enum UserEvent { Ipc(String) }

// ---- docked chrome geometry (logical px) ----
const TOP_H: f64 = 84.0;   // menu row + context bar
const LEFT_W: f64 = 52.0;  // tool rail
const PANEL_W: f64 = 280.0; // inspector / layers
const ZOOM_W: f64 = 156.0;
const ZOOM_H: f64 = 38.0;

// ============================ web panels (HTML/CSS/JS) ============================
// shared dark+azure tokens are duplicated per panel (separate webviews).

const TOPBAR_HTML: &str = r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><style>
  :root{--bg-app:#1c1c1e;--bg-surface:#26262b;--bg-hover:#2c2c33;--border:#2a2a30;
    --text:#f0f0f2;--muted:#a0a0a8;--faint:#6b6b72;--accent:#0c8ce9;
    --ui:'Inter','Segoe UI Variable','Segoe UI',system-ui,sans-serif;--mono:'JetBrains Mono','Cascadia Code',Consolas,monospace;}
  *{box-sizing:border-box;margin:0;padding:0}
  html,body{height:100%;background:var(--bg-app);color:var(--text);font:13px var(--ui);overflow:hidden;user-select:none;-webkit-font-smoothing:antialiased}
  .menu{height:37px;display:flex;align-items:center;padding:0 10px;border-bottom:1px solid var(--border)}
  .logo{width:25px;height:25px;border-radius:7px;background:var(--accent);color:#fff;display:flex;align-items:center;justify-content:center;font-weight:700;font-size:13px;margin-right:8px}
  .brand{font-weight:600;font-size:13px;margin-right:7px}
  .ver{font-family:var(--mono);font-size:10px;color:var(--faint);background:var(--bg-surface);padding:2px 6px;border-radius:5px;margin-right:14px}
  .mi{padding:5px 9px;color:var(--muted);border-radius:6px;cursor:default;font-size:13px}
  .mi:hover{background:var(--bg-hover);color:var(--text)}
  .ctx{height:46px;display:flex;align-items:center;gap:5px;padding:0 12px}
  .grp{display:flex;gap:2px;align-items:center}
  .btn{width:30px;height:30px;border-radius:7px;display:flex;align-items:center;justify-content:center;color:var(--muted);cursor:pointer;transition:background .1s,color .1s}
  .btn:hover{background:var(--bg-hover);color:var(--text)}
  .btn.dis{opacity:.3;pointer-events:none}
  .btn svg{width:18px;height:18px}
  .sep{width:1px;height:20px;background:var(--border);margin:0 6px}
  .info{margin-left:auto;font-family:var(--mono);font-size:11px;color:var(--faint)}
</style></head><body>
  <div class="menu">
    <div class="logo">V</div><div class="brand">Varos</div><div class="ver">pre-alpha</div>
    <div class="mi">File</div><div class="mi">Edit</div><div class="mi">Object</div><div class="mi">Select</div><div class="mi">View</div>
  </div>
  <div class="ctx">
    <div class="grp" id="align"></div><div class="sep"></div>
    <div class="grp" id="dist"></div><div class="sep"></div>
    <div class="grp" id="pf"></div>
    <div class="info" id="info">No selection</div>
  </div>
<script>
  const S='fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"';
  const svg=p=>'<svg viewBox="0 0 24 24" '+S+'>'+p+'</svg>';
  const AL=[
    ['align:left',svg('<path d="M4 4V20M4 9H17M4 15H11"/>')],
    ['align:centerh',svg('<path d="M12 4V20M5 9H19M8 15H16"/>')],
    ['align:right',svg('<path d="M20 4V20M7 9H20M13 15H20"/>')],
    ['align:top',svg('<path d="M4 4H20M9 4V17M15 4V11"/>')],
    ['align:middle',svg('<path d="M4 12H20M9 5V19M15 8V16"/>')],
    ['align:bottom',svg('<path d="M4 20H20M9 7V20M15 13V20"/>')],
  ];
  const DI=[['dist:h',svg('<path d="M4 5V19M12 5V19M20 5V19"/>')],['dist:v',svg('<path d="M5 4H19M5 12H19M5 20H19"/>')]];
  const sq=(x,y,f)=>'<rect x="'+x+'" y="'+y+'" width="11" height="11" rx="1.5" '+f+'/>';
  const PF=[
    ['pf:unite','<svg viewBox="0 0 24 24" fill="currentColor">'+sq(4,4,'')+sq(9,9,'')+'</svg>'],
    ['pf:minus','<svg viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="1.3">'+sq(4,4,'')+sq(9,9,'fill="#1c1c1e"')+'</svg>'],
    ['pf:intersect','<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.3">'+sq(4,4,'')+sq(9,9,'')+'<rect x="9" y="9" width="6" height="6" fill="currentColor" stroke="none"/></svg>'],
    ['pf:exclude','<svg viewBox="0 0 24 24" fill="currentColor">'+sq(4,4,'')+sq(9,9,'')+'<rect x="9" y="9" width="6" height="6" fill="#1c1c1e"/></svg>'],
  ];
  function fill(id,items){ const g=document.getElementById(id); for(const [cmd,ic] of items){ const b=document.createElement('div'); b.className='btn'; b.innerHTML=ic; b.onclick=()=>{ if(!b.classList.contains('dis')) window.ipc.postMessage(cmd); }; g.appendChild(b);} }
  fill('align',AL); fill('dist',DI); fill('pf',PF);
  window.varosTop=(s)=>{
    const n=(s&&s.n)||0;
    document.querySelectorAll('#align .btn,#pf .btn').forEach(b=>b.classList.toggle('dis',n<2));
    document.querySelectorAll('#dist .btn').forEach(b=>b.classList.toggle('dis',n<3));
    document.getElementById('info').textContent=(s&&s.info)||'';
  };
  function fwdkey(e){var t=(e.target&&e.target.tagName)||'';if(t==='INPUT'||t==='TEXTAREA'||(e.target&&e.target.isContentEditable))return;var m=(e.ctrlKey?1:0)|(e.shiftKey?2:0)|(e.altKey?4:0);window.ipc.postMessage((e.type==='keyup'?'keyup:':'keydown:')+m+':'+e.code);if(['Space','ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].indexOf(e.code)>=0)e.preventDefault();}
  addEventListener('keydown',fwdkey);addEventListener('keyup',fwdkey);
  window.addEventListener('DOMContentLoaded',()=>window.ipc.postMessage('ready'));
</script></body></html>"##;

const TOOLS_HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><style>
  :root{--bg-app:#1c1c1e;--bg-hover:#2c2c33;--border:#2a2a30;--muted:#a0a0a8;--text:#f0f0f2;--accent:#0c8ce9;}
  *{box-sizing:border-box;margin:0;padding:0}
  html,body{height:100%;background:var(--bg-app);overflow:hidden;user-select:none}
  .rail{display:flex;flex-direction:column;align-items:center;height:100%;padding:8px 0;gap:2px}
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
  const TOOLS=[['object',A],['direct',Ao],null,['pen',PEN],null,['rect',RECT],['ellipse',ELL],['triangle',TRI],null,['eyedropper',EYE]];
  const rail=document.getElementById('rail'); let active='pen';
  function render(){
    rail.innerHTML='';
    for(const t of TOOLS){
      if(!t){ const s=document.createElement('div'); s.className='sep'; rail.appendChild(s); continue; }
      const [id,svg]=t; const d=document.createElement('div');
      d.className='tb'+(id===active?' on':''); d.innerHTML=svg;
      d.onclick=()=>window.ipc.postMessage('tool:'+id); rail.appendChild(d);
    }
  }
  window.varosUI=(s)=>{ if(s&&s.tool){ active=s.tool; render(); } };
  render();
  function fwdkey(e){var t=(e.target&&e.target.tagName)||'';if(t==='INPUT'||t==='TEXTAREA'||(e.target&&e.target.isContentEditable))return;var m=(e.ctrlKey?1:0)|(e.shiftKey?2:0)|(e.altKey?4:0);window.ipc.postMessage((e.type==='keyup'?'keyup:':'keydown:')+m+':'+e.code);if(['Space','ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].indexOf(e.code)>=0)e.preventDefault();}
  addEventListener('keydown',fwdkey);addEventListener('keyup',fwdkey);
  window.addEventListener('DOMContentLoaded',()=>window.ipc.postMessage('ready'));
</script></body></html>"#;

const PANEL_HTML: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><style>
  :root{--bg-panel:#202024;--bg-surface:#26262b;--bg-hover:#2c2c33;--border:#2a2a30;
    --text:#f0f0f2;--muted:#a0a0a8;--faint:#6b6b72;--accent:#0c8ce9;
    --ui:'Inter','Segoe UI Variable','Segoe UI',system-ui,sans-serif;--mono:'JetBrains Mono','Cascadia Code',Consolas,monospace;}
  *{box-sizing:border-box;margin:0;padding:0}
  html,body{height:100%;background:var(--bg-panel);color:var(--text);font:13px/1.5 var(--ui);overflow:hidden;user-select:none;-webkit-font-smoothing:antialiased}
  .tabs{display:flex;gap:2px;padding:10px 12px 0}
  .tab{padding:7px 12px 9px;font-size:13px;font-weight:500;color:var(--muted);cursor:pointer;position:relative}
  .tab.on{color:var(--text)}
  .tab.on::after{content:'';position:absolute;left:8px;right:8px;bottom:0;height:2px;border-radius:2px;background:var(--accent)}
  .tab:hover:not(.on){color:var(--text)}
  .scroll{height:calc(100% - 39px);overflow:auto;border-top:1px solid var(--border)}
  .sec{padding:13px 16px;border-bottom:1px solid var(--border)}
  .sec-h{font-size:11px;letter-spacing:.5px;text-transform:uppercase;color:var(--muted);font-weight:600;margin-bottom:10px;display:flex;align-items:center;justify-content:space-between}
  .count{font-family:var(--mono);color:var(--faint);font-size:11px;font-weight:400}
  .empty{padding:22px 8px;color:var(--faint);font-size:12px;line-height:1.7;text-align:center}
  .grid2{display:grid;grid-template-columns:1fr 1fr;gap:8px}
  .fld{background:var(--bg-surface);border:1px solid var(--border);border-radius:7px;padding:6px 9px;display:flex;align-items:center;gap:8px}
  .fld label{color:var(--faint);font-size:11px;font-family:var(--mono);min-width:9px}
  .fld span{color:var(--text);font-family:var(--mono);font-size:12px}
  .row{display:flex;align-items:center;gap:9px}
  .cpick{width:30px;height:30px;border:1px solid var(--border);border-radius:8px;background:none;padding:0;cursor:pointer}
  .cpick::-webkit-color-swatch-wrapper{padding:3px}
  .cpick::-webkit-color-swatch{border:none;border-radius:5px}
  .hex{flex:1;font-family:var(--mono);font-size:12px;color:var(--text)}
  .clr{width:26px;height:26px;border-radius:7px;display:flex;align-items:center;justify-content:center;color:var(--faint);cursor:pointer;font-size:12px}
  .clr:hover{background:var(--bg-hover);color:var(--text)}
  .num{flex:1;background:var(--bg-surface);border:1px solid var(--border);border-radius:7px;padding:6px 9px;color:var(--text);font-family:var(--mono);font-size:12px}
  .num:focus{outline:1px solid var(--accent)}
  .wl{color:var(--faint);font-size:11px;min-width:42px}
  .layer{display:flex;align-items:center;gap:9px;padding:7px 9px;border-radius:8px;cursor:pointer;color:var(--muted);font-size:12.5px}
  .layer:hover{background:var(--bg-hover);color:var(--text)}
  .layer.sel{background:var(--accent);color:#fff}
  .dot{width:11px;height:11px;border-radius:3px;background:#4a4a54;flex:0 0 auto}
  .layer.grp .dot{border-radius:50%;background:#c9a23a}
  .layer.sel .dot{background:#fff}
  .nm{flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
</style></head><body>
  <div class="tabs">
    <div class="tab on" data-t="design">Design</div>
    <div class="tab" data-t="layers">Layers</div>
  </div>
  <div class="scroll">
    <div id="design">
      <div class="empty" id="d-empty">Select an object to edit<br>its size, fill &amp; stroke.</div>
      <div id="d-props" style="display:none">
        <div class="sec"><div class="sec-h">Layout</div>
          <div class="grid2">
            <div class="fld"><label>X</label><span id="fx">0</span></div>
            <div class="fld"><label>Y</label><span id="fy">0</span></div>
            <div class="fld"><label>W</label><span id="fw">0</span></div>
            <div class="fld"><label>H</label><span id="fh">0</span></div>
          </div>
        </div>
        <div class="sec"><div class="sec-h">Fill</div>
          <div class="row"><input type="color" class="cpick" id="fillc"><span class="hex" id="fillh">None</span><div class="clr" id="fillx" title="No fill">&#10005;</div></div>
        </div>
        <div class="sec"><div class="sec-h">Stroke</div>
          <div class="row"><input type="color" class="cpick" id="strokec"><span class="hex" id="strokeh">None</span><div class="clr" id="strokex" title="No stroke">&#10005;</div></div>
          <div class="row" style="margin-top:9px"><span class="wl">Width</span><input type="number" class="num" id="sw" min="0" step="0.5"></div>
        </div>
      </div>
    </div>
    <div id="layers" style="display:none">
      <div class="sec"><div class="sec-h"><span>Layers</span><span class="count" id="lc">0</span></div>
        <div id="list"><div class="empty">No objects yet</div></div></div>
    </div>
  </div>
<script>
  const $=id=>document.getElementById(id);
  const tabs=document.querySelectorAll('.tab');
  function show(t){ for(const el of tabs) el.classList.toggle('on',el.dataset.t===t);
    $('design').style.display=t==='design'?'':'none'; $('layers').style.display=t==='layers'?'':'none'; }
  for(const el of tabs) el.addEventListener('click',()=>show(el.dataset.t));
  function setSw(which,hex){ const c=$(which+'c'),h=$(which+'h'); if(hex){ c.value=hex; h.textContent=hex.toUpperCase(); } else { h.textContent='None'; } }
  window.varosDesign=(p)=>{
    if(!p||!p.sel){ $('d-props').style.display='none'; $('d-empty').style.display=''; return; }
    $('d-empty').style.display='none'; $('d-props').style.display='';
    $('fx').textContent=p.x; $('fy').textContent=p.y; $('fw').textContent=p.w; $('fh').textContent=p.h;
    setSw('fill',p.fill); setSw('stroke',p.stroke); $('sw').value=p.sw;
  };
  $('fillc').addEventListener('input',e=>window.ipc.postMessage('fill:'+e.target.value));
  $('fillx').addEventListener('click',()=>window.ipc.postMessage('fill:none'));
  $('strokec').addEventListener('input',e=>window.ipc.postMessage('stroke:'+e.target.value));
  $('strokex').addEventListener('click',()=>window.ipc.postMessage('stroke:none'));
  $('sw').addEventListener('change',e=>window.ipc.postMessage('sw:'+e.target.value));
  window.varosLayers=(rows)=>{
    $('lc').textContent=rows.length; const list=$('list');
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
  function fwdkey(e){var t=(e.target&&e.target.tagName)||'';if(t==='INPUT'||t==='TEXTAREA'||(e.target&&e.target.isContentEditable))return;var m=(e.ctrlKey?1:0)|(e.shiftKey?2:0)|(e.altKey?4:0);window.ipc.postMessage((e.type==='keyup'?'keyup:':'keydown:')+m+':'+e.code);if(['Space','ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].indexOf(e.code)>=0)e.preventDefault();}
  addEventListener('keydown',fwdkey);addEventListener('keyup',fwdkey);
  window.addEventListener('DOMContentLoaded',()=>window.ipc.postMessage('ready'));
</script></body></html>"#;

const ZOOM_HTML: &str = r#"<!doctype html><html><head><meta charset="utf-8"><style>
  :root{--bg-app:#1c1c1e;--bg-float:#232327;--bg-hover:#2c2c33;--border:#2a2a30;--text:#f0f0f2;--muted:#a0a0a8;
    --ui:'Inter','Segoe UI Variable','Segoe UI',system-ui,sans-serif;--mono:'JetBrains Mono','Cascadia Code',Consolas,monospace;}
  *{box-sizing:border-box;margin:0;padding:0}
  html,body{height:100%;background:var(--bg-app);overflow:hidden;user-select:none;display:flex;align-items:center;justify-content:center;font-family:var(--ui)}
  .pill{display:flex;align-items:center;gap:1px;background:var(--bg-float);border:1px solid var(--border);border-radius:11px;padding:3px;box-shadow:0 8px 24px rgba(0,0,0,.45)}
  .b{width:28px;height:28px;border-radius:8px;display:flex;align-items:center;justify-content:center;color:var(--muted);cursor:pointer;font-size:16px}
  .b:hover{background:var(--bg-hover);color:var(--text)}
  .z{min-width:48px;text-align:center;font-family:var(--mono);font-size:12px;color:var(--text)}
</style></head><body>
  <div class="pill"><div class="b" id="out">&#8722;</div><div class="z" id="z">100%</div><div class="b" id="in">+</div><div class="b" id="fit">&#9974;</div></div>
<script>
  const send=c=>window.ipc.postMessage('zoom:'+c);
  document.getElementById('out').onclick=()=>send('out');
  document.getElementById('in').onclick=()=>send('in');
  document.getElementById('fit').onclick=()=>send('fit');
  window.varosZoom=(p)=>{ document.getElementById('z').textContent=p+'%'; };
  function fwdkey(e){var t=(e.target&&e.target.tagName)||'';if(t==='INPUT'||t==='TEXTAREA'||(e.target&&e.target.isContentEditable))return;var m=(e.ctrlKey?1:0)|(e.shiftKey?2:0)|(e.altKey?4:0);window.ipc.postMessage((e.type==='keyup'?'keyup:':'keydown:')+m+':'+e.code);if(['Space','ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].indexOf(e.code)>=0)e.preventDefault();}
  addEventListener('keydown',fwdkey);addEventListener('keyup',fwdkey);
  window.addEventListener('DOMContentLoaded',()=>window.ipc.postMessage('ready'));
</script></body></html>"#;

// ============================ helpers ============================

fn hex(c: Rgba) -> String {
    format!("#{:02x}{:02x}{:02x}", (c[0]*255.0).round() as u8, (c[1]*255.0).round() as u8, (c[2]*255.0).round() as u8)
}
fn parse_hex(s: &str) -> Option<Rgba> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 { return None; }
    let r = u8::from_str_radix(&s[0..2], 16).ok()? as f32 / 255.0;
    let g = u8::from_str_radix(&s[2..4], 16).ok()? as f32 / 255.0;
    let b = u8::from_str_radix(&s[4..6], 16).ok()? as f32 / 255.0;
    Some([r, g, b, 1.0])
}
fn tool_from(s: &str) -> Option<ToolKind> {
    Some(match s {
        "object" => ToolKind::Object, "direct" => ToolKind::Direct, "pen" => ToolKind::Pen,
        "rect" => ToolKind::Rect, "ellipse" => ToolKind::Ellipse, "triangle" => ToolKind::Triangle,
        "eyedropper" => ToolKind::Eyedropper, _ => return None,
    })
}
fn tool_id(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Object => "object", ToolKind::Direct => "direct", ToolKind::Pen => "pen",
        ToolKind::Rect => "rect", ToolKind::Ellipse => "ellipse", ToolKind::Triangle => "triangle",
        ToolKind::Eyedropper => "eyedropper", ToolKind::Polygon => "polygon", ToolKind::Convert => "convert",
    }
}
/// Set absolute stroke width on the object selection.
fn set_stroke_width(ed: &mut Editor, w: f32) {
    let pids: Vec<u32> = ed.objsel.iter().copied().collect();
    if pids.is_empty() { return; }
    ed.begin();
    for pid in pids { if let Some(pi) = ed.doc.pidx(pid) { ed.doc.paths[pi].stroke_width = w.max(0.0); } }
    ed.dirty = true; ed.commit();
}

/// Layers rows (top of z-order first); groups = header (depth 0) + members (depth 1).
fn layers_json(ed: &Editor) -> String {
    let d = &ed.doc;
    let mut rows: Vec<String> = Vec::new();
    let mut cur_group: Option<u32> = None;
    for p in d.paths.iter().rev() {
        let sel = ed.objsel.contains(&p.id);
        match d.top_group_of_path(p.id) {
            None => { cur_group = None;
                rows.push(format!("{{\"pid\":{},\"name\":\"Object\",\"depth\":0,\"group\":false,\"sel\":{}}}", p.id, sel)); }
            Some(g) => {
                if cur_group != Some(g) { cur_group = Some(g);
                    rows.push(format!("{{\"pid\":{},\"name\":\"Group\",\"depth\":0,\"group\":true,\"sel\":{}}}", p.id, sel)); }
                rows.push(format!("{{\"pid\":{},\"name\":\"Object\",\"depth\":1,\"group\":false,\"sel\":{}}}", p.id, sel));
            }
        }
    }
    format!("[{}]", rows.join(","))
}
/// Inspector (Design tab) props of the object selection.
fn inspector_json(ed: &Editor) -> String {
    if ed.objsel.is_empty() { return "{\"sel\":false}".into(); }
    let (x, y, w, h) = match ed.obj_bbox() { Some((x0,y0,x1,y1)) => (x0, y0, x1-x0, y1-y0), None => (0.0,0.0,0.0,0.0) };
    let first = ed.objsel.iter().copied().filter_map(|pid| ed.doc.pidx(pid)).next();
    let (fill, stroke, sw) = match first { Some(pi) => { let p = &ed.doc.paths[pi]; (p.fill, p.stroke, p.stroke_width) }, None => (None, None, 1.0) };
    let fj = fill.map(|c| format!("\"{}\"", hex(c))).unwrap_or_else(|| "null".into());
    let sj = stroke.map(|c| format!("\"{}\"", hex(c))).unwrap_or_else(|| "null".into());
    format!("{{\"sel\":true,\"x\":{:.0},\"y\":{:.0},\"w\":{:.0},\"h\":{:.0},\"fill\":{},\"stroke\":{},\"sw\":{:.1}}}", x, y, w, h, fj, sj, sw)
}

fn push_layers(p: &WebView, ed: &Editor) { let _ = p.evaluate_script(&format!("window.varosLayers&&window.varosLayers({});", layers_json(ed))); }
fn push_inspector(p: &WebView, ed: &Editor) { let _ = p.evaluate_script(&format!("window.varosDesign&&window.varosDesign({});", inspector_json(ed))); }
fn push_ui(t: &WebView, ed: &Editor) { let _ = t.evaluate_script(&format!("window.varosUI&&window.varosUI({{\"tool\":\"{}\"}});", tool_id(ed.tool))); }
fn push_top(t: &WebView, ed: &Editor) {
    let n = ed.objsel.len();
    let info = match n { 0 => "No selection".to_string(), 1 => "1 object".to_string(), _ => format!("{n} objects") };
    let _ = t.evaluate_script(&format!("window.varosTop&&window.varosTop({{\"n\":{},\"info\":{:?}}});", n, info));
}
fn push_zoom(z: &WebView, zoom: f32) { let _ = z.evaluate_script(&format!("window.varosZoom&&window.varosZoom({});", (zoom*100.0).round() as i32)); }
fn refresh_all(panel: &WebView, tools: &WebView, topbar: &WebView, zoom: &WebView, ed: &Editor, zoomf: f32) {
    push_layers(panel, ed); push_inspector(panel, ed); push_ui(tools, ed); push_top(topbar, ed); push_zoom(zoom, zoomf);
}

fn tool_name(t: ToolKind) -> &'static str {
    match t {
        ToolKind::Pen => "Pen (P)", ToolKind::Direct => "Direct Select (A)", ToolKind::Object => "Select (V)",
        ToolKind::Rect => "Rectangle (M)", ToolKind::Ellipse => "Ellipse (L)", ToolKind::Triangle => "Triangle",
        ToolKind::Polygon => "Polygon", ToolKind::Convert => "Convert", ToolKind::Eyedropper => "Eyedropper (I)",
    }
}
fn full_title(t: ToolKind) -> String { format!("Varos \u{3b1} \u{b7} pre-alpha \u{2014} {}", tool_name(t)) }

/// Professional per-tool cursor (native OS cursors — crisp at any DPI).
fn cursor_for(t: ToolKind) -> CursorIcon {
    match t {
        ToolKind::Object | ToolKind::Direct => CursorIcon::Default,
        ToolKind::Pen | ToolKind::Convert => CursorIcon::Crosshair,
        ToolKind::Rect | ToolKind::Ellipse | ToolKind::Triangle | ToolKind::Polygon => CursorIcon::Crosshair,
        ToolKind::Eyedropper => CursorIcon::Crosshair,
    }
}

/// Apply a keyboard shortcut. `code` is a W3C key code ("KeyV", "ArrowLeft", "BracketRight", …) —
/// the same strings winit's `KeyCode` Debug and the webviews' `event.code` produce, so canvas
/// focus and forwarded-from-panel keys share ONE path. (Space-pan is handled by the callers.)
fn apply_key(ed: &mut Editor, view: &mut View, code: &str, ctrl: bool, shift: bool, _alt: bool) {
    if ctrl {
        match code {
            "Digit0" => *view = View::identity(),
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
        "Escape" | "Enter" => ed.escape(),
        "Delete" | "Backspace" => ed.delete_selected(),
        "ArrowLeft" => ed.nudge(-s, 0.0),
        "ArrowRight" => ed.nudge(s, 0.0),
        "ArrowUp" => ed.nudge(0.0, -s),
        "ArrowDown" => ed.nudge(0.0, s),
        _ => {}
    }
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
        .with_inner_size(winit::dpi::LogicalSize::new(1460.0, 860.0)).build(&event_loop).unwrap());
    let size = window.inner_size();
    let mut renderer = pollster::block_on(Renderer::new(window.clone(), size.width, size.height));

    let scale = window.scale_factor();
    let lsz = window.inner_size().to_logical::<f64>(scale);
    let topbar = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new(0.0, 0.0).into(), size: WSize::new(lsz.width, TOP_H).into() })
        .with_background_color((28,28,30,255)).with_html(TOPBAR_HTML)
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window).unwrap();
    let tools_panel = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new(0.0, TOP_H).into(), size: WSize::new(LEFT_W, (lsz.height-TOP_H).max(1.0)).into() })
        .with_background_color((28,28,30,255)).with_html(TOOLS_HTML)
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window).unwrap();
    let panel = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new((lsz.width-PANEL_W).max(0.0), TOP_H).into(), size: WSize::new(PANEL_W, (lsz.height-TOP_H).max(1.0)).into() })
        .with_background_color((32,32,36,255)).with_html(PANEL_HTML)
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window).unwrap();
    let zoom_panel = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new((lsz.width-PANEL_W-ZOOM_W-14.0).max(0.0), (lsz.height-ZOOM_H-14.0).max(0.0)).into(), size: WSize::new(ZOOM_W, ZOOM_H).into() })
        .with_background_color((28,28,30,255)).with_html(ZOOM_HTML)
        .with_ipc_handler(move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); })
        .build_as_child(&*window).unwrap();

    let mut ed = Editor::new();
    window.set_cursor_icon(cursor_for(ed.tool));
    let mut last_click: Option<(Instant, Pt)> = None;
    let mut view = View::identity();
    let mut screen_cursor: Pt = [0.0, 0.0];
    let mut panning = false;
    let mut pan_last: Pt = [0.0, 0.0];
    let mut space_down = false;

    // zoom around the centre of the visible canvas region (physical px)
    let zoom_about_canvas = |view: &mut View, ww: f64, wh: f64, scale: f64, f: f32| {
        let cx = (LEFT_W * scale + (ww - PANEL_W * scale)) * 0.5;
        let cy = (TOP_H * scale + wh) * 0.5;
        let c = [cx as f32, cy as f32];
        let wc = view.s2w(c);
        view.zoom = (view.zoom * f).clamp(0.05, 40.0);
        view.pan = [c[0] - wc[0]*view.zoom, c[1] - wc[1]*view.zoom];
    };

    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run(move |event, elwt| {
        match event {
        Event::UserEvent(UserEvent::Ipc(msg)) => {
            if msg == "ready" {
                // a panel finished loading → send full state
            } else if let Some(rest) = msg.strip_prefix("keydown:") {
                // a panel had keyboard focus and forwarded the key (so shortcuts work everywhere)
                if let Some((m, code)) = rest.split_once(':') {
                    let m: u8 = m.parse().unwrap_or(0);
                    let (kc, ks, ka) = (m & 1 != 0, m & 2 != 0, m & 4 != 0);
                    if code == "Space" { space_down = true; window.set_cursor_icon(CursorIcon::Grab); }
                    else { ed.mods = Mods { shift: ks, alt: ka, ctrl: kc }; apply_key(&mut ed, &mut view, code, kc, ks, ka); }
                }
            } else if let Some(rest) = msg.strip_prefix("keyup:") {
                if rest.split_once(':').map_or(false, |(_, c)| c == "Space") { space_down = false; panning = false; }
            } else if let Some(t) = msg.strip_prefix("tool:") {
                if let Some(tk) = tool_from(t) { ed.set_tool(tk); }
            } else if let Some(a) = msg.strip_prefix("align:") {
                match a { "left"=>ed.align(AlignMode::Left), "centerh"=>ed.align(AlignMode::CenterH), "right"=>ed.align(AlignMode::Right),
                          "top"=>ed.align(AlignMode::Top), "middle"=>ed.align(AlignMode::Middle), "bottom"=>ed.align(AlignMode::Bottom), _=>{} }
            } else if let Some(d) = msg.strip_prefix("dist:") {
                match d { "h"=>ed.distribute(DistAxis::Horizontal), "v"=>ed.distribute(DistAxis::Vertical), _=>{} }
            } else if let Some(p) = msg.strip_prefix("pf:") {
                match p { "unite"=>ed.pathfinder(BoolOp::Unite), "minus"=>ed.pathfinder(BoolOp::MinusFront),
                          "intersect"=>ed.pathfinder(BoolOp::Intersect), "exclude"=>ed.pathfinder(BoolOp::Exclude), _=>{} }
            } else if let Some(c) = msg.strip_prefix("fill:") {
                let col = if c == "none" { None } else { parse_hex(c) }; ed.paint = PaintTarget::Fill; ed.apply_paint(col);
            } else if let Some(c) = msg.strip_prefix("stroke:") {
                let col = if c == "none" { None } else { parse_hex(c) }; ed.paint = PaintTarget::Stroke; ed.apply_paint(col);
            } else if let Some(v) = msg.strip_prefix("sw:") {
                if let Ok(w) = v.parse::<f32>() { set_stroke_width(&mut ed, w); }
            } else if let Some(z) = msg.strip_prefix("zoom:") {
                let psz = window.inner_size();
                let (ww, wh) = (psz.width as f64, psz.height as f64);
                match z { "in"=>zoom_about_canvas(&mut view, ww, wh, scale, 1.25),
                          "out"=>zoom_about_canvas(&mut view, ww, wh, scale, 0.8),
                          _=>{ view = View::identity(); } }
            } else if let Ok(id) = msg.parse::<u32>() {
                if ed.doc.pidx(id).is_some() {
                    ed.set_tool(ToolKind::Object);
                    ed.objsel = ed.doc.group_members(id).into_iter().collect();
                    ed.selected.clear(); ed.obj_angle = 0.0;
                }
            }
            window.set_title(&full_title(ed.tool));
            if !space_down { window.set_cursor_icon(cursor_for(ed.tool)); }
            refresh_all(&panel, &tools_panel, &topbar, &zoom_panel, &ed, view.zoom);
            window.request_redraw();
        }
        Event::WindowEvent { event, window_id } => {
            if window_id != window.id() { return; }
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => {
                    renderer.resize(size.width, size.height);
                    let (lw, lh) = (size.width as f64 / scale, size.height as f64 / scale);
                    let _ = topbar.set_bounds(WryRect { position: WPos::new(0.0, 0.0).into(), size: WSize::new(lw, TOP_H).into() });
                    let _ = tools_panel.set_bounds(WryRect { position: WPos::new(0.0, TOP_H).into(), size: WSize::new(LEFT_W, (lh-TOP_H).max(1.0)).into() });
                    let _ = panel.set_bounds(WryRect { position: WPos::new((lw-PANEL_W).max(0.0), TOP_H).into(), size: WSize::new(PANEL_W, (lh-TOP_H).max(1.0)).into() });
                    let _ = zoom_panel.set_bounds(WryRect { position: WPos::new((lw-PANEL_W-ZOOM_W-14.0).max(0.0), (lh-ZOOM_H-14.0).max(0.0)).into(), size: WSize::new(ZOOM_W, ZOOM_H).into() });
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
                    window.set_cursor_icon(if panning { CursorIcon::Grabbing } else if space_down { CursorIcon::Grab } else { cursor_for(ed.tool) });
                    refresh_all(&panel, &tools_panel, &topbar, &zoom_panel, &ed, view.zoom);
                    window.request_redraw();
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let (dx, dy) = match delta { MouseScrollDelta::LineDelta(x, y) => (x, y), MouseScrollDelta::PixelDelta(p) => (p.x as f32 / 40.0, p.y as f32 / 40.0) };
                    if ed.mods.alt {
                        let f = (1.0 + dy * 0.12).clamp(0.2, 5.0);
                        let wc = view.s2w(screen_cursor);
                        view.zoom = (view.zoom * f).clamp(0.05, 40.0);
                        view.pan = [screen_cursor[0]-wc[0]*view.zoom, screen_cursor[1]-wc[1]*view.zoom];
                        push_zoom(&zoom_panel, view.zoom);
                    } else if ed.mods.shift { view.pan[0] += (dy + dx) * 30.0; }
                    else { view.pan[1] += dy * 30.0; view.pan[0] += dx * 30.0; }
                    window.request_redraw();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if let PhysicalKey::Code(code) = event.physical_key {
                        if code == KeyCode::Space {
                            space_down = event.state == ElementState::Pressed;
                            if !space_down { panning = false; }
                            window.set_cursor_icon(if space_down { CursorIcon::Grab } else { cursor_for(ed.tool) });
                            window.request_redraw();
                        } else if event.state == ElementState::Pressed {
                            let (mc, ms, ma) = (ed.mods.ctrl, ed.mods.shift, ed.mods.alt);
                            apply_key(&mut ed, &mut view, &format!("{:?}", code), mc, ms, ma);
                            window.set_title(&full_title(ed.tool));
                            window.set_cursor_icon(cursor_for(ed.tool));
                            refresh_all(&panel, &tools_panel, &topbar, &zoom_panel, &ed, view.zoom);
                            window.request_redraw();
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    let world = build_scene(&ed, view.zoom);
                    renderer.render(&world, &[], view);
                }
                _ => {}
            }
        }
        _ => {}
        }
    }).unwrap();
}
