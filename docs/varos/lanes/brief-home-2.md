# Lane home-2 — the new Home (owner-approved mockup "Home" view)

Repo: varos-next (Rust, egui 0.37). Work ONLY in this worktree (branch lane-home2 off main). This lane
touches the Home screen only: `crates/ui-egui/src/varos/home.rs`, `home_capture.rs`, the Home entry in
`menus.rs` (`home_key`), `icons.rs` / Lucide assets, tests. It does NOT touch the shell (`varos/shell/*`),
chrome.rs, toolbar.rs — another lane owns those right now; if you need a hook there, write it down in the
report instead of editing.

READ FIRST: docs/varos/SHELL_SIMPLE_DIRECTION.md §4–§6, docs/varos/mockups/shell-simple-2026-10-28.html
("Home" view — open it and match it), then today's Home (`varos/home.rs`, how recents/open/new are
wired: find the recent-files store and the commands Home runs; `menus.rs` Home), `theme.rs` tokens,
`docs/varos/feedback` notes on Home if present. Reference (read-only, outside the repo, MIT/Apache):
~/Documents/AI workspace/reference/artcraft/figcraft/crates/ui-egui/src/home.rs and
home/home_recents.rs for the STRUCTURE (sections, search, drop-to-open, tips) — our look, not theirs; no
copying of code without an attribution header + a row in NOTICE.

## The Home (mockup)
- Top bar 40 pt: brand tile + "Varos", right: search field (files + templates, 260 pt), user avatar
  (initial of the macOS user name; no accounts).
- Sidebar 220 pt (`t.panel`): rows 30 pt, radius 6, active = `t.raised`: Recents · Projects ·
  Shared with me (badge "later", disabled) · Templates · hairline · Getting started · (spacer) ·
  Preferences (opens the existing preferences).
- Main (48 pt side padding, 36 top): greeting "Good evening, <name>" (time-of-day; 20 pt semibold)
  + one muted line; actions row: **+ New board** (primary = light fill, dark text), **Open…**,
  **Join with a code** (disabled, badge "later", tooltip tl!("Sharing comes later")).
- **Recent**: cards 180 pt min, thumbnail 4:3 radius 10 with a REAL thumbnail of the file (render the
  document's first artboard/page through the existing software renderer at open/save time into a
  cache at ~/Library/Application Support/Varos/thumbs/<hash>.png; placeholder tile until it exists),
  name 12 pt, detail 11 pt quiet ("Edited 2 minutes ago · 3 pages" → today: "· N artboards"); sort
  menu Last modified / Name / Date created; right-click: Open, Show in Finder, Remove from Recents.
- **Projects**: a project = a folder the user added (Add project… picks a folder); rows 36 pt with
  folder icon, name, right: "N files · path"; click = that folder's .varos files as cards. Stored in
  the existing prefs store (`projects: Vec<PathBuf>`); missing folders shown dimmed with "Not found".
- **Templates**: the existing starter documents (if none exist: 3 built-in: A4 poster, Square post,
  Business card) as cards; click = new document from it.
- **Getting started**: 3 tips as cards (Artboard tool; Write in Arabic → Character/Kashida; Let an
  agent help → Bridge) with "Hide" (pref; Help › Getting started shows them again).
- Drop a file anywhere on Home → opens it (existing open path); non-openable → toast.
- Search filters Recent + Projects + Templates live; Escape clears.
- Keyboard: ⌘N new, ⌘O open, arrows move between cards, Return opens.

Not in this lane: online/cloud, accounts, sharing, Pages model (detail text uses artboards).

## Tests (Harness + kittest, `varos/tests_home.rs`): sections and actions exist and run the right
commands; disabled items do nothing and show the tooltip; recent card opens its file; sort orders; a
project folder lists its .varos files, a missing folder is dimmed; tips hide/show via pref; drop opens;
search filters; greeting by hour; thumbnail cache writes a PNG for a saved document and the card shows
it. Capture (ignored test) `VAROS_HOME_CAPTURE_SIMPLE` → 1440×900 to /tmp/varos-home/home-capture.png
and copy to docs/varos/mockups/home-simple-capture.png; compare with the mockup before reporting.

## Gates / rules
HEAT COORDINATION (hard rule): another cargo lane is running on this Mac. Implement WITHOUT cargo (read,
design, code, tests). Before your FIRST cargo command, wait until
/private/tmp/claude-501/-Users-gomaa-Documents-AI-workspace-varos/90bf49a1-794a-43e8-b7df-7f72ceea7227/scratchpad/go-home2-cargo
exists (`test -f`; if absent `sleep 60`, up to 240 times). Then: ONE cargo command at a time; `-j 2`;
`--test-threads=2`; `--offline`; `CARGO_TARGET_DIR="~/Documents/AI workspace/lanes/_target-shared"`;
ui-egui release lib tests (known 10 release-only failures: 9 `panels::gradient::tests::*` +
`tests_rotateview::escape_after_closing_a_popup…`); `cargo clippy --workspace --all-targets -D warnings`;
rustfmt --check on touched files; `git diff --check`. No commits/stash. Forbidden: engine/render/text
crates, `theme.rs` colour values, the shell/chrome/toolbar files. Report:
docs/varos/lanes/home-2-report.md — what changed with file:line, gate table with exact result lines,
capture path, 5 hand-test steps for the owner (plain), and any hook you needed in the shell files.
