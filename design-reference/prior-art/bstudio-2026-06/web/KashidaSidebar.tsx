// KashidaSidebar — the Phase 0 wasm engine plugged into tldraw.
//
// When the user selects a text shape with Arabic content, this sidebar
// surfaces a kashida (تطويل) slider. Moving the slider re-shapes the
// selected text via the Rust wasm engine (harfrust + Hallberg + skrifa)
// and writes the modified string back into the shape's props.text.
//
// This is the "moat" — kashida at HarfBuzz-safe positions, lām-alif
// never splits, mathematically grounded.

import { useEffect, useRef, useState } from 'react'
import { track, useEditor, type Editor, type TLShape, type TLShapeId } from 'tldraw'

// ────────────── WASM LOADING ──────────────
type WasmApi = {
  insert_kashida: (text: string, n: number) => {
    canonical_text: string
    inserted_count: number
    dropped_unsafe: number
    glyph_count: number
  }
  set_active_font: (familyIndex: number) => void
}

let wasmLoad: Promise<WasmApi> | null = null

function loadWasm(): Promise<WasmApi> {
  if (wasmLoad) return wasmLoad
  wasmLoad = (async () => {
    // The build is served from /editor/, so wasm sits at /editor/wasm/pivot_web.js
    // Use a runtime URL so Vite doesn't try to bundle the wasm sidecar.
    // @ts-ignore — dynamic-imported runtime module, no static types
    const mod = await import(/* @vite-ignore */ '/editor/wasm/pivot_web.js')
    await mod.default()
    return {
      insert_kashida: (text: string, n: number) => {
        // The wasm export is shape_with_kashida — returns a struct with
        // canonical_text, inserted_count, etc.
        const r = mod.shape_with_kashida(text, n)
        return {
          canonical_text: r.canonical_text,
          inserted_count: r.inserted_count,
          dropped_unsafe: r.dropped_unsafe,
          glyph_count: r.glyph_count,
        }
      },
      set_active_font: (familyIndex: number) => mod.set_active_font(familyIndex),
    }
  })()
  return wasmLoad
}

// ────────────── ARABIC DETECTION ──────────────
const ARABIC_RE = /[؀-ۿݐ-ݿࢠ-ࣿﭐ-﷿ﹰ-﻿]/

function containsArabic(s: string): boolean {
  return ARABIC_RE.test(s || '')
}

// Strip U+0640 (tatweel) from a string — used to recover the original text
// when we want to re-apply with a different kashida count.
function stripKashida(s: string): string {
  return (s || '').replace(/ـ/g, '')
}

// ────────────── SHAPE TYPING ──────────────
// tldraw's text shape has `props.text` (label-like) for type='text'.
// Note shapes have `props.richText` (TipTap doc). We focus on `text` shapes.
function getTextContent(shape: TLShape): string | null {
  if (shape.type !== 'text') return null
  const t = (shape as any).props?.text
  return typeof t === 'string' ? t : null
}

function setTextContent(editor: Editor, shape: TLShape, text: string) {
  editor.updateShape({
    id: shape.id,
    type: shape.type,
    props: { ...(shape as any).props, text },
  } as any)
}

// ────────────── COMPONENT ──────────────
const FONT_OPTIONS = [
  { idx: 0, name: 'Amiri' },
  { idx: 1, name: 'Cairo' },
  { idx: 2, name: 'IBM Plex Arabic' },
  { idx: 3, name: 'Tajawal' },
  { idx: 4, name: 'Noto Sans Arabic' },
]

export const KashidaSidebar = track(() => {
  const editor = useEditor()
  const [api, setApi] = useState<WasmApi | null>(null)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [kashidaByShape, setKashidaByShape] = useState<Record<string, number>>({})
  const [fontByShape, setFontByShape] = useState<Record<string, number>>({})
  // Cache the "original" (kashida-stripped) text per shape so the slider
  // always reapplies fresh against the canonical content.
  const originalByShape = useRef<Map<TLShapeId, string>>(new Map())

  // Boot wasm once
  useEffect(() => {
    loadWasm().then(setApi).catch(e => setLoadError(String(e)))
  }, [])

  // What's selected? Just look at the first Arabic-text shape in the selection.
  const selectedShapes = editor.getSelectedShapes()
  const arabicShape = selectedShapes.find(s => {
    const t = getTextContent(s)
    return t != null && containsArabic(t)
  })

  // Capture the original text the first time we see a shape, then keep using it
  // as the baseline for any future kashida count.
  if (arabicShape && !originalByShape.current.has(arabicShape.id)) {
    const cur = getTextContent(arabicShape) ?? ''
    originalByShape.current.set(arabicShape.id, stripKashida(cur))
  }

  const currentKashida = arabicShape ? (kashidaByShape[arabicShape.id] ?? 0) : 0
  const currentFont = arabicShape ? (fontByShape[arabicShape.id] ?? 0) : 0
  const original = arabicShape ? (originalByShape.current.get(arabicShape.id) ?? getTextContent(arabicShape) ?? '') : ''

  function applyKashida(n: number) {
    if (!api || !arabicShape) return
    api.set_active_font(currentFont)
    try {
      const r = api.insert_kashida(original, n)
      setTextContent(editor, arabicShape, r.canonical_text)
      setKashidaByShape(prev => ({ ...prev, [arabicShape.id]: n }))
    } catch (e) {
      console.error('insert_kashida failed:', e)
    }
  }

  function changeFont(idx: number) {
    if (!arabicShape) return
    setFontByShape(prev => ({ ...prev, [arabicShape.id]: idx }))
    if (api) {
      api.set_active_font(idx)
      // Re-apply with current kashida count so the new font's safe-positions
      // get used.
      const r = api.insert_kashida(original, currentKashida)
      setTextContent(editor, arabicShape, r.canonical_text)
    }
  }

  function resetText() {
    if (!arabicShape) return
    setTextContent(editor, arabicShape, original)
    setKashidaByShape(prev => ({ ...prev, [arabicShape.id]: 0 }))
  }

  const ready = api != null && arabicShape != null

  return (
    <aside style={style.sidebar}>
      <header style={style.header}>
        <div style={style.brand}>Pivot · Arabic</div>
        <div style={style.subtitle}>kashida engine</div>
      </header>

      {loadError && (
        <div style={style.err}>wasm load failed: {loadError}</div>
      )}

      {!api && !loadError && (
        <div style={style.note}>Loading kashida engine…</div>
      )}

      {api && !arabicShape && (
        <div style={style.note}>
          Select a text shape that contains Arabic to see kashida controls.
        </div>
      )}

      {ready && arabicShape && (
        <>
          <div style={style.section}>
            <label style={style.label}>Source text</label>
            <div style={style.sourceText} dir="rtl">{original}</div>
          </div>

          <div style={style.section}>
            <label style={style.label}>
              Kashida count <span style={style.value}>{currentKashida}</span>
            </label>
            <input
              type="range" min={0} max={8} step={1}
              value={currentKashida}
              onChange={(e) => applyKashida(parseInt(e.target.value, 10))}
              style={style.slider}
            />
            <div style={style.scale}>
              <span>0</span><span>4</span><span>8</span>
            </div>
          </div>

          <div style={style.section}>
            <label style={style.label}>Font</label>
            <select
              value={currentFont}
              onChange={(e) => changeFont(parseInt(e.target.value, 10))}
              style={style.select}
            >
              {FONT_OPTIONS.map(f => (
                <option key={f.idx} value={f.idx}>{f.name}</option>
              ))}
            </select>
          </div>

          <button style={style.resetBtn} onClick={resetText}>
            Reset to source
          </button>

          <div style={style.footnote}>
            Insertion uses HarfBuzz's <code>safe_to_insert_tatweel</code> +
            Hallberg's joining-class pair table — lām-alif never splits.
          </div>
        </>
      )}
    </aside>
  )
})

// ────────────── STYLES ──────────────
const style: Record<string, React.CSSProperties> = {
  sidebar: {
    position: 'fixed',
    top: 60,
    right: 12,
    width: 260,
    maxHeight: 'calc(100vh - 80px)',
    overflowY: 'auto',
    background: 'rgba(28, 28, 32, 0.92)',
    backdropFilter: 'blur(20px)',
    color: '#e6e7ea',
    borderRadius: 12,
    border: '1px solid rgba(255,255,255,0.08)',
    boxShadow: '0 12px 32px rgba(0,0,0,0.4)',
    padding: 16,
    fontFamily: 'Inter, system-ui, sans-serif',
    fontSize: 13,
    zIndex: 1000,
    pointerEvents: 'auto',
  },
  header: {
    paddingBottom: 12,
    marginBottom: 12,
    borderBottom: '1px solid rgba(255,255,255,0.08)',
  },
  brand: { fontWeight: 600, fontSize: 13, color: '#fff' },
  subtitle: {
    fontSize: 10.5, color: '#888c93',
    fontFamily: 'ui-monospace, monospace',
    textTransform: 'uppercase', letterSpacing: 0.6,
    marginTop: 2,
  },
  err: {
    fontSize: 11, color: '#ff8b80', background: 'rgba(255, 100, 80, 0.10)',
    padding: 8, borderRadius: 6,
  },
  note: { fontSize: 12, color: '#a8aab2', lineHeight: 1.5 },
  section: { marginBottom: 14 },
  label: {
    fontSize: 10, color: '#888c93',
    textTransform: 'uppercase', letterSpacing: 0.6,
    fontWeight: 600, display: 'flex', justifyContent: 'space-between',
    marginBottom: 6,
  },
  value: {
    fontFamily: 'ui-monospace, monospace',
    color: '#fff', textTransform: 'none', fontWeight: 500,
  },
  sourceText: {
    background: 'rgba(255,255,255,0.05)',
    padding: '8px 10px', borderRadius: 6,
    fontSize: 14, lineHeight: 1.6,
    color: '#fff',
    fontFamily: 'Amiri, "Noto Naskh Arabic", serif',
    maxHeight: 80, overflowY: 'auto',
  },
  slider: {
    width: '100%',
    accentColor: '#c9a961',
  },
  scale: {
    display: 'flex', justifyContent: 'space-between',
    fontFamily: 'ui-monospace, monospace',
    fontSize: 9.5, color: '#666',
    marginTop: 2,
  },
  select: {
    width: '100%',
    background: 'rgba(255,255,255,0.05)',
    color: '#fff',
    border: '1px solid rgba(255,255,255,0.1)',
    borderRadius: 6,
    padding: '6px 8px',
    fontSize: 12,
    fontFamily: 'inherit',
  },
  resetBtn: {
    width: '100%',
    background: 'rgba(255,255,255,0.06)',
    color: '#e6e7ea',
    border: '1px solid rgba(255,255,255,0.10)',
    borderRadius: 6,
    padding: '7px 10px',
    fontSize: 12,
    cursor: 'pointer',
    marginBottom: 12,
    fontFamily: 'inherit',
  },
  footnote: {
    fontSize: 10.5, color: '#666',
    lineHeight: 1.55,
    paddingTop: 10,
    borderTop: '1px solid rgba(255,255,255,0.06)',
    fontFamily: 'ui-monospace, monospace',
  },
}
