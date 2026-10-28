# Shell 5 — review fixes and final report

Status: review fixes and all requested gates complete (2026-10-28); only the ten known release-only test failures remain.
Scope: this worktree only. No commit, staging or stash. Forbidden files remain unchanged.

## Review fixes

| Review item | Change / regression coverage |
|---|---|
| P2-1 | Disable input before restoring transition opacity; same-event strip closes restore opacity too. Real tool and strip close-frame tests. |
| P2-2 | Compare container IDs before/after a drop; settle keys are container IDs. Only newly created boxes/floats/strips settle. Real existing-box and new-box drop tests, including reduced motion. |
| P2-3 | Restore Edit Toolbar, Color/Gradient/None and Draw mode below the wells via existing command controls; honor visibility flags including fill/stroke. Screen mode remains in View. The whole strip is the drawer outside-click exclusion; compact spacing keeps the opener inside its bounds, and scrollbars do not cover flyout triangles. |
| P2-4 | Shared tab pills toggle Character/Paragraph option flags; tests cover boxes, floats and flyouts. |
| P2-5 | Suppress in-app menus whenever a native menu backend exists; retain brand, Home, title, undo/redo, search and build-identity hover. Tests with/without backend. |
| P3-6 | Tear-off deletes docked flyout motion immediately; regression checks cancellation on the tear-off frame. |
| P3-8 | Floating tools use rounded fill/hairline and content-derived height; docked tools have the Board seam. |
| P3-9 | Localized grip and undo/redo tooltips, undo/redo shortcuts; grip is inert in screen modes 1–2. |
| P3-11 | Share strip width and app-bar padding constants. |
| Additional | Tools Escape/workspace tests, v4 ui.json fixture, chrome hit sizes, both slide directions and restored exact settled flyout rect assertions. |
| P3-7 (deferred) | Docked tools settle can be obscured by canvas; not changed in this round, as requested. |
| P3-10 (accepted) | First-open 5% alpha remains the fade start. |

## Gate table

| Gate | Result | Evidence |
|---|---|---|
| UI-egui release library tests | BASELINE ONLY: 2524 passed, 10 known failures, 15 ignored; no new failures | `/tmp/varos-shell5/review-ui-verified.log` |
| Workspace clippy, all targets, `-D warnings` | PASS, final source (5.41 s) | `/tmp/varos-shell5/review-clippy-final.log` |
| rustfmt check, all touched Rust sources | PASS | `/tmp/varos-shell5/review-fmt.log` |
| `git diff --check` | PASS | Final scope check |
| Native release build | PASS (5m 01s) | `/tmp/varos-shell5/review-build.log` |
| WASM check | PASS (2.49 s) | `/tmp/varos-shell5/review-wasm.log` |
| Final software capture | PASS; regenerated and visually inspected, 1440×1000 | `/tmp/varos-shell5/review-capture.log` |
| Forbidden files / staged files / error.log | PASS: no forbidden diff, nothing staged, error.log removed if present | `/tmp/varos-shell5/review-scope.json` |

Exact final test result lines:

```text
test result: FAILED. 2524 passed; 10 failed; 15 ignored; 0 measured; 0 filtered out; finished in 19.55s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2548 filtered out; finished in 0.23s
```

The ten failures match the supplied baseline exactly: nine `panels::gradient::tests::*`, plus
`tests_rotateview::escape_after_closing_a_popup_or_during_a_ruler_origin_drag_keeps_the_rotation`.
Every `shell5_review_*` regression and the existing shell/tool tests passed. The earlier runs
exposed scrollbar interception, bottom-control spacing and the drawer outside-click exclusion;
those were fixed before the final run above. No all-green library-suite claim is made.

## Regression evidence

- `shell5_review_real_close_opacity_and_tear_off`: actual tool/strip UI opacity on first close,
  immediate tear-off cancellation and absence of a docked text copy during the old fade interval.
- `shell5_review_real_drop_never_moves_an_existing_box`: unchanged rect on every sampled frame
  after an actual tab drop, with normal and zero animation time.
- `shell5_review_real_drop_new_box_settles_and_reduced_motion_snaps`: real edge drop, new-box
  settle, exact endpoint and instant reduced-motion placement.
- `shell5_review_character_paragraph_options_in_all_hosts`: both flags toggled twice in boxes,
  floating panels and strip flyouts.
- `shell5_review_native_menu_and_chrome_hit_areas`: shell ON with and without a native backend;
  menus absent/present respectively; panel/strip/header controls at least 16×16.
- `shell5_review_tools_escape_workspace_and_screen_modes`: real drag cancellation, independent
  tools positions through workspace switches and inert grips in screen modes 1–2.
- `shell5_review_bottom_controls_and_fill_stroke_visibility`: visible/in-bounds Edit Toolbar
  opens the drawer, chips and Draw mode remain accessible, hiding wells reduces float height.
- `shell5_review_strip_slide_toward_its_rail_and_exact_final_rect`: both rail sides, exact rect
  after at least 90 ms; the original first-open test also restores an exact settled-rect assertion.
- `shell5_v4_migrates_and_v5_tools_roundtrip`: standalone `shell/fixtures/v4.json`, authored
  against the stage-4 schema at `109db6dc72c6636860cbc15534c8fe0ca2f1bad1`, with custom workspace
  and workspace-change arrangements and no v5 `tools_pos` fields. No v5 serialization/relabeling.

## Commands

All cargo commands ran one at a time, with:

```sh
export CARGO_TARGET_DIR="~/Documents/AI workspace/lanes/_target-shared"
cargo test -p vectorcraft-ui-egui --lib --release --offline -j 2 -- --test-threads=2
cargo clippy --workspace --all-targets --offline -j 2 -- -D warnings
VAROS_SHELL_CAPTURE_FINAL=/tmp/varos-shell5/shell5-capture.png cargo test -p vectorcraft-ui-egui --lib --release --offline -j 2 capture_shell5_final -- --ignored --test-threads=2
cargo build -p vectorcraft --release --offline -j 2
cargo check -p vectorcraft-web --target wasm32-unknown-unknown --offline -j 2
```

Formatting used `rustfmt --edition 2024 --check` on every touched Rust source, followed by
`git diff --check`. No forbidden source or asset was changed; no commit or stash was created.

## Final report

All requested review fixes are implemented. Automated review regressions pass; the full library
suite retains only its ten known release-only failures. Final capture:
[shell5-capture.png](/tmp/varos-shell5/shell5-capture.png).

Visual inspection confirms the Board tools, seam, bottom controls, app bar, open panel flyout,
workspace row and separate status area. This is a software-rendered capture and headless test
coverage; no installed app was replaced and native owner hand acceptance remains pending.
Native release build, WASM, clippy, formatting and whitespace gates passed. Screen mode remains
in View; P3-7 remains deferred and the P3-10 5% fade start remains accepted, as requested.
