# Slice 2 review follow-ups — 2026-10-07

Completed in `fix/bridge-slice2-followups`, only in this worktree. No commit, push, merge, GUI, osascript, installation or owner-window verification. Artboard code, `format/` and `model.rs` are unchanged. The original slice-2 report below is historical.

| Follow-up | Implementation / evidence (relative to `varos/`) |
|---|---|
| Explicit grouped leaves | `crates/varos-bridge/src/design.rs:237` permits resize/rotate without widening to siblings; `crates/varos-core/src/editor.rs:1824` rotates partial leaves about their own bounds. `tests/contracts.rs:1542` checks leaf bounds, sibling preservation, group bounds and one-step undo, including rotated groups. Partial rotated-group resize returns its missing-local-frame reason and the alternative `move` or whole-group `resize`. Partial rotation is baked geometry in the inherited unit frame, not a separately persisted leaf angle; documented in README. |
| Whole-group order | `crates/varos-bridge/tests/contracts.rs:1593` exercises all four order values with explicit `node:<group>`, checks contiguous member order and undo. |
| Geometry limit | `crates/varos-bridge/src/service.rs:345` advertises 16 KiB pages, roughly 300 typical anchors, 1,000 absolute anchors/object and no anchor pagination. `:785` explains individual-object overflow without suggesting a smaller limit; `tests/contracts.rs:1663` checks byte overflow at `limit:1`. |
| Snapshot worker | `crates/varos-bridge/src/service.rs:29` owns the pinned clone/render job; `crates/varos-app/src/bridge_host.rs:114` runs raster/encode on a worker and replies through the existing channel. Cancellation checkpoints precede raster, encode and delivery. Worker-spawn failure replies `busy`. App test `:187` checks a captured revision survives a later human edit without field commit/history side effects. |
| Legacy AddShape | `crates/varos-cli/README.md:84` and `:111` document API 0.x acceptance and its RGBA payload. Chosen because the provisional API exposes the checked core command table; a separate legacy filter would introduce another verb policy. |
| Confirmation | `crates/varos-bridge/tests/contracts.rs:1645` rejects edit-envelope `confirm:true`. `src/service.rs:442` and `:508` refresh missing/expired exact grants into full challenges; `:1185` expires both destructive/history grants deterministically, resends their digests, checks no mutation and successfully retries the refreshed challenge. |
| Explicit creation paint | `crates/varos-bridge/src/design.rs:171` refuses absent/null fill and stroke with `invalid_argument: paint required`, preserving ADR-0009 explicit paint. `tests/contracts.rs:1645` covers both forms; README documents the choice. |

## Snapshot measurements

Headless **release** probe: 5,000 four-anchor filled paths, one warm-up and five measured samples per size. Times include owning-thread document clone, CPU raster and PNG encode; they exclude service observation/projection and scheduling. `crates/varos-raster/src/lib.rs:455`:

```sh
cargo test -p varos-raster --release snapshot_five_thousand_paths_timings -- --ignored --nocapture
```

| Dimensions | Median clone | Median raster | Median encode | Median total | Total range |
|---|---:|---:|---:|---:|---:|
| 544×246 | 0.100 ms | 337.190 ms | 0.364 ms | **337.655 ms** | 277.184–399.350 ms |
| 1024×1024 | 0.100 ms | 292.115 ms | 1.689 ms | **293.906 ms** | 277.149–326.248 ms |

Both exceed 8 ms, so desktop raster/encode now run off the owning thread. Clone remains on it. This is a bounded fixture measurement, not a general large-document/UI responsiveness claim. Probe: **1 passed / 0 failed**. All samples: `/tmp/bridge-followups-snapshot.log`.

## Final gates

From `varos/`, all exit **0**:

1. `cargo test --workspace -j 4`: **1,183 passed / 0 failed / 11 ignored**, 69 suites. Bridge: **46 passed / 2 ignored** (unit + integration). The additional ignored test is the explicit timing probe above.
2. `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
3. `cargo fmt --all --check`: PASS.
4. `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`: PASS, compilation only.
5. `python3 ../tools/check_dep_directions.py`: PASS.

`git diff --check`: PASS. No tests construct a GPU Renderer or EventLoop. Logs: `/tmp/bridge-followups-workspace-tests.log`, `/tmp/bridge-followups-clippy-native.log`, `/tmp/bridge-followups-clippy-windows.log`. Native socket/owner-window behavior and Windows runtime remain unverified; no independent review, merge or installed binary is claimed.

---

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
