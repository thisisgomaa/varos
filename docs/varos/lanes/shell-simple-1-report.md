# Simple shell look — PR #6 fix-round validation

Branch: `lane-look`, continued from `f3a9780f` (`wip(shell-simple-1)`). Reference: the Editor view of `docs/varos/mockups/shell-simple-2026-10-28.html`, with the owner's floating/docking amendment in `SHELL_SIMPLE_DIRECTION.md`. Contributor credit stays `@thisisgomaa` as requested. This review round resumes local validation after main was merged; owner visual acceptance remains pending.

## A–F implementation and evidence

Source references below are relative to `crates/ui-egui/src/`.

| Item | Result and implementation | Pointer/regression evidence |
|---|---|---|
| A — flat docked look | Zero outer gutter; one-point tree seams; flat panel surfaces without box outlines. Dividers have an eight-point hit area, one hairline at rest, a two-point hover line, and azure drag feedback. Only floats use radius 10 and a one-point border, without shadows. `varos/shell/mod.rs:42`, `varos/shell/tree.rs:14`, `varos/shell/divider.rs:22`, `varos/shell/boxes.rs:254`, `varos/shell/floats.rs:142`. Design defaults to a 232-point left group and 248-point right group. | `simple_flat_boxes_and_pointer_divider_feedback`, `simple_default_layout_matches_the_editor_columns`; existing split, resize, docking, float, collapse and maximize tests. |
| B — top bar and Control | A 40-point bar owns document tabs, dirty markers, close controls and New Document; it retains native-menu handling, Home, search and workspace dialogs. In-app menus reserve only their title width, leaving room for document tabs. Home keeps its existing chrome. User/agent presence and disabled Share use existing state. Board document-header, workspace row and fixed control row are gone. The Control panel opens from Window/strip, docks, floats, duplicates, and returns to its strip icon when closed. `chrome.rs:363`, `chrome.rs:2685`, `chrome.rs:2750`, `varos/shell/workspace.rs:330`, `varos/shell/mod.rs:341`. | `simple_top_bar_document_tabs_switch_and_new_button_opens_dialog`, `simple_control_opens_from_strip_and_command_and_floats`, `simple_control_docks_duplicates_and_returns_to_strip_after_close`, app-bar and workspace regression tests. |
| C — strips and tool columns | Flat 40-point strips, 28×28 tiles, consistent icon size, one approach-revealed top grip; separators remain drop targets without becoming extra grips. Tool-column choices produce 40/68/96-point widths and persist per workspace. Floating tools derive their height from the chosen grid and dock at the window edge even with a panel left of the Board. The Edit Toolbar menu opens the drawer without its opening click dismissing it. `varos/shell/tools.rs:8`, `varos/shell/strip.rs:212`, `varos/shell/tokens.rs:4`, `toolbar.rs:523`, `edit_toolbar.rs:406`. | `simple_columns_menu_uses_pointer_and_no_fixed_control_row`, `simple_tools_columns_shift_canvas_and_survive_workspace_roundtrip`, `simple_only_top_strip_grip_drags_and_legacy_icons_keep_their_sources`; tool selection/flyout/tear-off tests. |
| D — panel interiors | Frameless legacy host; 36-point docked and 32-point floating tab rows; 24-point pills, 11-point medium labels and overflow menus. Hidden tabs have no invisible hit targets. Properties uses 32-point section headers, 16-point body margins and 24-point fields. Stroke controls fit the 240-point flyout; Properties height accounts for its margins; panels with their own scrolling viewport retain their saved flyout height instead of feeding that height back into content sizing. `dock.rs:15`, `varos/shell/boxes.rs:442`, `panels/properties.rs:1354`, `widgets.rs:738`, `panels/stroke.rs:149`. | `simple_overflow_menu_selects_hidden_tab_with_pointer`, `flyout_first_open_fits_stroke_and_area_matches_visible_bounds`, `flyouts_keep_their_first_presented_height_on_open_and_switch`; Properties section reorder/collapse tests. |
| E — icons, type and motion | Chrome controls use Lucide textures; added overrides have shell-specific keys so OFF controls retain their original icon sources. Shell text roles and shared panel widgets use 11-point labels/buttons, 12-point body and 14-point semibold headings. Stage-5 motion remains unchanged. `varos/icons.rs:111`, `varos/shell/boxes.rs:448`, `widgets.rs:64`, `chrome.rs:2892`. | No-text-glyph, icon-rasterization, immediate/settled motion, reduced-motion and real tear-off regression tests. |
| F — flyouts and picker | Default flyout/picker width 240, radius 10, hairline border and six-point strip gap; placement excludes both strips. User-resized flyouts retain their width. Picker wheel diameter is at most 180; shell scrollbars are thin and auto-hide. `varos/shell/strip.rs:61`, `varos/shell/strip.rs:389`, `varos/picker/mod.rs:760`, `varos/picker/wheel.rs:171`. | `simple_flyout_clear_of_strips_and_no_text_glyph_controls`, strip resizing/persistence, floating-icon flyout placement and picker interaction tests. |

Persistence is version 6, with a pure 5→6 migration and `shell/fixtures/v5.json`; existing layouts and workspace arrangements remain supported. Historical interaction tests use their original layout explicitly, while a separate test checks the new Design default.

OFF isolation: legacy panel and picker bodies execute directly, without an added UI scope or changed widget IDs. Legacy icon keys retain their original sources. No theme colour values, engine/render/text files or `home*.rs` were edited. Existing cursor tests exposed a Linux cache mismatch: the generic cursor cache now resolves the same default Varos bitmap already displayed by the canvas; displayed pixels and system-fallback behavior stay the same. Those cursor tests are unchanged (`cursors.rs:676`).

## PR #6 review fixes (2026-10-28)

- Home bypasses the editor app bar and gives its own 40-point header the full content rect (`lib.rs`).
- Tool flyouts use the Tools strip's right edge plus six points, including during animation. Shell rows, shortcuts and the grab bar fit a 240-point frame; long tool labels ellipsize (`toolbar.rs`).
- Workspace controls cap their label at 128 points (160 including padding/chevron), ellipsize, and have room reserved beside document tabs (`chrome.rs`, `varos/shell/workspace.rs`).
- Picker placement excludes floating Tools and floating panel strips, trying nearby obstacle edges while retaining the original placement when no floating strips are present (`varos/picker/mod.rs`).
- Four `simple_review_*` frame regressions cover these findings, including a 60-character name and both kinds of floating strip (`varos/tests_shell.rs`).

## Gates

Current validation: macOS lane-look, shared target directory `~/Documents/AI workspace/lanes/_target-shared`, one Cargo command at a time, `--offline -j 2`, test execution with `--test-threads=2`. Implementation and regression tests were written before Cargo. The requested handoff marker is awaited before the first Cargo command. The earlier Linux partial run is superseded by the results below; its ten failures are no longer an accepted baseline.

| Gate | Exact result / status |
|---|---|
| UI release library tests | Pending handoff marker. |
| Two ignored 1440×900 captures | Pending. |
| Clippy, workspace/all targets, warnings denied | Pending. |
| rustfmt check, touched Rust files | Exit 0, no output (pre-Cargo static check). |
| `git diff --check` | Exit 0, no output (pre-Cargo static check). |
| Native release build | Pending. |

## Captures and comparison

Pending generation and direct PNG inspection:

- `/tmp/varos-simple/shell-simple-capture.png` → `docs/varos/mockups/shell-simple-capture.png`.
- `/tmp/varos-simple/shell-simple-cols-capture.png` → `docs/varos/mockups/shell-simple-cols-capture.png`.

Browser access was unavailable when opening the HTML mockup. Its source was inspected for the approved dimensions and structure. The mockup includes future Pages content; this lane retains existing Layers/Artboards/Properties bodies and unchanged theme tokens. Owner hand testing remains separate from local gates.

## Five owner hand tests

1. Open two documents, turn on **Preferences → User Interface → Varos shell (new)**, and choose/reset **Design**. Switch the top-bar document tabs, close one, and use `+` to open New Document. Check that the canvas starts below the top bar, without extra workspace/control rows.
2. Drag a panel tab onto the canvas, resize its floating card, then drag it to a dock edge. Check the azure drop line, flat docked surface and one hairline between neighbours; hover and drag a divider.
3. Open **Window → Control**, use a control, drag its tab into a docked group, then float and close it. Reopen it from the right strip. Open Color and Stroke and check that their flyouts stay clear of the strips.
4. Use the tools strip's `⋯` menu to choose one, two and three columns. Switch workspaces and return; check that the choice is remembered. Select a tool, open its triangle flyout, and tear it off.
5. Turn **Varos shell (new)** off. Check the familiar legacy tabs, panel frames, control bar, toolbar, fill/stroke picker and shortcuts. Turn it on again and confirm the saved shell arrangement returns.
