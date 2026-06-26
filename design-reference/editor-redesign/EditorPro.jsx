// EditorPro.jsx — BStudio interactive editor (Pro skin). Exports EditorPro.
// Real selectable/draggable/resizable shapes, live inspector, working kashida,
// tools, layers, menus, zoom. Built on the .bs-focus .skin-pro chrome.
const { useState: useS, useRef: useR, useEffect: useE, useLayoutEffect: useLE, useCallback: useCB } = React;

const TOOLS = [
  { n: 'cursor', k: 'V', label: 'Move' },
  { n: 'frame', k: 'F', label: 'Frame' },
  { d: 1 },
  { n: 'rect', k: 'R', label: 'Rectangle' },
  { n: 'ellipse', k: 'O', label: 'Ellipse' },
  { n: 'pen', k: 'P', label: 'Pen', soon: 1 },
  { d: 1 },
  { n: 'text', k: 'T', label: 'Type' },
  { n: 'hand', k: 'H', label: 'Hand' },
];
const SWATCHES = ['#2f8fe0', '#1c1714', '#f7f3ea', '#c9a961', '#d9534f', '#4f9d69', '#6b5bd6', '#9a8f7a'];
const HEX = (c) => (c || '').replace('#', '').toUpperCase();

function EditorPro({ tweaks }) {
  const t = tweaks || {};
  const [shapes, setShapes] = useS(() => INITIAL_SHAPES.map(s => ({ ...s })));
  const [sel, setSel] = useS('accent');
  const [tool, setTool] = useS('cursor');
  const [hover, setHover] = useS(null);
  const [view, setView] = useS({ z: 0.55, x: 360, y: 70 });
  const [draft, setDraft] = useS(null);       // shape being created
  const [menu, setMenu] = useS(null);          // open menu name
  const [insTab, setInsTab] = useS('design');
  const [fillPop, setFillPop] = useS(false);
  const [toast, setToast] = useS(null);
  const canvasRef = useR(null);
  const drag = useR(null);                      // {mode, ...}
  const toastTimer = useR(null);

  const accent = t.accent || '#2f8fe0';
  const selShape = shapes.find(s => s.id === sel) || null;

  const flash = useCB((msg) => {
    setToast(msg); clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(null), 1500);
  }, []);

  // ---- initial fit ----
  useLE(() => {
    const el = canvasRef.current; if (!el) return;
    const r = el.getBoundingClientRect();
    const z = Math.min((r.width - 380) / ARTBOARD.w, (r.height - 150) / ARTBOARD.h);
    setView({ z, x: (r.width - ARTBOARD.w * z) / 2, y: (r.height - ARTBOARD.h * z) / 2 });
  }, []);

  // ---- coordinate helpers ----
  const toWorld = (e) => {
    const r = canvasRef.current.getBoundingClientRect();
    return { x: (e.clientX - r.left - view.x) / view.z, y: (e.clientY - r.top - view.y) / view.z };
  };
  const hit = (p) => {
    for (let i = shapes.length - 1; i >= 0; i--) {
      const s = shapes[i];
      if (!s.visible) continue;
      if (p.x >= s.x && p.x <= s.x + s.w && p.y >= s.y && p.y <= s.y + s.h) return s.id;
    }
    return null;
  };

  const patch = useCB((id, p) => setShapes(ss => ss.map(s => s.id === id ? { ...s, ...p } : s)), []);
  const patchSel = (p) => selShape && patch(selShape.id, p);

  // ---- pointer interaction on canvas ----
  const onDown = (e) => {
    if (e.button !== 0) return;
    setMenu(null); setFillPop(false);
    const p = toWorld(e);
    canvasRef.current.setPointerCapture(e.pointerId);

    if (tool === 'rect' || tool === 'ellipse' || tool === 'text') {
      drag.current = { mode: 'create', start: p, type: tool };
      setDraft({ x: p.x, y: p.y, w: 0, h: 0 });
      return;
    }
    if (tool === 'hand') { drag.current = { mode: 'pan', sx: e.clientX, sy: e.clientY, ox: view.x, oy: view.y }; return; }
    // cursor: select + move
    const id = hit(p);
    setSel(id);
    if (id) {
      const s = shapes.find(z => z.id === id);
      drag.current = { mode: 'move', id, start: p, ox: s.x, oy: s.y, moved: false };
    } else {
      drag.current = { mode: 'pan', sx: e.clientX, sy: e.clientY, ox: view.x, oy: view.y };
    }
  };

  const onMove = (e) => {
    setHover(tool === 'cursor' && !drag.current ? hit(toWorld(e)) : null);
    const d = drag.current; if (!d) return;
    if (d.mode === 'pan') { setView(v => ({ ...v, x: d.ox + (e.clientX - d.sx), y: d.oy + (e.clientY - d.sy) })); return; }
    const p = toWorld(e);
    if (d.mode === 'move') {
      patch(d.id, { x: Math.round(d.ox + (p.x - d.start.x)), y: Math.round(d.oy + (p.y - d.start.y)) });
      d.moved = true;
    } else if (d.mode === 'create') {
      setDraft({ x: Math.min(d.start.x, p.x), y: Math.min(d.start.y, p.y), w: Math.abs(p.x - d.start.x), h: Math.abs(p.y - d.start.y) });
    } else if (d.mode === 'resize') {
      let { x, y, w, h } = d.box; const dx = p.x - d.start.x, dy = p.y - d.start.y;
      if (d.c.includes('l')) { x = d.box.x + dx; w = d.box.w - dx; }
      if (d.c.includes('r')) { w = d.box.w + dx; }
      if (d.c.includes('t')) { y = d.box.y + dy; h = d.box.h - dy; }
      if (d.c.includes('b')) { h = d.box.h + dy; }
      if (w < 8 || h < 8) return;
      patch(d.id, { x: Math.round(x), y: Math.round(y), w: Math.round(w), h: Math.round(h) });
    }
  };

  const onUp = (e) => {
    const d = drag.current; drag.current = null;
    try { canvasRef.current.releasePointerCapture(e.pointerId); } catch {}
    if (d && d.mode === 'create' && draft) {
      if (draft.w > 6 && draft.h > 6) {
        const s = newShape(d.type, Math.round(draft.x), Math.round(draft.y), Math.round(draft.w), Math.round(draft.h), accent);
        setShapes(ss => [...ss, s]); setSel(s.id);
        flash(`${s.name} created`);
      }
      setDraft(null); setTool('cursor');
    }
  };

  const startResize = (e, c) => {
    e.stopPropagation();
    canvasRef.current.setPointerCapture(e.pointerId);
    drag.current = { mode: 'resize', id: selShape.id, c, start: toWorld(e), box: { x: selShape.x, y: selShape.y, w: selShape.w, h: selShape.h } };
  };

  // ---- zoom ----
  const zoomTo = (nz, cx, cy) => {
    const r = canvasRef.current.getBoundingClientRect();
    cx = cx ?? r.width / 2; cy = cy ?? r.height / 2;
    setView(v => {
      const z = clamp(nz, 0.1, 4);
      const wx = (cx - v.x) / v.z, wy = (cy - v.y) / v.z;
      return { z, x: cx - wx * z, y: cy - wy * z };
    });
  };
  const onWheel = (e) => {
    e.preventDefault();
    if (e.ctrlKey || e.metaKey || true) {
      const r = canvasRef.current.getBoundingClientRect();
      zoomTo(view.z * (1 - e.deltaY * 0.0015), e.clientX - r.left, e.clientY - r.top);
    }
  };
  const fit = () => {
    const r = canvasRef.current.getBoundingClientRect();
    const z = Math.min((r.width - 380) / ARTBOARD.w, (r.height - 150) / ARTBOARD.h);
    setView({ z, x: (r.width - ARTBOARD.w * z) / 2, y: (r.height - ARTBOARD.h * z) / 2 });
  };

  // ---- object ops ----
  const del = useCB(() => { if (!selShape) return; const n = selShape.name; setShapes(ss => ss.filter(s => s.id !== sel)); setSel(null); flash(`${n} deleted`); }, [sel, selShape]);
  const dup = useCB(() => {
    if (!selShape) return;
    const c = { ...selShape, id: `${selShape.type}-${Date.now()}`, x: selShape.x + 40, y: selShape.y + 40, name: selShape.name + ' copy' };
    setShapes(ss => [...ss, c]); setSel(c.id); flash('Duplicated');
  }, [selShape]);
  const reorder = (dir) => {
    if (!selShape) return;
    setShapes(ss => {
      const i = ss.findIndex(s => s.id === sel); if (i < 0) return ss;
      const j = clamp(i + dir, 0, ss.length - 1); if (i === j) return ss;
      const a = [...ss]; const [m] = a.splice(i, 1); a.splice(j, 0, m); return a;
    });
  };

  // ---- keyboard ----
  useE(() => {
    const onKey = (e) => {
      const tag = (document.activeElement && document.activeElement.tagName) || '';
      if (tag === 'INPUT' || tag === 'TEXTAREA') return;
      const k = e.key.toLowerCase();
      const map = { v: 'cursor', f: 'frame', r: 'rect', o: 'ellipse', t: 'text', h: 'hand' };
      if (map[k] && !e.metaKey && !e.ctrlKey) { setTool(map[k]); return; }
      if ((e.key === 'Backspace' || e.key === 'Delete')) { e.preventDefault(); del(); }
      else if (k === 'd' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); dup(); }
      else if (e.key === 'Escape') setSel(null);
      else if (e.key === '=' || e.key === '+') zoomTo(view.z * 1.2);
      else if (e.key === '-') zoomTo(view.z / 1.2);
      else if (e.key.startsWith('Arrow') && selShape) {
        e.preventDefault(); const step = e.shiftKey ? 10 : 1;
        const dx = (e.key === 'ArrowRight' ? step : e.key === 'ArrowLeft' ? -step : 0);
        const dy = (e.key === 'ArrowDown' ? step : e.key === 'ArrowUp' ? -step : 0);
        patch(selShape.id, { x: selShape.x + dx, y: selShape.y + dy });
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [view.z, selShape, del, dup, patch]);

  // ---- render a shape ----
  const renderShape = (s) => {
    if (!s.visible) return null;
    const base = { position: 'absolute', left: s.x, top: s.y, width: s.w, height: s.h };
    if (s.type === 'text') {
      const txt = s.kashida ? kashidaText(s.text, s.kashida) : s.text;
      return <div key={s.id} className="pe-shape text" style={base}>
        <div className="txt" style={{ fontFamily: s.font, fontSize: s.size, fontWeight: s.weight, color: s.color,
          textAlign: s.align, direction: s.rtl ? 'rtl' : 'ltr', lineHeight: s.lh, letterSpacing: `${s.ls}em`,
          textTransform: s.font === 'var(--font-ui)' ? 'uppercase' : 'none' }}>{txt}</div>
      </div>;
    }
    return <div key={s.id} className="pe-shape" style={{ ...base, background: s.fill, opacity: s.opacity,
      borderRadius: s.type === 'ellipse' ? '50%' : s.radius }} />;
  };

  // screen-space rect of a shape
  const scr = (s) => ({ left: view.x + s.x * view.z, top: view.y + s.y * view.z, width: s.w * view.z, height: s.h * view.z });

  const MENUS = {
    File: [['New file', '⌘N'], ['Save', '⌘S'], ['—'], ['Export PNG', '⇧⌘E'], ['Export SVG', '']],
    Edit: [['Undo', '⌘Z', 1], ['Redo', '⇧⌘Z', 1], ['—'], ['Duplicate', '⌘D', 0, dup], ['Delete', '⌫', 0, del]],
    Object: [['Bring forward', ']', 0, () => reorder(1)], ['Send backward', '[', 0, () => reorder(-1)], ['—'], ['Group', '⌘G', 1]],
    Type: [['Justify with kashida', '', 0, () => { if (selShape?.type === 'text') { patch(selShape.id, { kashida: 100 }); flash('Kashida justify applied'); } }], ['Reset kashida', '', 0, () => selShape && patch(selShape.id, { kashida: 0 })]],
    View: [['Zoom in', '+', 0, () => zoomTo(view.z * 1.2)], ['Zoom out', '−', 0, () => zoomTo(view.z / 1.2)], ['Zoom to fit', '⇧1', 0, fit]],
  };

  return (
    <div className="bs-editor bs-focus skin-pro" style={{ '--acc': accent }}>
      <div className="pe-root">
        {/* CANVAS */}
        <div ref={canvasRef} className={`pe-canvas tool-${tool}${t.grid === false ? ' nogrid' : ''}`} onPointerDown={onDown} onPointerMove={onMove}
          onPointerUp={onUp} onWheel={onWheel}>
          <div className="pe-world" style={{ transform: `translate(${view.x}px,${view.y}px) scale(${view.z})` }}>
            <div className="pe-artboard" style={{ left: 0, top: 0, width: ARTBOARD.w, height: ARTBOARD.h }}>
              <div className={`pe-art-label${!sel ? ' sel' : ''}`} style={{ fontSize: 11 / view.z, marginBottom: 7 / view.z }}>
                {ARTBOARD.name} · {ARTBOARD.dim}
              </div>
              {shapes.map(renderShape)}
            </div>
          </div>

          {/* OVERLAY (screen space) */}
          {tool === 'cursor' && hover && hover !== sel && shapes.find(s => s.id === hover)?.visible && (
            <div className="pe-hoverline" style={scr(shapes.find(s => s.id === hover))} />
          )}
          {selShape && selShape.visible && !draft && (
            <div className="pe-sel" style={scr(selShape)}>
              {['tl', 'tr', 'bl', 'br'].map(c => (
                <div key={c} className={`pe-h ${c}`} onPointerDown={(e) => startResize(e, c)} />
              ))}
              <div className="pe-dim">{fmt(selShape.w)} × {fmt(selShape.h)}</div>
            </div>
          )}
          {draft && (
            <div className="pe-marquee" style={{ left: view.x + draft.x * view.z, top: view.y + draft.y * view.z, width: draft.w * view.z, height: draft.h * view.z }} />
          )}

          {toast && <div className={`pe-toast show`}>{toast}</div>}
        </div>

        {/* FILE PILL + MENUS */}
        <div className="d2-file bs-glass" style={{ alignItems: 'center' }}>
          <div className="d2-mark">B</div>
          <div className="meta">
            <div className="crumb">Type Specimens /</div>
            <div className="name">calligraphy-poster<span className="ext">.bs</span></div>
          </div>
          <div style={{ display: 'flex', gap: 1, marginLeft: 6, position: 'relative' }}>
            {Object.keys(MENUS).map(m => (
              <div key={m} style={{ position: 'relative' }}>
                <button className="d2-pill" style={{ padding: '5px 8px', background: menu === m ? 'var(--fhover)' : 'none', color: menu === m ? 'var(--ftx)' : undefined }}
                  onClick={() => setMenu(menu === m ? null : m)}>{m}</button>
                {menu === m && (
                  <div className="pe-menu-pop">
                    {MENUS[m].map((it, i) => it[0] === '—'
                      ? <div key={i} className="pe-msep" />
                      : <div key={i} className={`pe-mi${it[2] ? ' disabled' : ''}`} onClick={() => { if (!it[2]) { it[3] && it[3](); setMenu(null); } }}>
                          <span>{it[0]}</span><span className="sc">{it[1]}</span>
                        </div>)}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>

        {/* TOP-RIGHT */}
        <div className="d2-topright">
          <span className="pe-selchip">{selShape ? '1 selected' : `${shapes.length} layers`}</span>
          <div className="d2-avs">
            <div className="d2-av" style={{ background: '#5a6066' }}>A</div>
            <div className="d2-av" style={{ background: '#3d6ea5' }}>N</div>
          </div>
          <button className="d2-share" onClick={() => flash('Share link copied')}><Icon name="share" size={14} /> Share</button>
        </div>

        {/* TOOL DOCK */}
        <div className="d2-dock bs-glass">
          {TOOLS.map((tl, i) => tl.d
            ? <div key={i} className="d2-tdiv" />
            : <button key={tl.n} className={`d2-tool${tool === tl.n ? ' on' : ''}${tl.soon ? ' soon' : ''}`}
                onClick={() => !tl.soon && setTool(tl.n)}>
                <Icon name={tl.n} size={18} />{tl.k && <span className="kbd">{tl.k}</span>}
                <span className="tip">{tl.label}{tl.k && <span className="tk">{tl.k}</span>}</span>
              </button>)}
        </div>

        {/* INSPECTOR */}
        <Inspector s={selShape} shapes={shapes} sel={sel} setSel={setSel} patch={patch} patchSel={patchSel}
          insTab={insTab} setInsTab={setInsTab} fillPop={fillPop} setFillPop={setFillPop} flash={flash}
          density={t.density} showGuides={t.guides} />

        {/* ZOOM */}
        <div className="d2-zoom bs-glass">
          <button onClick={() => zoomTo(view.z / 1.2)}><Icon name="search" size={14} /></button>
          <span><b>{Math.round(view.z * 100)}%</b></span>
          <div className="vd" />
          <button onClick={fit}><Icon name="fitView" size={14} /></button>
        </div>

        {/* hints */}
        {t.hints !== false && <div className="d2-hints">
          <span className="d2-key"><kbd>R</kbd> rect</span>
          <span className="d2-key"><kbd>T</kbd> type</span>
          <span className="d2-key"><kbd>⌫</kbd> delete</span>
        </div>}
      </div>
    </div>
  );
}

window.EditorPro = EditorPro;
