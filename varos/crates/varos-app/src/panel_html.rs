// The right inspector dock — two compact floating cards (Ahmed's layout, 2026-06-26):
//   Card A: Align · Pathfinder   ·   Card B: Properties · Layers · Libraries
// Transform lives INSIDE Properties; Align/Pathfinder are their own card; Arrange sits in the
// Layers footer. Colour + Swatches live in the floating Color Picker, not here. The webview is
// transparent so the canvas shows behind/around the cards. One `varosState(...)` push feeds all.

const PANEL_CSS: &str = r##"
html,body{background:var(--bg-app)}
#dock{height:100%;overflow-y:auto;overflow-x:hidden}
#dockinner{padding:10px;display:flex;flex-direction:column;gap:10px}
.dockbar{position:relative;display:flex;align-items:center;justify-content:flex-end;height:26px}
.winbtn{display:flex;align-items:center;gap:6px;height:24px;padding:0 9px;border-radius:6px;background:var(--bg-surface);border:1px solid var(--border);color:var(--text-2);font-size:11px;cursor:pointer}
.winbtn:hover{background:var(--bg-hover);color:var(--text)}
.winbtn svg{width:13px;height:13px}
.dd{position:absolute;top:28px;right:0;min-width:160px;background:var(--bg-panel);border:1px solid var(--border-2);border-radius:10px;box-shadow:var(--shadow);padding:5px;z-index:50}
.dd[hidden]{display:none}
.dd .it{display:flex;align-items:center;gap:8px;height:27px;padding:0 9px;border-radius:6px;color:var(--text-2);font-size:12px;cursor:pointer}
.dd .it:hover{background:var(--bg-hover);color:var(--text)}
.dd .ck{width:12px;color:var(--accent)}
.dd .hr{height:1px;background:var(--border);margin:4px 4px}
#dock::-webkit-scrollbar{width:8px}#dock::-webkit-scrollbar-thumb{background:#3a3a40;border-radius:6px;border:2px solid transparent;background-clip:padding-box}
.card[hidden]{display:none}
.card.collapsed .body{display:none}
.chev{transition:transform .12s}.card.collapsed .chev{transform:rotate(-90deg)}
.sec{padding:8px 10px}
.sub{font-size:10px;letter-spacing:.3px;color:var(--faint);margin:8px 0 5px;text-transform:uppercase;font-weight:600}
.sub:first-child{margin-top:0}
.ic{height:25px;min-width:0}.ic svg{width:15px;height:15px}
.fld{height:25px}
.row+.row{margin-top:5px}
.ref{width:40px;height:40px;flex:0 0 auto;position:relative;border-radius:6px;background:var(--bg-surface);border:1px solid var(--border)}
.ref .d{position:absolute;width:5px;height:5px;border-radius:50%;background:#56565e;transform:translate(-50%,-50%);cursor:pointer}
.ref .d.on{background:var(--accent);box-shadow:0 0 0 3px rgba(12,140,233,.2)}
.ref .ln{position:absolute;background:#3a3a40}
.rm{width:22px;height:22px;border-radius:5px;display:flex;align-items:center;justify-content:center;color:var(--faint);cursor:pointer;flex:0 0 auto;font-size:12px}
.rm:hover{background:var(--bg-hover);color:var(--text)}
.swh{width:22px;height:22px;border-radius:5px;border:1px solid var(--border-2);cursor:pointer;flex:0 0 auto}
.swh.none{background:repeating-conic-gradient(#3a3b3d 0 25%,#262627 0 50%) 50%/8px 8px}
/* layers */
.lrow{display:flex;align-items:center;gap:6px;height:27px;padding:0 7px;border-radius:5px;color:var(--text-2);cursor:pointer}
.lrow:hover{background:var(--bg-hover)}
.lrow.sel{background:rgba(12,140,233,.16);color:var(--text)}
.lrow .eye,.lrow .lk{width:17px;height:17px;display:flex;align-items:center;justify-content:center;color:var(--faint);flex:0 0 auto}
.lrow .eye:hover,.lrow .lk:hover{color:var(--text)}
.lrow .eye svg,.lrow .lk svg{width:13px;height:13px}
.lrow .dt{width:9px;height:9px;border-radius:2px;background:#4a4a54;flex:0 0 auto}
.lrow.grp .dt{border-radius:50%;background:#c9a23a}
.lrow .nm{flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;font-size:12px}
.lrow .nm input{width:100%;background:var(--bg-active);border:1px solid var(--accent);border-radius:4px;color:var(--text);font:12px var(--ui);padding:1px 4px;outline:none}
.lrow.off{opacity:.45}
.lfoot{display:flex;align-items:center;gap:3px;padding:7px 8px 3px;border-top:1px solid var(--border);margin-top:5px}
.lfoot .seg{flex:0 0 auto}.lfoot .seg .ic{min-width:26px}
.lfoot .icx{width:25px;height:25px}
"##;

const PANEL_BODY: &str = r##"
<div id="dock"><div id="dockinner">
 <div class="dockbar"><div class="winbtn" id="winbtn"></div><div class="dd" id="dd" hidden></div></div>
 <!-- CARD A: Align · Pathfinder -->
 <div class="card" data-card="A">
  <div class="tabs">
   <div class="tab" data-tab="align">Align</div>
   <div class="tab" data-tab="pathfinder">Pathfinder</div>
   <div class="cmenu"><div class="icx chev" data-collapse title="Collapse / expand">&#9662;</div></div>
  </div>
  <div class="body">
   <div class="pane" data-pane="align">
    <div class="sec">
     <div class="seg" id="alobj"></div>
     <div class="row" style="margin-top:6px">
      <div class="seg" style="flex:0 0 auto" id="distobj"></div>
      <div class="seg" style="flex:0 0 auto" id="distsp"></div>
      <div class="fld" data-role="gap" style="flex:1"><span class="lab">&#8596;</span><input value="20"><span class="suf">px</span></div>
     </div>
    </div>
   </div>
   <div class="pane" data-pane="pathfinder" hidden>
    <div class="sec">
     <div class="sub">Shape Modes</div><div class="seg" id="pfmodes"></div>
     <div class="sub">Pathfinders</div><div class="seg" id="pfops"></div>
    </div>
   </div>
  </div>
 </div>

 <!-- CARD B: Properties · Layers · Libraries -->
 <div class="card" data-card="B">
  <div class="tabs">
   <div class="tab" data-tab="properties">Properties</div>
   <div class="tab" data-tab="layers">Layers</div>
   <div class="tab" data-tab="libraries">Libraries</div>
   <div class="cmenu"><div class="icx chev" data-collapse title="Collapse / expand">&#9662;</div></div>
  </div>
  <div class="body">
   <div class="pane" data-pane="properties">
    <div class="sec" style="padding-bottom:4px"><div class="sec-h" style="margin:0"><span id="ptype">No selection</span></div></div>
    <div class="empty" id="pempty" style="padding:14px 10px">Select an object to edit<br>its transform, fill &amp; stroke.</div>
    <div id="pbody" hidden>
     <div class="sec"><div class="sub">Transform</div>
      <div class="row" style="gap:8px">
       <div class="ref" id="ref"></div>
       <div style="flex:1;display:grid;grid-template-columns:1fr 1fr;gap:5px">
        <div class="fld" data-role="x" data-cmd="set:x:" data-int="1"><span class="lab">X</span><input></div>
        <div class="fld" data-role="y" data-cmd="set:y:" data-int="1"><span class="lab">Y</span><input></div>
        <div class="fld" data-role="w" data-cmd="set:w:" data-int="1" data-min="0"><span class="lab">W</span><input></div>
        <div class="fld" data-role="h" data-cmd="set:h:" data-int="1" data-min="0"><span class="lab">H</span><input></div>
       </div>
      </div>
      <div class="row">
       <div class="fld" style="flex:1" data-role="rot" data-cmd="set:rot:"><span class="lab">&#8736;</span><input><span class="suf">&#176;</span></div>
       <div class="seg" style="flex:0 0 auto"><div class="ic" id="fliph" title="Flip Horizontal"></div><div class="ic" id="flipv" title="Flip Vertical"></div></div>
      </div>
     </div>
     <div class="sec"><div class="sub">Appearance</div>
      <div class="row"><div class="swh" id="pfillsw" title="Fill"></div><input class="hex" id="pfillhex" placeholder="None" style="height:25px"><div class="rm" id="pfillx" title="No fill">&#10005;</div></div>
      <div class="row"><div class="swh" id="pstrokesw" title="Stroke"></div><input class="hex" id="pstrokehex" placeholder="None" style="height:25px"><div class="rm" id="pstrokex" title="No stroke">&#10005;</div></div>
      <div class="row">
       <div class="fld" style="flex:1" data-role="sw" data-cmd="sw:" data-min="0"><span class="lab" title="Stroke weight">W</span><input><span class="suf">px</span></div>
       <div class="fld" style="flex:1" data-role="opacity" data-cmd="opacity:" data-int="1" data-min="0" data-max="100"><span class="lab" title="Opacity">O</span><input><span class="suf">%</span></div>
      </div>
     </div>
    </div>
   </div>
   <div class="pane" data-pane="layers" hidden>
    <div class="sec" style="padding-bottom:5px"><div class="row"><input class="hex" id="lsearch" placeholder="Search" style="text-transform:none;height:25px"><div class="icx" id="lfilter" title="Filter"></div></div></div>
    <div id="llist"></div>
    <div class="sec lfoot"><div class="seg" id="parrange"></div><div style="flex:1"></div><div class="icx" id="ldel" title="Delete"></div></div>
   </div>
   <div class="pane" data-pane="libraries" hidden>
    <div class="empty">Libraries are offline-only for now.</div>
   </div>
  </div>
 </div>
</div></div>
"##;

const PANEL_JS: &str = r##"
var IC={
 aL:'<path d="M4 4V20M4 9H17M4 15H11"/>',aC:'<path d="M12 4V20M5 9H19M8 15H16"/>',aR:'<path d="M20 4V20M7 9H20M13 15H20"/>',
 aT:'<path d="M4 4H20M9 4V17M15 4V11"/>',aM:'<path d="M4 12H20M9 5V19M15 8V16"/>',aB:'<path d="M4 20H20M9 7V20M15 13V20"/>',
 dV:'<path d="M5 4H19M5 12H19M5 20H19"/>',dH:'<path d="M4 5V19M12 5V19M20 5V19"/>',
 eye:'<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/>',
 eyeoff:'<path d="m2 2 20 20M6.7 6.7C4 8.4 2 12 2 12s3.5 7 10 7c2 0 3.7-.5 5.2-1.3M9.9 5.2A10 10 0 0 1 12 5c6.5 0 10 7 10 7a18 18 0 0 1-2.4 3.3"/>',
 lock:'<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
 unlock:'<rect x="5" y="11" width="14" height="9" rx="2"/><path d="M8 11V7a4 4 0 0 1 7-2"/>',
 trash:'<path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/>',
 funnel:'<path d="M3 4h18l-7 8v6l-4 2v-8z"/>'
};
function ico(p){return svg(p)}
var ST={paint:'fill'};
function reportH(){} // dock is full-height docked now — no dynamic height reporting

// ---------- module / card visibility ----------
var mv={align:1,pathfinder:1,properties:1,layers:1,libraries:1};
var activeTab={A:'align',B:'properties'};
function layout(){
 document.querySelectorAll('.card[data-card]').forEach(function(card){
  var tabs=card.querySelectorAll('.tab'),vis=[];
  tabs.forEach(function(t){var k=t.dataset.tab,on=!!mv[k];t.hidden=!on;if(on)vis.push(k)});
  var cardId=card.dataset.card;
  if(vis.indexOf(activeTab[cardId])<0)activeTab[cardId]=vis[0]||null;
  tabs.forEach(function(t){t.classList.toggle('on',t.dataset.tab===activeTab[cardId])});
  card.querySelectorAll('.pane').forEach(function(p){p.hidden=p.dataset.pane!==activeTab[cardId]});
  card.hidden=vis.length===0;
 });
 setTimeout(reportH,0);
}
window.varosWin=function(id,vis){if(id in mv){mv[id]=vis?1:0;if(vis){for(var k in activeTab){var c=document.querySelector('.card[data-card="'+k+'"]');if(c&&c.querySelector('.tab[data-tab="'+id+'"]')){activeTab[k]=id}}}layout()}};

// ---------- buttons ----------
function mkseg(host,items){host.innerHTML='';items.forEach(function(it){var b=document.createElement('div');b.className='ic'+(it[3]?' dis':'');b.innerHTML=ico(it[1]);if(it[2])b.title=it[2];if(!it[3])b.onclick=function(){if(!b.classList.contains('dis'))ipc(it[0])};host.appendChild(b)})}
var ALIGN=[['align:left',IC.aL,'Align Left'],['align:centerh',IC.aC,'Align Center'],['align:right',IC.aR,'Align Right'],['align:top',IC.aT,'Align Top'],['align:middle',IC.aM,'Align Middle'],['align:bottom',IC.aB,'Align Bottom']];
var PFM=[['pf:unite','<rect x="4" y="4" width="11" height="11" rx="1" fill="currentColor"/><rect x="9" y="9" width="11" height="11" rx="1" fill="currentColor"/>','Unite'],['pf:minus','<rect x="4" y="4" width="11" height="11" rx="1" fill="currentColor"/><rect x="9" y="9" width="11" height="11" rx="1" fill="var(--bg-surface)" stroke="currentColor"/>','Minus Front'],['pf:intersect','<rect x="4" y="4" width="11" height="11" rx="1"/><rect x="9" y="9" width="11" height="11" rx="1"/><rect x="9" y="9" width="6" height="6" fill="currentColor"/>','Intersect'],['pf:exclude','<rect x="4" y="4" width="11" height="11" rx="1" fill="currentColor"/><rect x="9" y="9" width="11" height="11" rx="1" fill="currentColor"/><rect x="9" y="9" width="6" height="6" fill="var(--bg-surface)"/>','Exclude']];
function boot(){
 var ref=$('ref');
 var hl=document.createElement('div');hl.className='ln';hl.style.cssText='left:5px;right:5px;top:50%;height:1px;transform:translateY(-50%)';ref.appendChild(hl);
 var vl=document.createElement('div');vl.className='ln';vl.style.cssText='top:5px;bottom:5px;left:50%;width:1px;transform:translateX(-50%)';ref.appendChild(vl);
 for(var i=0;i<9;i++){(function(i){var d=document.createElement('div');d.className='d'+(i===0?' on':'');var col=i%3,rowi=(i/3)|0;d.style.left=(7+col*13)+'px';d.style.top=(7+rowi*13)+'px';d.onclick=function(){ref.querySelectorAll('.d').forEach(function(x){x.classList.remove('on')});d.classList.add('on');ipc('ref:'+i)};ref.appendChild(d)})(i)}
 $('fliph').innerHTML=svg('<path d="M12 3v18M7 7l-3 5 3 5M17 7l3 5-3 5"/>');
 $('flipv').innerHTML=svg('<path d="M3 12h18M7 7l5-3 5 3M7 17l5 3 5-3"/>');
 $('fliph').onclick=function(){ipc('flip:h')};$('flipv').onclick=function(){ipc('flip:v')};
 mkseg($('alobj'),ALIGN);
 mkseg($('distobj'),[['dist:v',IC.dV,'Distribute Vertically'],['dist:h',IC.dH,'Distribute Horizontally']]);
 var dsp=$('distsp');dsp.innerHTML='';[['v',IC.dV,'Vertical spacing'],['h',IC.dH,'Horizontal spacing']].forEach(function(it){var b=document.createElement('div');b.className='ic';b.innerHTML=ico(it[1]);b.title=it[2];b.onclick=function(){if(b.classList.contains('dis'))return;var g=parseFloat(document.querySelector('[data-role="gap"] input').value)||0;ipc('distsp:'+it[0]+':'+g)};dsp.appendChild(b)});
 mkseg($('pfmodes'),PFM);
 mkseg($('pfops'),[['',IC.aL,'Divide — soon',1],['',IC.aC,'Trim — soon',1],['',IC.aR,'Merge — soon',1],['',IC.aT,'Crop — soon',1],['',IC.aM,'Outline — soon',1],['',IC.aB,'Minus Back — soon',1]]);
 mkseg($('parrange'),[['arrange:back','<path d="M8 8h12v12H8z" fill="currentColor"/><path d="M4 4h12v12H4z" fill="var(--bg-surface)" stroke="currentColor"/>','Send to Back ( Ctrl+Shift+[ )'],['arrange:backward','<path d="M5 5h14v14H5z"/><path d="M9 9h6v6H9z"/>','Send Backward ( Ctrl+[ )'],['arrange:forward','<path d="M5 5h14v14H5z"/><path d="M9 9h6v6H9z" fill="currentColor"/>','Bring Forward ( Ctrl+] )'],['arrange:front','<path d="M4 4h12v12H4z" fill="currentColor"/><path d="M8 8h12v12H8z" fill="var(--bg-surface)" stroke="currentColor"/>','Bring to Front ( Ctrl+Shift+] )']]);
 $('ldel').innerHTML=ico(IC.trash);$('lfilter').innerHTML=ico(IC.funnel);
 $('ldel').onclick=function(){ipc('deletesel')};
 $('pfillsw').onclick=function(){ipc('pick:open:fill')};
 $('pstrokesw').onclick=function(){ipc('pick:open:stroke')};
 $('pfillx').onclick=function(){ipc('fill:none')};
 $('pstrokex').onclick=function(){ipc('stroke:none')};
 $('pfillhex').addEventListener('keydown',function(e){if(e.key==='Enter'){ipc('fill:'+normHex(this.value));this.blur()}e.stopPropagation()});
 $('pstrokehex').addEventListener('keydown',function(e){if(e.key==='Enter'){ipc('stroke:'+normHex(this.value));this.blur()}e.stopPropagation()});
 document.querySelectorAll('.tab').forEach(function(t){t.onclick=function(){var card=t.closest('.card');card.classList.remove('collapsed');activeTab[card.dataset.card]=t.dataset.tab;layout()}});
 document.querySelectorAll('[data-collapse]').forEach(function(x){x.onclick=function(e){e.stopPropagation();x.closest('.card').classList.toggle('collapsed');setTimeout(reportH,0)}});
 $('lsearch').addEventListener('input',renderLayers);
 $('lsearch').addEventListener('keydown',function(e){e.stopPropagation()});
 buildWin();
 proAll();
 layout();
}
// Window menu (lives in the dock so the dropdown stays within panel bounds — §0.1)
var WMENU=[['align','Align'],['pathfinder','Pathfinder'],['properties','Properties'],['layers','Layers'],['libraries','Libraries'],['__sep',''],['tools','Tools']];
var toolsVis=1;
function buildWin(){
 $('winbtn').innerHTML=svg('<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M3 9h6"/>')+'<span>Panels</span>';
 var dd=$('dd');dd.innerHTML='';
 WMENU.forEach(function(w){
  if(w[0]==='__sep'){var h=document.createElement('div');h.className='hr';dd.appendChild(h);return}
  var it=document.createElement('div');it.className='it';
  var on=function(){return w[0]==='tools'?toolsVis:mv[w[0]]};
  it.innerHTML='<span class="ck">'+(on()?'✓':'')+'</span>'+w[1];
  it.onclick=function(e){e.stopPropagation();if(w[0]==='tools'){toolsVis=toolsVis?0:1;ipc('win:tools:'+(toolsVis?1:0))}else{mv[w[0]]=mv[w[0]]?0:1;layout()}it.querySelector('.ck').textContent=on()?'✓':''};
  dd.appendChild(it);
 });
 $('winbtn').onclick=function(e){e.stopPropagation();dd.hidden=!dd.hidden};
 addEventListener('click',function(){dd.hidden=true});
}

function setSwatchEl(el,hexv){if(hexv){el.style.background=hexv;el.classList.remove('none')}else{el.style.background='';el.classList.add('none')}}
window.varosState=function(s){
 ST=s;
 setRole('x',s.x);setRole('y',s.y);setRole('w',s.w);setRole('h',s.h);setRole('rot',(Math.round(s.rot*10)/10));
 setRole('sw',s.sw);setRole('opacity',s.opacity);
 $('ptype').textContent=s.sel?(s.type||'Path'):'No selection';
 $('pbody').hidden=!s.sel;$('pempty').hidden=!!s.sel;
 setSwatchEl($('pfillsw'),s.fill);$('pfillhex').value=s.fill?s.fill.toUpperCase():'';
 setSwatchEl($('pstrokesw'),s.stroke);$('pstrokehex').value=s.stroke?s.stroke.toUpperCase():'';
 document.querySelectorAll('#alobj .ic, #pfmodes .ic').forEach(function(b){b.classList.toggle('dis',s.n<2)});
 document.querySelectorAll('#distobj .ic, #distsp .ic').forEach(function(b){b.classList.toggle('dis',s.n<2)});
 document.querySelectorAll('#parrange .ic').forEach(function(b){b.classList.toggle('dis',s.n<1)});
 renderLayers();
};
function renderLayers(){
 var s=ST,list=$('llist');if(!list)return;var q=($('lsearch').value||'').toLowerCase();
 var rows=(s.layers||[]).filter(function(r){return !q||(r.name||'').toLowerCase().indexOf(q)>=0});
 if(!rows.length){list.innerHTML='<div class="empty">'+((s.layers&&s.layers.length)?'No matches':'No objects yet')+'</div>';setTimeout(reportH,0);return}
 list.innerHTML='';
 rows.forEach(function(r){
  var d=document.createElement('div');d.className='lrow'+(r.sel?' sel':'')+(r.group?' grp':'')+(r.hidden?' off':'');
  d.style.paddingLeft=(7+r.depth*13)+'px';
  var eye=document.createElement('div');eye.className='eye';eye.innerHTML=ico(r.hidden?IC.eyeoff:IC.eye);
  if(!r.group)eye.onclick=function(e){e.stopPropagation();ipc('vis:'+r.pid+':'+(r.hidden?0:1))};else eye.style.visibility='hidden';
  var dt=document.createElement('div');dt.className='dt';
  var nm=document.createElement('div');nm.className='nm';nm.textContent=r.name;
  if(!r.group)nm.ondblclick=function(e){e.stopPropagation();startRename(nm,r)};
  var lk=document.createElement('div');lk.className='lk';lk.innerHTML=ico(r.locked?IC.lock:IC.unlock);if(r.locked)lk.style.color='var(--accent)';
  if(!r.group)lk.onclick=function(e){e.stopPropagation();ipc('lock:'+r.pid+':'+(r.locked?0:1))};else lk.style.visibility='hidden';
  d.appendChild(eye);d.appendChild(dt);d.appendChild(nm);d.appendChild(lk);
  d.onclick=function(){ipc(String(r.pid))};
  list.appendChild(d);
 });
 setTimeout(reportH,0);
}
function startRename(nm,r){
 var cur=nm.textContent;nm.innerHTML='';var inp=document.createElement('input');inp.value=cur;nm.appendChild(inp);inp.focus();inp.select();
 function done(commit){if(commit&&inp.value.trim()!==cur)ipc('rename:'+r.pid+':'+inp.value.trim());renderLayers()}
 inp.addEventListener('keydown',function(e){e.stopPropagation();if(e.key==='Enter')done(true);else if(e.key==='Escape')done(false)});
 inp.addEventListener('blur',function(){done(true)});
}
"##;
