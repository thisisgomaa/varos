// Shared Arabic text-layout helpers — measuring, justify-fill and area-text
// word-wrap, all measured through the shaping engine so advances are correct.
//
// These are used by BOTH the canvas paint (TextOverlay) and the document
// exporters, so engine-outline export lays text out identically to the canvas.
// Keep this the single source of truth for line breaking + kashida fill.

import { resolveEngineFontId } from './fonts/fontRegistry'
import { set_active_font, set_active_weight, set_active_tracking, shape_with_kashida, engineIsReady } from './engine/loader'
import type { TextShape } from './editorState'

// Run width in *em-relative* px for a line at the active font, given a kashida
// count. Returns advance / units-per-em (multiply by font-size to get pixels).
export function runWidthPerEm(line: string, kashida: number): number {
  const r = shape_with_kashida(line, kashida)
  const w = r.units_per_em > 0 ? r.total_x_advance / r.units_per_em : 0
  r.free()
  return w
}

// Kashida count that fills a box of `widthEm` (box width / font-size) for a
// justified line. Zoom-independent, so memoised on the line's intrinsic key.
const justifyCache = new Map<string, number>()
export function justifyKashida(line: string, fontFamily: number, widthEm: number, weight = 400, tracking = 0): number {
  const key = `${fontFamily}|${weight}|${tracking}|${widthEm.toFixed(3)}|${line}`
  const hit = justifyCache.get(key)
  if (hit !== undefined) return hit
  set_active_font(resolveEngineFontId(fontFamily))
  set_active_weight(weight)
  set_active_tracking(tracking)
  // No fixed cap: stretch as far as the box needs, stopping only when the box
  // is filled or the letters physically can't elongate any further (kashida
  // saturates → width stops growing). HARD is a pathological-loop safety net.
  const HARD = 4000
  let best = 0
  let prevW = -1
  for (let n = 0; n <= HARD; n++) {
    const w = runWidthPerEm(line, n)
    // Stop just short of the box edge so a justified line never overflows —
    // a small margin covers the leftmost glyph's ink (side bearing). R43: the
    // old 0.5em margin left auto-kashida visibly short of filling the box.
    if (w > widthEm - 0.12) break
    if (n > 0 && w <= prevW) break  // kashida saturated — width stopped growing
    prevW = w
    best = n
  }
  justifyCache.set(key, best)
  return best
}

// One visual (wrapped) line plus its source span — `text === line.slice(start, end)`
// in UTF-16 units. The single space dropped at a wrap point lies between end(i)
// and start(i+1), so spans are non-contiguous by exactly that one space.
export interface VisualSpan {
  text: string
  start: number
  end: number
}

// Greedy word-wrap a logical line into visual lines that each fit `widthEm`,
// returning each line's source span so the inline editor can map a caret offset
// onto the very glyphs it paints. This is the break algorithm; `wrapLine` is the
// `.text`-only projection of it, so layout and caret can never diverge.
export function wrapLineSpans(line: string, fontFamily: number, kashida: number, widthEm: number, weight = 400, tracking = 0): VisualSpan[] {
  if (widthEm <= 0 || !line) return [{ text: line, start: 0, end: line.length }]
  set_active_font(resolveEngineFontId(fontFamily))
  set_active_weight(weight)
  set_active_tracking(tracking)
  // Leave a hair for glyph side bearings, so a line that "fits" by advance
  // doesn't graze past the box edge visually. R43 (spec 003): this used to be
  // a huge 0.5em — at 67px type that wrapped lines a visible 33px short of the
  // box edge ("الكلام مش بيوصل لآخر البوكس"). Real side bearings are ~0.02-0.08em.
  const fit = widthEm > 1 ? widthEm - 0.06 : widthEm
  if (runWidthPerEm(line, kashida) <= fit) return [{ text: line, start: 0, end: line.length }]
  const out: VisualSpan[] = []
  const words = line.split(' ')
  let cur = ''
  let curStart = 0  // source offset of cur's first char
  let cursor = 0    // source offset of the word about to be considered
  for (const word of words) {
    const cand = cur ? cur + ' ' + word : word
    if (cur && runWidthPerEm(cand, kashida) > fit) {
      out.push({ text: cur, start: curStart, end: curStart + cur.length })
      cur = word
      curStart = cursor
    } else {
      if (!cur) curStart = cursor
      cur = cand
    }
    cursor += word.length + 1  // +1 for the space `split(' ')` removed
  }
  if (cur) out.push({ text: cur, start: curStart, end: curStart + cur.length })
  return out
}

// Greedy word-wrap a logical line into visual lines that each fit `widthEm`
// (box width / font-size), measuring candidates through the shaping engine so
// Arabic advances are correct. Whitespace-delimited; a single over-long word is
// kept intact. Memoised on the intrinsic key so repaints/exports don't re-shape.
const wrapCache = new Map<string, string[]>()
export function wrapLine(line: string, fontFamily: number, kashida: number, widthEm: number, weight = 400, tracking = 0): string[] {
  if (widthEm <= 0 || !line) return [line]
  const key = `${fontFamily}|${weight}|${tracking}|${widthEm.toFixed(3)}|${kashida}|${line}`
  const hit = wrapCache.get(key)
  if (hit !== undefined) return hit
  const out = wrapLineSpans(line, fontFamily, kashida, widthEm, weight, tracking).map((s) => s.text)
  wrapCache.set(key, out)
  return out
}

// The box a given content implies — the content plus the reflowed width/height
// (and x for point text, which keeps its writing-start edge fixed). The single
// source of truth for "what box does this text need", shared by the inline
// editor's live overlay + commit (TextOverlay), the resize handler (SkiaCanvas)
// and the point/area + height-mode toggles (PropertiesPanel), so every path
// agrees pixel-for-pixel. `s` is the anchor shape (its w/x/fontSize/flags), NOT
// necessarily the live-reflowed one.
//
// Crucially the wrap here is kashida-free (0), identical to what the renderer
// (layoutTextLines) and the exporter actually lay out — kashida only FILLS a
// line, it never forces a break. Wrapping with the shape's kashida over-counted
// lines and grew the box taller than the painted text (the bottom-gap bug).
export function computeTextReflow(s: TextShape, content: string): Partial<TextShape> {
  const patch: Partial<TextShape> = { content }
  if (!engineIsReady() || s.fontSize <= 0) return patch
  set_active_weight(s.weight ?? 400)
  set_active_tracking(s.tracking ?? 0)
  const leading = s.lineHeight && s.lineHeight > 0 ? s.lineHeight : 1.2
  if (s.autoWidth) {
    // Point text: width tracks the widest line (natural, no kashida); height
    // tracks the line count; the writing-start edge stays put (RTL = right).
    set_active_font(resolveEngineFontId(s.fontFamily ?? 0))
    let maxW = 0
    const logical = content.split('\n')
    for (const lg of logical) if (lg) maxW = Math.max(maxW, runWidthPerEm(lg, 0))
    const newW = Math.max(s.fontSize, Math.ceil(maxW * s.fontSize))
    patch.w = newW
    if ((s.textDir ?? 0) !== 1) patch.x = (s.x + s.w) - newW
    patch.h = Math.max(s.fontSize, Math.ceil(logical.length * s.fontSize * leading))
  } else if (s.autoHeight !== false) {
    // Auto-height area text: width stays; grow the box down to fit the wrapped
    // content (kashida-free wrap, see note above).
    const widthEm = s.w / s.fontSize
    let vlines = 0
    for (const lg of content.split('\n')) {
      vlines += lg ? wrapLine(lg, s.fontFamily ?? 0, 0, widthEm).length : 1
    }
    patch.h = Math.max(s.fontSize, Math.ceil(vlines * s.fontSize * leading))
  }
  // Fixed-height area text (autoHeight === false): the box height is the user's;
  // only the content changes — the text wraps + clips inside the fixed box.
  return patch
}
