# Independent review fix round — 2026-10-08

Completed in the same `feat/bridge-file-keystore` worktree, uncommitted. No commit, push, GUI, osascript, installation, /Applications writes or real Keychain access. The moderator's `"api":"1.0"` history request fix is preserved (`tests/contracts.rs:615`). The reports below this fix-round section describe earlier source states.

## Review items (paths relative to this crate unless stated otherwise)

1. `src/service.rs:300`: canonical bN validation restored before observation/prepare; regression in `tests/contracts.rs:2225`.
2. `../varos-app/src/bridge_host.rs:95`, `src/files.rs:14,27`: passwd home, never HOME; home / refused. Unset/forged HOME subprocess checks at `src/files.rs:164`.
3. `src/conn/credentials.rs:277`: platform store uses `Paths.state/keys`; override subprocess test at `:375`. Debug VAROS_BRIDGE_HOME relocates keys with state/discovery; release ignores it.
4. `src/files.rs:23,121`: iCloud Mobile Documents and CloudStorage provider exceptions; Application Support still refused. Both acceptance and canonical use-time paths covered; /Volumes requires a volume component.
5. `src/service.rs:361`: home_or_external_volume_or_cloud_drive and local_volume_only guards; `../varos-app/src/bridge_fs.rs:33,122`: network volume not supported, typed scope_refused.
6. `../varos-app/src/bridge_fs.rs:66,240`: linkat first, macOS ENOTSUP/EPERM/EXDEV exclusive rename fallback. Fake ENOTSUP success, collision and link-success cases tested at `:81`. Real FAT32/exFAT volumes remain unverified.
7. `../../../docs/adr/ADR-0011-bridge-connection-and-trust.md:194,196,207`, `README.md:222,260`, repository `README.md:54`, `src/mcp.rs:153`: supersession, precise destination policy and legacy token listener all-scopes behavior documented.
8. `src/conn/credentials.rs:203`, `src/dto.rs:536`, `src/conn/manage.rs:10`, `tests/contracts.rs:702`, `src/conn/audit.rs:2`, `src/conn/attach.rs:35,95,125`, `src/service.rs:106`, `src/ipc.rs:336,692,790`: obsolete pairing text/retryability/usage/parameter/profile-error codes and dead Context fields/literal gaps cleaned up.
9. `src/conn/audit.rs:23`: empty label omitted by serde.
10. `src/conn/credentials.rs:130`: state parent checked before keys creation/read; symlinked-state regression at `:361`.
11. `../../../tools/mac/bridge-connect.sh:17`: owner command restored to cargo build -q.
12. `src/files.rs:14`, `../varos-app/src/bridge_host.rs:117,398`: containment rejects /tmp and /private/var immediately at acceptance; canonical use-time check retained at `src/files.rs:60`.
13. Evidence boundary: cross-uid refusal: policy function tested; kernel path not testable without a second user. Real exFAT unverified.

Optional durability improvement completed: `src/conn/credentials.rs:234` fsyncs keys directory after rename on Unix.

## Gates on final Rust source

All compile/test/lint commands use --offline -j 2, from varos/.

- cargo test --offline -j 2 --workspace: PASS, exit 0; 1,306 passed / 0 failed / 15 ignored, 79 workspace suites. Child subprocess executions are excluded from this count. Bridge: 94 passed / 0 failed / 6 ignored (16 unit, 12 artboard, 14 connection, 52 contracts).
- cargo clippy --offline -j 2 --workspace --all-targets -- -D warnings: PASS, exit 0.
- cargo clippy --offline -j 2 --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings: PASS, exit 0; compile only.
- cargo fmt --all --check: PASS, exit 0.
- python3 ../tools/check_dep_directions.py: PASS, exit 0.
- cargo test --offline -j 2 -p varos-bridge --test contracts -- --ignored: exit 101; 0 passed / 4 failed, all socket binds blocked by expected sandbox EPERM. No native IPC behavioral pass claimed here. Moderator reported all four socket tests passing on the Mac before this fix round.
- git diff --check: PASS.

Logs: /tmp/bridge-fix-workspace.log, /tmp/bridge-fix-clippy-native.log, /tmp/bridge-fix-clippy-windows.log, /tmp/bridge-fix-sockets.log. No requested implementation item deferred. Windows runtime, real network mounts, real FAT32/exFAT, the cross-uid kernel path and live GUI/file-worker behavior are unverified here.

---

# Open local trust verification — 2026-10-08

Current work on `feat/bridge-file-keystore` builds on the existing uncommitted file key store change. The older reports below are historical; their pairing/grant requirements are superseded by ADR-0011 Amendment 3. No commit, push, merge, GUI, osascript, installation, `/Applications` write or real Keychain access.

## Changes and retained guards

- `src/ipc.rs:185,649,688`: same-uid check remains; the signed handshake admits every scope without consulting trust/pairing files. Cross-uid is refused. Ed25519 profiles identify audit/history attribution. `conn/trust.rs` now contains only the capability vocabulary and profile lock name.
- `src/service.rs:300,491,503`: removed scope gates and destructive/history confirmation state. Board/revision checks, atomic batch publication, one-step undo, cancellation and idempotent receipts remain.
- `src/conn/manage.rs:25,65`: pair/approve/deny and agents list/revoke are compatibility no-ops, with the exact owner-decision message and exit 0. Register code is unchanged; status prints `trust: local user`.
- `../varos-app/src/bridge_host.rs:79` and `src/files.rs:35`: removed `VAROS_BRIDGE_FILE_ROOTS`. Current-backing save retains fingerprint checks; new save_as/export names may resolve under home or `/Volumes`. Parent aliases are canonicalized before containment checks; internal aliases remain safe, escaping aliases are refused. Protected roots, dot components, extensions and existing-destination refusal remain.
- `../varos-app/src/bridge_fs.rs:63,180`: canonical directory identity remains pinned, final symlinks are never followed, and fresh publication uses linkat without replacement. Existing names/hard-link aliases and publication races return `save_conflict`. Local-volume checks remain. Grant rechecks were removed throughout file_jobs/file_ports/lifecycle.
- `src/conn/audit.rs:23` and `src/ipc.rs:767`: accepted calls retain profile id and now include sanitized client label; asynchronous file-completion audit copies both. Mutations still reserve audit capacity first.
- `src/service.rs:363`, `src/mcp.rs:147`: capabilities reports local-user trust and the guard list; files_roots_granted is gone. Tool descriptions no longer direct clients through grants. Optional digest fields and the unused pairing error-details field remain API 1.0 wire compatibility only; no challenge/pairing state is produced or consumed.
- ADR-0011 Amendment 3 quotes the owner, explains same-uid full edit/save access and the undo + review model, and supersedes the old C2 authorization plan. ADR-0009 §8, READMEs, bridge-connect text and PLAN track A are updated.

Core history has no metadata slot: `varos-core/src/editor.rs:452` stores Vec<Document> snapshots. Core is untouched; no `by` field is added to document snapshots. “هيستوري منفصل لكل AI للمراجعة” is the next Bridge piece in PLAN A3-next, not a completed feature. The local audit and claimed labels are not tamper-proof vendor attestation.

## Gates

All Cargo compile/test/lint runs use `--offline -j 2`.

- `cargo test --offline -j 2 --workspace`: PASS, exit 0; **1,301 passed / 0 failed / 15 ignored**, 79 suites. Bridge: **90 passed / 0 failed / 6 ignored** (13 unit + 12 artboard + 14 connection + 51 contract).
- `cargo clippy --offline -j 2 --workspace --all-targets -- -D warnings`: PASS, exit 0.
- `cargo fmt --all --check`: PASS, exit 0.
- `cargo clippy --offline -j 2 --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`: PASS, exit 0 (compilation only).
- `python3 ../tools/check_dep_directions.py`: PASS, exit 0.
- `cargo test --offline -j 2 -p varos-bridge --test contracts -- --ignored`: attempted, exit 101; **0 passed / 4 failed**, all at Unix socket bind with EPERM. Sandbox-blocked IPC, not a behavioral pass.
- `git diff --check`: PASS. Core and all frozen fixtures are untouched.

Headless evidence includes the actual signed host handshake over in-memory framed IO (all scopes, no persistent pairing/trust state), uid refusal, remembered/concurrent profiles, CLI no-ops, schema/capability reporting, direct destructive/history execution and receipts, fake-home save/save-as/PDF without an environment grant, protected/dot/extension/overwrite/hard-link/symlink-escape refusals, and pinned publication races. Obsolete pairing/confirmation/revocation tests were replaced or removed with those obsolete policies.

The actual CLI was also run: `bridge pair --approve ignored` and `bridge agents revoke ignored` print exactly `not needed: local agents are trusted (owner decision 2026-10-08)`; `bridge status` prints `trust: local user`. All exit 0. Register's existing syntax/output test remains passing.

Unverified: native socket attachment/restart/audit lifecycle (four updated ignored contracts blocked at bind), live desktop/external-client behavior, mounted external-volume IO and Windows runtime. Per-AI review history is planned because core has no entry metadata slot. No independent review, installation or owner visual acceptance is claimed.

Logs: `/tmp/open-trust-gate-workspace.log`, `/tmp/open-trust-gate-native.log`, `/tmp/open-trust-gate-windows.log`, `/tmp/open-trust-gate-fmt.log`, `/tmp/open-trust-gate-dependencies.log`, `/tmp/open-trust-final-sockets.log`.

---

# File key store verification — 2026-10-08

Implemented locally on `feat/bridge-file-keystore` (base `945f6d6`). No commit, push,
merge, GUI, osascript, installation or `/Applications` changes. No real Keychain read,
write, deletion or migration was attempted. All Cargo compile/test/lint commands used
`--offline -j 2`.

## Changes

`conn/credentials.rs:107`: `FileKeyStore` is the default on every OS. Raw 32-byte seeds
use `keys/host.key` and `keys/agent-<profile-id>.key`. Unix creation makes missing
ancestors and the key directory 0700, and exclusive temporary files 0600; synced
contents are published by rename under an owner-only creation lock. Windows uses
exclusive temporary creation and `MoveFileW` (no overwrite), with Unix permission
checks cfg'd out. Reads validate the parent and key type/owner/mode, reject symlinks
and unsafe permissions, and report repair instructions without secrets. Existing
keys win creation races. Delete removes the file. `MemoryStore` remains for tests.

`conn/fsutil.rs:69` shares the per-user application-support path with `Paths::resolve`;
Windows keys use `%APPDATA%\Varos\bridge\keys\`. `security-framework`, its sys
package, `KeychainStore`, the unavailable platform backend and the ignored real
Keychain round-trip test are removed from code/manifests/lockfile.

`conn/attach.rs:123` reports `pairing_required` for a missing explicit identity;
`conn/manage.rs:205` adds the one-time re-pair notice to `bridge hosts` (there is no
separate bridge status command). Registration code/output is unchanged. The app's
existing unavailable notice and IPC/CLI messages now describe file keys and repairs.

ADR-0011 Amendment 2 records the owner's override of “never downgrade to a plaintext
file”, rebuild/ACL prompt rationale, untouched old keys, one-time re-pair and the
same-uid threat limit. README, conn module docs and bundle signing comments agree.
0600 protects against other users, not same-uid processes. Windows user-profile ACLs
and Windows runtime behavior were not tested.

## Final gates

- `cargo test --offline --workspace -j 2`: PASS, **1,307 passed / 0 failed / 15 ignored**, across 79 suite summaries (including doc tests).
- `cargo clippy --offline --workspace --all-targets -j 2 -- -D warnings`: PASS.
- `cargo clippy --offline --workspace --all-targets --target x86_64-pc-windows-msvc -j 2 -- -D warnings`: PASS.
- `cargo fmt --all --check`: PASS.
- `python3 ../tools/check_dep_directions.py`: PASS.
- `git diff --check`: PASS.
- `cargo test --offline -p varos-bridge -j 2 --test contracts -- --ignored`: attempted, exit 101, **0 passed / 4 failed**, all at native Unix socket bind with **EPERM**. Sandbox-blocked, not a pass.

Coverage: file create/load/delete, host/agent isolation, exact modes, 0644 refusal
with chmod repair instruction, symlink-parent refusal, two simultaneous creators
retaining one key, file-backed profile reuse/revocation, full signed handshake →
pairing → approval → host-key replacement → re-approval, and old public profiles
without file credentials. The two paired real-socket contracts now use file stores.
An initial workspace attempt caught non-private missing state ancestors; fixed with
recursive 0700 directory creation, then the full final suite passed.

## Re-pair and unverified

First launch creates a new host identity and fresh agent profile keys; old Keychain
items remain untouched. Previously paired agents require one owner approval again.
Registry-matching host identity changes retain the existing typed `pairing_required`
flow. An explicit old missing `--identity` asks to remove that reference and pair again.
`varos-cli bridge register claude` remains unchanged; registration tests passed.

Unverified: native socket lifecycle and real-binary attached MCP/CLI behavior (rerun
ignored contracts outside this sandbox), live external-agent pairing, installed-app
behavior, Windows runtime/ACLs. No independent review, rebuild/install or merge claim.

Logs: `/tmp/bridge-keystore-tests.log`, `/tmp/bridge-keystore-clippy-native.log`,
`/tmp/bridge-keystore-clippy-windows.log`, `/tmp/bridge-keystore-fmt.log`,
`/tmp/bridge-keystore-dependencies.log`, `/tmp/bridge-keystore-sockets.log`.

---

The earlier slice verification below is historical evidence, not this change's gate run.

# Slice 4 verification — 2026-10-07

Implemented locally on `feat/bridge-slice4`, based on `9058ab5`. No commit, push, merge, GUI,
osascript, installation, `/Applications` write, network access or real Keychain write. Cargo build,
test and lint commands used `--offline -j 2`; format and dependency checks made no network calls.
No ratchet ceilings or existing frozen fixture bytes were changed.

## Changes (paths relative to `varos/crates/`)

- `varos-core/src/command.rs:27`, `:403` and `bridge.rs:110`: checked `AddPath`, finite explicit
  points/handles, 2–1,000 anchors, fresh high-water IDs, existing parent/paint validation.
- `varos-bridge/src/design.rs:305`, `:580`: creation and rounded-rect cubics, request-local names,
  radius refusal rules; rounded rectangles are editable paths without a live radius property.
- `varos-core/src/bridge.rs:938`: reorder/duplicate/color/clip; `editor.rs:2453` shares desktop
  artwork duplication, and desktop duplicate now assigns a fresh page ID. Active page follows ID.
- `varos-bridge/src/{dto,mcp,lib,service}.rs`: standalone save/save_as/export tools, complete
  schemas/capabilities, selection detail and filename-only list metadata. `conn/trust.rs:25` and
  `ipc.rs`: files scope defaults OFF, owner approval parsing/intersection/rechecks include it.
- `varos-app/src/bridge_host.rs:59`, `file_jobs.rs:176`, `lifecycle.rs:413`, `file_ports.rs:308`:
  worker submission without inline fallback, accepted ticket/status, pinned document snapshot,
  successful checkpoint and truthful durability; no completion dialogs/field settling.
- `varos-bridge/src/files.rs:9`, `varos-app/src/bridge_fs.rs:16`: canonical root/backing-alias
  policy; pinned no-follow directory traversal, parent identity recheck, fresh-file publication
  without overwrite, current-file fingerprint checks, worker/pre-publication scope rechecks and
  Mac local-volume check. Launch roots are temporary owner policy; eight pending file jobs maximum.
- `varos-bridge/tests/contracts.rs:2117`, `tests/artboards.rs:503`, new logo fixtures: indexed
  rollback, path/radius validation, fresh identities after undo, page properties, schema completeness,
  fake-host save tickets, new frozen adapter parity and actual MCP stdio. Core/app tests cover
  nonfinite handles, pinned-directory/destination races, revocation, save/reopen, pure PDF and the
  saved-snapshot checkpoint while later human edits remain dirty. Old poster/story fixtures pass.
- `varos-bridge/README.md:231`, `:275`: new verbs, files policy and second logo/export example;
  ADR-0009/0011 have dated implementation amendments.

## Final gates

| Gate | Result |
|---|---|
| `cargo test --offline -j 2 --workspace` | **1,255 passed, 0 failed, 16 ignored**, 72 result lines |
| `cargo clippy --offline -j 2 --workspace --all-targets -- -D warnings` | PASS |
| `cargo fmt --all --check` | PASS |
| `cargo clippy --offline -j 2 --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` | PASS (compilation only) |
| `python3 ../tools/check_dep_directions.py` | PASS |
| `cargo test --offline -j 2 -p varos-bridge --test contracts -- --ignored` | **0 passed, 4 failed**, exit 101: all listener binds refused with EPERM |

Five gates pass; the sixth is sandbox-blocked native IPC, not a pass. The four socket tests were
attempted unchanged. `git diff --check` passes. Logs use `/tmp/bridge-slice4-` with suffixes
`workspace-tests.log`, `clippy-native.log`, `clippy-windows.log`, `fmt.log`,
`dep-directions.log`, and `sockets.log`.

## Limits / unverified

- Native attachment, peer credentials, real attached CLI/MCP behavior and socket lifecycle remain
  unverified because this sandbox denies Unix listener bind. No test was weakened.
- Live desktop appearance, UI responsiveness and owner hand-testing were not performed (GUI forbidden).
  Windows runtime/file hosting remains unsupported; only target compilation is verified.
- Explicit destinations must be fresh filenames inside existing owner roots. Existing-file overwrite
  is refused under ADR-0009 §8; no exact overwrite-confirmation ceremony was added. Worker path/policy
  refusals are completion receipts, so `accepted` alone does not imply a write.
- No independent review, merge, installed build or owner acceptance is claimed.

---

# Slice 3 / slice-2 follow-ups merge resolution — 2026-10-07

Resolved in `fix/bridge-slice2-followups`, merging main `8fd8659` into follow-ups `e5dc666`. The reports below are historical, retained newest slice first: slice 3, slice-2 follow-ups, original slice 2.

- `SnapshotJob` carries an optional persistent artboard id. The service validates the id and captures the revision-pinned document; both whole-board and per-artboard raster/PNG work go through `Host::snapshot` and the desktop worker. Page images report actual fitted dimensions, `artboard:N` and `CPU page preview`. Cancellation checkpoints and transport image limits are retained. Board defaults remain 544×246; page defaults remain 1024×1024.
- Capabilities retain readable formats 1–4, writable format 4, persistent artboard ids, artboard presets, deprecated aliases, and all geometry limits (16 KiB page budget, roughly 300 typical anchors, 1,000 anchors/object, no anchor pagination).
- Both README sections and both verification reports are retained. Automatic merges retain leaf resize inside groups, refusal of partial-group absolute rotation and partial rotated-group resize, paintless/invisible shape refusal, expired-grant re-challenges, legacy AddShape documentation, active-artboard settings tracking, and deprecated-alias refusal in page-verb batches. The older follow-up report's partial-rotation statement is superseded by the refusal behavior in the current README and contracts.
- Regression evidence: `bridge_host::tests::page_snapshot_worker_replies_with_captured_revision_after_human_edit` exercises the actual desktop worker, page metadata/aspect ratio, revision pinning and absence of field/history side effects. The artboard capabilities test asserts both slice-3 and geometry-budget fields.

## Gates on the resolved source (from `varos/`)

| Gate | Result | Exit |
|---|---|---:|
| `cargo test --workspace -j 4` | **1,214 passed / 0 failed / 11 ignored**, 71 suites; Bridge: 60 passed / 2 ignored | 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | 0 |
| `cargo fmt --all --check` | PASS | 0 |
| `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` | PASS, compilation only | 0 |
| `python3 ../tools/check_dep_directions.py` | PASS | 0 |

Logs: `/tmp/bridge-merge-workspace-tests.log`, `/tmp/bridge-merge-clippy-native.log`, `/tmp/bridge-merge-fmt.log`, `/tmp/bridge-merge-clippy-windows.log`, `/tmp/bridge-merge-dep-directions.log`. `git diff --check` on the five resolution/regression files passes; no conflict markers remain. Full staged whitespace checking reports existing PDF fixture whitespace from main (`future_v5_pdf.vrs`), whose bytes are preserved.

Staging and the merge commit are **blocked by the session filesystem restriction**: `git add` cannot create `/Users/gomaa/Documents/AI workspace/varos/.git/worktrees/bridge-followups/index.lock` (`Operation not permitted`). The working files are resolved, but the index still records the three conflicts. No commit, push, installation, GUI verification, independent review or Windows runtime verification is claimed for this resolution.

---

# Slice 3 verification — artboards (2026-10-07)

Completed locally in worktree `bridge-slice3`, branch `feat/bridge-slice3-artboards` on `a5f687b`. No commit, push, merge, installation, GUI launch, `/Applications` operation or osascript. Live owner acceptance (the Instagram Story batch on an open board, a page snapshot, then one ⌘Z) is pending.

## Done

| Piece | Where |
|---|---|
| Persistent artboard ids (`Artboard.id`, `Document::artboard_index`, `assign_artboard_ids`; allocated at every commit from the counter/high-water mark) | `varos-core/src/model.rs`, `editor.rs` (`commit`, `replace_doc`), `board.rs` (presets) |
| Format 4: `FORMAT_VERSION = 4`, `migrate_v3_to_v4`, artboard-id key refused in v1–v3 before typed decode, unique/non-zero ids, no dangling `active`, save-side id assignment on the clone | `varos-core/src/format/{mod,migrate,structure,validate,error}.rs`; ADR-0008 amendment; `docs/reference/VRS_FORMAT.md` §5/§6c/§12/§13 |
| Frozen fixtures: `fixtures/v4/` (4 files + SHA256SUMS + README), six refusals appended (`v5_future`, `future_v5_pdf`, `v3_artboard_id`, `artboard_duplicate_id`, `artboard_missing_id`, `active_out_of_range`); `v4_future`/`future_v4_pdf` now Malformed (bytes unchanged); v3-reader gate in the old-reader harness; native PDF byte fixtures re-blessed (model stream only) | `varos-core/tests/fixtures/`, `varos-pdf/tests/{frozen_v2,old_reader_harness,export_pdf}.rs` |
| Checked core page operations (add / set rect / rename / delete with the active rule / set active), id-stable `active` in staging, human artboard multi-selection re-pointed by id on publish | `varos-core/src/bridge.rs` |
| Bridge page verbs, `artboard:N` + request-locals, id/local align targets (deprecated `aN@rev` kept), describe ids + `active_artboard`, receipts `artboards_created`/`artboards_removed`, page `snapshot`, capabilities, complete MCP schemas | `src/{dto,design,service,mcp,lib}.rs` |
| One-page CPU raster at the page's ratio and background | `varos-raster/src/lib.rs` (`rasterize_artboard`) |
| `varos-cli export-pdf --artboard artboard:N` (id → ActiveArtboard scope on an in-memory copy) | `varos-cli/src/main.rs` |

## Gates (from `varos/`, final source)

Review fixes (Codex Sol REQUEST CHANGES, same day): `export-pdf` refuses an `--out` that resolves to its input (path, `..`, symlink, Unix hard link; no `--in-place`); the deprecated `aN@rev` alias is refused in any batch that also has a page verb (A/B/C delete-then-align regression); the page snapshot paints its background once (half-alpha test); ADR-0010 notes the TextBox schema is now format v5.

1. `cargo test --workspace -j 4`: **1,203 passed / 0 failed / 10 ignored** (71 result lines). Bridge: 10 artboard tests + 38 contracts (2 ignored by default).
2. `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
3. `cargo fmt --all --check`: PASS.
4. `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`: PASS (compile only).
5. `python3 ../tools/check_dep_directions.py`: PASS (no new crate edges).
6. `cargo test -p varos-bridge --test contracts -- --ignored`: the two real Unix-socket tests **ran and passed** in this environment (2 passed, exit 0).

`git diff --check`: PASS. No test constructs a GPU renderer or EventLoop.

## Not verified here

Live desktop: the story batch on Ahmed's open board, the page and design appearing, one ⌘Z removing both, the Layers panel/export after a Bridge-added page, and an old (format-3) Varos build actually refusing a saved v4 file (only the frozen gate logic is tested). Page reorder/duplicate/colour edits, headless hosting and Windows runtime are out of scope.

---

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
