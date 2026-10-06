> **Status:** reference — harvested 2026-10-07 from the June-2026 web prototype so the Text tool does not start from zero.

# Prior art: the kashida engine and Arabic controls from the BStudio/Varos web prototype (June 2026)

Owner (2026-10-07): «في النسخ القديمة أنظمة كشيدة وحاجات كتير اشتغلنا عليها… منبدأش العجلة تاني من الصفر». This note records what already exists in this repository and what carries over into ADR-0010 / P1b / the justification gate.

## Where it lives

- `design-reference/editor-redesign/editor-model.jsx` — `kashidaText(base, amount)`: the working kashida rules (JS, string-level). Committed in `326d99a` (2026-06-27, "Pro panels").
- `design-reference/editor-redesign/Inspector.jsx` ~145–155 — the **"Arabic — the moat"** inspector group: *Kashida elongation* slider 0–100 % with a mono readout, caption `Naskh … auto-justify`.
- `design-reference/editor-redesign/EditorPro.jsx` ~201, ~219 — text renders through `kashidaText`; **Type** menu rows *Justify with kashida* / *Reset kashida*.
- `design-reference/design-system.md` ~100 — the Arabic font set used then: Amiri, Cairo, IBM Plex Sans Arabic, Tajawal, Noto Sans Arabic.
- History docs: `docs/history/MASTER_PLAN_V1_LAUNCH.md` §10 (kashida deferred by owner decision 2026-07-07; "smart kashida = the real innovation, justification by kashida with rules, not word-spacing"), `docs/history/DETAILED_ROADMAP.md` 2.2.d / 15.3, `docs/reference/ELEMENTS_CATALOG.md` (kashida/harakat controls as explicit items).
- The server (`srv`) `~/VAROS/varos` is empty; no other copy of this work was found there (2026-10-07 scan).

## The rules the prototype encoded (carry over as requirements)

1. **Slot = a connector letter whose next base letter is Arabic, not alef, not a space.** Connectors list in the prototype: `ب ت ث ن ي ئ س ش ص ض ط ظ ف ق ك ل م ه ع غ ح ج خ` (letters that join to the left). Final letters, non-joiners (ا د ذ ر ز و) and word ends are never slots.
2. **Elongation goes on the connection AFTER the full letter + its marks cluster** (harakat, tanwin, shadda, sukun, superscript alef `U+064B–U+065F, U+0670`), never between a letter and its diacritic.
3. **Never into a lām-alif** (`لا` and its hamza/madda forms) — the next base being alef excludes the slot.
4. **Amount is a single 0–100 % control per text object**; the prototype mapped it to 0–3 units per slot and used a stride (every other slot below ~55 %) so low amounts spread elongation sparsely instead of thinly everywhere.
5. **UX:** one slider in the text inspector under an "Arabic" group, a numeric %, and two menu actions (justify with kashida / reset). This is the owner-approved shape of the control; the final look still needs a mockup under the current design system.

## What does NOT carry over (and why)

- The prototype inserted **literal tatweel characters (U+0640)** into the string. ADR-0010 rejects that for production: it mutates the author's text, breaks copy/search, and cannot produce font-correct curved elongation. The rules above must be re-implemented at the **glyph level** (OpenType `jalt`/`cswh` alternates or kashida glyph insertion between shaped glyphs, with the font's own tatweel glyph and metrics), after P1b and the area-text piece (P4), as the separate justification research gate the ADR already names.
- The browser did shaping; Varos's engine (cosmic-text + HarfRust) must expose the slot information itself (cluster map + joining classes) — P1b's caret/cluster work gives us that map.

## How to use this

- P1b: add the connector/slot rules as a **pure Rust function over the shaped cluster map** with the prototype's cases as tests (the لا exclusion, marks clusters, non-joiners) — no insertion yet, just "where may elongation go, and how much".
- P4/P5: the inspector group and the two Type-menu rows go into the Properties/Type mockups as named requirements.
- Fonts: the five-font set is a candidate list for the owner's visual choice (licences to audit; Plex Arabic is already bundled).
