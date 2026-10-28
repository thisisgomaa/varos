# Task — Pages inside a Varos file (repo thisisgomaa/varos-next, branch off `main`)

Create branch `lane-pages-cloud` from `main`. When done, push it and open a pull request to `main` titled
"feat(pages): a .varos file holds pages (project → pages → artboards)". Do not merge.

## Decision (owner, 2026-10-28) — read `docs/varos/SHELL_SIMPLE_DIRECTION.md` §6
A `.varos` file is a **project**; it holds **pages** (like Figma pages); each page is its own canvas with
its own artboards, layers, guides and view. Today's file = a project with exactly one page named "Page 1".

## Model — the least invasive shape (do NOT refactor the document model)
Keep `Document` exactly as it is (the engine, render, tools, undo all stay per document). Add a project
layer above it in the file/session code:
- `Project { pages: Vec<Page>, active: usize }`, `Page { id: Uuid/stable id, name: String, doc: Document }`.
- The editor always edits the active page's `Document`; switching pages swaps which Document the
  session/tab shows (keep each page's undo history, selection and view; a page switch is not an undo step).
- The file format: the top-level JSON gains `"pages": [ { "id", "name", "doc": <today's document> } ]`
  and `"active_page"`. A legacy file (no `pages`) loads as one page "Page 1" wrapping the old document;
  saving always writes the new shape (format version bump per the repo's format rules: named pure
  migration, fixtures for old and new files, refusal fixtures, old-reader note). `.vectorcraft`/
  `.drawcraft` legacy opening still works.
- Dirty state, autosave/recovery, "Save a Copy", export (PDF/SVG/PNG: export the active page; PDF gets
  an "All pages" option that concatenates pages), Document Info (counts per page), Package: all per project.
- Clipboard works across pages (copy on one page, paste on another) through the existing clipboard.

## Commands + Bridge
Engine/app commands (through the existing command registry, so menus/shortcuts/MCP share them):
`page.list`, `page.add` (name optional → "Page N"), `page.rename`, `page.duplicate`, `page.delete`
(refuses the last page), `page.move` (reorder), `page.switch`. Bridge/MCP: `list_boards`/`describe`
report pages (`page_id`, `page_name`, `active`), `select`/`edit` accept an optional `page` target, and the
API version is bumped opt-in (1.0/1.1 fixtures byte-frozen; 1.2 adds the fields). CLI: `pages` subcommand
listing pages of a file.

## UI — a regular panel, not a shell change
Register a **Pages** panel (id `pages`, Lucide `file` icon) in the generic panel registry so it works in
the old dock and in the new shell alike (Window › Pages). Do NOT edit `crates/ui-egui/src/varos/shell/*`,
`chrome.rs`, `toolbar.rs`, `home*.rs` (other branches own them; if you need a hook, describe it in the PR).
Panel look (see the mockup `docs/varos/mockups/shell-simple-2026-10-28.html`, left column "Pages"):
header "Pages" + `+`, rows 28 pt (page icon, name, right: artboard count), active row = azure tint,
double-click renames inline, drag to reorder, context menu: Rename, Duplicate, Delete, Move up/down.
The document tab title stays the file name; the status bar shows "Page 2 of 3 · Artboard 1 / 4".

## Tests
- Format: new fixtures `fixtures/pages_v<N>.varos`, legacy file loads as one page and re-saves in the new
  shape; refusal fixture for a future version; round-trip byte-stable for a plain file.
- Commands: add/rename/duplicate/delete(last refused)/move/switch, each undo-safe; switching keeps each
  page's selection and view; clipboard across pages.
- Bridge: 1.0/1.1 fixtures unchanged; 1.2 lists pages; CLI `pages` output.
- Panel (egui Harness + kittest, `varos/tests_pages.rs`): rows, active tint, rename, reorder, context
  menu; `window.panel pages` opens it.
- Ignored capture test `VAROS_PAGES_CAPTURE` → commit `docs/varos/mockups/pages-panel-capture.png`.

## Gates (all must pass; exact result lines in the PR)
`cargo test --workspace --release` (known pre-existing release-only failures you may ignore: 9
`panels::gradient::tests::*` and
`tests_rotateview::escape_after_closing_a_popup_or_during_a_ruler_origin_drag_keeps_the_rotation`;
anything else must be green), `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`,
`git diff --check`, `cargo build -p vectorcraft --release`. Conventional commits (`feat(pages): …`,
`feat(format): …`, `test(pages): …`). PR description: the format change with the migration name and
fixtures, commands and Bridge fields, panel, gate results, the capture, 5 plain hand-test steps, and any
hook you needed in files you were told not to edit.
