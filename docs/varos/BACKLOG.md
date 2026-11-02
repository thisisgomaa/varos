# Varos fork: backlog

Ordered. Owner feedback comes first. Each item has one acceptance line; an item is closed only when
its acceptance is met and recorded in `STATE.md`. Updated 2026-11-02; older dated entries below retain their original context.

Tags: **[done, owner to look]** = built, local gates green, waiting for the owner's eye in the real
window. Same meaning as **provisional (awaits owner)** in `STATE.md`. GitHub CI has never run on this
project; "gates green" always means the local commands in `AGENTS.md`.

## Summary — 2026-11-02

| Item | Status |
|---|---|
| Panel system (unified panel workspace, Tools and Control bars, side Color panel, tab reordering, modeless Preferences) | **done, provisional (awaits owner)**: built and committed locally, review findings fixed; owner hand test pending (section below) |
| Upstream merge | **pending**: 975 upstream non-merge commits (1,468 with merges) since `1cdc9873`; 86 predicted conflicted paths, 127 hunks (staged plan below) |
| Repository unification | in progress: this repository's code replaces the public `thisisgomaa/varos`; the earlier engine moves to `legacy-engine` |

## Upstream merge — pending, staged plan (scan of 2026-11-02)

Measured with `git merge-tree` against upstream `639c9d6ce` (2026-11-01); nothing merged yet.

- **Why staged:** upstream PR #4294 (merged 2026-10-26, right after our last merge) is 168
  mechanical commits that split large files into module folders (882 files). Varos hook lines live
  in several of the emptied files (`ui-egui/src/{chrome,toolbar,lib,menus,canvas}.rs`,
  `engine/cmd/fileio/mod.rs`, `pdf/src/export.rs`, `text/src/layout.rs`, `mcp/src/tools.rs`,
  `apps/vectorcraft/src/control_server.rs`). Git reports only textual conflicts; hooks in emptied
  files must be re-homed by hand and the result must build under upstream's stricter workspace
  lints (`wildcard_imports`, `too_many_arguments`, `cognitive_complexity` = deny).
- **Where conflicts fall (86 paths):** 25 i18n catalogs and 9 docs/packaging/lockfile files are
  mechanical; about 60 need hand resolution (22 ui-egui, 17 core incl. `format/lib.rs` and the
  deleted `fileio/mod.rs`, 6 text, 4 pdf, 3 apps).
- **Order:**
  0. *Clean picks first* (simulate with zero conflicts), after the panel branch reaches `main`:
     undo-scope fix `389dce727`, shadow/blur raster cache `747050511`, placed-doc redraw
     `1c4ec81ec`, `.ai` RTL paragraphs `2dcfbb0bf`, MCP reader hardening `de6b14b54`, `76e0e5b0a`.
  1. *Stage A*: `1cdc98735..5815205324`, 28 commits, 9 predicted conflicted files.
  2. *Stage B*: the refactor PR alone (`5815205324..eb0e99a55`), 166 commits, 36 files / 45 hunks,
     nearly all relocations; one lane, one build + test + clippy gate before anything else.
  3. *Stage C*: `eb0e99a55..upstream/main`, 781 commits, in 3-4 batches: engine/format/import
     hardening first (port the native-file read budgets and "beyond canvas" check to every page of
     the v5 Pages loader, with a v5 round-trip test), then type/text (language shaping `locl`, marks
     kept in one fallback font, lam-alef not an alternate glyph, keyboard-layout shortcut fixes),
     then UI behaviours, re-implemented on the Varos shell where they collide with it.
- **Skip or defer:** Windows/Linux-only commits, Flatpak/installer/CI changes, the Contextual Task
  Bar, panel function-key behaviour, Control-bar Distribute buttons, ArtCraft branding text in
  README/NOTICE.
- **Kashida is not affected:** `crates/text/src/kashida.rs` is unchanged upstream since our base.
- **Acceptance:** each stage merged in its own worktree, local gates green
  (`cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, then
  `cargo xtask ci`), merge recorded in `STATE.md` (date, upstream commit, conflict count), owner
  hand test of the shell after Stage B and after each UI batch.

## Current owner priority — 2026-11-02

**Current mode: native panel workspace built and committed locally (not pushed); owner hand test
pending.** Builds 20261102.13–.16 implemented the unified panel workspace (groups, docking at four
edges, floating, Expanded / icon+name / icon-only, Tools and Control as special bars), the side Color
panel separated from the toolbar's wheel picker, horizontal document-tab reordering, and a modeless
Preferences with Apply. Two read-only reviews (Codex Sol, Cursor/Grok, 2026-11-02) requested changes;
all twelve findings were fixed or answered with evidence, each with a test that runs with the panel
workspace presenting the editor (now in the behaviour test files over the shared
`varos/panel_system/test_support.rs` harness). Four code-cleanliness passes (2026-11-02) did the
review's P1 and P2 items and the third review's sign-off list; `STATE.md` (revision history)
separates what was extracted, what still lives in upstream files, and what is deferred. Still
pending, by decision: (#6) one presentation representation per column (it needs a layout format
version step, a declared format change); P3: a smaller `panel_system/mod.rs` facade, render
functions without parallel mutable outputs, narrower model field visibility, a
presenter-independent panel-content module (the workspace still renders ordinary bodies through
`shell::boxes`), host title controls (close, column toggles and grips) measured into the geometry
result like the group header controls, and the two documented keyed-state bridges to upstream
(the tool flyout's own tool list/anchor for the classic shell, the All Tools drawer's bounds).
Done in the fourth pass: measured, localized tab widths and one measured group-header geometry
(the flyout's post-measurement tab trim is gone), egui drag ownership ending with a cancelled
panel gesture, one Tools well/chip implementation, the Preferences workflow in
`varos/preferences/`. Nothing here is accepted by the owner yet; `STATE.md` lists each piece as
provisional.

Open, in order:
1. Owner hand test of the side Color panel and horizontal tab dragging in build .16.
2. Owner hand test of the review fixes: Control-bar Opacity/Style links open their panels; library
   panels close/restore; Presentation Mode hides all chrome; Window items and panel keys toggle;
   Tab hides Tools + Control + panels while Shift+Tab hides panels only; turning "Varos shell" off
   returns to the classic dock; Control's X/Y/W/H reachable by scrolling the bar; a tab released in
   the scrolling edge lands at the very end/start; Preferences Apply sends every field you edited,
   and Escape in a Preferences field keeps the window and the draft.
3. Detaching a document into its own window is not supported (dropping outside the strip cancels);
   a separate task if wanted.
4. Other dialogs stay modal; make them modeless one at a time only if asked.

Revision: 2026-11-02 — native panel workspace and review fixes recorded; owner acceptance pending.
Revision: 2026-11-02 (docs unification) — summary table and staged upstream-merge plan added; stale
items 6, 8, 9, 10 and the PDF ligature entry corrected.

## Panel queue as approved — 2026-11-01

This queue takes precedence over the older list below. The owner approved the larger-icon preview
with «ممتاز اعتمد» and then asked how to implement it professionally. See the
[implementation plan and approval record](PANEL_IMPLEMENTATION_PLAN.md) for the frozen reference,
metrics, staged migration and per-slice acceptance. Its items below were then built natively (see the
2026-11-02 section above); Color clipping acceptance remains separate. The paragraphs below preserve
the earlier research gate; their pending visual-preview status is superseded by this update.

Earlier foundation-review scope:
The owner explicitly stopped native implementation, builds and UI testing until the foundations are
agreed. The owner has now approved fixed context-specific Properties order, workspace-wide floating,
and whole-container X hiding with reopening from the top Window menu. **Current delivery: visual
preview of groups and all three presentation states; approval to implement is still pending.**
Read [the architecture study](PANEL_ARCHITECTURE_STUDY_2026-11-01.md) first; it covers
current and legacy source, earlier agreements, public Blender documentation and open decisions.
Study may proceed while Color clipping in 0.19.1 awaits owner review. Neither that review alone nor
the existence of this proposal authorizes implementation. After architecture agreement, work one
small item at a time; the owner reviews and accepts it before the next.

0. Color palette clipping — owner acceptance pending.
   **Next, before item 1: P0A, unified panel/group/container model.** One layout model for tab
   groups and horizontal/vertical stacks, whether floating, docked at any of the four edges, or
   attached to internal dividers as in the earlier approved architecture;
   expanded, icon-with-name and icon-only presentations preserve identity, membership and position.
   Control and Tools must use the same move/join/split/collapse/persistence rules. Start with a
   decision ledger and diagram, then an approved visual preview and a two-panel slice; stacking,
   docking, Control, Tools and saved-layout migration follow
   one at a time; do not launch a wholesale rewrite or parallel implementation.
1. Panel header geometry, tab appearance, control/icon sizes and separators.
2. Dark-header and name-tab drag regions.
3. Expanded / icon-only / icon-with-name states; retain floating position on collapse and reopen.
4. Icon groups, group grips and separators in both compact modes.
5. Adjacent flyouts, active icon/tab agreement and unclipped content.
6. Dock, join, reorder and detach panels, icon groups and columns.
7. Smooth dragging without jumps or unintended snapping.
8. Compact top Control bar.
9. Consistent internal panel layout, one existing panel at a time.
10. Tool strip groups and icon/control sizing using existing Varos assets and tools.
11. Final integrated build and owner review, including workspace persistence.

Detailed reference mapping, expected behavior and per-item acceptance:
[Panel system review queue](PANEL_SYSTEM_REVIEW_QUEUE.md).
Use clearly labeled image previews during iteration; defer full release packaging to the end.
Do not copy reference assets, run Adobe applications, or turn pictured features into new scope.
The earlier keyboard-undo follow-up remains deferred and excluded from the 0.19.1 clipping build.

Revision: 2026-11-01 — research-only architecture gate added at the owner's request; prior agreements,
legacy foundations, numeric scrubbing and internal-divider rails included. The owner subsequently
approved the three decisions above and requested a visual preview before implementation. No native
implementation, Color acceptance or visual acceptance claimed.

## Older backlog — retained for context, below the current owner queue

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
6. **The remaining icons:** 77 Lucide imports and 69 icons to draw in Varos style (counts from
   `icon-map.tsv`, last updated 2026-10-27; the override registry has since grown from 146 to 157).
   *Acceptance:* `icon-map.tsv` has no `EMPTY` rows left except deliberate ones; every asset in
   `ASSETS.md`; owner accepts the drawn set.
7. **Port the Varos colour picker, export sheet and control bar.**
   *Acceptance:* each works on VectorCraft's own commands/model, matches the Varos design, passes
   gates; owner accepts each piece separately.
8. **[done, owner to look] Bridge without pairing + agent presence over upstream's MCP.** Any
   same-user agent reads, edits and saves with no approval prompt; agent edits show on the canvas in
   orange. Merged as `31de359c` after two security reviews (STATE "Agent-native layer").
   *Acceptance:* an external agent edits an open document with no prompt, the owner sees it happen,
   undo reverts it.
9. **Upstream merge routine.** Last base: `1cdc9873` (2026-10-26); the next merge is the staged
   plan in "Upstream merge — pending" above. *Acceptance:* `git fetch upstream` every session, merges
   in stages, conflicts resolved, local gates green, merge recorded (date, upstream commit, conflict
   count) in `STATE.md`.
10. **Publish checklist.** *Acceptance:* a written list covering the three bars in `FORK.md`
    (Arabic, performance, design), licences and `ASSETS.md` complete, `cargo xtask brands` clean,
    the `varos-text` dependency reachable over HTTPS after the repository replacement (rev
    `5936eaf` must stay on the `legacy-engine` history), no ArtCraft marks (logo files removed
    2026-11-02), no people's names in docs or commit authors, name and trademark check; owner signs
    it off.
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
    - ~~PDF copy-back bug: ligature words read back reversed~~ fixed by the 2026-10-29 PDF merge;
      the regression in `crates/pdf/tests/arabic_copy_back.rs` passes (STATE "PDF copy-back of
      ligature words"; external-viewer acceptance pending).
13. **Native-speaker review of translations.** The new Arabic Essentials, kashida and font-menu
    labels were machine-written into 23 catalogs. *Acceptance:* a native reader signs off each
    language, or the rows fall back to English on purpose.

## VSTAB.1 — owner stability request (2026-11-01)

Three reproduced shell gesture cancellation defects are repaired in the working branch; four
upstream stability fixes are backported without importing the newer design. Automated checks,
installed build identity and native mouse acceptance are tracked separately in
[STABILITY_2026-11-01.md](STABILITY_2026-11-01.md). This item remains open until the tested build is
installed and real-window panel, drag, collapse/restore and picker checks pass.

Revision: 2026-11-01 — stability task added; no owner acceptance inferred from headless tests.

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
