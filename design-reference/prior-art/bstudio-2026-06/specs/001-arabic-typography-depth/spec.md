# Feature Specification: Arabic Typography Depth + Universal Font Loading

**Feature Branch**: `001-arabic-typography-depth` (work happens on `ui-redesign-port`; no separate branch)

**Created**: 2026-06-10

**Status**: ACTIVE — un-parked 2026-06-10 PM4 (Ahmed: long autonomous run; 003 shipped)

**Input**: User description: "Arabic typography depth + universal font loading (the moat spec) — Google Fonts catalog + font file upload + device fonts, all shaping through the real WASM engine; kashida-support warning at load; harakat layer v1 (toggle + size + color); tracking; variable weight slider; canvas == export. Acceptance: Ahmed designs a real calligraphic poster himself inside the tool."

> **Constitution check**: This is the Wave-1 moat feature (Principle I). Every control it ships must work (Principle II). Done = live + Playwright/QA evidence (Principle IV). Fonts come from free/official sources (Principle V).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Bring any Arabic font into the engine (Priority: P1)

A designer opens the font picker and can get a font from three sources: (a) browse/search an in-app catalog of free Arabic Google Fonts with live preview, (b) upload a `.ttf`/`.otf` file from disk, (c) activate a font already installed on their device. Whichever source they choose, the text on canvas is shaped by BStudio's own Arabic engine — real contextual forms, ligatures, and kashida — not browser preview rendering.

**Why this priority**: This is Ahmed's core product call ("اقرا خطوط جهازي + جوجل + المواقع المجانية") and the gate to everything else: typography depth is worthless if designers are stuck with 5 bundled fonts. It also converts an existing dead control (device fonts are "preview-only" today) into a real one — paying down Principle-II debt.

**Independent Test**: Load one font from each of the three sources, type the same Arabic sentence in each, and verify all three render with correct contextual shaping on canvas (engine-shaped, not browser-shaped).

**Acceptance Scenarios**:

1. **Given** the font picker is open, **When** the user searches the Google Fonts catalog for an Arabic family (e.g. "Aref Ruqaa") and activates it, **Then** the font downloads, appears in the family list, and selected text reshapes with it on canvas via the engine.
2. **Given** the user has a `.ttf`/`.otf` file, **When** they upload it, **Then** it is validated, added to the family list, and shapes text on canvas via the engine.
3. **Given** device fonts are listed, **When** the user picks one, **Then** the canvas actually renders with it through the engine (no longer a silent preview-only no-op).
4. **Given** a font is being loaded (any source), **When** the engine detects it lacks safe tatweel/kashida insertion data, **Then** the font still loads and works, and the user sees a clear warning at load time that kashida features are unsupported for this font (Ahmed's decision: warn-at-load, don't block).
5. **Given** a document using a loaded font is saved and reopened (autosave or `.bs`), **Then** the same font renders again without the user re-loading it.

---

### User Story 2 - What you export is what you saw (Priority: P2)

A designer exports their Arabic artwork to SVG/PNG and the result is visually identical to the canvas — same glyphs, same kashida lengths, same font weight. The known divergence (export shapes kashida at default weight while canvas uses the set weight) is gone.

**Why this priority**: Constitutional requirement ("the kashida moat must survive export — one shaping path; divergence is a bug"). A poster that looks right on canvas but exports wrong destroys the moat's credibility — and the final demo (US5) depends on a correct export.

**Independent Test**: Create bold variable-weight Arabic text with kashida justification, export to SVG and PNG, and visually compare against a canvas screenshot — glyph outlines and kashida widths must match.

**Acceptance Scenarios**:

1. **Given** Arabic text set to a non-default variable weight with kashida applied, **When** it is exported to SVG, **Then** the exported outlines match the canvas shaping (same weight, same kashida insertion points and widths).
2. **Given** the same text, **When** exported to PNG at 1x/2x/3x, **Then** the raster matches the canvas rendering at the corresponding scale.

---

### User Story 3 - Real weight and tracking control (Priority: P3)

A designer selects Arabic text and adjusts: a continuous variable-font **weight slider** (not just 3 preset buttons) for fonts with a weight axis, and a **tracking (letter-spacing)** control. Both update the canvas live and persist with the document.

**Why this priority**: The engine already accepts arbitrary weights — the UI under-exposes it. This is the cheapest depth win and is needed for real poster work (display type lives on precise weight + spacing).

**Independent Test**: Load a variable Arabic font (e.g. Cairo), drag the weight slider through its range and set a tracking value; canvas updates live, and the values survive save/reload and export identically.

**Acceptance Scenarios**:

1. **Given** a text shape in a variable font, **When** the user drags the weight slider, **Then** the canvas reshapes live at that exact weight (advances and mark positions update — not a faux-bold).
2. **Given** a font WITHOUT a variable weight axis, **When** the user opens the weight control, **Then** it gracefully falls back to the font's available static weights only (no dead slider).
3. **Given** any Arabic text, **When** the user adjusts tracking, **Then** spacing between glyph clusters changes on canvas without breaking joining behavior (connected letters stay connected; tracking applies at safe boundaries).
4. **Given** weight/tracking set, **When** the document is exported or reloaded, **Then** the values are preserved exactly.

---

### User Story 4 - Harakat (tashkeel) layer v1 (Priority: P4)

A designer working with vocalized Arabic text can: toggle harakat visibility for the whole text element, scale harakat size relative to letters, and give harakat an independent color — turning diacritics into a designed element (as in classic calligraphic posters) rather than an all-or-nothing input artifact.

**Why this priority**: First visible "no other design tool does this" feature beyond kashida. Scoped to text-level controls (Ahmed's decision: toggle + size + color now; per-mark editing later).

**Independent Test**: Type a fully vocalized sentence; toggle harakat off/on, change their size and color; canvas reflects each change and export matches.

**Acceptance Scenarios**:

1. **Given** text containing harakat, **When** the user toggles "Show harakat" off, **Then** marks disappear from canvas (base letters and their shaping/kashida are unaffected) and reappear when toggled on.
2. **Given** harakat visible, **When** the user adjusts the harakat size slider, **Then** marks scale around their anchor positions without colliding with base glyphs at reasonable sizes.
3. **Given** harakat visible, **When** the user picks a harakat color different from the text color, **Then** marks render in that color on canvas and in SVG/PNG export.
4. **Given** harakat hidden, **When** the text is exported, **Then** the export matches the canvas (no hidden marks reappearing).

---

### User Story 5 - The proof: Ahmed designs a real poster (Priority: P5)

Ahmed — the designer, not a developer — designs a complete calligraphic Arabic poster entirely inside BStudio: he pulls fonts from Google Fonts / his device / an uploaded file, sets weights with the slider, applies tracking, controls harakat as a design element, stretches kashida, and exports a final SVG/PNG that matches the canvas. No step requires a developer or an external tool.

**Why this priority**: This is the feature's acceptance gate and the moat's first real validation (Ahmed's decision: he designs the demo himself). It is last because it depends on US1–US4.

**Independent Test**: A timed, unassisted design session by Ahmed producing a finished poster + matching export, with any friction points logged as review notes.

**Acceptance Scenarios**:

1. **Given** the live editor with US1–US4 shipped, **When** Ahmed designs a poster start-to-finish, **Then** he completes it without developer intervention and the exported file matches the canvas.
2. **Given** the finished poster, **When** its source document is reopened later (or on another machine via `.bs` file), **Then** it renders identically (fonts restored, all typography values intact).

---

### Edge Cases

- Uploaded file is corrupt / not a real font → friendly error, nothing added to the list, no crash.
- Uploaded/chosen font has no Arabic glyph coverage (e.g. Latin-only) → it loads and works for Latin text; selecting it for Arabic text shows a coverage notice (Arabic falls back visibly rather than rendering tofu silently).
- Font lacks safe tatweel data → warn at load (per Ahmed); kashida tools subsequently have no effect for that font and say why, rather than producing broken stretching.
- Variable font without a `wght` axis → weight slider replaced by static-weight choices (no inert slider — Principle II).
- Google Fonts catalog unreachable (offline/network error) → catalog shows a clear offline state; previously activated fonts keep working from local storage.
- Document references a font that is no longer available on this machine (device font on another computer, deleted upload) → text renders in a fallback with a visible "missing font" indicator and a one-click path to re-link/replace the font; document data is never lost.
- Harakat controls on text that contains no harakat → controls are present but clearly inactive for that text (or hidden), never silently dead.
- Tracking on mixed Arabic/Latin (bidi) text → both directions space correctly; Arabic joining is never broken mid-word.
- Very large font files (e.g. >10 MB CJK-ish fonts) → loading stays responsive (progress indication), and a sane size cap with a clear message protects autosave/storage.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The font picker MUST offer an in-app catalog of free, officially-licensed Arabic Google Fonts with search and per-family Arabic preview, activable with one action.
- **FR-002**: Users MUST be able to add a font by uploading a `.ttf` or `.otf` file, with validation and friendly failure for invalid files.
- **FR-003**: The existing device/system-font listing MUST become fully functional: choosing a device font renders it through the engine on canvas (replacing today's preview-only behavior — no dead control remains).
- **FR-004**: Fonts from ALL three sources MUST shape through the same Arabic engine pipeline used by bundled fonts (contextual forms, ligatures, kashida, marks) — never through browser-only rendering.
- **FR-005**: At font load time the system MUST detect whether the font supports safe kashida insertion; if not, it MUST still load the font and MUST show the user a clear non-blocking warning (load-time warning was Ahmed's explicit decision). Kashida tools MUST subsequently explain their inactivity for that font instead of failing silently or producing broken output.
- **FR-006**: Fonts in use MUST persist with the document: autosave and `.bs` save/load MUST restore the exact fonts (uploaded fonts embedded or re-linkable; missing fonts surfaced per the edge-case behavior, never silent substitution).
- **FR-007**: Text elements MUST expose a continuous weight control bound to the font's variable weight axis, applied through the engine so advances and mark positions match the rendered weight; fonts without the axis get their static weights only.
- **FR-008**: Text elements MUST expose a tracking (letter-spacing) control that preserves Arabic joining (spacing applies at safe boundaries only).
- **FR-009**: Text elements MUST expose harakat controls: show/hide toggle (whole element), size scale, and independent color — all reflected on canvas live.
- **FR-010**: Canvas and export (SVG + PNG at all offered scales) MUST produce identical shaping for the same text: same glyph forms, kashida insertions/widths, weight, tracking, and harakat state. The known weight divergence in kashida export MUST be fixed.
- **FR-011**: Every control introduced by this feature MUST be functional on the live build at ship time (Constitution Principle II — no dead controls, no "soon" placeholders).
- **FR-012**: All new typography values (font source/identity, weight, tracking, harakat settings) MUST round-trip through save/reload and be covered by automated end-to-end checks (Playwright on the live build and/or the VPS QA harness).

### Key Entities

- **Font Source**: where a family came from — `bundled` | `google-fonts` | `uploaded-file` | `device`. Determines persistence strategy (bundled ship with app; google re-fetchable by name; uploads embedded/stored; device re-linked by name with missing-font fallback).
- **Font Family record**: family name, source, file data or reference, declared weights / variable axes, Arabic coverage flag, kashida-support flag (safe-tatweel capability), license note (catalog fonts are free/OFL).
- **Text Typography state** (per text element): family reference, size, weight (continuous), tracking, line height, alignment/direction (existing), kashida settings (existing), harakat settings (visible, size scale, color).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A designer can find, activate, and type with a new Arabic font from each of the three sources in under 60 seconds per source, with correct Arabic shaping on first render.
- **SC-002**: For a test poster using a variable weight + kashida + harakat + tracking, the SVG/PNG export is visually indistinguishable from the canvas (side-by-side comparison shows no shaping, weight, or spacing differences).
- **SC-003**: Ahmed completes a real calligraphic poster entirely inside the tool, unassisted, and signs it off as the reference demo (the feature's acceptance gate).
- **SC-004**: Zero dead controls shipped: 100% of the new controls visibly change the canvas when used, verified control-by-control on the live build.
- **SC-005**: Automated coverage exists and passes: end-to-end checks for font loading (≥1 per source), weight slider effect, tracking effect, harakat toggle/size/color, and a canvas-vs-export fidelity check.
- **SC-006**: A document using loaded fonts reopens correctly after reload and on a fresh session (fonts restored or explicitly flagged missing — never silently substituted).

## Assumptions

- Google Fonts' Arabic families are free to bundle/fetch under their open licenses (OFL); the catalog lists only such fonts. No paid/foundry licensing is in scope.
- The primary browser target supports the device-font listing capability already used by the existing preview-only picker (Chromium-based; the editor's current baseline). Browsers without it simply don't show the device source.
- The engine can accept additional font binaries at runtime (it already loads multiple bundled families); confirming the exact mechanism is plan-phase work, not a spec risk.
- Harakat come from the text content itself (the user types vocalized text); automatic tashkeel (AI vocalization) is OUT of scope for this feature.
- Per-mark harakat editing (move/delete individual marks) is OUT of scope for v1 (Ahmed chose toggle+size+color now, per-mark later).
- Calligraphic styles needing OpenType `jalt`/stylistic-set work, multi-line justification solver, and the Parley decision remain SEPARATE backlog items (B24, B27/B28, B49) — this spec deepens the panel and font intake, not the line-layout engine.
- The poster demo (US5) doubles as the Phase-4 validation artifact from the compass; showing it to external Arabic designers happens after Ahmed's own sign-off and is outside this spec.
