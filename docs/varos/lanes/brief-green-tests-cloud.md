# Task — make the release test suite fully green (repo thisisgomaa/varos-next, branch off `main`)

Create branch `lane-green-tests-cloud` from `main`; when done push it and open a PR to `main` titled
"test(ui): the release suite is green (gradient panel + rotate-view tests)". Do not merge.

Context: `cargo test -p vectorcraft-ui-egui --lib --release` has had 10 "known release-only failures" for
weeks, so every lane ignores them. End that:
- 9 tests in `crates/ui-egui/src/panels/gradient.rs` (`panels::gradient::tests::*`) panic at
  `gradient.rs:951` in release only (they pass in debug). Find the root cause (release-only behaviour:
  debug_assert vs assert, overflow checks, float rounding in hit testing, timing/animation, test
  harness geometry) and fix it properly — in the test if the test is wrong, in the panel if the panel is
  wrong. No `#[ignore]`, no `cfg(debug_assertions)` gating of the tests.
- `tests_rotateview::escape_after_closing_a_popup_or_during_a_ruler_origin_drag_keeps_the_rotation`
  (`crates/ui-egui/src/tests_rotateview.rs:337`) fails at line ~344 in release only; same treatment.
- Also `tests_typo::layout_10k_area_text_is_fast` is load-sensitive: make its budget robust (measure
  relative to a baseline in the same process, or widen with a documented reason), never flaky.
Scope: only those test files/modules and the minimal panel code they prove wrong. Do NOT touch
`crates/ui-egui/src/varos/shell/*`, `chrome.rs`, `toolbar.rs`, `home*.rs`, `theme.rs` colours, the
picker, engine/render/text crates (other branches own them).
Gates (exact result lines in the PR): `cargo test -p vectorcraft-ui-egui --lib --release` with 0 failures,
`cargo test -p vectorcraft-ui-egui --lib` (debug) with 0 failures, `cargo clippy --workspace --all-targets
-- -D warnings`, `cargo fmt --check`, `git diff --check`. In the PR: the root cause of each failure in one
sentence each.
