> **Status:** reference — harvested 2026-10-07 from the June-2026 web prototype so the Text tool does not start from zero.

# Prior art: the kashida engine and Arabic controls from the BStudio/Varos web prototype (June 2026)

Owner (2026-10-07): «في النسخ القديمة أنظمة كشيدة وحاجات كتير اشتغلنا عليها… منبدأش العجلة تاني من الصفر». This note records what already exists in this repository and what carries over into ADR-0010 / P1b / the justification gate.

## Where it lives

- `design-reference/editor-redesign/editor-model.jsx` — `kashidaText(base, amount)`: the working kashida rules (JS, string-level). Committed in `326d99a` (2026-06-27, "Pro panels").
- `design-reference/editor-redesign/Inspector.jsx` ~145–155 — the **"Arabic — the moat"** inspector group: *Kashida elongation* slider 0–100 % with a mono readout, caption `Naskh … auto-justify`.
- `design-reference/editor-redesign/EditorPro.jsx` ~201, ~219 — text renders through `kashidaText`; **Type** menu rows *Justify with kashida* / *Reset kashida*.
- `design-reference/design-system.md` ~100 — the Arabic font set used then: Amiri, Cairo, IBM Plex Sans Arabic, Tajawal, Noto Sans Arabic.
- History docs: `docs/history/MASTER_PLAN_V1_LAUNCH.md` §10 (kashida deferred by owner decision 2026-07-07; "smart kashida = the real innovation, justification by kashida with rules, not word-spacing"), `docs/history/DETAILED_ROADMAP.md` 2.2.d / 15.3, `docs/reference/ELEMENTS_CATALOG.md` (kashida/harakat controls as explicit items).
- **Discovery 2026-10-07:** `/root/wt/wt-w8-engine` contains the real Rust BStudio engine (`v0/crates/bstudio-text`; later `v1` restructure), specs and research. The earlier empty `~/VAROS/varos` scan did not establish absence elsewhere.
- Server frontend backup: `/root/backups/v0-frontend-2026-06-16.tar.gz` — `web/bstudio/src/{textLayout.ts,TextOverlay.tsx}` and archived `editor-tldraw/src/KashidaSidebar.tsx`.
- Owner typography rules: `/root/shared_workspace/company_os/FIGMA_ARABIC_RULES.md`.
- Local read-only discovery root: `/private/tmp/claude-501/-Users-gomaa-Documents-AI-workspace-varos/90bf49a1-794a-43e8-b7df-7f72ceea7227/scratchpad/prior-art/`; engine under `wt-w8-engine/`, extracted frontend under `v0/web/`. Server paths above are owner-supplied, not remotely reverified here.
- Durable, checksummed MPL reference: [BStudio archive](../../../design-reference/prior-art/bstudio-2026-06/README.md); measured comparison and recommendation: [TEXT_PRIOR_ART_EVALUATION.md](TEXT_PRIOR_ART_EVALUATION.md).

## The rules the prototype encoded (carry over as requirements)

1. **Slot = a connector letter whose next base letter is Arabic, not alef, not a space.** Connectors list in the prototype: `ب ت ث ن ي ئ س ش ص ض ط ظ ف ق ك ل م ه ع غ ح ج خ` (letters that join to the left). Final letters, non-joiners (ا د ذ ر ز و) and word ends are never slots.
2. **Elongation goes on the connection AFTER the full letter + its marks cluster** (harakat, tanwin, shadda, sukun, superscript alef `U+064B–U+065F, U+0670`), never between a letter and its diacritic.
3. **Never into a lām-alif** (`لا` and its hamza/madda forms) — the next base being alef excludes the slot.
4. **Amount is a single 0–100 % control per text object**; the prototype mapped it to 0–3 units per slot and used a stride (every other slot below ~55 %) so low amounts spread elongation sparsely instead of thinly everywhere.
5. **UX:** one slider in the text inspector under an "Arabic" group, a numeric %, and two menu actions (justify with kashida / reset). This is the owner-approved shape of the control; the final look still needs a mockup under the current design system.

## What does NOT carry over (and why)

- **Correction after source inspection:** JS `kashidaText(base, amount)` returns a derived display string; its `EditorPro` caller does not overwrite `s.text`. Rust v0 `insert_kashida(&str, …)` also preserves the source, inserts U+0640 into a separate **working string**, then reshapes with HarfRust. It is not source-text mutation. However, output clusters index the expanded working string: exact source↔working mapping, caret/selection/copy and font-quality proofs are still missing. Neither working-string insertion nor direct glyph insertion is automatically approved by ADR-0010; arbitrary author-text insertion remains rejected, and a shaped-run implementation must satisfy the separate justification gate. Width growth alone is not proof of ligature/mark safety.
- The small JSX mockup delegated shaping to the browser; the newly recovered Rust engine did real HarfRust shaping and exposed SAFE_TO_INSERT_TATWEEL plus joining filters. Preserve that distinction. Port the tested rules/fixtures through the chosen engine's shaped-cluster seam; the BStudio single-valued scalar caret implementation cannot replace Varos's grapheme + affinity contract verbatim.

## How to use this

- P1b: add the connector/slot rules as a **pure Rust function over the shaped cluster map** with the prototype's cases as tests (the لا exclusion, marks clusters, non-joiners) — no insertion yet, just "where may elongation go, and how much".
- P4/P5: the inspector group and the two Type-menu rows go into the Properties/Type mockups as named requirements.
- Fonts: the five-font set is a candidate list for the owner's visual choice (licences to audit; Plex Arabic is already bundled).
