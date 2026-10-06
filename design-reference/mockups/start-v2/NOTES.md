# A v2 — Workbench, board-first

> **Removed 2026-10-06:** band Search and the Custom… artboard preset were removed in `736feba` by owner decision. Search filtering was subsequently removed from the Start model in cycle-close cleanup. References and mockups below preserve the original approved direction; they do not request rebuilding these controls. New board still opens a free canvas. Recovery is now the card / in-place Review (`1532637`), and its status follows the real setting.
**Idea.** Home opens a BOARD: one free canvas with a name, a short description and tags. Artboards inside are optional. "New board ⌘N" is the one confident (inverted, light) control. The artboard presets are a secondary panel ("…or start with an artboard"). Recent boards are cards that show the work itself on the canvas: the dot grid from the editor, the artboards on it, or free artwork bounds when a board has no artboard.

**Rail removed (full width).** The v1 rail only repeated Recent/Recovered. A tag list in it would duplicate the tag filter and break the ONE-HOME rule. So the filter row is the single home for tags, the recovery status moved to the status seam, and the board gets the full width.

**Grid.** The window is 1512×982. Bar 28 (SEAM, unchanged; V mark kept). One BG box at x12 y40, 1488×910, with an 8 px radius and a LINE border. The status line sits in the void below it. Padding 32/40 gives 1408 of content, split into 5 columns of 272 with 12 px gutters (the seam number). Hero, 152 tall: actions take cols 1–2 (556: two 272×64 buttons, lede, key hints). The preset panel takes cols 3–5 (840). Below the hero: 28 → recovered band 52 → 28 → head/filter row 28 → 16 → grid of 2 rows of 266-tall cards (123 well + 143 meta), 12 apart. Presets share one scale (1920 units = 56 px) on one baseline. Rhythm is 4/8 throughout.

**Type scale** (weights 600 headings/names/primary, 500 buttons/presets/recovered name, 400 text; mono and dates use tabular-nums):
- Inter + JetBrains Mono: h1 30 · h2 18 · button 15 · board name 14 · body 13 · small 12 · tag 11 · mono 11; tracking −0.02em on headings.
- Geist + Geist Mono: same sizes; −0.025em on headings.
- Plex Sans var + Plex Mono: h1 32 · h2 19 · button 16 · name 15 · body 13.5 · small 12.5 · tag 11.5 · mono 11.5; −0.015em. Plex has a smaller x-height, so it needs about +0.5–1 px.

**Elements.** Top bar: Search reads "Search boards" (it filters the cards/table). Recovered band: name, time, path, Discard/Recover. Head row: "Recent boards 10", tag filter (All · client 5 · personal 3 …), grid/list toggle. Only the selected filter gets the 2 px azure bar; tags are SURFACE pills with MUTED text. Card: name (largest), date, optional 2-line description, tags + "2 artboards / 1 artboard / free", folder path. Hover shows LINE2 + the "…" chip. Menu: Locate… (only when Missing) / Remove from Recent. Focus: 2 px azure ring with a 1 px gap. List: numbered table (# · Name + description · Tags · Folder · Modified). Empty / first launch: centred "Start with a board", lede, the two big buttons, preset panel. No drop-zone box.

**New engineering.**
1. Board metadata (name, description, tags) is an addition to the .vrs schema, so it needs a versioned bump per ADR-0004, plus somewhere to edit it (the board's own Section). 2. The recents store must cache name, description, tags and artboard count so Home never parses files. 3. Tag filter plus Search over that cache. 4. Thumbnails: render the board's content bounds to a small texture on save, cached by path+mtime. Until then, cards fall back to v1's typographic placeholder. 5. Presets/Custom… and "New board" = a document with zero artboards (the canvas must allow it). 6. List view. 7. Fonts: egui (ab_glyph) draws only a variable font's default instance. Ship static Regular/Medium/SemiBold cuts made with fonttools. 8. Middle elision for paths.

**Tokens.** No new colours. Layout: START_COL 272, START_GUTTER 12, CARD_H 266, WELL_H 123, HERO_H 152, BIG_BTN 272×64, PRESET_SCALE 56/1920. The primary button uses TEXT as its fill and #5c5753 for its secondary text.

**Font recommendation: IBM Plex Sans (with real weights) + Plex Mono.**
1. It is the only family with a matching Arabic companion, and Plex Sans Arabic is already bundled. That is the product's long-term moat. 2. Its warmer, slightly humanist letterforms suit the warm-black ramp better than Inter's neutrality. 3. Plex Mono's figures and paths read clearly at 11.5.

Runner-up is Inter: the best screen legibility at 11–13 px and the tightest UI feel, but it is generic and would need a separate Arabic pairing. Geist looks crisp and modern, but it is narrower at small sizes, has no Arabic, and is the house face of a web company.
