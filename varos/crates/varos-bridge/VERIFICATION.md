# Slice 2 verification — 2026-10-07

Completed locally in `feat/bridge-slice2`. No commit, push, merge, installation, GUI launch, `/Applications` operation, or osascript. Slice 1's live move/recolour/one-undo acceptance is confirmed by the owner in the work order; slice 2's live acceptance remains pending.

## Done / deferred

| Done | Source |
|---|---|
| `add_shape (rect/ellipse)` | [design.rs:134](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:134); [core command / allocated ID:384](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-core/src/command.rs:384) |
| `resize` | [design.rs:235](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:235) |
| `rotate (absolute)` | [design.rs:260](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:260) |
| `rename` | [design.rs:203](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:203) |
| `delete` | [design.rs:266](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:266) |
| `align` | [design.rs:267](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:267); [rigid core units:1532](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-core/src/editor.rs:1532) |
| `distribute (h/v centers)` | [design.rs:313](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:313) |
| `group / ungroup` | [design.rs:328](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:328) / [design.rs:352](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:352) |
| `order` | [design.rs:364](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:364) |
| `set_paint including opacity` | [design.rs:191](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/design.rs:191) |
| `describe geometry` | [service.rs:719](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/service.rs:719) |
| `snapshot (CPU PNG)` | [service.rs:325](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/service.rs:325); [CLI output:31](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/cli.rs:31) |

Request-local `$name` resolution, exact destructive grants and final publication use the same service: [service.rs:355](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/src/service.rs:355). Checked isolated staging: [bridge.rs:701](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-core/src/bridge.rs:701). Successful create/delete no-ops reserve exposed IDs without an undo step: [editor.rs:3347](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-core/src/editor.rs:3347). Human tool/selection/defaults are preserved; removed references are pruned. Equal-center distribution ties and grouping transform choices use stable IDs.

Deferred: `add_path` (no safe anchor-creation command/schema in this slice) and corner radius (plain rectangle anchors, [model.rs:1041](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-core/src/model.rs:1041)). Finer within-path geometry pagination, group/gap distribution, reparenting, files/export/save, headless hosting and Windows attachment are also outside this slice. Geometry is bounded to 1,000 anchors/object and 16 KiB pages, with explicit refusal for an oversized object.

## Gates

All five requested gates exit **0** on the final source, from `varos/`:

1. `cargo test --workspace -j 4`: **1,176 passed / 0 failed / 10 ignored**, across 69 unit/integration/doc-test suites. Bridge: 40 passed / 2 ignored.
2. `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
3. `cargo fmt --all --check`: PASS.
4. `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`: PASS (compilation only).
5. `python3 ../tools/check_dep_directions.py`: PASS. Intentional Bridge → raster edge added; core/raster/CLI/Bridge UI-dependency prohibitions retained.

`git diff --check`: PASS. Tests construct no GPU renderer or EventLoop. Coverage includes each verb's outcomes/errors/indexed rollback; request-local names; grant refusal/payload binding/receipt retries; one undo/redo; allocator branching and successful create/delete no-ops; rigid group alignment; rotated local resize; deterministic center ties; bounded geometry; decoded valid PNG dimensions; explicit CLI output; frozen poster MCP/CLI parity through the actual MCP stdio binding. Existing slice-1 contracts remain green.

The two ignored socket tests were **attempted**, exit **101**, **0 passed / 2 failed**, both at Unix bind with `Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" }` (`EPERM`). This is sandbox-blocked native IPC, not a pass. Moderator rerun: `cargo test -p varos-bridge --test contracts -- --ignored`.

## Moderator handoff / unverified

The [README poster batch](/Users/gomaa/Documents/AI workspace/varos/.claude/worktrees/bridge-slice2/varos/crates/varos-bridge/README.md:98) is the exact [request fixture](tests/fixtures/poster-request-1.0.json). Replace only board/revision/request ID with live values. The [frozen result](tests/fixtures/poster-result-1.0.json) proves five paths, one named group, equal-circle centers and one undo step. [CPU poster preview](/tmp/varos-bridge-slice2-poster.png) was rendered and visually inspected; it is not a live-window screenshot.

Local native binaries rebuilt successfully: `varos/target/debug/varos`, `varos-cli`, `varos-bridge`. Desktop build and separate CLI/Bridge build both exit 0. Nothing installed or launched.

Unverified: native socket lifecycle/peer credentials and real binary attached MCP/CLI parity (moderator socket rerun), live poster creation/visible rendering/human one-step undo, actual external-agent attachment, Windows runtime, large-board responsiveness and GPU/CPU pixel parity. No independent review or merge is claimed.

Logs: `/tmp/varos-bridge-slice2-workspace-tests.log`, `...-clippy-native.log`, `...-clippy-windows.log`, `...-socket-tests.log`, `...-build.log`, `...-build-tools.log` (same `/tmp/varos-bridge-slice2` prefix).
