// Inspector.jsx — live, two-way bound inspector + layers for EditorPro.
// Exports: Inspector.
const { useState: useIS, useRef: useIR } = React;

// number field bound to a shape prop
function NumF({ k, value, onChange, unit }) {
  return (
    <div className="d2-fld pe-fld">
      <span className="k">{k}</span>
      <input type="number" value={Math.round(value)} onChange={(e) => onChange(e.target.value === '' ? 0 : parseFloat(e.target.value))} />
      {unit && <span className="u">{unit}</span>}
    </div>
  );
}

// draggable slider 0..max
function Slider({ value, max = 100, onChange }) {
  const ref = useIR(null);
  const set = (clientX) => {
    const r = ref.current.getBoundingClientRect();
    onChange(Math.round(clamp((clientX - r.left) / r.width, 0, 1) * max));
  };
  const down = (e) => {
    ref.current.setPointerCapture(e.pointerId); set(e.clientX);
    const move = (ev) => set(ev.clientX);
    const up = (ev) => { try { ref.current.releasePointerCapture(ev.pointerId); } catch {} window.removeEventListener('pointermove', move); window.removeEventListener('pointerup', up); };
    window.addEventListener('pointermove', move); window.addEventListener('pointerup', up);
  };
  const pct = (value / max) * 100;
  return (
    <div ref={ref} className="pe-slider" onPointerDown={down}>
      <div className="fill" style={{ width: `${pct}%` }} />
      <div className="knob" style={{ left: `${pct}%` }} />
    </div>
  );
}

function Inspector({ s, shapes, sel, setSel, patch, patchSel, insTab, setInsTab, fillPop, setFillPop, flash }) {
  return (
    <div className="d2-inspect bs-glass" style={{ bottom: 16, top: 60, display: 'flex', flexDirection: 'column' }}>
      <div className="pe-tabs">
        <div className={`pe-tab${insTab === 'design' ? ' on' : ''}`} onClick={() => setInsTab('design')}>Design</div>
        <div className={`pe-tab${insTab === 'layers' ? ' on' : ''}`} onClick={() => setInsTab('layers')}>Layers</div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', background: 'var(--finput)' }}>
        {insTab === 'design' ? (
          !s ? (
            <div className="pe-empty">
              <div className="ic"><Icon name="cursor" size={20} /></div>
              Nothing selected.<br />Pick a layer, or draw with <b style={{ color: 'var(--ftx-mut)' }}>R</b> / <b style={{ color: 'var(--ftx-mut)' }}>O</b> / <b style={{ color: 'var(--ftx-mut)' }}>T</b>.
            </div>
          ) : (
            <div style={{ padding: '13px' }}>
              <div className="d2-ins-head" style={{ padding: '0 0 12px' }}>
                <span className="t">{s.name}</span>
                <span className="m">{s.type}</span>
              </div>

              {/* Frame */}
              <div className="d2-grp">
                <div className="d2-grp-h">Frame</div>
                <div className="d2-row" style={{ marginBottom: 6 }}>
                  <NumF k="X" value={s.x} onChange={(v) => patchSel({ x: v })} />
                  <NumF k="Y" value={s.y} onChange={(v) => patchSel({ y: v })} />
                </div>
                <div className="d2-row">
                  <NumF k="W" value={s.w} onChange={(v) => patchSel({ w: Math.max(4, v) })} />
                  <NumF k="H" value={s.h} onChange={(v) => patchSel({ h: Math.max(4, v) })} />
                </div>
              </div>

              {s.type !== 'text' ? (
                <React.Fragment>
                  <div className="d2-grp">
                    <div className="d2-grp-h">Fill</div>
                    <div className="d2-swrow" style={{ position: 'relative', cursor: 'pointer' }} onClick={() => setFillPop(f => !f)}>
                      <span className="sw" style={{ background: s.fill }} />
                      <span className="hex">{HEX(s.fill)}</span>
                      <span className="op">{Math.round(s.opacity * 100)}%</span>
                      {fillPop && (
                        <div className="pe-menu-pop" style={{ top: 'calc(100% + 7px)', left: 0, right: 0, minWidth: 0, display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 7, padding: 9 }} onClick={(e) => e.stopPropagation()}>
                          {SWATCHES.map(c => (
                            <span key={c} onClick={() => { patchSel({ fill: c }); setFillPop(false); }}
                              style={{ height: 26, borderRadius: 5, background: c, cursor: 'pointer', outline: s.fill === c ? '2px solid var(--acc)' : '1px solid var(--fbd)', outlineOffset: s.fill === c ? 1 : 0 }} />
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                  {s.type === 'rect' && (
                    <div className="d2-grp">
                      <div className="d2-grp-h">Corner radius</div>
                      <div className="d2-row one" style={{ gridTemplateColumns: '1fr' }}>
                        <NumF k="R" value={s.radius} unit="px" onChange={(v) => patchSel({ radius: Math.max(0, v) })} />
                      </div>
                    </div>
                  )}
                  <div className="d2-grp">
                    <div className="d2-grp-h" style={{ display: 'flex', justifyContent: 'space-between' }}>Opacity <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--ftx-mut)' }}>{Math.round(s.opacity * 100)}%</span></div>
                    <Slider value={s.opacity * 100} onChange={(v) => patchSel({ opacity: v / 100 })} />
                  </div>
                </React.Fragment>
              ) : (
                <React.Fragment>
                  <div className="d2-grp">
                    <div className="d2-grp-h">Type</div>
                    <div className="d2-swrow" style={{ marginBottom: 6 }}>
                      <Icon name="text" size={14} style={{ color: 'var(--ftx-dim)' }} />
                      <span className="hex" style={{ fontFamily: 'var(--font-ui)', fontSize: 12 }}>{s.font.includes('serif') ? 'Amiri' : s.font.includes('ar') ? 'Tajawal' : 'Inter'}</span>
                      <span className="op">{s.weight}</span>
                    </div>
                    <div className="d2-row">
                      <NumF k="Size" value={s.size} onChange={(v) => patchSel({ size: Math.max(8, v) })} />
                      <NumF k="LH" value={s.lh * 100} onChange={(v) => patchSel({ lh: v / 100 })} />
                    </div>
                  </div>
                  <div className="d2-grp">
                    <div className="d2-grp-h">Align</div>
                    <div className="d2-seg" style={{ display: 'flex', background: 'var(--felev)', borderRadius: 6, overflow: 'hidden' }}>
                      {['right', 'center', 'left'].map(a => (
                        <button key={a} onClick={() => patchSel({ align: a })}
                          style={{ flex: 1, padding: '7px 0', display: 'grid', placeItems: 'center',
                            background: s.align === a ? 'var(--acc)' : 'transparent', color: s.align === a ? '#fff' : 'var(--ftx-mut)' }}>
                          <Icon name={a === 'right' ? 'alR' : a === 'center' ? 'alCX' : 'alL'} size={15} />
                        </button>
                      ))}
                    </div>
                  </div>
                  <div className="d2-grp">
                    <div className="d2-grp-h">Color</div>
                    <div className="d2-swrow" style={{ position: 'relative', cursor: 'pointer' }} onClick={() => setFillPop(f => !f)}>
                      <span className="sw" style={{ background: s.color }} />
                      <span className="hex">{HEX(s.color)}</span>
                      {fillPop && (
                        <div className="pe-menu-pop" style={{ top: 'calc(100% + 7px)', left: 0, right: 0, minWidth: 0, display: 'grid', gridTemplateColumns: 'repeat(4,1fr)', gap: 7, padding: 9 }} onClick={(e) => e.stopPropagation()}>
                          {SWATCHES.map(c => (
                            <span key={c} onClick={() => { patchSel({ color: c }); setFillPop(false); }}
                              style={{ height: 26, borderRadius: 5, background: c, cursor: 'pointer', outline: s.color === c ? '2px solid var(--acc)' : '1px solid var(--fbd)' }} />
                          ))}
                        </div>
                      )}
                    </div>
                  </div>
                  {/* the moat — real kashida control */}
                  <div className="d2-grp">
                    <div className="d2-grp-h">Arabic — the moat</div>
                    <div className="d2-moat">
                      <div className="h" style={{ justifyContent: 'space-between' }}>
                        <span style={{ display: 'flex', alignItems: 'center', gap: 7 }}><span className="ico"><Icon name="kashida" size={15} /></span> Kashida elongation</span>
                        <span style={{ fontFamily: 'var(--font-mono)', color: 'var(--ftx-mut)', fontWeight: 400 }}>{s.kashida || 0}%</span>
                      </div>
                      <Slider value={s.kashida || 0} onChange={(v) => patchSel({ kashida: v })} />
                      <div className="cap"><span>Naskh</span><span>auto-justify</span></div>
                    </div>
                  </div>
                </React.Fragment>
              )}
            </div>
          )
        ) : (
          // LAYERS tab
          <div style={{ padding: '6px' }}>
            {[...shapes].reverse().map(sh => (
              <div key={sh.id} className={`d2-lp-row${sh.id === sel ? ' sel' : ''}`} style={{ opacity: sh.visible ? 1 : 0.45 }}
                onClick={() => setSel(sh.id)}>
                <span className="gi"><Icon name={sh.type === 'text' ? 'text' : sh.type === 'ellipse' ? 'ellipse' : sh.name === 'Divider' ? 'line' : 'rect'} size={14} /></span>
                <span className="nm">{sh.name}</span>
                <span className="gi" onClick={(e) => { e.stopPropagation(); patch(sh.id, { visible: !sh.visible }); }}>
                  <Icon name="eye" size={13} style={{ opacity: sh.visible ? 1 : 0.4 }} />
                </span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

window.Inspector = Inspector;
