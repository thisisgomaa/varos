# Attached slice verification — 2026-10-06

Final source verified in the bridge-slice1 worktree, without starting the GUI, installing binaries, committing or pushing.

Required gates (from `varos/`), all exit 0:

- `cargo test --workspace -j 4`: 1,155 passed, 0 failed, 10 ignored across workspace unit/integration/doc tests.
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`

Additional checks: `python3 tools/check_dep_directions.py` and `git diff --check` passed. The missing existing raster SVG-test `resvg` dev dependency was supplied to make the workspace tests runnable.

Builds completed: `cargo build -p varos-app --bin varos` and `cargo build -p varos-cli -p varos-bridge`. Native binaries are `target/debug/varos`, `target/debug/varos-cli`, and `target/debug/varos-bridge` (relative to `varos/`). Nothing was installed.

Headless coverage includes frozen MCP/CLI fixtures, indexed atomic rollback, single-step undo, revision conflict, invalid/human field settlement, active gestures including Pen paths between clicks, inactive/Home boards, scope and token/UID refusal with fake hosts, cancellation, bounded journals/cursors, confirmation digests and receipts. No test constructs a GPU renderer or event loop.

Two real Unix-socket tests were attempted and failed at bind with sandbox `EPERM`; they are explicitly ignored by default, not counted as passes. The other eight ignored tests predate this slice. Run the two socket tests in a normal macOS terminal using the README command. Native listener permissions/peer credentials, actual Claude attachment, visible rendering, owner-driven single ⌘Z, saving, and large-board performance remain unverified. No independent reviewer was used.

README contains the exact owner trial sequence and official Claude Code command-syntax source; the documentation was verified live, but no Claude registration was made. The temporary owner-issued history grant has no UI.

Gate/build logs from this run: `/tmp/varos-bridge-workspace-tests.log`, `/tmp/varos-bridge-clippy-native.log`, `/tmp/varos-bridge-clippy-windows.log`, `/tmp/varos-bridge-build.log`, `/tmp/varos-bridge-build-tools.log`.
