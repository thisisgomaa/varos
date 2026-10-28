# Simple shell look — partial validation report (draft)

Branch: `lane-look`, continued from `f3a9780f` (`wip(shell-simple-1)`). Reference: the Editor view of `docs/varos/mockups/shell-simple-2026-10-28.html`, with the owner's floating/docking amendment in `SHELL_SIMPLE_DIRECTION.md`. Contributor credit stays `@thisisgomaa` as requested. The owner stopped validation and requested a draft PR with the current work; this report does not claim full acceptance.

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

## Gates

Validation runs in the Linux workspace with Rust 1.95.0, one Cargo lane, `-j 2`, `--offline`, and `--test-threads=2` for tests. The Mac target directory from the original brief is unavailable here; the local `target` directory is used. A one-time `cargo fetch --locked` supplied missing dependencies before offline validation. The owner replaced the workspace release gate with debug tests because the uncached release compilation was too slow. The release workspace compilation was stopped before test execution; its completed native `vectorcraft` binary is retained. The debug workspace compilation was also interrupted by an environment transition, resumed from cache, and then stopped at the owner’s request before test execution. Neither workspace run has a completed test result. No additional release compilation was started.

Completed release test command:

```sh
cargo test -p vectorcraft-ui-egui --lib --release --offline -j 2 -- --test-threads=2
```

Stopped debug workspace command:

```sh
cargo test --workspace --no-fail-fast --offline -j 2 -- --test-threads=2
```

| Gate | Exact result / status |
|---|---|
| UI release library tests | Expected baseline only, exit 101: `test result: FAILED. 2533 passed; 10 failed; 16 ignored; 0 measured; 0 filtered out; finished in 77.71s` |
| Workspace debug tests (owner-approved replacement) | Stopped at owner request during compilation; no completed test result. |
| Two ignored 1440×900 captures | Not run; no captures generated. |
| Clippy, workspace/all targets, warnings denied | Not run; no result line available. |
| `cargo fmt --check` | Not run; no result line available. Source was formatted earlier, but the requested check did not complete. |
| `git diff --check` | Not run; no result line available. |
| Native release build | Already produced by the stopped workspace release compilation, before the owner changed the gate. Existing executable verified with exit 0: `vectorcraft 0.17.0`. No additional release build started. |

The completed UI release run has exactly the permitted failures below; every other UI library test passes. There is no completed workspace result.

```text
panels::gradient::tests::a_color_dropped_on_the_ramp_keeps_its_model_in_a_cmyk_document
panels::gradient::tests::a_dropped_swatch_adds_or_recolours_a_stop
panels::gradient::tests::a_selected_diamond_shows_and_sets_its_midpoint_in_location
panels::gradient::tests::alt_drag_copies_a_stop_and_alt_dropping_on_a_stop_swaps_colours
panels::gradient::tests::double_clicking_a_stop_opens_the_shared_popover_beside_it
panels::gradient::tests::the_dither_box_dithers_the_gradient_fill_and_is_off_for_strokes
panels::gradient::tests::the_proxy_toggles_fill_and_stroke_and_stays_with_options_hidden
panels::gradient::tests::the_stroke_buttons_lay_the_gradient_within_along_or_across_the_stroke
panels::gradient::tests::the_thumbnail_menu_applies_gradient_swatches_and_saves_one
tests_rotateview::escape_after_closing_a_popup_or_during_a_ruler_origin_drag_keeps_the_rotation
```

Supplemental targeted checks passed for all 27 cursor tests, the shared picker interaction in both shells, and all 150 icon overrides. The shell suite also caught and verified the menu-width, drawer-opening, flyout-height, overflow and window-edge docking fixes before the full release run.

## Captures and comparison

Neither capture was generated. The following requested paths are absent, and visual comparison of the implementation is unfinished:

- `docs/varos/mockups/shell-simple-capture.png` — Design, Color flyout, one tool column, 1440×900.
- `docs/varos/mockups/shell-simple-cols-capture.png` — the same arrangement with three tool columns, 1440×900.

The mockup includes future Pages/Home content and a different colour ramp. This lane uses the existing real Layers/Artboards/Properties bodies and unchanged theme tokens, as required by the brief.

## What did not complete

- Workspace release tests: stopped during compilation after the owner replaced this gate.
- Workspace debug tests: stopped during compilation at the owner’s request; no test result.
- Both ignored capture runs, PNG generation, and visual inspection against the Editor mockup.
- `cargo clippy --workspace --all-targets -- -D warnings`.
- `cargo fmt --check` and `git diff --check`.
- Owner hand testing and final visual acceptance.

No further validation commands were run after the owner requested the draft PR. The native release executable had already been built and its version check passed; the standalone `cargo build -p vectorcraft --release` command was not run separately.

## Five owner hand tests

1. Open two documents, turn on **Preferences → User Interface → Varos shell (new)**, and choose/reset **Design**. Switch the top-bar document tabs, close one, and use `+` to open New Document. Check that the canvas starts below the top bar, without extra workspace/control rows.
2. Drag a panel tab onto the canvas, resize its floating card, then drag it to a dock edge. Check the azure drop line, flat docked surface and one hairline between neighbours; hover and drag a divider.
3. Open **Window → Control**, use a control, drag its tab into a docked group, then float and close it. Reopen it from the right strip. Open Color and Stroke and check that their flyouts stay clear of the strips.
4. Use the tools strip's `⋯` menu to choose one, two and three columns. Switch workspaces and return; check that the choice is remembered. Select a tool, open its triangle flyout, and tear it off.
5. Turn **Varos shell (new)** off. Check the familiar legacy tabs, panel frames, control bar, toolbar, fill/stroke picker and shortcuts. Turn it on again and confirm the saved shell arrangement returns.
