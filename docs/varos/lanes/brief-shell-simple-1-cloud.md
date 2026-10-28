# Task — finish and validate the simple shell look (repo thisisgomaa/varos-next, branch `lane-look`)

Check out the existing branch `lane-look` (do NOT branch from main). Its last commit
"wip(shell-simple-1): …" is an unvalidated implementation by a previous Codex run of
`docs/varos/lanes/brief-shell-simple-1.md` (read that brief fully first, then
`docs/varos/SHELL_SIMPLE_DIRECTION.md` and the mockup `docs/varos/mockups/shell-simple-2026-10-28.html`,
"Editor" view). The previous run stopped while the test binary was compiling; its last notes:
- it restored the drop geometry between groups after removing the extra grips (only the top grip drags);
- it fixed an unintended shell-OFF change (a UI scope added to legacy panels) — the legacy path must be
  byte-for-byte unchanged when the pref "Varos shell (new)" is OFF.

## Do, in order
1. Read the branch diff against `main` (`git diff main...lane-look`) and re-check every item A–F of the brief
   against the mockup; finish anything missing, tests included.
2. `cargo test -p vectorcraft-ui-egui --lib --release` — fix every failure except the known pre-existing
   release-only ones (9 `panels::gradient::tests::*` and
   `tests_rotateview::escape_after_closing_a_popup_or_during_a_ruler_origin_drag_keeps_the_rotation`).
   Repeat until only those remain. Then `cargo test --workspace --release` once (same known list).
3. Captures: run the ignored tests `VAROS_SHELL_CAPTURE_SIMPLE` and `VAROS_SHELL_CAPTURE_SIMPLE_COLS`
   (1440×900), commit the PNGs as `docs/varos/mockups/shell-simple-capture.png` and
   `shell-simple-cols-capture.png`, LOOK at them and compare with the mockup; fix visual misses
   (flat look, no rounded box frames, one hairline between neighbours, 28×28 tiles, 11/12 pt type,
   doc tabs in the top bar, no Workspaces row, no fixed control-bar row, Control panel exists).
4. `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --check`, `git diff --check`,
   `cargo build -p vectorcraft --release`.
5. Write `docs/varos/lanes/shell-simple-1-report.md`: per item A–F what changed with file:line, a gate
   table with the exact result lines, the captures, 5 plain hand-test steps for the owner.
6. Commit as conventional commits on `lane-look` (`feat(shell): …`, `fix(shell): …`, `test(shell): …`),
   push, and open a pull request to `main` titled "feat(shell): the simple look (flat docked panels, top-bar
   document tabs, Control panel, tool columns)". Do not merge.

Rules: never edit `theme.rs` colour values, engine/render/text crates, or `home*.rs` (another branch);
icons only from `crates/ui-egui/assets/varos/icons` (Lucide, ISC). The OFF window must stay identical
(existing non-shell tests prove it).
