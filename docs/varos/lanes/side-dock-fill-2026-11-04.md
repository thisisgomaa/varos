# Side-docked scrolling panels — 2026-11-04

Status: done, owner accepted 2026-11-04. Owner reported empty space below Layers and Properties.

The side-dock arranger limited every full host to its saved expanded height. Layers, Properties
and Libraries have internally scrolling bodies and should instead consume the available column
height. `geometry::fills_side_dock` now selects hosts with an active full internally scrolling
panel, including mixed full/compact columns. Such side hosts use the available dock height and
have no ineffective vertical host resize grip. Fixed control tabs still fit their content;
floating hosts keep remembered sizes. Saved workspace data is not rewritten by rendering.

The existing inner margin budgets were checked and retained. No upstream body implementation,
shortcut, tab identity or drag event routing was changed. New real-frame regressions cover window
height changes, fixed/list tab switching without saved-size mutation, and compact sibling columns.

Local checks passed: `cargo test --locked -j 4 -p vectorcraft-ui-egui -- --test-threads=4`
(2,792 unit + 2 integration, zero failures, 27 ignored); workspace Clippy/all-targets/-D warnings;
`cargo xtask wasm`; fmt and diff checks. The optional software capture passed and the Layers
bottom toolbar was visually inspected in `plan/side-dock-fill-2026-11-04/panels.png`.
An additional regression verifies the actual Layers bottom-bar count sits at its expected inset.
Claude Fable independently reviewed the scoped geometry fix and found no blocker; response saved
in `plan/side-dock-fill-2026-11-04/fable-review.md`. Release build passed in 4m 39s. Signed build **20261104.4** was installed after the owner saved and closed the app; all five installed files match the
manifest and deep/strict codesign verification passed. Settings retained; no app launch.
Manifest/source patch: `dist/side-dock-fill-2026-11-04/`; patch SHA-256
`7acc805ff5c6204de030e47cfcb9d885df5902adc207f94eb59a6752d7af2e64`.
The preceding build 20261104.3 is retained for rollback.
