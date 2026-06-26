// shared.jsx — icon set + the canvas artwork shared by all three directions.
// Exports to window: Icon, CanvasArt.
const { createElement: h } = React;

// ── Icon set ─────────────────────────────────────────────────────────
// Stroke icons (1.6 weight) keyed by name. fill:none, stroke:currentColor.
const PATHS = {
  cursor:   'M4 3l8 14 1.8-5.6L19 9 4 3z',
  rect:     null, // drawn specially
  ellipse:  null,
  line:     'M4 20L20 4',
  pen:      'M14 3l7 7-9 9-5 1 1-5 6-12z M13 4l7 7',
  text:     'M5 5h14M12 5v15M9 20h6',
  hand:     'M8 11V5.5a1.5 1.5 0 013 0V11M11 11V4a1.5 1.5 0 013 0v7M14 11V5.5a1.5 1.5 0 013 0V11M5 12c0-1.4 1.6-1.4 1.6 0V15a5 5 0 005 5h2.4a5 5 0 005-5V8a1.5 1.5 0 013 0',
  frame:    'M8 3v18M16 3v18M3 8h18M3 16h18',
  polygon:  'M12 3l8 6-3 9.5H7L4 9z',
  star:     'M12 3l2.6 5.6 6.4.8-4.6 4.4 1.2 6.2L12 17l-5.6 3 1.2-6.2L3 9.4l6.4-.8z',
  zoom:     'M11 11m-7 0a7 7 0 1014 0 7 7 0 10-14 0M16 16l5 5M8 11h6M11 8v6',
  undo:     'M9 7L4 11l5 4M4 11h11a5 5 0 010 10h-4',
  redo:     'M15 7l5 4-5 4M20 11H9a5 5 0 000 10h4',
  save:     'M5 3h11l3 3v15H5zM8 3v6h8V3M8 14h8v7H8z',
  open:     'M3 6h6l2 3h10v10H3z',
  layers:   'M12 3l9 5-9 5-9-5 9-5zM3 13l9 5 9-5M3 17l9 5 9-5',
  search:   'M10 10m-6 0a6 6 0 1012 0 6 6 0 10-12 0M15 15l5 5',
  plus:     'M12 5v14M5 12h14',
  share:    'M7 12m-2.5 0a2.5 2.5 0 105 0 2.5 2.5 0 10-5 0M17 6m-2.5 0a2.5 2.5 0 105 0 2.5 2.5 0 10-5 0M17 18m-2.5 0a2.5 2.5 0 105 0 2.5 2.5 0 10-5 0M9.2 10.8l5.6-3.4M9.2 13.2l5.6 3.4',
  settings: 'M12 9a3 3 0 100 6 3 3 0 000-6M19.4 13a1.6 1.6 0 00.3 1.8l.1.1a2 2 0 11-2.8 2.8l-.1-.1a1.6 1.6 0 00-2.7 1.1V21a2 2 0 11-4 0v-.2a1.6 1.6 0 00-2.7-1.1l-.1.1a2 2 0 11-2.8-2.8l.1-.1A1.6 1.6 0 004.6 13H4.4a2 2 0 110-4h.2A1.6 1.6 0 006 6.3l-.1-.1a2 2 0 112.8-2.8l.1.1A1.6 1.6 0 0011.4 4.6V4.4a2 2 0 014 0v.2a1.6 1.6 0 002.7 1.1l.1-.1a2 2 0 112.8 2.8l-.1.1a1.6 1.6 0 00.3 1.8',
  comment:  'M4 5h16v11H9l-4 4V5z',
  eye:      'M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7zM12 9a3 3 0 100 6 3 3 0 000-6z',
  lock:     'M6 11V8a6 6 0 0112 0v3M5 11h14v10H5z',
  chevDown: 'M5 8l7 7 7-7',
  chevR:    'M9 5l7 7-7 7',
  more:     'M5 12h.01M12 12h.01M19 12h.01',
  fitView:  'M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5',
  alL:'M3 3v18M7 6h7v4H7zM7 14h11v4H7z',
  alCX:'M12 3v18M8 6h8v4H8zM6 14h12v4H6z',
  alR:'M21 3v18M10 6h7v4h-7zM6 14h11v4H6z',
  alT:'M3 3h18M6 7h4v7H6zM14 7h4v11h-4z',
  alCY:'M3 12h18M6 8h4v8H6zM14 6h4v12h-4z',
  alB:'M3 21h18M6 10h4v7H6zM14 6h4v11h-4z',
  distH:'M4 4v16M20 4v16M9 8h6v8H9z',
  distV:'M4 4h16M4 20h16M8 9v6h8V9z',
  boolU:'M4 4h10v6h6v10H10v-6H4z',
  sparkle:'M12 3l1.8 5.4L19 10l-5.2 1.6L12 17l-1.8-5.4L5 10l5.2-1.6zM18 15l.8 2.2L21 18l-2.2.8L18 21l-.8-2.2L15 18l2.2-.8z',
  send:'M4 12l16-8-6 16-3-7-7-1z',
  wand:'M5 19l9-9M14 5l1 2 2 1-2 1-1 2-1-2-2-1 2-1zM18 12l.6 1.4L20 14l-1.4.6L18 16l-.6-1.4L16 14l1.4-.6z',
  kashida:'M3 12h18M3 12c2 0 2-3 4-3M21 12c-2 0-2 3-4 3',
  grid:'M4 4h16v16H4zM4 10h16M4 16h16M10 4v16M16 4v16',
  history:'M12 7v5l3 2M12 3a9 9 0 109 9M3 6V3h3',
};

function Icon({ name, size = 18, sw = 1.6, fill = 'none', style }) {
  const common = { width: size, height: size, viewBox: '0 0 24 24', fill, stroke: 'currentColor',
    strokeWidth: sw, strokeLinecap: 'round', strokeLinejoin: 'round', style, 'aria-hidden': true };
  if (name === 'rect') return h('svg', common, h('rect', { x: 4, y: 5, width: 16, height: 14, rx: 1.5 }));
  if (name === 'ellipse') return h('svg', common, h('ellipse', { cx: 12, cy: 12, rx: 9, ry: 7 }));
  return h('svg', common, h('path', { d: PATHS[name] || '' }));
}

// ── Canvas artwork ───────────────────────────────────────────────────
// The same poster, selection and workspace beneath every direction's chrome.
// `pad` lets a direction inset the stage so chrome doesn't cover the poster.
function CanvasArt({ pad = {}, showRulers = false, scale = 1 }) {
  return h('div', { className: 'bs-workspace' },
    showRulers && h('div', { className: 'bs-rulers' }),
    h('div', { className: 'bs-stage', style: { padding: '40px', ...pad } },
      h('div', { className: 'bs-art-wrap', style: { transform: `scale(${scale})` } },
        h('div', { className: 'bs-art-label' },
          h('span', { className: 'bs-art-dot' }),
          h('b', null, 'Poster'),
          ' · ', '1080 × 1350'),
        h('div', { className: 'bs-poster' },
          h('div', { className: 'bs-poster-pad' },
            h('div', { className: 'kicker' }, 'معرض الخط ٢٠٢٦'),
            h('div', { className: 'ar-head' }, 'جَمـــالُ الحَرْفِ'),
            h('div', { className: 'rule' }),
            h('div', { className: 'ar-sub' }, 'نظامُ تصميمٍ عربيٌّ أوّلاً، بكشيدةٍ صحيحةٍ من النواة.'),
            h('div', { className: 'latin' }, 'Arabic-First Type'),
          ),
          h('div', { className: 'bs-sel-block' }),
        ),
        // selection chrome over the gold block (matches .bs-sel-block geometry)
        h('div', { className: 'bs-selection', style: { left: 46, bottom: 54, width: 150, height: 64 } },
          h('div', { className: 'bs-handle tl' }), h('div', { className: 'bs-handle tr' }),
          h('div', { className: 'bs-handle bl' }), h('div', { className: 'bs-handle br' }),
          h('div', { className: 'bs-dim-badge bs-mono' }, '150 × 64'),
        ),
      ),
    ),
  );
}

Object.assign(window, { Icon, CanvasArt });
