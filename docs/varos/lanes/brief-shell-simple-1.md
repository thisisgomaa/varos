# Lane shell-simple-1 — the simple look (owner-approved mockup), shell ON only

Repo: varos-next (Rust, egui 0.37). Work ONLY in this worktree (branch lane-look, = main). OFF (the
pref "Varos shell (new)" off) stays byte-for-byte today's window; everything below is shell-ON only.

READ FIRST, in this order:
1. docs/varos/SHELL_SIMPLE_DIRECTION.md — the decisions (incl. the owner amendment: panels keep OUR
   floating/docking system; only the LOOK changes).
2. docs/varos/mockups/shell-simple-2026-10-28.html — the approved mockup (open it; the "Editor" view is
   this lane; "Home" is a later lane). Match its sizes/structure; colours come from `theme::Tokens`
   (map: base→t.base, panel→t.panel, raised→t.hover/raised, line→t.line, text/muted/quiet→t.*, azure
   only for selection/focus/active/drop line).
3. docs/varos/lanes/brief-look.md and brief-look2.md — earlier briefs; their items apply EXCEPT where
   this brief says otherwise (precedence: this brief > SHELL_SIMPLE_DIRECTION.md > brief-look2 > brief-look).
4. docs/varos/NEW_SHELL_PLAN.md (stage records 2–5), then crates/ui-egui/src/varos/shell/*.rs,
   chrome.rs, toolbar.rs, varos/picker/, theme.rs, dock.rs (panel_body), varos/tests_shell.rs.

## Scope of this lane (one lane, one PR-sized diff)
A. **Flat docked look** (direction §1): no rounded box frames, no outlines, no 12 pt seams, no grips
   on boxes; neighbours separated by ONE 1 px `t.line` hairline; divider hit area 8 pt, invisible
   until hovered (2 px `t.hover` line), azure only while dragging/dropping; all docked groups and the
   Board share flat `t.panel`/`t.base` as in the mockup (Board canvas = t.base, columns = t.panel);
   no gutter at the window edges. Floats = the only cards: 1 px `t.line2`/line, radius 10, no shadow.
B. **Top bar (40 pt)** per the mockup: brand tile + Home; the open documents as tabs HERE (pill
   style, dirty dot, `+`), removed from the Board header (the Board keeps only rulers); right side:
   presence avatars placeholder (the current user initial; an orange agent avatar appears only when an
   agent is connected — reuse the existing agent presence state; `+` opens nothing, tooltip
   tl!("Sharing comes later")), a "Share" button (disabled look, same tooltip), search icon (existing
   search), the workspace control (name + chevron; list = presets, custom, New…, rename/delete/reset
   via the existing dialogs). In-app menus only when no native menu backend (unchanged rule).
   The Workspaces row is deleted (brief-look2 §1). The fixed Control bar row is deleted; its content
   becomes the dockable panel `control` (brief-look2 §2), not open by default.
C. **Strips** (tools left, panel icons right): flat columns 40 pt wide (mockup), `t.panel`, hairline
   on the canvas side only, ONE grip at the top (approach-revealed 30×3), group separators = 20 pt
   hairlines, 28×28 tiles, 16–18 pt glyphs of one optical size, flyout triangle 3 px bottom-right,
   wells cluster + `⋯` at the bottom; tools strip columns 1/2/3 from its `⋯` menu, per workspace
   (brief-look2 §5; widths 40 / 68 / 96). Floating strip = card look (A).
D. **Panels inside boxes/floats/flyouts**: frameless host mode for legacy bodies (brief-look §4), tab
   rows 36 pt in columns / 32 pt in floats, pills 24 pt radius 6, 11 pt medium, `⋯` icon right, overflow
   → `…` menu never clipping (brief-look §6); section headers 32 pt (11 pt semibold + `+`/`⋯` right),
   hairline between sections, fields 24 pt radius 5, 16 pt side padding (mockup right column).
E. **Icons, type, motion**: chrome controls as Lucide icon textures (no text glyphs `<<` `>>` `×` `≡`
   `◇`; brief-look §2); labels 11 / body 12 / titles 14 semibold; motion exactly as stage 5.
F. **Flyouts + picker**: 240 pt, radius 10, 1 px line, 6 pt from the strip, never over a strip, wheel
   ≤ 180 pt, thin auto-hide scrollbars (brief-look §8).
Not in this lane: Home, Pages, colours/theme values, stage 6 deletions, Properties pin.

## Persistence
`VarosShell` version 6 (5→6 pure migration; adds `tools_columns`, the `control` panel id; fixture
`shell/fixtures/v5.json`). Older layouts load without loss.

## Tests (tests_shell.rs + toolbar/chrome tests; Harness + kittest) — each item above gets at least one
real pointer-level test; keep the stage 2–5 suites green. Specifically: no rounded-rect strokes around
boxes; exactly one hairline between neighbours; divider hover line only on hover; top bar has doc tabs
and no workspaces row; `control` opens from strip + `window.panel control`, docks/floats; no
control-bar row rect when ON; tool columns 3 widths + canvas shift + per workspace; no text-glyph
controls painted; tab overflow menu; flyout never intersects strips; OFF: existing tests untouched.
Captures (ignored tests): `VAROS_SHELL_CAPTURE_SIMPLE` → 1440×900 shell ON, Design workspace
(Layers|Artboards left, Properties right, Color flyout open from the right strip, tools 1 column) to
/tmp/varos-simple/shell-simple-capture.png and `…_SIMPLE_COLS` with tools at 3 columns; copy both to
docs/varos/mockups/. Compare against the mockup yourself before reporting.

## Gates / rules
ONE cargo lane; `-j 2`; `--test-threads=2`; `--offline`; `CARGO_TARGET_DIR="<workspace>/lanes/_target-shared"`; ui-egui release lib tests (known 10 release-only failures: 9
`panels::gradient::tests::*` + `tests_rotateview::escape_after_closing_a_popup…`); `cargo clippy
--workspace --all-targets -D warnings`; rustfmt --check on touched files; `git diff --check`; release
build of `vectorcraft`. No commits/stash. Forbidden: engine/render/text crates; `theme.rs` colour values
(adding icon entries is fine); Lucide SVGs only (crates/ui-egui/assets/varos/icons, ISC). Report:
docs/varos/lanes/shell-simple-1-report.md — per item A–F what changed with file:line, gate table with
exact result lines, capture paths, 5 hand-test steps for the owner (plain).
