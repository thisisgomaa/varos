// editor-model.jsx — shape data + helpers for the BStudio interactive editor.
// Exports: INITIAL_SHAPES, ARTBOARD, newShape, kashidaText, fmt, clamp.

// Poster design space (matches a real 1080×1350 social poster).
const ARTBOARD = { w: 1080, h: 1350, name: 'Poster', dim: '1080 × 1350' };

const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
const fmt = (n) => Math.round(n).toString();

// The starting poster, broken into real selectable objects (top-left origin, design units).
const INITIAL_SHAPES = [
  { id: 'kicker', type: 'text', name: 'Kicker', x: 150, y: 150, w: 810, h: 48,
    text: 'معرض الخط ٢٠٢٦', font: 'var(--font-ar)', size: 30, weight: 500, color: '#9a8f7a',
    align: 'right', rtl: true, ls: 0, lh: 1.4, visible: true },
  { id: 'heading', type: 'text', name: 'Heading', x: 150, y: 330, w: 810, h: 360,
    text: 'جَمالُ الحَرْف', font: 'var(--font-ar-serif)', size: 188, weight: 700, color: '#1c1714',
    align: 'right', rtl: true, ls: 0, lh: 1.16, visible: true, kashida: 0 },
  { id: 'rule', type: 'rect', name: 'Divider', x: 728, y: 712, w: 232, h: 8,
    fill: '#1c1714', radius: 0, opacity: 1, visible: true },
  { id: 'subhead', type: 'text', name: 'Subhead', x: 150, y: 770, w: 810, h: 150,
    text: 'نظامُ تصميمٍ عربيٌّ أوّلاً، بكشيدةٍ صحيحةٍ من النواة.', font: 'var(--font-ar)', size: 46,
    weight: 500, color: '#5d5854', align: 'right', rtl: true, ls: 0, lh: 1.7, visible: true },
  { id: 'accent', type: 'rect', name: 'Accent block', x: 150, y: 1126, w: 360, h: 150,
    fill: '#2f8fe0', radius: 6, opacity: 1, visible: true },
  { id: 'latin', type: 'text', name: 'Latin label', x: 540, y: 1196, w: 420, h: 44,
    text: 'ARABIC-FIRST TYPE', font: 'var(--font-ui)', size: 28, weight: 600, color: '#9a8f7a',
    align: 'right', rtl: false, ls: 0.14, lh: 1.3, visible: true },
];

let _seq = 1;
function newShape(type, x, y, w, h, accent) {
  const id = `${type}-${_seq++}`;
  if (type === 'text') {
    return { id, type, name: 'Text', x, y, w: Math.max(w, 200), h: Math.max(h, 60),
      text: 'نصّ جديد', font: 'var(--font-ar)', size: 64, weight: 500, color: '#1c1714',
      align: 'right', rtl: true, ls: 0, lh: 1.4, visible: true };
  }
  return { id, type, name: type === 'ellipse' ? 'Ellipse' : 'Rectangle', x, y, w, h,
    fill: accent || '#2f8fe0', radius: type === 'ellipse' ? 0 : 8, opacity: 1, visible: true };
}

// Insert kashida (tatweel ـ) on the CONNECTION after a base letter + its marks,
// proportional to amount 0..100. Never elongates into a lām-alif (لا) and never
// splits a letter from its harakat — the tatweel always follows the full
// letter+diacritic cluster, which is the typographically correct join point.
const KASHIDA = '\u0640';
const MARK = /[\u064B-\u065F\u0670]/;            // harakat, tanwin, shadda, sukun, superscript alef
const ARABIC = /[\u0600-\u06FF]/;
const CONNECTORS = 'بتثنيئسشصضطظفقكلمهعغحجخ';     // letters that connect to the left
// index of the next base (non-mark) character at or after i
function nextBase(chars, i) { while (i < chars.length && MARK.test(chars[i])) i++; return i; }
function kashidaText(base, amount) {
  if (!amount) return base;
  const chars = [...base];
  // collect valid connection slots: a connector whose following base letter is connectable
  const slots = [];
  for (let i = 0; i < chars.length; i++) {
    if (!CONNECTORS.includes(chars[i])) continue;
    const j = nextBase(chars, i + 1);                 // skip this letter's marks
    const nb = chars[j];
    if (nb && ARABIC.test(nb) && nb !== '\u0627' && nb !== ' ') slots.push(i);
  }
  if (!slots.length) return base;
  const reps = Math.round((amount / 100) * 3);        // 0..3 tatweels per slot
  const stride = amount < 55 ? 2 : 1;                 // sparser at low amounts
  let out = '';
  for (let i = 0; i < chars.length; i++) {
    out += chars[i];
    const idx = slots.indexOf(i);
    if (idx !== -1 && reps > 0 && idx % stride === 0) {
      // emit this letter's trailing marks BEFORE the tatweel, then the tatweel
      let j = i + 1;
      while (j < chars.length && MARK.test(chars[j])) { out += chars[j]; j++; }
      out += KASHIDA.repeat(reps);
      i = j - 1;                                       // marks already emitted
    }
  }
  return out;
}

Object.assign(window, { INITIAL_SHAPES, ARTBOARD, newShape, kashidaText, fmt, clamp });
