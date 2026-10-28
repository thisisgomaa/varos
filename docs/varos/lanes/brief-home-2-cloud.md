# Task for Codex Cloud — Varos new Home screen (repo thisisgomaa/varos-next, branch off `main`)

Create branch `lane-home2-cloud` from `main`. When done, push it and open a pull request to `main` titled
"feat(home): new Home screen (simple shell direction)". Do not merge.

## Read first (all in the repo)
1. `docs/varos/SHELL_SIMPLE_DIRECTION.md` §4–§6 (decisions).
2. `docs/varos/mockups/shell-simple-2026-10-28.html` — open it; the **Home** view is the target. Match
   its structure and sizes exactly; colours come only from `crates/ui-egui/src/theme.rs` tokens.
3. Today's Home: `crates/ui-egui/src/varos/home.rs`, `home_capture.rs`, the Home entry in `menus.rs`
   (`home_key`), how recents/open/new are wired; `varos/icons.rs` + `crates/ui-egui/assets/varos/icons`
   (Lucide SVGs, ISC — add missing icons the same way, no other icon sources).

## Scope — Home only
Touch only `crates/ui-egui/src/varos/home*.rs`, a new `varos/tests_home.rs`, icons, `menus.rs` Home
entry, prefs for the new settings. Do NOT edit `varos/shell/*`, `chrome.rs`, `toolbar.rs`, `theme.rs`
colour values, or the engine/render/text crates (another branch owns the shell; if you need a hook there,
describe it in the PR instead).

## What to build (the mockup)
- Top bar 40 pt: brand tile + "Varos"; right: search field (files + templates, 260 pt) and a user
  avatar with the macOS user's initial (no accounts).
- Sidebar 220 pt: rows 30 pt radius 6, active = raised token: Recents · Projects · Shared with me
  (badge "later", disabled, tooltip "Sharing comes later") · Templates · hairline · Getting started ·
  spacer · Preferences (opens the existing preferences).
- Main (48 pt side padding, 36 top): greeting "Good morning/afternoon/evening, <name>" (20 pt
  semibold) + one muted line; actions: **+ New board** (primary: light fill, dark text), **Open…**,
  **Join with a code** (disabled, badge "later", same tooltip).
- **Recent**: cards (min 180 pt), thumbnail 4:3 radius 10 showing a real thumbnail of the file
  (render its first artboard with the existing software renderer at open/save into
  `~/Library/Application Support/Varos/thumbs/<hash>.png`; placeholder tile until it exists); name 12 pt;
  detail 11 pt ("Edited 2 minutes ago · N artboards"); sort menu Last modified / Name / Date created;
  right-click: Open, Show in Finder, Remove from Recents.
- **Projects**: a project = a folder the user added ("Add project…" picks a folder); rows 36 pt: folder
  icon, name, right "N files · path"; click = that folder's `.varos` files as cards; stored in prefs
  (`projects: Vec<PathBuf>`); a missing folder is dimmed with "Not found".
- **Templates**: existing starter documents, or 3 built-ins (A4 poster, Square post, Business card) as
  cards; click = new document from it.
- **Getting started**: 3 tip cards (Artboard tool; Write in Arabic → Character panel/Kashida; Let an
  agent help → Bridge) with "Hide" (pref; Help › Getting started shows them again).
- Drop a file anywhere on Home → opens it through the existing open path; otherwise a toast.
- Search filters Recent + Projects + Templates live; Escape clears. Keys: ⌘N, ⌘O, arrows between cards,
  Return opens.
- All strings through the existing `tl!`/translation mechanism (English + Arabic rows).

## Tests (egui Harness + kittest, `varos/tests_home.rs`)
Sections and actions run the right commands; disabled items do nothing and show the tooltip; a recent
card opens its file; sort orders; a project folder lists its `.varos` files and a missing folder is
dimmed; tips hide/show via pref; drop opens; search filters; greeting by hour; the thumbnail cache writes
a PNG for a saved document and the card shows it. Add an ignored capture test
`VAROS_HOME_CAPTURE_SIMPLE` (1440×900) and commit the PNG as `docs/varos/mockups/home-simple-capture.png`;
compare it with the mockup before finishing.

## Gates (all must pass; put the exact result lines in the PR description)
`cargo test -p vectorcraft-ui-egui --lib --release` (known pre-existing release-only failures you may
ignore: 9 `panels::gradient::tests::*` and
`tests_rotateview::escape_after_closing_a_popup_or_during_a_ruler_origin_drag_keeps_the_rotation`;
anything else must be green), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
`git diff --check`, `cargo build -p vectorcraft --release`. Commit as conventional commits
(`feat(home): …`, `test(home): …`). PR description: what changed per section with file:line, gate
results, the capture, and 5 plain hand-test steps.
