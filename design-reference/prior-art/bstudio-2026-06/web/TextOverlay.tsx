import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from 'react'
import { resolveEngineFontId } from './fonts/fontRegistry'
import { editor, type Shape, type TextShape } from './editorState'
import { cameraStore, worldToScreen, type Viewport } from './cameraStore'
import {
  engineReady,
  engineIsReady,
  render_arabic_line,
  set_active_font,
  set_active_weight,
  set_active_tracking,
  set_harakat,
  set_harakat_color,
  caret_x_px,
  byte_at_x_px,
  layout_line_json,
} from './engine/loader'
import { wrapLineSpans, justifyKashida, computeTextReflow } from './textLayout'

// Engine font index → CSS family for the inline editor, so the textarea shows
// roughly the same glyphs the engine paints (kills the jarring browser-font →
// engine-font jump on commit). Falls back gracefully if the webfont isn't up.
const EDIT_FONT = [
  "'Amiri', serif",
  "'Cairo', sans-serif",
  "'IBM Plex Sans Arabic', sans-serif",
  "'Tajawal', sans-serif",
  "'Noto Sans Arabic', sans-serif",
]
function editFontFamily(ff: number | undefined): string {
  return EDIT_FONT[ff ?? 0] ?? EDIT_FONT[0]
}

function useViewport() {
  return useSyncExternalStore(cameraStore.subscribe, cameraStore.get, cameraStore.get)
}

function useShapes(): Shape[] {
  // Snapshot is the version counter (a stable primitive); the array is read
  // separately so useSyncExternalStore doesn't loop on a fresh reference.
  useSyncExternalStore(
    (cb) => editor.subscribe(cb),
    () => editor.getVersion(),
    () => 0,
  )
  return editor.listShapes()
}

// Build a Map<shapeId, frameScreenRect> for every text shape that has a
// parent frame. Returns screen coords (px) so both the 2D canvas and the DOM
// overlay can use it directly. Frames never rotate (locked at the bus).
function buildFrameClipMap(
  shapes: Shape[],
  vp: Viewport,
): Map<string, { x: number; y: number; w: number; h: number }> {
  const m = new Map<string, { x: number; y: number; w: number; h: number }>()
  for (const s of shapes) {
    if (s.type !== 'text') continue
    const frame = editor.frameOfShape(s.id)
    if (!frame || frame.type !== 'frame') continue
    const tl = worldToScreen(vp, frame.x, frame.y)
    m.set(s.id, { x: tl.x, y: tl.y, w: frame.w * vp.zoom, h: frame.h * vp.zoom })
  }
  return m
}

function rgb(s: { fillR: number; fillG: number; fillB: number }) {
  return `rgb(${s.fillR}, ${s.fillG}, ${s.fillB})`
}

// The engine speaks UTF-8 byte offsets; the textarea's selectionStart is a UTF-16
// code-unit index. These bridge the two for caret placement.
const __utf8 = new TextEncoder()
function u8len(s: string): number {
  return s ? __utf8.encode(s).length : 0
}
function u16FromByte(text: string, byte: number): number {
  if (byte <= 0) return 0
  let b = 0
  let i = 0
  for (const ch of text) {
    if (b >= byte) break
    b += u8len(ch)
    i += ch.length
  }
  return i
}

// One visual (wrapped) line of a text shape: the engine-shaped text, its source
// span in `content` (UTF-16), the per-line kashida fill used to paint it, and the
// screen geometry. ONE layout feeds BOTH the canvas paint and the inline caret,
// so the glyphs and the caret/selection can never drift apart.
interface VisualLineGeom {
  text: string
  start: number
  end: number
  baselineX: number
  baselineY: number
  kEff: number
  advancePx: number
}
interface TextLayout {
  lines: VisualLineGeom[]
  fontPx: number
  fill: string
  ff: number
  clip: { x: number; y: number; w: number; h: number } | null
}

// THE single RTL text layout — shared by the canvas paint (paintText) and the
// inline editor's caret/selection. Wraps area text to the box (kashida-free wrap,
// so kashida only fills a line, never forces a break), applies the same per-line
// kashida fill the paint uses, and right-anchors RTL runs. Because the caret is
// measured from exactly this, edit == preview by construction. (LTR shapes are
// painted by paintLtrText and edited via the legacy textarea — never here.)
function layoutTextLines(s: TextShape, vp: Viewport): TextLayout {
  const p = worldToScreen(vp, s.x, s.y)
  const fontPx = s.fontSize * vp.zoom
  const ff = s.fontFamily ?? 0
  set_active_font(resolveEngineFontId(ff))
  set_active_weight(s.weight ?? 400); set_active_tracking(s.tracking ?? 0); set_harakat(s.harakatShow ?? true, s.harakatScale ?? 1); set_harakat_color(s.harakatColor ?? '')
  const leading = s.lineHeight && s.lineHeight > 0 ? s.lineHeight : 1.2
  const lineH = fontPx * leading
  const boxLeft = p.x
  const boxW = s.w * vp.zoom
  const boxRight = boxLeft + boxW
  const align = s.textAlign ?? 0
  const widthEm = s.fontSize > 0 ? s.w / s.fontSize : 0
  // Visual-line spans: point text = one per logical line (no wrap); area text =
  // word-wrapped to the box width. Empty logical lines (bare `\n`) are preserved.
  const spans: { text: string; start: number; end: number }[] = []
  let base = 0
  for (const lg of s.content.split('\n')) {
    if (!lg) {
      spans.push({ text: '', start: base, end: base })
      base += 1
      continue
    }
    if (s.autoWidth) {
      spans.push({ text: lg, start: base, end: base + lg.length })
    } else {
      for (const sp of wrapLineSpans(lg, ff, 0, widthEm, s.weight ?? 400, s.tracking ?? 0)) {
        spans.push({ text: sp.text, start: base + sp.start, end: base + sp.end })
      }
    }
    base += lg.length + 1
  }
  const nLines = spans.length
  const boxH = s.h * vp.zoom
  const contentH = nLines * lineH
  // A FIXED-height area box can be taller than its text; vAlign places the block
  // in that slack (top = no shift, middle = half, bottom = all of it). Point text
  // and auto-height area text have no slack, so they always sit at the top.
  let vOffset = 0
  if (!s.autoWidth && s.autoHeight === false) {
    const slack = boxH - contentH
    if (slack > 0) {
      const va = s.vAlign ?? 0
      vOffset = va === 1 ? slack / 2 : va === 2 ? slack : 0
    }
  }
  const lines: VisualLineGeom[] = spans.map((sp, i) => {
    const baselineY = p.y + vOffset + fontPx * 0.8 + i * lineH
    // Per-line kashida fill, identical to the paint: justify fills every visual
    // line except the last of a multi-line paragraph; a manual count is capped to
    // what fits. Point text and empty lines never fill.
    let kEff = 0
    if (sp.text && !s.autoWidth) {
      if (align === 3) {
        const isLast = i === nLines - 1
        kEff = (isLast && nLines > 1) ? 0 : justifyKashida(sp.text, ff, widthEm, s.weight ?? 400, s.tracking ?? 0)
      } else {
        const req = s.kashida ?? 0
        kEff = req > 0 ? Math.min(req, justifyKashida(sp.text, ff, widthEm, s.weight ?? 400, s.tracking ?? 0)) : 0
      }
    }
    let advancePx = 0
    if (sp.text) {
      try {
        advancePx = (JSON.parse(layout_line_json(sp.text, fontPx, kEff)).totalAdvancePx as number) || 0
      } catch {
        advancePx = 0
      }
    }
    // render_arabic_line right-anchors the run at baselineX; centre/left shift it
    // by the run width. Default (0) + justify (3) right-anchor at the box's right.
    let baselineX = boxRight
    if (align === 1) baselineX = boxLeft + (boxW + advancePx) / 2
    else if (align === 2) baselineX = boxLeft + advancePx
    return { text: sp.text, start: sp.start, end: sp.end, baselineX, baselineY, kEff, advancePx }
  })
  // Auto-height area text grows to its content (never clips); a fixed-height box
  // clips whatever overflows its user-set height.
  const clipH = (!s.autoWidth && s.autoHeight === false) ? boxH : Math.max(boxH, contentH)
  const clip = s.autoWidth ? null : { x: boxLeft, y: p.y, w: boxW, h: clipH }
  return { lines, fontPx, fill: rgb(s), ff, clip }
}

// Word boundaries (UTF-16) around an offset — a run of non-whitespace, for
// double-click select. Clicking on whitespace selects the whitespace run instead;
// neither ever crosses a line break.
function wordBoundsAt(text: string, off: number): { start: number; end: number } {
  const isSpace = (c: string | undefined) => !!c && /\s/.test(c)
  let a = off
  let b = off
  if (isSpace(text[off]) && text[off] !== '\n') {
    while (a > 0 && isSpace(text[a - 1]) && text[a - 1] !== '\n') a--
    while (b < text.length && isSpace(text[b]) && text[b] !== '\n') b++
  } else {
    while (a > 0 && !isSpace(text[a - 1])) a--
    while (b < text.length && !isSpace(text[b])) b++
  }
  return { start: a, end: b }
}

// The whole logical line (paragraph) around an offset — between the surrounding
// newlines — for triple-click select.
function lineBoundsAt(text: string, off: number): { start: number; end: number } {
  let a = off
  let b = off
  while (a > 0 && text[a - 1] !== '\n') a--
  while (b < text.length && text[b] !== '\n') b++
  return { start: a, end: b }
}

// The visual line a (UTF-16) caret offset falls on, plus the offset within it.
function lineForOffset(lines: VisualLineGeom[], off: number): { line: VisualLineGeom; localU16: number } {
  let idx = 0
  for (let i = 0; i < lines.length; i++) {
    if (off >= lines[i].start) idx = i
    else break
  }
  const line = lines[idx] ?? lines[lines.length - 1]
  const localU16 = Math.max(0, Math.min(off - line.start, line.text.length))
  return { line, localU16 }
}

// Paint a left-to-right text shape with the native canvas. The Arabic engine
// reorders glyph runs for RTL, which reverses Latin text, so LTR text (the
// RTL/LTR toggle, dir=1) bypasses the engine. Greedy word-wrap to the box width;
// in LTR the box's natural side is the left, so the default + the "L" align both
// sit left, "C" centres (right-align is an RTL concept). The box grows downward.
function paintLtrText(
  ctx: CanvasRenderingContext2D,
  s: TextShape,
  p: { x: number; y: number },
  fontPx: number,
  lineH: number,
  boxLeft: number,
  boxW: number,
  boxH: number,
  align: number,
  ff: number,
  fill: string,
) {
  ctx.save()
  ctx.font = `${fontPx}px ${editFontFamily(ff)}`
  ctx.fillStyle = fill
  ctx.textBaseline = 'alphabetic'
  ctx.direction = 'ltr'
  const lines: string[] = []
  for (const lg of s.content.split('\n')) {
    if (!lg) { lines.push(''); continue }
    let cur = ''
    for (const tok of lg.split(/(\s+)/)) {
      const test = cur + tok
      if (cur.trim() && ctx.measureText(test).width > boxW) {
        lines.push(cur.trimEnd())
        cur = tok.replace(/^\s+/, '')
      } else {
        cur = test
      }
    }
    lines.push(cur.trimEnd())
  }
  const contentH = lines.length * lineH
  ctx.beginPath()
  ctx.rect(boxLeft, p.y, boxW, Math.max(boxH, contentH))
  ctx.clip()
  let x = boxLeft
  if (align === 1) { ctx.textAlign = 'center'; x = boxLeft + boxW / 2 }
  else { ctx.textAlign = 'left'; x = boxLeft }
  lines.forEach((line, i) => {
    if (line) ctx.fillText(line, x, p.y + fontPx * 0.8 + i * lineH)
  })
  ctx.restore()
}

// Draw all (non-editing) text shapes through our own Arabic engine: every glyph
// is shaped + outlined by `bstudio-text` and painted onto this 2D canvas via
// `render_arabic_line` — the browser font system is not used. RTL runs are
// right-anchored to the box's right edge; kashida elongation is engine-driven.
function paintText(
  ctx: CanvasRenderingContext2D,
  vp: Viewport,
  shapes: TextShape[],
  dpr: number,
  frameClips: Map<string, { x: number; y: number; w: number; h: number }>,
) {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, vp.w, vp.h)
  for (const s of shapes) {
    // An empty shape paints nothing; its box + caret still show while editing.
    if (!s.content) continue
    const fontPx = s.fontSize * vp.zoom
    if (fontPx < 1) continue
    // Left-to-right text: the Arabic engine reorders runs for RTL, which reverses
    // Latin text — so LTR is painted through the native canvas instead.
    if ((s.textDir ?? 0) === 1) {
      const p = worldToScreen(vp, s.x, s.y)
      const ff = s.fontFamily ?? 0
      const leading = s.lineHeight && s.lineHeight > 0 ? s.lineHeight : 1.2
      const fclipLtr = frameClips.get(s.id)
      if (fclipLtr) {
        ctx.save()
        ctx.beginPath()
        ctx.rect(fclipLtr.x, fclipLtr.y, fclipLtr.w, fclipLtr.h)
        ctx.clip()
      }
      paintLtrText(ctx, s, p, fontPx, fontPx * leading, p.x, s.w * vp.zoom, s.h * vp.zoom, s.textAlign ?? 0, ff, rgb(s))
      if (fclipLtr) ctx.restore()
      continue
    }
    // RTL: the ONE shared layout — the exact geometry the inline editor measures
    // its caret/selection from, so the shape being edited paints identically to
    // every other text shape (edit == preview). Kashida fill is per-line.
    const L = layoutTextLines(s, vp)
    set_active_font(resolveEngineFontId(L.ff))
    set_active_weight(s.weight ?? 400); set_active_tracking(s.tracking ?? 0); set_harakat(s.harakatShow ?? true, s.harakatScale ?? 1); set_harakat_color(s.harakatColor ?? '')
    const fclipRtl = frameClips.get(s.id)
    const needClip = L.clip || fclipRtl
    if (needClip) {
      ctx.save()
      ctx.beginPath()
      if (fclipRtl && L.clip) {
        // Intersect: clamp text-box clip to frame rect
        const ix = Math.max(L.clip.x, fclipRtl.x)
        const iy = Math.max(L.clip.y, fclipRtl.y)
        const ix2 = Math.min(L.clip.x + L.clip.w, fclipRtl.x + fclipRtl.w)
        const iy2 = Math.min(L.clip.y + L.clip.h, fclipRtl.y + fclipRtl.h)
        if (ix2 > ix && iy2 > iy) ctx.rect(ix, iy, ix2 - ix, iy2 - iy)
        // empty intersection => empty path => clips everything (correct)
      } else if (fclipRtl) {
        ctx.rect(fclipRtl.x, fclipRtl.y, fclipRtl.w, fclipRtl.h)
      } else if (L.clip) {
        ctx.rect(L.clip.x, L.clip.y, L.clip.w, L.clip.h)
      }
      ctx.clip()
    }
    for (const ln of L.lines) {
      if (!ln.text) continue
      const report = render_arabic_line(ctx, ln.baselineX, ln.baselineY, ln.text, fontPx, ln.kEff, L.fill)
      report.free()
    }
    if (needClip) ctx.restore()
  }
}

// A layer painted over the WebGL canvas. A 2D canvas renders the text via our
// engine; a textarea hosts inline editing. Pointer-transparent except for the
// active textarea, so selection/move of text still happens on the canvas below.
export function TextOverlay() {
  const vp = useViewport()
  const shapes = useShapes()
  const [editId, setEditId] = useState<string | null>(null)
  const [draft, setDraft] = useState('')
  const [caretOffset, setCaretOffset] = useState(0)
  // Active selection range in the draft (UTF-16 offsets). Collapsed (start===end)
  // when there's just a caret. Drives the engine-aligned highlight bands.
  const [selRange, setSelRange] = useState<{ start: number; end: number }>({ start: 0, end: 0 })
  const [engineUp, setEngineUp] = useState(engineIsReady())
  const taRef = useRef<HTMLTextAreaElement | null>(null)
  const canvasRef = useRef<HTMLCanvasElement | null>(null)
  const overlayRef = useRef<HTMLDivElement | null>(null)
  // Anchor offset for a mouse drag-select (the end that stays put). Null when not dragging.
  const dragAnchorRef = useRef<number | null>(null)
  // The shape exactly as it was when editing opened — the fixed anchor that
  // computeReflow grows the live box from (so point text keeps its right edge and
  // area text keeps its width, no matter how many keystrokes re-reflow it).
  const baseShapeRef = useRef<TextShape | null>(null)
  // Kashida drag-handle: live readout while dragging (null = idle) + the drag session.
  const [kashidaHint, setKashidaHint] = useState<number | null>(null)
  const kashidaDragRef = useRef<{ id: string; startX: number; startK: number } | null>(null)

  // Mirror the textarea's native selection (set by typing, arrows, shift-arrows)
  // into our engine-side state: the band range + the caret at the moving end.
  function syncFromTa(ta: HTMLTextAreaElement) {
    const start = ta.selectionStart ?? 0
    const end = ta.selectionEnd ?? start
    setSelRange({ start, end })
    setCaretOffset(ta.selectionDirection === 'backward' ? start : end)
  }

  useEffect(() => {
    if (!engineUp) engineReady().then(() => setEngineUp(true)).catch(() => {})
  }, [engineUp])

  useEffect(() => {
    function onEdit(e: Event) {
      const id = (e as CustomEvent).detail?.id as string | undefined
      if (!id) return
      const s = editor.getShape(id)
      if (!s || s.type !== 'text') return
      setEditId(id)
      setDraft(s.content)
      // Remember the shape's opening geometry (the reflow anchor) and open the
      // live overlay, so from the first keystroke the one real shape — its glyphs
      // AND its selection box — tracks the draft. Edit == preview, one shape.
      baseShapeRef.current = { ...s }
      editor.setEditingText(id)
      // Engine (RTL) edit drives the live overlay, so the one real shape tracks the
      // draft (edit == preview). LTR keeps the legacy visible textarea — no overlay.
      if (engineIsReady() && (s.textDir ?? 0) !== 1) {
        editor.setLiveEdit(id, computeTextReflow(s, s.content))
      }
    }
    window.addEventListener('bstudio:edit-text', onEdit)
    return () => window.removeEventListener('bstudio:edit-text', onEdit)
  }, [])

  useEffect(() => {
    if (editId && taRef.current) {
      const ta = taRef.current
      ta.focus()
      // Place the caret at the end rather than selecting everything — the
      // full-selection highlight made the edit box look nothing like the final
      // render (Ahmed: "inside the box is one thing, outside another").
      const n = ta.value.length
      ta.setSelectionRange(n, n)
      setCaretOffset(n)
      setSelRange({ start: n, end: n })
    }
  }, [editId])

  // Auto-grow the editor box downward with every new line, matching the painted
  // text's auto-height behaviour (no clipping of lines below the box).
  useEffect(() => {
    const ta = taRef.current
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = `${ta.scrollHeight}px`
    // Point text grows its width with the content too (it never wraps). The
    // textarea is right-anchored for RTL (CSS `right`), so the extra width
    // extends leftward as you type, matching the engine's right-anchored render.
    const es = editId ? editor.getShape(editId) : null
    if (es && es.type === 'text' && es.autoWidth) {
      ta.style.width = 'auto'
      ta.style.width = `${ta.scrollWidth + 2}px`
    }
  }, [draft, editId, vp.zoom])

  const textShapes = shapes.filter((s): s is TextShape => s.type === 'text' && s.visible !== false)
  // Frame clip rects in screen space for every text shape inside a frame.
  // Recomputed whenever shapes or viewport change (same budget as canvas repaint).
  const frameClips = useMemo(
    () => buildFrameClipMap(shapes, vp),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [shapes, vp],
  )
  const editShape = editId ? textShapes.find(s => s.id === editId) : undefined
  // Engine-edit (M2 "one engine"): the RTL shape under edit is painted by our
  // engine — kashida-free, exactly as it will look on commit — instead of the
  // browser drawing it in a webfont. The textarea becomes a transparent keystroke
  // capture and a custom caret (engine-aligned) marks the insertion point. LTR
  // (dir=1) and the engine-not-ready window keep the legacy visible textarea.
  const engineEdit = !!editShape && engineUp && (editShape.textDir ?? 0) !== 1

  // The edited shape already carries the live draft (via the overlay), so its
  // caret layout is just the shared layout of that shape — the very same one
  // paintText draws from. Recomputes per keystroke (editShape is a fresh merge).
  const editLayout = useMemo<TextLayout | null>(() => {
    if (!engineEdit || !editShape) return null
    return layoutTextLines(editShape, vp)
  }, [engineEdit, editShape, vp])

  // The custom caret rectangle (screen px), derived from the same layout as the
  // painted glyphs: absolute x = run's left edge + offset-from-left, where the
  // left edge is `baselineX - totalAdvancePx` (the engine right-anchors RTL runs).
  const caret = useMemo(() => {
    if (!editLayout || editLayout.lines.length === 0) return null
    const { line, localU16 } = lineForOffset(editLayout.lines, caretOffset)
    const localByte = u8len(line.text.slice(0, localU16))
    let xFromLeft = 0
    try {
      xFromLeft = caret_x_px(line.text, editLayout.fontPx, line.kEff, localByte)
    } catch {
      xFromLeft = 0
    }
    return {
      x: line.baselineX - line.advancePx + xFromLeft,
      y: line.baselineY - editLayout.fontPx * 0.8,
      w: Math.max(1.2, editLayout.fontPx * 0.06),
      h: editLayout.fontPx,
      color: editLayout.fill,
    }
  }, [editLayout, caretOffset])

  // Selection highlight bands — one rect per visual line the selection covers,
  // measured from the same engine layout as the glyphs. RTL runs right-anchor, so
  // caret_x_px (distance from the run's left edge) shrinks as the byte offset
  // grows; min/max keeps the band correct regardless of selection direction.
  const selBands = useMemo(() => {
    if (!editLayout || selRange.start === selRange.end) return [] as { x: number; y: number; w: number; h: number }[]
    const lo = Math.min(selRange.start, selRange.end)
    const hi = Math.max(selRange.start, selRange.end)
    const out: { x: number; y: number; w: number; h: number }[] = []
    for (const ln of editLayout.lines) {
      const a = Math.max(lo, ln.start) - ln.start
      const b = Math.min(hi, ln.end) - ln.start
      if (b <= a) continue
      const left = ln.baselineX - ln.advancePx
      let xa = 0
      let xb = 0
      try {
        xa = caret_x_px(ln.text, editLayout.fontPx, ln.kEff, u8len(ln.text.slice(0, a)))
        xb = caret_x_px(ln.text, editLayout.fontPx, ln.kEff, u8len(ln.text.slice(0, b)))
      } catch {
        continue
      }
      out.push({
        x: left + Math.min(xa, xb),
        y: ln.baselineY - editLayout.fontPx * 0.8,
        w: Math.max(1, Math.abs(xa - xb)),
        h: editLayout.fontPx,
      })
    }
    return out
  }, [editLayout, selRange])

  // Repaint the engine canvas whenever the viewport, shapes, edit target or
  // engine readiness changes.
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas || !engineUp || vp.w === 0 || vp.h === 0) return
    const dpr = window.devicePixelRatio || 1
    const pxW = Math.round(vp.w * dpr)
    const pxH = Math.round(vp.h * dpr)
    if (canvas.width !== pxW) canvas.width = pxW
    if (canvas.height !== pxH) canvas.height = pxH
    const ctx = canvas.getContext('2d')
    if (!ctx) return
    // ONE paint path for every text shape — including the one being edited, whose
    // live draft is merged into it by the editor's overlay. So the shape you edit
    // renders exactly like the others; the only edit-only marks are the caret +
    // selection bands drawn as DOM overlays on top. (LTR shapes paint via the
    // native-canvas branch and keep the legacy visible textarea.)
    paintText(ctx, vp, textShapes, dpr, frameClips)
  }, [vp, textShapes, engineUp, frameClips])

  function commit() {
    if (!editId) return
    const text = draft
    const id = editId
    const base = baseShapeRef.current
    setEditId(null)
    baseShapeRef.current = null
    // Drop the live overlay BEFORE reading the shape, so getShape returns the
    // committed (core) content — then write the final box as ONE undo step.
    editor.clearLiveEdit()
    // Restore the shape's normal selection chrome now that the inline editor closed.
    editor.setEditingText(null)
    const s = editor.getShape(id)
    if (!s || s.type !== 'text') return
    if (text.trim() === '') {
      // An empty text box is noise — drop it.
      editor.removeShape(id)
    } else if (text !== s.content) {
      // The exact same reflow the live overlay used while typing, off the same
      // opening anchor — so the committed box equals the box you were editing in
      // (no jump). One updateShape = one undo step for the whole edit session.
      editor.updateShape(id, computeTextReflow(base ?? s, text))
    }
  }

  // Keep a live ref to commit so the global listener always sees the latest
  // draft (the listener is installed once per edit session).
  const commitRef = useRef(commit)
  useEffect(() => {
    commitRef.current = commit
  })

  // Clicking anywhere outside the textarea ends editing. The canvas isn't
  // focusable, so it never blurs the textarea on its own — Ahmed got "stuck" in
  // text. Capture phase so we commit before the canvas handles the same click,
  // which then falls through to select whatever is under the pointer.
  useEffect(() => {
    if (!editId) return
    const onDown = (e: MouseEvent) => {
      const ta = taRef.current
      if (ta && e.target instanceof Node && !ta.contains(e.target)) commitRef.current()
    }
    window.addEventListener('mousedown', onDown, true)
    return () => window.removeEventListener('mousedown', onDown, true)
  }, [editId])

  const editPos = editShape ? worldToScreen(vp, editShape.x, editShape.y) : null
  // Point text anchors at its writing-start edge: RTL keeps the RIGHT edge fixed
  // (via CSS `right`) so it grows leftward; LTR uses `left` and grows right.
  const editRight = editShape ? worldToScreen(vp, editShape.x + editShape.w, editShape.y) : null
  const ptRTL = !!editShape && editShape.autoWidth === true && (editShape.textDir ?? 0) !== 1

  // ── Kashida-on-shape handle ─────────────────────────────────
  // Direct-manipulation grip for the per-shape kashida (tatweel) elongation — the
  // moat made physical. When a SINGLE RTL area-text shape is selected and not being
  // edited, a grip rides the left edge of its box; dragging it left adds elongation,
  // right removes it, writing s.kashida live with the whole gesture collapsed into
  // one undo step. (Point text takes no kashida fill, and justify auto-fills it, so
  // neither shows a grip — the grip only appears where it actually does something.)
  const KASHIDA_MAX = 24
  const KASHIDA_PX_PER_UNIT = 12
  const kashidaShape: TextShape | null = (() => {
    if (editId) return null
    const sel = editor.selectionList()
    if (sel.length !== 1) return null
    const s = textShapes.find(t => t.id === sel[0])
    if (!s) return null
    if ((s.textDir ?? 0) === 1) return null            // RTL only
    if (s.autoWidth) return null                        // area text only (point text takes no fill)
    if ((s.textAlign ?? 0) === 3) return null           // justify auto-fills; manual kashida is ignored
    if (s.rotation) return null                         // a rotated box's left edge isn't axis-aligned
    return s
  })()
  const kashidaHandle = kashidaShape
    ? (() => {
        const tl = worldToScreen(vp, kashidaShape.x, kashidaShape.y)
        const bl = worldToScreen(vp, kashidaShape.x, kashidaShape.y + kashidaShape.h)
        return { x: tl.x - 20, y: (tl.y + bl.y) / 2, k: kashidaShape.kashida ?? 0 }
      })()
    : null

  function onKashidaPointerDown(e: React.PointerEvent<HTMLDivElement>) {
    if (!kashidaShape) return
    e.preventDefault()
    e.stopPropagation()
    const id = kashidaShape.id
    const startK = kashidaShape.kashida ?? 0
    kashidaDragRef.current = { id, startX: e.clientX, startK }
    setKashidaHint(startK)
    editor.beginUndoGroup()
    const onMove = (me: PointerEvent) => {
      const d = kashidaDragRef.current
      if (!d) return
      const delta = d.startX - me.clientX // drag LEFT → more elongation
      const k = Math.max(0, Math.min(KASHIDA_MAX, Math.round(d.startK + delta / KASHIDA_PX_PER_UNIT)))
      editor.updateShape(d.id, { kashida: k } as Partial<TextShape>)
      setKashidaHint(k)
    }
    const onUp = () => {
      kashidaDragRef.current = null
      editor.endUndoGroup()
      setKashidaHint(null)
      window.removeEventListener('pointermove', onMove)
      window.removeEventListener('pointerup', onUp)
    }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }

  // Map a page point onto an engine-layout offset (a UTF-16 draft index): pick the
  // nearest visual line by baseline, then the byte under x — from the engine's own
  // metrics, not the invisible textarea's webfont layout, so caret/anchor land
  // exactly where the painted glyphs are.
  function offsetFromClientXY(clientX: number, clientY: number): number | null {
    if (!editLayout || editLayout.lines.length === 0) return null
    const rect = overlayRef.current?.getBoundingClientRect()
    const cx = clientX - (rect ? rect.left : 0)
    const cy = clientY - (rect ? rect.top : 0)
    let best = 0
    let bestD = Infinity
    editLayout.lines.forEach((ln, i) => {
      const d = Math.abs(cy - (ln.baselineY - editLayout.fontPx * 0.4))
      if (d < bestD) { bestD = d; best = i }
    })
    const ln = editLayout.lines[best]
    const xFromLeft = cx - (ln.baselineX - ln.advancePx)
    let byte = 0
    try {
      byte = byte_at_x_px(ln.text, editLayout.fontPx, ln.kEff, xFromLeft)
    } catch {
      byte = 0
    }
    return ln.start + u16FromByte(ln.text, byte)
  }

  // Mouse caret-placement + drag-select in engine-edit mode. preventDefault stops
  // the textarea's own (webfont-based, misaligned) caret/selection; we drive
  // selection from the engine layout and keep the textarea focused for keystrokes.
  // A plain click (no drag) collapses anchor==head, i.e. just places the caret.
  function onEngineMouseDown(e: React.MouseEvent<HTMLTextAreaElement>) {
    const ta = taRef.current
    if (!ta) return
    const off = offsetFromClientXY(e.clientX, e.clientY)
    if (off == null) return
    e.preventDefault()
    ta.focus()
    // Double-click = select the word under the cursor; triple-click = the whole
    // visual/logical line. `e.detail` is the native click count, so no manual timing.
    const selectRange = (r: { start: number; end: number }) => {
      ta.setSelectionRange(r.start, r.end, 'forward')
      setSelRange(r)
      setCaretOffset(r.end)
    }
    if (e.detail === 2) { selectRange(wordBoundsAt(draft, off)); return }
    if (e.detail >= 3) { selectRange(lineBoundsAt(draft, off)); return }
    dragAnchorRef.current = off
    ta.setSelectionRange(off, off)
    setSelRange({ start: off, end: off })
    setCaretOffset(off)
    const onMove = (me: MouseEvent) => {
      const anchor = dragAnchorRef.current
      if (anchor == null) return
      const o = offsetFromClientXY(me.clientX, me.clientY)
      if (o == null) return
      const start = Math.min(anchor, o)
      const end = Math.max(anchor, o)
      ta.setSelectionRange(start, end, o < anchor ? 'backward' : 'forward')
      setSelRange({ start, end })
      setCaretOffset(o)
    }
    const onUp = () => {
      dragAnchorRef.current = null
      window.removeEventListener('mousemove', onMove)
      window.removeEventListener('mouseup', onUp)
    }
    window.addEventListener('mousemove', onMove)
    window.addEventListener('mouseup', onUp)
  }

  // Frame clip for inline-edit DOM overlays (caret, selection bands).
  // The textarea itself stays unclipped while editing — per spec this is acceptable.
  const editFrameClip = editShape ? (frameClips.get(editShape.id) ?? null) : null

  return (
    <div ref={overlayRef} className="text-overlay" style={{ position: 'absolute', inset: 0, overflow: 'hidden', pointerEvents: 'none' }}>
      <canvas
        ref={canvasRef}
        style={{ position: 'absolute', inset: 0, width: '100%', height: '100%', pointerEvents: 'none' }}
      />
      {engineEdit && selBands.map((b, i) => (
        <div
          key={i}
          className="text-sel-band"
          style={{
            position: 'absolute',
            left: b.x,
            top: b.y,
            width: b.w,
            height: b.h,
            ...(editFrameClip ? {
              clipPath: `inset(${Math.max(0, editFrameClip.y - b.y)}px ${Math.max(0, (b.x + b.w) - (editFrameClip.x + editFrameClip.w))}px ${Math.max(0, (b.y + b.h) - (editFrameClip.y + editFrameClip.h))}px ${Math.max(0, editFrameClip.x - b.x)}px)`,
            } : {}),
          }}
        />
      ))}
      {engineEdit && caret && (
        <div
          className="text-caret"
          style={{
            position: 'absolute',
            left: caret.x,
            top: caret.y,
            width: caret.w,
            height: caret.h,
            background: caret.color,
            ...(editFrameClip ? {
              clipPath: `inset(${Math.max(0, editFrameClip.y - caret.y)}px ${Math.max(0, (caret.x + caret.w) - (editFrameClip.x + editFrameClip.w))}px ${Math.max(0, (caret.y + caret.h) - (editFrameClip.y + editFrameClip.h))}px ${Math.max(0, editFrameClip.x - caret.x)}px)`,
            } : {}),
          }}
        />
      )}
      {kashidaHandle && (
        <div
          className="kashida-handle"
          onPointerDown={onKashidaPointerDown}
          title={`Kashida elongation: ${kashidaHint ?? kashidaHandle.k} — drag to stretch`}
          style={{
            position: 'absolute',
            left: kashidaHandle.x - 11,
            top: kashidaHandle.y - 11,
            width: 22,
            height: 22,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            borderRadius: 11,
            background: 'var(--accent)',
            color: '#fff',
            font: '600 14px system-ui, sans-serif',
            lineHeight: 1,
            cursor: 'ew-resize',
            boxShadow: '0 1px 3px rgba(0,0,0,0.35)',
            userSelect: 'none',
            pointerEvents: 'auto',
            zIndex: 5,
          }}
        >
          ـ
          {/* Directional cue (shown on hover via CSS) so the grip reads as drag-to-stretch. */}
          <span className="kashida-arrows" aria-hidden>↔</span>
          {kashidaHint !== null && (
            <span
              style={{
                position: 'absolute',
                left: '50%',
                bottom: 26,
                transform: 'translateX(-50%)',
                padding: '1px 6px',
                borderRadius: 4,
                background: 'var(--accent)',
                color: '#fff',
                font: '600 11px system-ui, sans-serif',
                whiteSpace: 'nowrap',
              }}
            >
              {kashidaHint}
            </span>
          )}
        </div>
      )}
      {editShape && editPos && (
        <textarea
          key={editShape.id}
          ref={taRef}
          className={engineEdit ? 'text-edit-ta engine-edit' : 'text-edit-ta'}
          value={draft}
          dir={(editShape.textDir ?? 0) === 1 ? 'ltr' : 'rtl'}
          wrap={editShape.autoWidth ? 'off' : 'soft'}
          onChange={(e) => {
            const v = e.target.value
            setDraft(v)
            // Push the draft into the one real shape (glyphs + box) via the live
            // overlay — no core write, no per-keystroke undo. Reflowed from the
            // opening shape so the box hugs the text exactly as commit will.
            if (engineEdit && editId && baseShapeRef.current) {
              editor.setLiveEdit(editId, computeTextReflow(baseShapeRef.current, v))
            }
            syncFromTa(e.target)
          }}
          onSelect={(e) => syncFromTa(e.target as HTMLTextAreaElement)}
          onMouseDown={engineEdit ? onEngineMouseDown : undefined}
          onBlur={commit}
          onKeyDown={(e) => {
            e.stopPropagation()
            if (e.key === 'Escape') { e.preventDefault(); commit() }
            if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) { e.preventDefault(); commit() }
          }}
          style={{
            position: 'absolute',
            top: editPos.y,
            ...(ptRTL
              ? { right: Math.max(0, vp.w - (editRight ? editRight.x : editPos.x)) }
              : { left: editPos.x }),
            ...(editShape.autoWidth
              ? { whiteSpace: 'pre' as const }
              : { width: editShape.w * vp.zoom }),
            minHeight: editShape.h * vp.zoom,
            fontSize: editShape.fontSize * vp.zoom,
            lineHeight: editShape.lineHeight && editShape.lineHeight > 0 ? editShape.lineHeight : 1.2,
            // Engine-edit: the engine paints the glyphs and a custom caret marks
            // the insertion point, so the textarea's own text + native caret are
            // transparent (it's pure keystroke/IME capture). Legacy edit (LTR /
            // engine-down) keeps the textarea visible in the matching webfont.
            color: engineEdit ? 'transparent' : rgb(editShape),
            caretColor: engineEdit ? 'transparent' : rgb(editShape),
            background: 'transparent',
            // Engine-edit: the one real shape keeps its normal selection chrome +
            // handles (drawn by SkiaCanvas, tracking the live overlay), so the
            // textarea is fully invisible — a border here would be a second box at
            // its auto-grown size. Legacy (LTR / engine-down) keeps the border.
            border: engineEdit ? 'none' : '1px solid #0c8ce9',
            outline: 'none',
            resize: 'none',
            padding: 0,
            margin: 0,
            fontFamily: editFontFamily(editShape.fontFamily),
            direction: (editShape.textDir ?? 0) === 1 ? 'ltr' : 'rtl',
            textAlign: (editShape.textAlign ?? 0) === 2 ? 'left'
              : (editShape.textAlign ?? 0) === 1 ? 'center'
              : (editShape.textDir ?? 0) === 1 ? 'left' : 'right',
            pointerEvents: 'auto',
            overflow: 'hidden',
            boxSizing: 'border-box',
          }}
        />
      )}
    </div>
  )
}
