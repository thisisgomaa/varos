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
