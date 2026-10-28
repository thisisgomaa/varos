# Pages inside a project

Updated: 2026-10-28. Implements the owner decision in
[SHELL_SIMPLE_DIRECTION.md §6](SHELL_SIMPLE_DIRECTION.md).

A `.varos` file is one project/tab. Each page has a stable ID, a name and an unchanged
`Document`. The session lends the active page's `DocState` to the existing tools, renderer and
undo machinery. Each page retains its own undo/redo, selection, guides, layer expansion and
view. Switching pages adds no undo entry. Page management leaves art history intact; structural
page changes are project operations outside the per-document undo stacks. The clipboard is
shared across pages.

## Native format v5

The serialized format identifier remains `vectorcraft`; `.varos`, `.vectorcraft` and `.drawcraft`
extensions still open. Version 5 replaces the root `document` and `images` fields with:

```json
{
  "format": "vectorcraft",
  "version": 5,
  "pages": [
    {"id": "page-1", "name": "Page 1", "doc": {}, "images": {}}
  ],
  "active_page": 0
}
```

`doc` is the existing document representation. Images belong to their page, so identical image
keys on separate canvases cannot overwrite each other. Profiles, the preview, PDF compatibility
data and the binary resource tail belong to the file. Existing compression and reader budgets apply.

The named pure migration `migrate_v4_to_v5_pages` wraps an old document in `page-1` / `Page 1`.
Earlier document migrations still run. Current saves write v5; explicit legacy saves remain
available for one page and refuse multiple pages. Readers supporting versions 1–4 cannot open v5. Older readers may report a missing
root document before checking the version; they must not silently open only its first page.

Fixtures authored by this project:

- `fixtures/pages_v4.varos`: legacy single document.
- `fixtures/pages_v5.varos`: two pages, second active.
- `fixtures/pages_future.varos`: future version 6, refused.

`crates/format/tests/pages.rs` checks migration, refusal, compression and byte-stable plain
round trips. Page IDs, names, count and active index are validated before installation.

Dirty state, background saves, Save a Copy, recovery and Package operate on the whole project.
Document Info includes per-page counts. SVG/PNG/PDF export the active page by default.
PDF `allPages: true` concatenates each page's artboards in project order, with independent
canvas coordinates. Preserve Editing embeds the full project. PDF layers, overprint and invisible text retain
their existing export behavior across the concatenated canvases.

## Commands and agent contract

All page actions use the existing command registry:

| Command | Parameters |
|---|---|
| `page.list` | `{}` |
| `page.add` | `{name?}`; default `Page N` |
| `page.rename` | `{page?, name}` |
| `page.duplicate` | `{page?, name?}` |
| `page.delete` | `{page?}`; last page refused |
| `page.move` | `{page?, index}`; zero-based destination |
| `page.switch` | `{page?}` |

`page` is a stable ID or zero-based index; omission means active. List entries contain
`page_id`, `page_name`, `active`, `index` and `artboards`.

The existing MCP contract is preserved without a version bump. `capabilities {}` and
`list_boards {}` retain their existing response shape. `capabilities {pages:true}` documents
the additive page contract; `list_boards {pages:true}` includes each project's pages.
Board-scoped select, edit and inspect tools accept optional `page: "<page_id>"`. They restore
the person's page and view afterward; private agent selections are keyed by project and page.
The report includes the targeted page's `page_id`, `page_name`, `active` and project page list.
Project revisions include content/name/order changes on every page, excluding page switching.

`vectorcraft-cli pages FILE` prints JSON page descriptors, also for migrated legacy files.

## Panel and shell integration

Window › Pages (`window.panel {"panel":"pages"}`) opens the generic registered panel `pages`,
using Lucide `file`. Both dock systems can host it. The panel has 28 pt rows, an azure active
row, artboard counts, inline double-click rename, drag reorder and the page context menu.

The protected shell, chrome, toolbar and Home files are untouched. The shell owner can call
`varos::pages::status(app)` where chrome currently prints the artboard count to display
`Page 2 of 3 · Artboard 1 / 4`. Page commands also update the existing transient status message.
Tab titles continue to use the file name.

The ignored `varos::tests_pages::capture_pages_panel` test writes the panel image to
`VAROS_PAGES_CAPTURE`; the committed capture is `mockups/pages-panel-capture.png`.

## Hand test

1. Open an old `.vectorcraft` file and open Window › Pages; confirm one “Page 1”.
2. Add a second page, draw on it, switch back, then undo; confirm each page keeps its own art and view.
3. Copy art on one page and paste on another; rename, duplicate and reorder pages.
4. Save as `.varos`, close and reopen; confirm names, order, active page and both canvases.
5. Export PDF first normally, then with All pages; compare page counts and reopen the latter with Preserve Editing.

Revision history: 2026-10-28 — initial v5 project/page contract and panel integration.
