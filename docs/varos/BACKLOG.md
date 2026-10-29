# Varos fork: backlog

Ordered. Owner feedback comes first. Each item has one acceptance line; an item is closed only when
its acceptance is met and recorded in `STATE.md`. Updated 2026-10-19, `main` = `3e81fe83`.

Tags: **[done, owner to look]** = built, gates green, waiting for the owner's eye in the real window.

1. **[done, owner to look] Home screen with no menus at all.** Owner, 2026-10-19: «خلي صفحة البداية القوائم متظهرش فيها
   خالص» ("on the start page, the menus must not appear at all"). Lane brand2 removed the in-window
   menus but kept the macOS "Varos" app menu with New/Open, and the owner says it is still not right.
   Landed as "Clean Home" (`lanes/home-report.txt`): Home shows only title bar + welcome page.
   Not yet re-verified by the owner; the lane could not do a native visual pass.
   *Acceptance:* on Home no menus appear, in the window or in the macOS menu bar (how far the
   macOS menu bar can be emptied is a design question for the owner); New/Open stay reachable from
   the Home page itself; menus return in the editor; owner accepts in the real window.
2. **[done, owner to look] Kashida deeper and automatic for any font.** Works for AAT fonts too (Geeza Pro, Al Nile,
   Baghdad, Damascus, KufiStandardGK), visibly stronger, with clear on/off buttons and a strength
   slider in the Paragraph panel; a `ParaStyle` attribute replaces the fixed Balanced `POLICY`.
   Landed as Off/Auto/Force + Strength with morx-font support (`lanes/kashida-ui-report.txt`).
   Open: Geeza Pro takes none with full harakat (not investigated); judge Force visually.
   *Acceptance:* the same justified Arabic paragraph shows kashida in Geeza Pro and SF Arabic, the
   slider visibly changes it, the setting saves and reloads; owner accepts.
3. **Cursors for the remaining states** (Type I-beam, plain crosshair family, others now on the
   system cursor), from a design source, never hand-drawn by an agent.
   *Acceptance:* every state in `crates/tools/src/cursors.toml` used on macOS maps to a Varos cursor
   or a deliberate system cursor; each new asset has an `ASSETS.md` row; owner accepts.
4. **A2 latency measurement and frame pacing.** Measure how far UI-frame overlays and readouts
   trail the pointer in a real window, then fix pacing if needed.
   *Acceptance:* a written measurement (method + numbers) before and after; owner says the pointer
   no longer lags.
5. **Tile-cache renderer** (tiled CPU raster + dirty-region cache, 2-4 weeks), then the GPU
   decision. *Acceptance:* a 20k-path document pans and zooms well under the current 64 ms/frame,
   measured with the same benchmark; GPU go/no-go written up for the owner.
6. **The remaining icons:** 77 Lucide imports and 69 icons to draw in Varos style.
   *Acceptance:* `icon-map.tsv` has no `EMPTY` rows left except deliberate ones; every asset in
   `ASSETS.md`; owner accepts the drawn set.
7. **Port the Varos colour picker, export sheet and control bar.**
   *Acceptance:* each works on VectorCraft's own commands/model, matches the Varos design, passes
   gates; owner accepts each piece separately.
8. **Bridge without pairing + agent presence over upstream's MCP.** Any same-user agent reads,
   edits and saves with no approval prompt; agent edits show on the canvas in orange. Built in
   `lane-agents` (commit `0d83c322`), not merged: security re-review pending first.
   *Acceptance:* an external agent edits an open document with no prompt, the owner sees it happen,
   undo reverts it.
9. **Weekly upstream merge routine.** Last base: `cdba0aac` (2026-10-18). *Acceptance:* `git merge upstream/main` done weekly, conflicts
   resolved, gates green, merge recorded (date, upstream commit, conflict count) in `STATE.md`.
10. **Publish checklist.** *Acceptance:* a written list covering the three bars in `FORK.md`
    (Arabic, performance, design), licences and `ASSETS.md` complete, `cargo xtask brands` clean,
    the `varos-text` dependency reachable without private SSH access (unverified today), name and
    trademark check; owner signs it off.
11. **Agents S4 verbs + S6 review UI** (after item 8 merges). *Acceptance:* S4 verbs and the S6
    review UI exist and pass gates; undo still reverts agent edits; owner accepts.
12. **Arabic follow-ups** (each one small; none started):
    - Middle Eastern composer: kashida-aware line breaking.
    - `jalt` (stylistic alternates) support.
    - Language tag `ar` / `fa` / `ur` on text.
    - Harakat vertical adjust.
    - Mixed-selection state for the Direction row and the harakat swatches (today they show the
      first paragraph / no state).
    - Harakat custom colour in the UI (today only 4 swatches; the command takes `#RRGGBB`).
    - **PDF copy-back bug:** ligature words (the Allah ligature) read back reversed; ignored test
      `a_word_with_a_ligature_reads_back_in_order` in `crates/pdf/tests/arabic_copy_back.rs`.
      *Acceptance:* the test is un-ignored and passes.
13. **Native-speaker review of translations.** The new Arabic Essentials, kashida and font-menu
    labels were machine-written into 23 catalogs. *Acceptance:* a native reader signs off each
    language, or the rows fall back to English on purpose.

## AI panel (owner request 2026-10-26)
See `docs/varos/AI_PANEL_PLAN.md` — A0 mockup → A1 Ask via Codex → A2 Claude Code → A3 Images with the user's own key → A4 edit/variations → A5 API agent loop → A6 History. Selling point: own model, own key, own files, no subscription.

## Deferred review findings — lane-ui-codex (Astra review 2026-10-27, P2, not fixed yet)
1. picker Gradient navigation uses `window.panel` (toggle) → can close an already-visible floating Gradient panel; use a show-only path.
2. tab_scroll: choosing the already-active document from the overflow list does not reveal its tab; scroll state survives a non-overflow resize.
3. picker header: Gradient button (x 98–184) overlaps Eyedropper (176–202) by 8 px.
4. clipping eligibility recomputed every frame in Properties + control bar; cache by (doc, revision, selection).
Owner direction 2026-10-27: no further investment in craft-ui chrome while the new UI is designed on the Varos box system; fix these only if cheap or when the new shell lands.

## Known after lane-arabic-codex (2026-10-27)
- PDF copy-back of a **stroked Force-kashida** word reads back with spaces at the unioned
  elongation bars ("بس م الرحم ن") — the continuous-outline fix (VQA-14) splits the text map.
  Test `crates/pdf/tests/arabic_copy_back.rs::stroked_force_kashida_keeps_authored_searchable_text`
  is `#[ignore]` until the text map spans the unioned glyphs. Unstroked text is unaffected.

## Colour picker integration — owner feedback 2026-10-27 («دي بايظة خالص»)
Seen in v14 (cards, old shell): the picker v3 sits inside a Color | Appearance card with a
"Gradient…" text button and an extra × in its mode row.
1. The colour panel must be a **modal/popover** opened from the colour well (Figma-like), closed
   by clicking outside — not a docked panel/menu with tabs.
2. **Gradient is a mode inside the picker**, in the same icon row as Wheel / Sliders / Harmony —
   not a "Gradient…" button that opens a separate Gradient panel.
3. Remove the extra × in the picker's mode row.
Owner-marked priority; its own lane right after shell stage 3 (one cargo lane on the Mac).

## New shell — known after stage 3 (2026-10-27)
- Collapse: an all-collapsed nested horizontal subtree with unequal heights (one leaf beside a
  vertical pair, above an expanded sibling) can leave a hole under the single leaf
  (`shell/tree.rs` `collapsed_height`/layout). Fix in stage 4 with a test.
- Collapse beside the Board is refused with a status line (no box above/below to take the room);
  the design notes promised collapse anywhere — revisit with the owner (narrow vertical tab strip?).

Picker lane implementation (2026-10-27, gates green, owner hand test pending): colour wells now
open a 240 pt frameless modal picker, independently of the docked Color panel, placed under the
Fill/Stroke pair (beside it when there is no room below). Gradient is an icon mode using the shared
Gradient editor; solid modes convert gradients back to a solid. The mode row has no close button
or Gradient navigation text. Headless interaction tests and software captures pass. No parity
score change. See `lanes/picker2-report.md`.

## 2026-10-29 — Arabic PDF lane 2 follow-ups
- PDF writer: krilla tagging is on by default and Varos builds no tag tree, so a span (`/Lang (ar)` on
  a mixed right-to-left line, `/ActualText` for a joiner) leaves MCIDs in the content and a page
  `/StructParents` with no `StructTreeRoot`. Valid enough for PDF/A-2b and PDF/X-4, but a strict
  preflight may warn. Write plain marked content, or a real tag tree.
- PDF writer: krilla writes spans only on the page surface. Inside a form XObject (PDF layers,
  overprint, invisible text for unpainted or pattern fills and bent glyphs) the `/Lang` and joiner
  `/ActualText` tags are dropped silently: the paint is right, copy-back degrades.
- PDF writer: `/Lang` is hard-coded `ar` for every right-to-left line, Persian and Hebrew included.
  It works as a direction flag but names the wrong language.
- PDF copy-back: a single-run right-to-left line whose visual start is Latin ("أنا أحب Rust") takes
  the unchanged run-index path with no `/Lang`, so our reader reads it left to right (wrong logical
  order). Unchanged from main.
- PDF reader: `/ActualText` now applies to every PDF. A span across several lines (a word hyphenated
  over a line break) places its string on the first line, and the later lines' glyphs in it come
  in as outlines.
- PDF reader: a run's piece stroked as one outline is held with its line, but `join_strokes` wants
  one stroke path per glyph, so those strokes come in as paths beside the type, not as its stroke.
- `crates/pdf/src/export.rs`: `glyph_text`'s doc comment moved onto `paint_visual` (with a
  duplicated `#[allow]`); `glyph_text` has none, and its `span` and `mark_rtl` are undocumented.

## 2026-10-29 — PDF visual-order z-order
- Mixed-run right-to-left PDF text is painted in visual order within a line (each run's piece, then
  its stroke). SVG export paints that line by run index. The orders differ where glyphs of different
  runs overlap. Those lines are marked `/Lang (ar)` so copy-back is the logical string when the
  visual line starts with left-to-right text.

## 2026-10-28 — cloud batch notes
- FIXED (#4): PDF copy-back of ligature words (Allah) and of stroked Force-kashida words; both tests un-ignored.
  Follow-ups from review: Auto-kashida fixture uses point text (no elongation) → add justified area text;
  mixed Arabic/Latin with punctuation and font changes untested; AAT fonts unverified on the cloud (no fonts);
  Hebrew/other RTL clusters share the mapping change, untested; fully vocalized Arabic extraction issue is
  pre-existing (documented in `crates/pdf/tests/fixtures/arabic-copy-back.md`).
- FIXED (#3): the 10 "known release-only failures" (debug-string ID lookups in test harnesses); the
  10k-text timing test now uses medians with a relative + absolute budget.
- Linux-only (cloud) debug failures seen by the lanes, not reproduced on the Mac: 8 tests panic dropping an
  uncleared `TexturesDelta` (epaint textures.rs:337) — clipping/picker/tab_scroll/tests_boards/tests_shell;
  investigate when a Linux build matters.
- Clippy with Rust 1.99 flags `single_element_loop` at `varos/shell/persist.rs:673` (fixed on lane-look).
