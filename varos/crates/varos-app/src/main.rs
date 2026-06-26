#![windows_subsystem = "windows"] // no console window alongside the app
//! Varos desktop shell: winit window + wgpu canvas, with the UI chrome as wry web panels
//! (top bar, left tool rail + Fill/Stroke swatch, right inspector dock, zoom pill). The canvas
//! event loop and renderer are untouched — panels are sibling child HWNDs over the dark canvas.
//!
//! The right dock is built as independent panel MODULES (Transform · Align · Pathfinder ·
//! Properties · Layers · Color · Swatches) grouped into floating cards (PANELS_PRO_SPEC). A
//! Window menu shows/hides each module; full drag-to-dock is deferred (§0.5). Dark skin + tokens
//! from UI_FIGMA_SPEC (#141313 / #1f1f22 / #262627 / #0c8ce9, 12px floating cards).

use std::collections::HashMap;
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
use varos_core::editor::{AlignMode, DistAxis, Drag, Editor, Mods, PaintTarget, PenHint, TfHit, ToolKind, ZOrder};
use varos_core::BoolOp;
use varos_core::geom::{Pt, Rgba, View};
use varos_core::scene::build_scene;
use varos_render_wgpu::Renderer;

mod cursors;
mod ui;
use cursors::CK;

#[derive(Debug, Clone)]
enum UserEvent { Ipc(String) }

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

// ---- chrome geometry (logical px). SOLID DOCKED (§0.1): opaque panels at the window edges, the
//      wgpu canvas is the NATIVE surface in the centre (native pointer input → protects pen feel). ----
const TOP_H: f64 = 88.0;    // top bar — full width, docked top
const LEFT_W: f64 = 56.0;   // left tool rail — full height, docked left
const DOCK_W: f64 = 300.0;  // right inspector dock — full height, docked right
const PAD: f64 = 14.0;      // inset for the zoom pill / centred popups
const PICKER_W: f64 = 300.0;// colour picker — own borderless centred window
const PICKER_H: f64 = 430.0;
const ZOOM_W: f64 = 150.0;
const ZOOM_H: f64 = 38.0;

// ============================ web panels (HTML/CSS/JS) ============================
// Built by `page()` from shared CSS + shared JS + per-panel body/script (concatenated, so the
// braces inside CSS/JS never have to be escaped).

const DOC_HEAD: &str = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><style>"#;
const STYLE_TO_BODY: &str = r#"</style></head><body>"#;
const SCRIPT_OPEN: &str = r#"<script>"#;
const SCRIPT_CLOSE: &str = r#"</script></body></html>"#;

/// Shared design tokens + widget styles (UI_FIGMA_SPEC §1/§3).
const CSS: &str = r#"
:root{
 --bg-app:#141313;--bg-app-2:#1e1e1e;--bg-panel:#1f1f22;--bg-surface:#262627;--bg-hover:#2c2c2c;--bg-active:#2e2e2e;
 --border:#2a2a2d;--border-2:#3a3b3d;--text:#e6e6e6;--text-2:#d2d2d2;--muted:#8a8a8a;--faint:#7c7c7c;--accent:#0c8ce9;
 --ui:'Inter','Segoe UI Variable','Segoe UI',system-ui,sans-serif;--mono:'JetBrains Mono','Cascadia Code',Consolas,monospace;
 --shadow:0 8px 30px rgba(0,0,0,.45),inset 0 1px 0 rgba(255,255,255,.03);
}
*{box-sizing:border-box;margin:0;padding:0}
html,body{height:100%;background:var(--bg-app);color:var(--text);font:13px/1.5 var(--ui);overflow:hidden;user-select:none;-webkit-font-smoothing:antialiased}
.card{background:var(--bg-panel);border:1px solid var(--border);border-radius:12px;box-shadow:var(--shadow);overflow:hidden}
.tabs{display:flex;align-items:center;height:36px;padding:0 6px;border-bottom:1px solid var(--border);gap:1px}
.tab{display:flex;align-items:center;height:26px;padding:0 9px;font-size:12px;font-weight:500;color:var(--muted);cursor:default;border-radius:6px;white-space:nowrap}
.tab.on{color:var(--text);background:var(--bg-surface)}
.tab:hover:not(.on){color:var(--text-2)}
.tab[hidden]{display:none}
.cmenu{margin-left:auto;display:flex;align-items:center;gap:1px;color:var(--faint)}
.icx{width:22px;height:22px;border-radius:5px;display:flex;align-items:center;justify-content:center;cursor:pointer;color:var(--faint)}
.icx:hover{background:var(--bg-hover);color:var(--text)}
.icx svg{width:14px;height:14px}
.body{padding:0}
.pane[hidden]{display:none}
.sec{padding:11px 12px;border-bottom:1px solid var(--border)}
.sec:last-child{border-bottom:none}
.sec-h{font-size:10px;letter-spacing:.6px;text-transform:uppercase;color:var(--faint);font-weight:600;margin-bottom:9px;display:flex;align-items:center;justify-content:space-between}
.sub{font-size:10.5px;color:var(--muted);margin:9px 0 6px}
.row{display:flex;align-items:center;gap:6px}
.row+.row{margin-top:6px}
.grid2{display:grid;grid-template-columns:1fr 1fr;gap:6px}
.grid4{display:grid;grid-template-columns:repeat(4,1fr);gap:6px}
.fld{display:flex;align-items:center;gap:5px;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;height:28px;padding:0 7px;min-width:0}
.fld:focus-within{border-color:var(--border-2);background:var(--bg-active)}
.fld .lab{color:var(--faint);font-size:11px;font-family:var(--mono);cursor:ew-resize;min-width:9px;text-align:center;touch-action:none}
.fld input{flex:1;min-width:0;width:100%;background:none;border:none;outline:none;color:var(--text);font-family:var(--mono);font-size:12px;padding:0}
.fld .suf{color:var(--faint);font-size:10px;font-family:var(--mono)}
.ic{height:28px;min-width:30px;border:1px solid transparent;border-radius:6px;display:flex;align-items:center;justify-content:center;color:var(--muted);cursor:pointer;flex:1}
.ic:hover{background:var(--bg-hover);color:var(--text)}
.ic.dis{opacity:.28;pointer-events:none}
.ic.on{background:var(--accent);color:#fff}
.ic svg{width:16px;height:16px}
.seg{display:flex;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;overflow:hidden}
.seg .ic{border-radius:0;border:none}
.seg .ic+.ic{border-left:1px solid var(--border)}
.btn{height:28px;padding:0 10px;border:1px solid var(--border);border-radius:6px;background:var(--bg-surface);color:var(--text-2);font:500 12px var(--ui);display:flex;align-items:center;justify-content:center;gap:6px;cursor:pointer}
.btn:hover{background:var(--bg-hover);color:var(--text)}
.btn.dis{opacity:.3;pointer-events:none}
.btn.accent{background:var(--accent);border-color:var(--accent);color:#fff}
.sw{width:24px;height:24px;border-radius:6px;border:1px solid var(--border-2);cursor:pointer;flex:0 0 auto}
.sw.none,.swnone{background:repeating-conic-gradient(#3a3b3d 0% 25%,#262627 0% 50%) 50%/8px 8px}
.hex{flex:1;background:var(--bg-surface);border:1px solid var(--border);border-radius:6px;height:28px;padding:0 8px;color:var(--text);font-family:var(--mono);font-size:12px;text-transform:uppercase}
.hex:focus{outline:none;border-color:var(--border-2)}
.empty{padding:18px 10px;color:var(--faint);font-size:12px;line-height:1.7;text-align:center}
"#;

/// Shared JS: the pro number-input component (§4) + key forwarding + tiny helpers.
const COMMON_JS: &str = r#"
function $(id){return document.getElementById(id)}
// wry injects window.ipc asynchronously — during initial parse it may not exist yet. Queue
// messages and flush once the bridge appears, so no 'ready'/'tool:'/etc. is ever lost.
var _q=[];
function _flush(){ if(!(window.ipc&&window.ipc.postMessage)){setTimeout(_flush,40);return;} while(_q.length){ try{window.ipc.postMessage(_q.shift());}catch(e){break;} } }
function ipc(m){ try{ if(window.ipc&&window.ipc.postMessage){window.ipc.postMessage(m);return;} }catch(e){} _q.push(m); _flush(); }
// Pro number input: upgrade every .fld (label scrub, wheel, arrows, type+Enter/blur). Reads
// data-min/max/int/cmd; updates by role via setRole(). Returns nothing; state set via setRole.
function pro(fld){
 var inp=fld.querySelector('input'), lab=fld.querySelector('.lab');
 if(!inp||fld._pro)return; fld._pro=1;
 var min=fld.dataset.min!==undefined?+fld.dataset.min:-1e9;
 var max=fld.dataset.max!==undefined?+fld.dataset.max:1e9;
 var isInt=fld.dataset.int==='1';
 function get(){var v=parseFloat(inp.value);return isNaN(v)?0:v}
 function clamp(v){return Math.max(min,Math.min(max,v))}
 function fmt(v){return isInt?String(Math.round(v)):String(Math.round(v*100)/100)}
 function commit(v){v=clamp(v);inp.value=fmt(v);if(fld.dataset.cmd)ipc(fld.dataset.cmd+v)}
 function bump(d){commit(get()+d)}
 inp.addEventListener('keydown',function(e){
  if(e.key==='Enter'){commit(get());inp.blur()}
  else if(e.key==='ArrowUp'){e.preventDefault();bump(e.shiftKey?10:1)}
  else if(e.key==='ArrowDown'){e.preventDefault();bump(e.shiftKey?-10:-1)}
  e.stopPropagation();
 });
 inp.addEventListener('change',function(){commit(get())});
 inp.addEventListener('wheel',function(e){if(document.activeElement===inp){e.preventDefault();bump((e.deltaY<0?1:-1)*(e.shiftKey?10:1))}},{passive:false});
 if(lab){ var sc=null;   // label-scrub only when the field actually has a label inside it
  lab.addEventListener('pointerdown',function(e){sc={x:e.clientX,v:get()};try{lab.setPointerCapture(e.pointerId)}catch(_){}; e.preventDefault()});
  lab.addEventListener('pointermove',function(e){if(!sc)return;var f=e.shiftKey?10:(e.altKey?0.1:1);commit(sc.v+(e.clientX-sc.x)*0.5*f)});
  lab.addEventListener('pointerup',function(e){if(sc){sc=null;try{lab.releasePointerCapture(e.pointerId)}catch(_){}}});
 }
}
function proAll(root){(root||document).querySelectorAll('.fld').forEach(pro)}
// set every pro field / element carrying data-role=R (skips the one being edited)
function setRole(r,v){document.querySelectorAll('[data-role="'+r+'"]').forEach(function(el){
 var inp=el.matches('input')?el:el.querySelector('input');
 if(inp){if(document.activeElement!==inp)inp.value=v;}else{el.textContent=v;}
});}
function fwdkey(e){var t=(e.target&&e.target.tagName)||'';if(t==='INPUT'||t==='TEXTAREA'||(e.target&&e.target.isContentEditable))return;var m=(e.ctrlKey?1:0)|(e.shiftKey?2:0)|(e.altKey?4:0);ipc((e.type==='keyup'?'keyup:':'keydown:')+m+':'+e.code);if(['Space','ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].indexOf(e.code)>=0)e.preventDefault();}
addEventListener('keydown',fwdkey);addEventListener('keyup',fwdkey);
// SVG icon helper (Lucide-style thin stroke)
function svg(p,f){return '<svg viewBox="0 0 24 24" fill="'+(f||'none')+'" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">'+p+'</svg>'}
// ---- shared colour math (dock Color panel + the floating Color Picker) ----
function clampn(v,a,b){return Math.max(a,Math.min(b,v))}
function hex2rgb(h){h=(h||'').replace('#','');if(h.length!==6)return null;return{r:parseInt(h.slice(0,2),16),g:parseInt(h.slice(2,4),16),b:parseInt(h.slice(4,6),16)}}
function rgb2hex(r,g,b){function c(x){x=clampn(Math.round(x),0,255).toString(16);return x.length<2?'0'+x:x}return '#'+c(r)+c(g)+c(b)}
function rgb2hsv(r,g,b){r/=255;g/=255;b/=255;var mx=Math.max(r,g,b),mn=Math.min(r,g,b),d=mx-mn,h=0;if(d){if(mx===r)h=((g-b)/d)%6;else if(mx===g)h=(b-r)/d+2;else h=(r-g)/d+4;h*=60;if(h<0)h+=360}return{h:h,s:mx?d/mx:0,v:mx}}
function hsv2rgb(h,s,v){h=(h%360+360)%360;var c=v*s,x=c*(1-Math.abs((h/60)%2-1)),m=v-c,r=0,g=0,b=0;if(h<60){r=c;g=x}else if(h<120){r=x;g=c}else if(h<180){g=c;b=x}else if(h<240){g=x;b=c}else if(h<300){r=x;b=c}else{r=c;b=x}return{r:(r+m)*255,g:(g+m)*255,b:(b+m)*255}}
function rgb2cmyk(r,g,b){r/=255;g/=255;b/=255;var k=1-Math.max(r,g,b);if(k>=1)return{c:0,m:0,y:0,k:100};return{c:Math.round((1-r-k)/(1-k)*100),m:Math.round((1-g-k)/(1-k)*100),y:Math.round((1-b-k)/(1-k)*100),k:Math.round(k*100)}}
function cmyk2rgb(c,m,y,k){c/=100;m/=100;y/=100;k/=100;return{r:255*(1-c)*(1-k),g:255*(1-m)*(1-k),b:255*(1-y)*(1-k)}}
function normHex(v){v=(v||'').trim();if(v[0]!=='#')v='#'+v;return v}
window.onerror=function(m,s,l,c){try{ipc('jserr:'+m+' @'+l+':'+c)}catch(e){}};
var _booted=false;
function _bootnow(){
 if(_booted)return; _booted=true;
 try{ boot(); }catch(e){ ipc('jserr:boot '+(e&&e.stack||e&&e.message||e)); }   // lexical name — WebView2 doesn't expose fn decls on window
 ipc('ready');
}
"#;

fn page(extra_css: &str, body: &str, js: &str) -> String {
    let mut s = String::with_capacity(8192);
    s.push_str(DOC_HEAD); s.push_str(CSS); s.push_str(extra_css); s.push_str(STYLE_TO_BODY);
    s.push_str(body);
    s.push_str(SCRIPT_OPEN);
    // Wrap everything in an IIFE + try/catch with a self-contained reporter, so ANY top-level
    // throw is surfaced (the page's own window.onerror is installed mid-script — too late to catch
    // an error before it). boot() is invoked at the tail, after all consts + the body DOM exist
    // (wry/WebView2 fires DOMContentLoaded before our listeners attach, so a direct call is needed).
    s.push_str("(function(){var _RPT=function(m){try{if(window.ipc&&window.ipc.postMessage){window.ipc.postMessage(m);}else{setTimeout(function(){_RPT(m)},50);}}catch(e){}};try{\n");
    s.push_str(COMMON_JS); s.push_str(js);
    s.push_str("\n_bootnow();\n}catch(e){_RPT('jserr:TOP '+(e&&e.message)+' :: '+(e&&e.stack||''));}})();");
    s.push_str(SCRIPT_CLOSE);
    s
}

// -------------------------------------- TOP BAR --------------------------------------
const TOPBAR_CSS: &str = r#"
html,body{background:var(--bg-app)}
.wrap{height:100%;display:flex;flex-direction:column}
.tabsrow{height:42px;display:flex;align-items:center;gap:8px;padding:8px 12px 0}
.ftabs{display:flex;align-items:center;gap:4px;background:var(--bg-panel);border:1px solid var(--border);border-radius:10px;box-shadow:var(--shadow);padding:3px}
.ftab{height:26px;padding:0 11px;display:flex;align-items:center;gap:7px;border-radius:7px;background:transparent;color:var(--muted);font-size:12.5px;cursor:default}
.ftab.on{background:var(--bg-surface);color:var(--text)}
.ftab .d{width:7px;height:7px;border-radius:50%;background:var(--accent)}
.plus{width:26px;height:26px;border-radius:7px;display:flex;align-items:center;justify-content:center;color:var(--faint);cursor:pointer}
.plus:hover{color:var(--text)}
.right{margin-left:auto;display:flex;align-items:center;gap:8px;background:var(--bg-panel);border:1px solid var(--border);border-radius:10px;box-shadow:var(--shadow);padding:4px 6px 4px 10px}
.status{display:flex;align-items:center;gap:6px;color:var(--muted);font-size:12px}
.status .g{width:8px;height:8px;border-radius:50%;background:#3ecf8e;box-shadow:0 0 6px rgba(62,207,142,.6)}
.share{height:30px;padding:0 14px;border-radius:8px;background:var(--accent);color:#fff;font:600 12.5px var(--ui);display:flex;align-items:center;cursor:pointer}
.share:hover{filter:brightness(1.08)}
.ctl{height:48px;display:flex;align-items:center;justify-content:center;padding:0 12px}
.pill{display:flex;align-items:center;gap:8px;background:var(--bg-panel);border:1px solid var(--border);border-radius:12px;box-shadow:var(--shadow);height:40px;padding:0 10px}
.pill .lbl{color:var(--faint);font-size:11px;letter-spacing:.3px}
.pill .sep{width:1px;height:20px;background:var(--border);margin:0 2px}
.pill .fld{height:26px;width:80px}
.pill .grp{display:flex;gap:1px}
.winmenu{position:relative}
.dd{position:absolute;top:32px;right:0;min-width:170px;background:var(--bg-panel);border:1px solid var(--border);border-radius:10px;box-shadow:var(--shadow);padding:5px;z-index:50}
.dd[hidden]{display:none}
.dd .it{display:flex;align-items:center;gap:8px;height:28px;padding:0 9px;border-radius:6px;color:var(--text-2);font-size:12.5px;cursor:default}
.dd .it:hover{background:var(--bg-hover);color:var(--text)}
.dd .ck{width:14px;color:var(--accent)}
.dd .hr{height:1px;background:var(--border);margin:5px 4px}
"#;
const TOPBAR_BODY: &str = r#"
<div class="wrap">
 <div class="tabsrow">
  <div class="ftabs">
   <div class="ftab on"><span class="d"></span>Untitled-1</div>
  </div>
  <div class="plus" title="New tab">+</div>
  <div class="right">
   <div class="status"><span class="g"></span><span id="info">No selection</span></div>
   <div class="share">Share</div>
  </div>
 </div>
 <div class="ctl">
  <div class="pill">
   <span class="lbl">Align</span>
   <div class="grp" id="al"></div>
   <div class="sep"></div>
   <span class="lbl">X</span><div class="fld" data-role="x" data-cmd="set:x:" data-int="1" style="width:78px"><input></div>
   <span class="lbl">Y</span><div class="fld" data-role="y" data-cmd="set:y:" data-int="1" style="width:78px"><input></div>
   <div class="sep"></div>
   <span class="lbl">Rotate</span><div class="fld" data-role="rot" data-cmd="set:rot:" style="width:72px"><input><span class="suf">°</span></div>
  </div>
 </div>
</div>
"#;
const TOPBAR_JS: &str = r#"
var AL=[['align:left','<path d="M4 4V20M4 9H17M4 15H11"/>'],['align:centerh','<path d="M12 4V20M5 9H19M8 15H16"/>'],['align:right','<path d="M20 4V20M7 9H20M13 15H20"/>']];
function boot(){
 var al=$('al');
 AL.forEach(function(a){var b=document.createElement('div');b.className='ic';b.style.flex='0 0 auto';b.style.minWidth='28px';b.innerHTML=svg(a[1]);b.onclick=function(){if(!b.classList.contains('dis'))ipc(a[0])};al.appendChild(b)});
 proAll();
}
window.varosTop=function(s){
 var n=(s&&s.n)||0;
 document.querySelectorAll('#al .ic').forEach(function(b){b.classList.toggle('dis',n<2)});
 if(s){setRole('x',s.x);setRole('y',s.y);setRole('rot',s.rot);$('info').textContent=s.info||'';}
};
"#;

// -------------------------------------- TOOL RAIL --------------------------------------
const TOOLS_CSS: &str = r#"
html,body{background:var(--bg-app)}
.col{height:100%;display:flex;flex-direction:column;align-items:center;justify-content:center}
.rail{display:flex;flex-direction:column;align-items:center;background:var(--bg-panel);border:1px solid var(--border);border-radius:14px;box-shadow:var(--shadow);padding:7px 6px}
#tools{display:flex;flex-direction:column;align-items:center;gap:2px}
.tb{position:relative;width:34px;height:34px;border-radius:7px;display:flex;align-items:center;justify-content:center;cursor:pointer;color:var(--muted)}
.tb:hover{background:var(--bg-hover);color:var(--text)}
.tb.on{background:var(--accent);color:#fff}
.tb svg{width:18px;height:18px}
.rsep{width:22px;height:1px;background:var(--border);margin:5px 0}
.paint{display:flex;flex-direction:column;align-items:center;gap:8px;margin-top:2px}
.fs{position:relative;width:38px;height:38px}
.fs .sq{position:absolute;width:23px;height:23px;border-radius:4px;border:1.5px solid var(--border-2);cursor:pointer}
.fs .fill{left:0;top:0;background:#e6e6e6;z-index:2}
.fs .stroke{right:0;bottom:0;background:transparent;box-shadow:inset 0 0 0 3px #111;z-index:1}
.fs .stroke.on,.fs .fill.on{border-color:var(--accent);z-index:3}
.fs .swap{position:absolute;right:-4px;top:-4px;width:13px;height:13px;color:var(--faint);cursor:pointer}
.fs .swap:hover{color:var(--text)}
.fs .def{position:absolute;left:-3px;bottom:-3px;width:12px;height:12px;border-radius:2px;background:#e6e6e6;box-shadow:inset 0 0 0 2px #111,2px 2px 0 -1px var(--bg-panel);cursor:pointer}
.modes{display:flex;gap:3px}
.modes .m{width:16px;height:16px;border-radius:4px;display:flex;align-items:center;justify-content:center;color:var(--faint);cursor:pointer;font-size:11px}
.modes .m:hover{color:var(--text);background:var(--bg-hover)}
.modes .m.none{background:repeating-conic-gradient(#3a3b3d 0% 25%,#262627 0% 50%) 50%/6px 6px}
"#;
const TOOLS_BODY: &str = r#"
<div class="col">
 <div class="rail">
  <div id="tools"></div>
  <div class="rsep"></div>
  <div class="paint">
   <div class="fs">
    <div class="sq fill on" id="pfill" title="Fill"></div>
    <div class="sq stroke" id="pstroke" title="Stroke"></div>
    <div class="swap" id="pswap" title="Swap fill/stroke (Shift+X)"></div>
    <div class="def" id="pdef" title="Default (D)"></div>
   </div>
   <div class="modes">
    <div class="m" id="mcolor" title="Color">&#9679;</div>
    <div class="m none" id="mnone" title="None (/)"></div>
   </div>
  </div>
 </div>
</div>
"#;
const TOOLS_JS: &str = r#"
var SEL=svg('<path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"/>');
var DIR=svg('<path d="M12.586 12.586 19 19"/><path d="M3.688 3.037a.497.497 0 0 0-.651.651l6.5 15.999a.501.501 0 0 0 .947-.062l1.569-6.083a2 2 0 0 1 1.448-1.479l6.124-1.579a.5.5 0 0 0 .063-.947z"/>');
var PEN=svg('<path d="M15.707 21.293a1 1 0 0 1-1.414 0l-1.586-1.586a1 1 0 0 1 0-1.414l5.586-5.586a1 1 0 0 1 1.414 0l1.586 1.586a1 1 0 0 1 0 1.414z"/><path d="m18 13-1.375-6.874a1 1 0 0 0-.746-.776L3.235 2.028a1 1 0 0 0-1.207 1.207L5.35 15.879a1 1 0 0 0 .776.746L13 18"/><path d="m2.3 2.3 7.286 7.286"/><circle cx="11" cy="11" r="2"/>');
var SQ=svg('<rect width="18" height="18" x="3" y="3" rx="2"/>');
var CI=svg('<circle cx="12" cy="12" r="10"/>');
var TRI=svg('<path d="M13.73 4a2 2 0 0 0-3.46 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/>');
var PIP=svg('<path d="m12 9-8.414 8.414A2 2 0 0 0 3 18.828v1.344a2 2 0 0 1-.586 1.414A2 2 0 0 1 3.828 21h1.344a2 2 0 0 0 1.414-.586L15 12"/><path d="m18 9 .4.4a1 1 0 1 1-3 3l-3.8-3.8a1 1 0 1 1 3-3l.4.4 3.4-3.4a1 1 0 1 1 3 3z"/><path d="m2 22 .414-.414"/>');
var TEXT=svg('<path d="M4 7V5h16v2M9 19h6M12 5v14"/>');
var TOOLS=[['object',SEL,'Selection — V'],['direct',DIR,'Direct Selection — A'],null,['pen',PEN,'Pen — P'],null,['rect',SQ,'Rectangle — M'],['ellipse',CI,'Ellipse — L'],['triangle',TRI,'Polygon'],null,['eyedropper',PIP,'Eyedropper — I']];
var active='pen';
function render(){var rail=$('tools');rail.innerHTML='';TOOLS.forEach(function(t){if(!t){var s=document.createElement('div');s.className='rsep';rail.appendChild(s);return}var d=document.createElement('div');d.className='tb'+(t[0]===active?' on':'');d.innerHTML=t[1];d.title=t[2];d.onclick=function(){ipc('tool:'+t[0])};rail.appendChild(d)});}
function boot(){
 render();
 $('pswap').innerHTML=svg('<path d="M16 3l4 4-4 4M20 7H10M8 21l-4-4 4-4M4 17h10"/>');
 $('pfill').onclick=function(){setTarget('fill');ipc('pick:open:fill')};
 $('pstroke').onclick=function(){setTarget('stroke');ipc('pick:open:stroke')};
 $('pswap').onclick=function(){ipc('swapcolors')};
 $('pdef').onclick=function(){ipc('defaultpaint')};
 $('mcolor').onclick=function(){ipc('pick:open:'+target)};
 $('mnone').onclick=function(){ipc('paintnone')};
}
var target='fill';
function setTarget(t){target=t;$('pfill').classList.toggle('on',t==='fill');$('pstroke').classList.toggle('on',t==='stroke');}
window.varosUI=function(s){
 if(!s)return;
 if(s.tool){active=s.tool;render();}
 if(s.paint){setTarget(s.paint);}
 if('fill' in s){$('pfill').style.background=s.fill||'transparent';$('pfill').classList.toggle('swnone',!s.fill);}
 if('stroke' in s){var st=$('pstroke');if(s.stroke){st.style.boxShadow='inset 0 0 0 3px '+s.stroke;st.classList.remove('swnone');}else{st.style.boxShadow='none';st.classList.add('swnone');}}
};
"#;

// -------------------------------------- ZOOM PILL --------------------------------------
const ZOOM_CSS: &str = r#"
html,body{display:flex;align-items:center;justify-content:center;background:var(--bg-app)}
.pill{display:flex;align-items:center;gap:1px;background:var(--bg-panel);border:1px solid var(--border);border-radius:11px;padding:3px;box-shadow:var(--shadow)}
.b{width:28px;height:28px;border-radius:8px;display:flex;align-items:center;justify-content:center;color:var(--muted);cursor:pointer;font-size:16px}
.b:hover{background:var(--bg-hover);color:var(--text)}
.z{min-width:46px;text-align:center;font-family:var(--mono);font-size:12px;color:var(--text)}
"#;
const ZOOM_BODY: &str = r#"<div class="pill"><div class="b" id="out">&#8722;</div><div class="z" id="z">100%</div><div class="b" id="in">+</div><div class="b" id="fit" title="Fit">&#9974;</div></div>"#;
const ZOOM_JS: &str = r#"
function boot(){$('out').onclick=function(){ipc('zoom:out')};$('in').onclick=function(){ipc('zoom:in')};$('fit').onclick=function(){ipc('zoom:fit')};}
window.varosZoom=function(p){$('z').textContent=p+'%'};
"#;

// -------------------------------------- COLOR PICKER (own centered floating webview) --------------------------------------
const PICKER_CSS: &str = r##"
html,body{background:transparent}
.pkcard{width:100%;height:100%;background:var(--bg-panel);border:1px solid var(--border-2);border-radius:14px;box-shadow:var(--shadow);padding:12px;display:flex;flex-direction:column}
.pkhd{display:flex;align-items:center;gap:2px;margin-bottom:10px}
.pkhd .tab{height:24px;padding:0 10px;border-radius:6px;font-size:12px;color:var(--muted);cursor:pointer;display:flex;align-items:center}
.pkhd .tab.on{background:var(--bg-surface);color:var(--text)}
.pkhd .x{margin-left:auto;width:22px;height:22px;display:flex;align-items:center;justify-content:center;border-radius:5px;color:var(--faint);cursor:pointer}
.pkhd .x:hover{background:var(--bg-hover);color:var(--text)}
.pkpane[hidden]{display:none}
.pktop{display:flex;gap:8px;height:150px}
.sv{flex:1;position:relative;border-radius:6px;cursor:crosshair;touch-action:none}
.svth{position:absolute;width:12px;height:12px;border-radius:50%;border:2px solid #fff;box-shadow:0 0 0 1px rgba(0,0,0,.5);transform:translate(-50%,-50%);pointer-events:none}
.hbv{width:18px;position:relative;border-radius:6px;cursor:pointer;touch-action:none;background:linear-gradient(to bottom,#f00,#ff0,#0f0,#0ff,#00f,#f0f,#f00)}
.hbth{position:absolute;left:-2px;right:-2px;height:4px;background:#fff;border:1px solid rgba(0,0,0,.5);border-radius:2px;transform:translateY(-50%);pointer-events:none}
.pkprev{display:flex;height:20px;margin:9px 0;border-radius:5px;overflow:hidden;border:1px solid var(--border)}
.pkprev .pp{flex:1}
.pkf{display:flex;align-items:center;gap:5px;margin-bottom:5px}
.pkf .l{width:14px;color:var(--faint);font-size:10.5px;font-family:var(--mono);text-align:center}
.pkf input{flex:1;min-width:0;background:var(--bg-surface);border:1px solid var(--border);border-radius:5px;height:24px;padding:0 6px;color:var(--text);font-family:var(--mono);font-size:11.5px;outline:none;text-transform:uppercase}
.pkf input:focus{border-color:var(--border-2)}
.pkgrid{display:grid;grid-template-columns:repeat(8,1fr);gap:6px}
.pkgrid .s{aspect-ratio:1;border-radius:5px;border:1px solid var(--border-2);cursor:pointer}
.pkgrid .s:hover{transform:scale(1.12)}
.pkbtns{display:flex;align-items:center;gap:6px;margin-top:auto;padding-top:10px}
.pkbtns .btn{height:30px}
"##;
const PICKER_BODY: &str = r##"
<div class="pkcard">
 <div class="pkhd">
  <div class="tab on" data-pp="color">Color</div>
  <div class="tab" data-pp="swatches">Swatches</div>
  <div class="x" id="pkx">&#10005;</div>
 </div>
 <div class="pkpane" data-pp="color">
  <div class="pktop">
   <div class="sv" id="sv"><div class="svth" id="svth"></div></div>
   <div class="hbv" id="hb"><div class="hbth" id="hbth"></div></div>
  </div>
  <div class="pkprev"><div class="pp" id="ppnew"></div><div class="pp" id="ppcur"></div></div>
  <div class="row" style="gap:6px">
   <div class="pkf" style="flex:1"><span class="l">H</span><input id="fH"></div>
   <div class="pkf" style="flex:1"><span class="l">S</span><input id="fS"></div>
   <div class="pkf" style="flex:1"><span class="l">B</span><input id="fV"></div>
  </div>
  <div class="row" style="gap:6px">
   <div class="pkf" style="flex:1"><span class="l">R</span><input id="fR"></div>
   <div class="pkf" style="flex:1"><span class="l">G</span><input id="fG"></div>
   <div class="pkf" style="flex:1"><span class="l">B</span><input id="fB"></div>
  </div>
  <div class="pkf"><span class="l">#</span><input id="fHex"></div>
  <div class="row" style="gap:6px">
   <div class="pkf" style="flex:1"><span class="l">C</span><input id="fC"></div>
   <div class="pkf" style="flex:1"><span class="l">M</span><input id="fM"></div>
   <div class="pkf" style="flex:1"><span class="l">Y</span><input id="fY"></div>
   <div class="pkf" style="flex:1"><span class="l">K</span><input id="fK"></div>
  </div>
 </div>
 <div class="pkpane" data-pp="swatches" hidden>
  <div class="pkgrid" id="pkgrid"></div>
  <div class="row" style="margin-top:9px"><div class="btn" id="pkadd" style="flex:1">+ Add current</div></div>
  <div class="sub" style="font-size:10px;color:var(--faint);margin-top:8px">Shift-click a swatch to remove it.</div>
 </div>
 <div class="pkbtns"><div style="flex:1"></div><div class="btn" id="pkcancel">Cancel</div><div class="btn accent" id="pkok">OK</div></div>
</div>
"##;
const PICKER_JS: &str = r##"
var PK={h:210,s:1,v:1,target:'fill',cur:'#0c8ce9'}, PKSW=[];
function showPane(p){document.querySelectorAll('.pkhd .tab').forEach(function(t){t.classList.toggle('on',t.dataset.pp===p)});document.querySelectorAll('.pkpane').forEach(function(el){el.hidden=el.dataset.pp!==p})}
function boot(){
 var sv=$('sv'),hb=$('hb');
 function svAt(e){var b=sv.getBoundingClientRect();PK.s=clampn((e.clientX-b.left)/b.width,0,1);PK.v=clampn(1-(e.clientY-b.top)/b.height,0,1);pkRender('sv')}
 function hbAt(e){var b=hb.getBoundingClientRect();PK.h=clampn((e.clientY-b.top)/b.height,0,1)*360;pkRender('hb')}
 function drag(el,fn){el.addEventListener('pointerdown',function(e){try{el.setPointerCapture(e.pointerId)}catch(_){}fn(e);el._d=1;e.preventDefault()});el.addEventListener('pointermove',function(e){if(el._d)fn(e)});el.addEventListener('pointerup',function(e){el._d=0;try{el.releasePointerCapture(e.pointerId)}catch(_){}})}
 drag(sv,svAt);drag(hb,hbAt);
 function bindHSV(id,key){$(id).addEventListener('input',function(){var v=parseFloat(this.value);if(isNaN(v))return;if(key==='h')PK.h=clampn(v,0,360);else PK[key]=clampn(v,0,100)/100;pkRender(id)});$(id).addEventListener('keydown',function(e){e.stopPropagation()})}
 bindHSV('fH','h');bindHSV('fS','s');bindHSV('fV','v');
 ['fR','fG','fB'].forEach(function(id){$(id).addEventListener('input',function(){var hsv=rgb2hsv(clampn(parseFloat($('fR').value)||0,0,255),clampn(parseFloat($('fG').value)||0,0,255),clampn(parseFloat($('fB').value)||0,0,255));PK.h=hsv.h;PK.s=hsv.s;PK.v=hsv.v;pkRender(id)});$(id).addEventListener('keydown',function(e){e.stopPropagation()})});
 $('fHex').addEventListener('input',function(){var rgb=hex2rgb(normHex(this.value));if(!rgb)return;var hsv=rgb2hsv(rgb.r,rgb.g,rgb.b);PK.h=hsv.h;PK.s=hsv.s;PK.v=hsv.v;pkRender('fHex')});
 $('fHex').addEventListener('keydown',function(e){e.stopPropagation()});
 ['fC','fM','fY','fK'].forEach(function(id){$(id).addEventListener('input',function(){var rgb=cmyk2rgb(parseFloat($('fC').value)||0,parseFloat($('fM').value)||0,parseFloat($('fY').value)||0,parseFloat($('fK').value)||0);var hsv=rgb2hsv(rgb.r,rgb.g,rgb.b);PK.h=hsv.h;PK.s=hsv.s;PK.v=hsv.v;pkRender(id)});$(id).addEventListener('keydown',function(e){e.stopPropagation()})});
 document.querySelectorAll('.pkhd .tab').forEach(function(t){t.onclick=function(){showPane(t.dataset.pp)}});
 $('pkadd').onclick=function(){var rgb=hsv2rgb(PK.h,PK.s,PK.v);ipc('swatchadd:'+rgb2hex(rgb.r,rgb.g,rgb.b))};
 $('pkcancel').onclick=function(){ipc('pickclose')};
 $('pkx').onclick=function(){ipc('pickclose')};
 $('pkok').onclick=function(){var rgb=hsv2rgb(PK.h,PK.s,PK.v);ipc(PK.target+':'+rgb2hex(rgb.r,rgb.g,rgb.b));ipc('pickclose')};
 pkRender('');
}
function renderSw(){var g=$('pkgrid');if(!g)return;g.innerHTML='';PKSW.forEach(function(h){var d=document.createElement('div');d.className='s';d.style.background=h;d.title=h;d.onclick=function(e){if(e.shiftKey){ipc('swatch:del:'+h)}else{var rgb=hex2rgb(h);if(rgb){var hsv=rgb2hsv(rgb.r,rgb.g,rgb.b);PK.h=hsv.h;PK.s=hsv.s;PK.v=hsv.v;showPane('color');pkRender('')}}};g.appendChild(d)})}
function pkRender(src){
 var rgb=hsv2rgb(PK.h,PK.s,PK.v),hexv=rgb2hex(rgb.r,rgb.g,rgb.b),cmyk=rgb2cmyk(rgb.r,rgb.g,rgb.b);
 var hueRgb=hsv2rgb(PK.h,1,1);
 $('sv').style.background='linear-gradient(to top,#000,rgba(0,0,0,0)),linear-gradient(to right,#fff,rgba(255,255,255,0)),'+rgb2hex(hueRgb.r,hueRgb.g,hueRgb.b);
 $('svth').style.left=(PK.s*100)+'%';$('svth').style.top=((1-PK.v)*100)+'%';
 $('hbth').style.top=(PK.h/360*100)+'%';
 $('ppnew').style.background=hexv;
 function set(id,v){if(src!==id)$(id).value=v}
 set('fH',Math.round(PK.h));set('fS',Math.round(PK.s*100));set('fV',Math.round(PK.v*100));
 set('fR',Math.round(rgb.r));set('fG',Math.round(rgb.g));set('fB',Math.round(rgb.b));
 set('fHex',hexv.toUpperCase());
 set('fC',cmyk.c);set('fM',cmyk.m);set('fY',cmyk.y);set('fK',cmyk.k);
}
window.varosPick=function(target,hexv){
 PK.target=target;PK.cur=hexv;var rgb=hex2rgb(hexv)||{r:0,g:0,b:0};var hsv=rgb2hsv(rgb.r,rgb.g,rgb.b);PK.h=hsv.h;PK.s=hsv.s;PK.v=hsv.v;
 $('ppcur').style.background=hexv;showPane('color');pkRender('');
};
window.varosSwatches=function(arr){PKSW=arr||[];renderSw()};
"##;

include!("panel_html.rs"); // the big right-dock module (PANEL_CSS / PANEL_BODY / PANEL_JS)

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
fn paint_id(p: PaintTarget) -> &'static str { match p { PaintTarget::Fill => "fill", PaintTarget::Stroke => "stroke" } }

fn set_stroke_width(ed: &mut Editor, w: f32) {
    let pids: Vec<u32> = ed.objsel.iter().copied().collect();
    if pids.is_empty() { return; }
    ed.begin();
    for pid in pids { if let Some(pi) = ed.doc.pidx(pid) { ed.doc.paths[pi].stroke_width = w.max(0.0); } }
    ed.dirty = true; ed.commit();
}

/// The paint color (hex) the picker should open with, for a target. Prefers the selection's, then
/// the current swatch. None ⇒ "no paint" (picker starts from black).
fn cur_paint_hex(ed: &Editor, target: PaintTarget) -> Option<String> {
    let first = ed.objsel.iter().copied().filter_map(|pid| ed.doc.pidx(pid)).next();
    let from_sel = first.and_then(|pi| match target { PaintTarget::Fill => ed.doc.paths[pi].fill, PaintTarget::Stroke => ed.doc.paths[pi].stroke });
    let c = from_sel.or(match target { PaintTarget::Fill => ed.cur_fill, PaintTarget::Stroke => ed.cur_stroke });
    c.map(hex)
}

fn jstr(s: &str) -> String { // minimal JSON string escape
    let mut o = String::with_capacity(s.len()+2); o.push('"');
    for ch in s.chars() { match ch { '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), _ => o.push(ch) } }
    o.push('"'); o
}

/// Layers rows (top of z-order first); groups = header (depth 0) + members (depth 1).
fn layers_json(ed: &Editor) -> String {
    let d = &ed.doc;
    let mut rows: Vec<String> = Vec::new();
    let mut cur_group: Option<u32> = None;
    let row = |pid: u32, name: String, depth: u8, group: bool, sel: bool, hidden: bool, locked: bool| {
        format!("{{\"pid\":{},\"name\":{},\"depth\":{},\"group\":{},\"sel\":{},\"hidden\":{},\"locked\":{}}}",
            pid, jstr(&name), depth, group, sel, hidden, locked)
    };
    for p in d.paths.iter().rev() {
        let sel = ed.objsel.contains(&p.id);
        let nm = p.name.clone().unwrap_or_else(|| "Path".to_string());
        match d.top_group_of_path(p.id) {
            None => { cur_group = None; rows.push(row(p.id, nm, 0, false, sel, p.hidden, p.locked)); }
            Some(g) => {
                if cur_group != Some(g) { cur_group = Some(g);
                    let gname = d.groups.iter().find(|x| x.id == g).map(|x| x.name.clone()).unwrap_or_else(|| "Group".into());
                    rows.push(row(p.id, gname, 0, true, sel, false, false)); }
                rows.push(row(p.id, nm, 1, false, sel, p.hidden, p.locked));
            }
        }
    }
    format!("[{}]", rows.join(","))
}

/// The single big state object the right-dock panel consumes.
fn state_json(ed: &Editor, swatches: &[String]) -> String {
    let n = ed.objsel.len();
    let (sel, x, y, w, h) = match ed.obj_bbox() {
        Some((x0,y0,x1,y1)) if n > 0 => (true, x0, y0, x1-x0, y1-y0),
        _ => (false, 0.0, 0.0, 0.0, 0.0),
    };
    let rot = ed.obj_angle.to_degrees();
    let first = ed.objsel.iter().copied().filter_map(|pid| ed.doc.pidx(pid)).next();
    let (fill, stroke, sw, op) = match first {
        Some(pi) => { let p = &ed.doc.paths[pi]; (p.fill, p.stroke, p.stroke_width, p.opacity) },
        None => (ed.cur_fill, ed.cur_stroke, ed.cur_sw, 1.0),
    };
    let fj = fill.map(|c| format!("\"{}\"", hex(c))).unwrap_or_else(|| "null".into());
    let sj = stroke.map(|c| format!("\"{}\"", hex(c))).unwrap_or_else(|| "null".into());
    // object type label
    let ty = if n == 0 { String::new() } else if n == 1 {
        ed.doc.paths[first.unwrap()].name.clone().unwrap_or_else(|| "Path".into())
    } else {
        let groups: std::collections::HashSet<Option<u32>> = ed.objsel.iter().map(|&p| ed.doc.top_group_of_path(p)).collect();
        if groups.len() == 1 && !groups.contains(&None) { "Group".into() } else { "Multiple".into() }
    };
    let sw_json: Vec<String> = swatches.iter().map(|s| jstr(s)).collect();
    format!("{{\"sel\":{},\"n\":{},\"type\":{},\"x\":{:.0},\"y\":{:.0},\"w\":{:.0},\"h\":{:.0},\"rot\":{:.1},\
             \"fill\":{},\"stroke\":{},\"sw\":{:.2},\"opacity\":{},\"paint\":\"{}\",\"layers\":{},\"swatches\":[{}]}}",
        sel, n, jstr(&ty), x, y, w, h, rot, fj, sj, sw, (op*100.0).round() as i32, paint_id(ed.paint),
        layers_json(ed), sw_json.join(","))
}

fn push_state(p: &WebView, ed: &Editor, swatches: &[String]) {
    let _ = p.evaluate_script(&format!("window.varosState&&window.varosState({});", state_json(ed, swatches)));
}
fn push_ui(t: &WebView, ed: &Editor) {
    let fj = ed.cur_fill.map(|c| format!("\"{}\"", hex(c))).unwrap_or_else(|| "null".into());
    let sj = ed.cur_stroke.map(|c| format!("\"{}\"", hex(c))).unwrap_or_else(|| "null".into());
    let _ = t.evaluate_script(&format!("window.varosUI&&window.varosUI({{\"tool\":\"{}\",\"paint\":\"{}\",\"fill\":{},\"stroke\":{}}});",
        tool_id(ed.tool), paint_id(ed.paint), fj, sj));
}
fn push_top(t: &WebView, ed: &Editor) {
    let n = ed.objsel.len();
    let info = match n { 0 => "No selection".to_string(), 1 => "1 object".to_string(), _ => format!("{n} objects") };
    let (x, y) = match ed.obj_bbox() { Some((x0,y0,_,_)) if n>0 => (x0, y0), _ => (0.0, 0.0) };
    let _ = t.evaluate_script(&format!("window.varosTop&&window.varosTop({{\"n\":{},\"x\":{:.0},\"y\":{:.0},\"rot\":{:.1},\"info\":{}}});",
        n, x, y, ed.obj_angle.to_degrees(), jstr(&info)));
}
fn push_zoom(z: &WebView, zoom: f32) { let _ = z.evaluate_script(&format!("window.varosZoom&&window.varosZoom({});", (zoom*100.0).round() as i32)); }
fn refresh_all(panel: &WebView, tools: &WebView, topbar: &WebView, zoom: &WebView, ed: &Editor, zoomf: f32, swatches: &[String]) {
    push_state(panel, ed, swatches); push_ui(tools, ed); push_top(topbar, ed); push_zoom(zoom, zoomf);
}

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
fn panel_data(ed: &Editor) -> ui::PanelData {
    let n = ed.objsel.len();
    let (sel, x, y, w, h) = match ed.obj_bbox() {
        Some((x0, y0, x1, y1)) if n > 0 => (true, x0, y0, x1 - x0, y1 - y0),
        _ => (false, 0.0, 0.0, 0.0, 0.0),
    };
    let first = ed.objsel.iter().copied().filter_map(|p| ed.doc.pidx(p)).next();
    let (fill, opacity) = match first {
        Some(pi) => { let p = &ed.doc.paths[pi];
            (p.fill.map(|c| [(c[0]*255.0) as u8, (c[1]*255.0) as u8, (c[2]*255.0) as u8]), (p.opacity*100.0).round() as i32) },
        None => (None, 100),
    };
    let kind = if n > 1 { "Multiple".to_string() } else { "Rectangle".to_string() };
    ui::PanelData { sel, kind, x, y, w, h, fill, opacity }
}

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

/// Dev-only: write each panel's generated HTML to target/html/ (plus a demo-state injector) so the
/// real CSS/BODY/JS can be eyeballed in a normal browser/preview without the native shell.
fn dump_html() {
    let _ = std::fs::create_dir_all("target/html");
    let demo = r#"<script>setTimeout(function(){
      try{window.varosUI&&window.varosUI({tool:'pen',paint:'fill',fill:'#0c8ce9',stroke:'#141313'});}catch(e){}
      try{window.varosTop&&window.varosTop({n:1,x:240,y:156,rot:0,info:'1 object'});}catch(e){}
      try{window.varosState&&window.varosState({sel:true,n:1,type:'Rectangle',x:240,y:156,w:120,h:78,rot:0,fill:'#0c8ce9',stroke:'#141313',sw:2,opacity:100,paint:'fill',layers:[{pid:3,name:'Rectangle',depth:0,group:false,sel:true,hidden:false,locked:false},{pid:2,name:'Ellipse',depth:0,group:false,sel:false,hidden:false,locked:false},{pid:1,name:'Path',depth:0,group:false,sel:false,hidden:true,locked:false}],swatches:['#0c8ce9','#e6e6e6','#141313','#ff5c5c','#3ecf8e','#f5a623','#9b6cf0','#ffffff']});}catch(e){}
    },50);</script>"#;
    let inject = |html: String| -> String { html.replace("</body>", &format!("{demo}</body>")) };
    let _ = std::fs::write("target/html/topbar.html", inject(page(TOPBAR_CSS, TOPBAR_BODY, TOPBAR_JS)));
    let _ = std::fs::write("target/html/tools.html",  inject(page(TOOLS_CSS, TOOLS_BODY, TOOLS_JS)));
    let _ = std::fs::write("target/html/panel.html",  inject(page(PANEL_CSS, PANEL_BODY, PANEL_JS)));
    let _ = std::fs::write("target/html/zoom.html",   inject(page(ZOOM_CSS, ZOOM_BODY, ZOOM_JS)));
    let pinit = "<script>setTimeout(function(){try{window.varosPick&&window.varosPick('fill','#0c8ce9')}catch(e){}},60);</script>";
    let _ = std::fs::write("target/html/picker.html", page(PICKER_CSS, PICKER_BODY, PICKER_JS).replace("</body>", &format!("{pinit}</body>")));
}

fn main() {
    if std::env::args().any(|a| a == "--dump-html") { dump_html(); return; }
    if std::env::args().any(|a| a == "--dump-cursors") { dump_cursors(); return; }
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(i) = args.iter().position(|a| a == "--preview") {
            if let Some(dir) = args.get(i + 1) { preview_svgs(dir); }
            return;
        }
    }
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let window = Arc::new(WindowBuilder::new().with_title(full_title(ToolKind::Pen))
        .with_window_icon(load_icon())
        .with_inner_size(winit::dpi::LogicalSize::new(1460.0, 860.0)).build(&event_loop).unwrap());
    let size = window.inner_size();
    let mut renderer = pollster::block_on(Renderer::new(window.clone(), size.width, size.height));

    let scale = window.scale_factor();
    let lsz = window.inner_size().to_logical::<f64>(scale);
    // SOLID DOCKED layout (§0.1): opaque webviews fill their docked regions exactly — no transparency
    // (so no black blocks), no floating. The native wgpu canvas occupies the centre gap.
    let ch = |lh: f64| (lh - TOP_H).max(1.0);
    let topbar = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new(0.0, 0.0).into(), size: WSize::new(lsz.width, TOP_H).into() })
        .with_background_color((20,19,19,255)).with_html(page(TOPBAR_CSS, TOPBAR_BODY, TOPBAR_JS))
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window).unwrap();
    let tools_panel = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new(0.0, TOP_H).into(), size: WSize::new(LEFT_W, ch(lsz.height)).into() })
        .with_background_color((20,19,19,255)).with_html(page(TOOLS_CSS, TOOLS_BODY, TOOLS_JS))
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window).unwrap();
    let panel = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new((lsz.width-DOCK_W).max(0.0), TOP_H).into(), size: WSize::new(DOCK_W, ch(lsz.height)).into() })
        .with_background_color((20,19,19,255)).with_html(page(PANEL_CSS, PANEL_BODY, PANEL_JS))
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window).unwrap();
    let zoom_panel = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new((lsz.width-DOCK_W-ZOOM_W-PAD).max(0.0), (lsz.height-ZOOM_H-PAD).max(0.0)).into(), size: WSize::new(ZOOM_W, ZOOM_H).into() })
        .with_background_color((20,19,19,255)).with_html(page(ZOOM_CSS, ZOOM_BODY, ZOOM_JS))
        .with_ipc_handler({ let proxy = proxy.clone(); move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); } })
        .build_as_child(&*window).unwrap();
    // colour picker — own borderless window over the canvas, parked off-screen until pick:open
    let picker = WebViewBuilder::new()
        .with_bounds(WryRect { position: WPos::new(-9999.0, -9999.0).into(), size: WSize::new(PICKER_W, PICKER_H).into() })
        .with_background_color((20,19,19,255)).with_html(page(PICKER_CSS, PICKER_BODY, PICKER_JS))
        .with_ipc_handler(move |req| { let _ = proxy.send_event(UserEvent::Ipc(req.body().clone())); })
        .build_as_child(&*window).unwrap();
    let mut picker_open = false;

    let mut gui = ui::Ui::new(&window); // native egui UI (spike) — paints on our surface via render_ui
    let mut ed = Editor::new();
    // a small default palette so Swatches isn't empty on first run (app-prefs tier; not persisted yet)
    let mut swatches: Vec<String> = ["#0c8ce9","#e6e6e6","#141313","#ff5c5c","#3ecf8e","#f5a623","#9b6cf0","#ffffff"]
        .iter().map(|s| s.to_string()).collect();
    let mut tools_visible = true;
    let mut ref_ax = 0.0f32; let mut ref_ay = 0.0f32; // Transform 9-point reference (top-left default)

    let hwnd: isize = {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(w)) => w.hwnd.get(),
            _ => 0,
        }
    };
    let installed = cursors::install(hwnd);
    let hcur: HashMap<CK, isize> = cursors::ALL_CURSORS.iter().map(|&ck| {
        let h = match cursors::ai_svg(ck) {
            Some((stem, hx, hy)) => cursors::hcursor_svg_file(stem, hx, hy).unwrap_or_else(|| cursors::hcursor(ck)),
            None => cursors::hcursor(ck),
        };
        (ck, h)
    }).collect();
    cursors::set(hcur[&CK::Pen]);
    {
        let zeros = cursors::ALL_CURSORS.iter().filter(|c| hcur[c] == 0).count();
        let _ = std::fs::write("target/cursor-debug-startup.txt",
            format!("hwnd={hwnd}\ninstalled={installed}\nhcursors_total={}\nhcursors_zero={zeros}\n", cursors::ALL_CURSORS.len()));
    }
    let mut last_ck: Option<CK> = None;
    let panel_css = cursors::panel_arrow_css();
    let mut last_click: Option<(Instant, Pt)> = None;
    let mut view = View::identity();
    let mut screen_cursor: Pt = [0.0, 0.0];
    let mut panning = false;
    let mut pan_last: Pt = [0.0, 0.0];
    let mut space_down = false;

    // Canvas occupies the centre gap between the docked panels → zoom about that region's centre.
    let zoom_about_canvas = |view: &mut View, ww: f64, wh: f64, scale: f64, f: f32| {
        let cx = (LEFT_W * scale + (ww - DOCK_W * scale)) * 0.5;
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
            let mut handled_redraw_only = false;
            if let Some(err) = msg.strip_prefix("jserr:") {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("target/js-errors.txt") {
                    let _ = writeln!(f, "{err}");
                }
                handled_redraw_only = true;
            } else if let Some(d) = msg.strip_prefix("diag:") {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open("target/js-diag.txt") {
                    let _ = writeln!(f, "{d}");
                }
                handled_redraw_only = true;
            } else if msg.starts_with("dockh:") {
                handled_redraw_only = true; // dock is full-height docked now; height report unused
            } else if msg == "ready" {
                let js = format!("document.documentElement.style.cursor={0:?};document.body.style.cursor={0:?};", panel_css);
                for w in [&topbar, &tools_panel, &panel, &zoom_panel, &picker] { let _ = w.evaluate_script(&js); }
            } else if let Some(rest) = msg.strip_prefix("keydown:") {
                if let Some((m, code)) = rest.split_once(':') {
                    let m: u8 = m.parse().unwrap_or(0);
                    let (kc, ks, ka) = (m & 1 != 0, m & 2 != 0, m & 4 != 0);
                    if code == "Space" { space_down = true; window.request_redraw(); handled_redraw_only = true; }
                    else { ed.mods = Mods { shift: ks, alt: ka, ctrl: kc }; apply_key(&mut ed, &mut view, code, kc, ks, ka); }
                }
            } else if let Some(rest) = msg.strip_prefix("keyup:") {
                if rest.split_once(':').map_or(false, |(_, c)| c == "Space") { space_down = false; panning = false; window.request_redraw(); }
                handled_redraw_only = true;
            } else if let Some(t) = msg.strip_prefix("tool:") {
                if let Some(tk) = tool_from(t) { ed.set_tool(tk); }
            } else if let Some(a) = msg.strip_prefix("align:") {
                match a { "left"=>ed.align(AlignMode::Left), "centerh"=>ed.align(AlignMode::CenterH), "right"=>ed.align(AlignMode::Right),
                          "top"=>ed.align(AlignMode::Top), "middle"=>ed.align(AlignMode::Middle), "bottom"=>ed.align(AlignMode::Bottom), _=>{} }
            } else if let Some(d) = msg.strip_prefix("dist:") {
                match d { "h"=>ed.distribute(DistAxis::Horizontal), "v"=>ed.distribute(DistAxis::Vertical), _=>{} }
            } else if let Some(rest) = msg.strip_prefix("distsp:") {
                if let Some((ax, v)) = rest.split_once(':') { if let Ok(gap) = v.parse::<f32>() {
                    match ax { "h"=>ed.distribute_spacing(DistAxis::Horizontal, gap), "v"=>ed.distribute_spacing(DistAxis::Vertical, gap), _=>{} }
                }}
            } else if let Some(p) = msg.strip_prefix("pf:") {
                match p { "unite"=>ed.pathfinder(BoolOp::Unite), "minus"=>ed.pathfinder(BoolOp::MinusFront),
                          "intersect"=>ed.pathfinder(BoolOp::Intersect), "exclude"=>ed.pathfinder(BoolOp::Exclude), _=>{} }
            } else if let Some(f) = msg.strip_prefix("flip:") {
                match f { "h"=>ed.flip(true), "v"=>ed.flip(false), _=>{} }
            } else if let Some(a) = msg.strip_prefix("arrange:") {
                match a { "front"=>ed.arrange(ZOrder::Front), "forward"=>ed.arrange(ZOrder::Forward),
                          "backward"=>ed.arrange(ZOrder::Backward), "back"=>ed.arrange(ZOrder::Back), _=>{} }
            } else if let Some(c) = msg.strip_prefix("fill:") {
                let col = if c == "none" { None } else { parse_hex(c) }; ed.paint = PaintTarget::Fill; ed.apply_paint(col);
            } else if let Some(c) = msg.strip_prefix("stroke:") {
                let col = if c == "none" { None } else { parse_hex(c) }; ed.paint = PaintTarget::Stroke; ed.apply_paint(col);
            } else if let Some(v) = msg.strip_prefix("sw:") {
                if let Ok(w) = v.parse::<f32>() { set_stroke_width(&mut ed, w); }
            } else if let Some(v) = msg.strip_prefix("opacity:") {
                if let Ok(o) = v.parse::<f32>() { ed.set_opacity(o / 100.0); }
            } else if let Some(r) = msg.strip_prefix("ref:") {
                if let Ok(i) = r.parse::<u8>() { ref_ax = (i % 3) as f32 * 0.5; ref_ay = (i / 3) as f32 * 0.5; }
                handled_redraw_only = true; // no geometry change
            } else if let Some(rest) = msg.strip_prefix("set:") {
                if let Some((field, v)) = rest.split_once(':') { if let Ok(val) = v.parse::<f32>() {
                    match field {
                        "x" => ed.set_obj_bbox(Some(val), None, None, None, ref_ax, ref_ay),
                        "y" => ed.set_obj_bbox(None, Some(val), None, None, ref_ax, ref_ay),
                        "w" => ed.set_obj_bbox(None, None, Some(val), None, ref_ax, ref_ay),
                        "h" => ed.set_obj_bbox(None, None, None, Some(val), ref_ax, ref_ay),
                        "rot" => ed.set_obj_rotation(val),
                        _ => {}
                    }
                }}
            } else if msg == "swapcolors" { ed.swap_colors();
            } else if msg == "defaultpaint" { ed.default_paint();
            } else if msg == "deletesel" { ed.delete_selected();
            } else if msg == "paintnone" { ed.apply_paint(None);
            } else if let Some(t) = msg.strip_prefix("paint:") {
                ed.paint = if t == "stroke" { PaintTarget::Stroke } else { PaintTarget::Fill };
            } else if let Some(t) = msg.strip_prefix("pick:open:") {
                let target = if t == "stroke" { PaintTarget::Stroke } else { PaintTarget::Fill };
                ed.paint = target;
                let hexv = cur_paint_hex(&ed, target).unwrap_or_else(|| "#000000".into());
                let _ = picker.evaluate_script(&format!("window.varosPick&&window.varosPick(\"{}\",{});", paint_id(target), jstr(&hexv)));
                let sw_js: Vec<String> = swatches.iter().map(|s| jstr(s)).collect();
                let _ = picker.evaluate_script(&format!("window.varosSwatches&&window.varosSwatches([{}]);", sw_js.join(",")));
                // float the picker centred over the canvas region (between rail and dock)
                let psz = window.inner_size(); let (lw, lh) = (psz.width as f64/scale, psz.height as f64/scale);
                let px = (LEFT_W + (lw - LEFT_W - DOCK_W - PICKER_W) * 0.5).max(LEFT_W + PAD);
                let py = (TOP_H + (lh - TOP_H - PICKER_H) * 0.5).max(TOP_H + PAD);
                let _ = picker.set_bounds(WryRect { position: WPos::new(px, py).into(), size: WSize::new(PICKER_W, PICKER_H).into() });
                picker_open = true;
                handled_redraw_only = true;
            } else if msg == "pickclose" {
                let _ = picker.set_bounds(WryRect { position: WPos::new(-9999.0, -9999.0).into(), size: WSize::new(PICKER_W, PICKER_H).into() });
                picker_open = false;
                handled_redraw_only = true;
            } else if let Some(h) = msg.strip_prefix("swatchadd:") {
                if parse_hex(h).is_some() && !swatches.iter().any(|s| s.eq_ignore_ascii_case(h)) { swatches.push(h.to_string()); }
                let sw_js: Vec<String> = swatches.iter().map(|s| jstr(s)).collect();
                let _ = picker.evaluate_script(&format!("window.varosSwatches&&window.varosSwatches([{}]);", sw_js.join(",")));
                handled_redraw_only = true;
            } else if let Some(rest) = msg.strip_prefix("swatch:") {
                if let Some(h) = rest.strip_prefix("del:") {
                    swatches.retain(|s| !s.eq_ignore_ascii_case(h));
                    let sw_js: Vec<String> = swatches.iter().map(|s| jstr(s)).collect();
                    let _ = picker.evaluate_script(&format!("window.varosSwatches&&window.varosSwatches([{}]);", sw_js.join(",")));
                    handled_redraw_only = true;
                } else if let Some(h) = rest.strip_prefix("apply:") {
                    if let Some(col) = parse_hex(h) { ed.apply_paint(Some(col)); }
                }
            } else if let Some(rest) = msg.strip_prefix("vis:") {
                if let Some((pid, st)) = rest.split_once(':') { if let Ok(pid) = pid.parse::<u32>() { ed.set_hidden(pid, st == "1"); } }
            } else if let Some(rest) = msg.strip_prefix("lock:") {
                if let Some((pid, st)) = rest.split_once(':') { if let Ok(pid) = pid.parse::<u32>() { ed.set_locked(pid, st == "1"); } }
            } else if let Some(rest) = msg.strip_prefix("rename:") {
                if let Some((pid, name)) = rest.split_once(':') { if let Ok(pid) = pid.parse::<u32>() { ed.rename_path(pid, name.to_string()); } }
            } else if let Some(rest) = msg.strip_prefix("win:") {
                if let Some((id, st)) = rest.split_once(':') {
                    let vis = st == "1";
                    if id == "tools" {
                        tools_visible = vis;
                        let psz = window.inner_size(); let lh = psz.height as f64 / scale;
                        if vis { let _ = tools_panel.set_bounds(WryRect { position: WPos::new(0.0, TOP_H).into(), size: WSize::new(LEFT_W, (lh-TOP_H).max(1.0)).into() }); }
                        else { let _ = tools_panel.set_bounds(WryRect { position: WPos::new(-9999.0, -9999.0).into(), size: WSize::new(LEFT_W, (lh-TOP_H).max(1.0)).into() }); }
                    } else {
                        let _ = panel.evaluate_script(&format!("window.varosWin&&window.varosWin(\"{}\",{});", id, vis));
                    }
                }
                handled_redraw_only = true;
            } else if let Some(z) = msg.strip_prefix("zoom:") {
                let psz = window.inner_size();
                let (ww, wh) = (psz.width as f64, psz.height as f64);
                match z { "in"=>zoom_about_canvas(&mut view, ww, wh, scale, 1.25),
                          "out"=>zoom_about_canvas(&mut view, ww, wh, scale, 0.8),
                          _=>{ view = View::identity(); } }
                handled_redraw_only = true;
            } else if let Ok(id) = msg.parse::<u32>() {
                if ed.doc.pidx(id).is_some() {
                    ed.set_tool(ToolKind::Object);
                    ed.objsel = ed.doc.group_members(id).into_iter().collect();
                    ed.selected.clear(); ed.obj_angle = 0.0;
                }
            }
            window.set_title(&full_title(ed.tool));
            if !handled_redraw_only { refresh_all(&panel, &tools_panel, &topbar, &zoom_panel, &ed, view.zoom, &swatches); }
            else { push_zoom(&zoom_panel, view.zoom); }
            window.request_redraw();
        }
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
                    let (lw, lh) = (size.width as f64 / scale, size.height as f64 / scale);
                    let ch = (lh - TOP_H).max(1.0);
                    let _ = topbar.set_bounds(WryRect { position: WPos::new(0.0, 0.0).into(), size: WSize::new(lw, TOP_H).into() });
                    if tools_visible { let _ = tools_panel.set_bounds(WryRect { position: WPos::new(0.0, TOP_H).into(), size: WSize::new(LEFT_W, ch).into() }); }
                    let _ = panel.set_bounds(WryRect { position: WPos::new((lw-DOCK_W).max(0.0), TOP_H).into(), size: WSize::new(DOCK_W, ch).into() });
                    let _ = zoom_panel.set_bounds(WryRect { position: WPos::new((lw-DOCK_W-ZOOM_W-PAD).max(0.0), (lh-ZOOM_H-PAD).max(0.0)).into(), size: WSize::new(ZOOM_W, ZOOM_H).into() });
                    if picker_open { let px = ((lw-DOCK_W-PICKER_W)*0.5).max(LEFT_W+PAD); let py = (TOP_H+(ch-PICKER_H)*0.5).max(TOP_H+PAD);
                        let _ = picker.set_bounds(WryRect { position: WPos::new(px, py).into(), size: WSize::new(PICKER_W, PICKER_H).into() }); }
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
                    refresh_all(&panel, &tools_panel, &topbar, &zoom_panel, &ed, view.zoom, &swatches);
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
                    if egui_consumed { /* typing into an egui field — don't trigger canvas shortcuts */ }
                    else if let PhysicalKey::Code(code) = event.physical_key {
                        if code == KeyCode::Space {
                            space_down = event.state == ElementState::Pressed;
                            if !space_down { panning = false; }
                            window.request_redraw();
                        } else if event.state == ElementState::Pressed {
                            let (mc, ms, ma) = (ed.mods.ctrl, ed.mods.shift, ed.mods.alt);
                            apply_key(&mut ed, &mut view, &format!("{:?}", code), mc, ms, ma);
                            window.set_title(&full_title(ed.tool));
                            refresh_all(&panel, &tools_panel, &topbar, &zoom_panel, &ed, view.zoom, &swatches);
                            window.request_redraw();
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    ed.ppu = view.zoom;
                    let ck = if panning { CK::Grab }
                        else if space_down { CK::Hand }
                        else { desired_ck(&ed, view.s2w(screen_cursor)) };
                    if Some(ck) != last_ck {
                        cursors::set(hcur[&ck]); last_ck = Some(ck);
                        let (hw, ins, hits, cur) = cursors::dbg();
                        let _ = std::fs::write("target/cursor-debug.txt",
                            format!("hwnd={hw}\ninstalled={ins}\nsetcursor_hits={hits}\ncurrent_hcursor={cur}\n"));
                    }
                    let world = build_scene(&ed, view.zoom);
                    let data = panel_data(&ed);
                    let (jobs, tdelta, screen) = gui.run(&window, &data, scale as f32);
                    let panels: Vec<[f32; 4]> = gui.rects.iter().map(|r| [r.min.x, r.min.y, r.width(), r.height()]).collect();
                    renderer.render_ui(&world, view, &jobs, &tdelta, &screen, &panels, gui.frosted);
                    if gui.repaint { window.request_redraw(); }
                }
                _ => {}
            }
        }
        _ => {}
        }
    }).unwrap();
}
