# MCP board-reference translation — 2026-11-04

Scope: the two confirmed defects in `plan/qa-2026-11-04/mcp/REPORT.md` on
`codex/stabilize-panels-2026-11-01`. No dependency, tool schema, reference syntax or existing
Rust function signature changed. `unqualify_tool` is an additive Rust helper. No GUI/app run,
installed bundle change, commit, branch change, stash, push or publication.

## Rules and implementation

`qualify` previously treated every unsigned scalar under `objects` or `selected` as an id.
Those two keys now qualify direct numbers only inside arrays; records inside arrays/objects
still qualify their declared id leaves. Other reply ID keys keep their existing behavior:
`id`, `ids`, `created`, `parent`, `path`, `selection`. Scalar counts, point indexes, booleans,
names and null remain unchanged.

`unqualify` previously parsed every string recursively. Its reference-record walker now visits
id leaves only (`ID_KEYS`, plus `target`/`targets`; scalar `objects`/`selected` are excluded).
The MCP adapter uses `unqualify_tool` in the Varos agents crate to distinguish tool/command
envelopes, overloaded fields and opaque user data. Only declared structures are traversed.
Board inference and `wrong_board` rejection still happen before backend dispatch.

## Request audit

Sources: `crates/mcp/src/tools.rs` schemas and consumers; `crates/engine/src/cmd/` command
specs, `id_param`/`ids_param`/`checked_id` consumers and serde input types.

| Field | Context carrying references |
|---|---|
| `id`, `ids` | Engine object selectors; `ids` in doc_inspect/inspect_document, set_paint, apply_effect, pathfinder, transform, text_wrap |
| `path` | add_text; text.createInPath; object.objectsOnPath.attach |
| `parent` | layer.newSublayer; node.move |
| `target` | layer.move; appearance.transfer |
| `source` | appearance.transfer; appearance.copyFrom |
| `from`, `to` | text.thread.add |
| `layer` | guide.add; guide.setLayer |
| `group` | livePaint.fill; livePaint.strokeEdge; livePaint.info |
| `member` | object.intertwine.arrange |
| `slices` | object.slice.* slice selectors |
| `layers`, `assets` | document.exportForScreens |
| `assets`; `asset` | assets.remove/export; assets.rename |
| `extend.id`, `connect.id`, `redraw.id` | path.freehand |
| `items[*].id` | select.anchorsMany |
| `source.node`; `source.{edge,first,second,a,b,vertex}.node` | dimension.create |
| `fill.id/ids`, `stroke.id/ids` | set_paint/draw_path/draw_shape, including drawing batch steps, only for full paint params objects with a color/none/swatch/gradient marker (the exact tools::paint_params rule) |
| `params` | command_run/run_command/invoke_menu, using the command's identity |
| `steps[*].params`; `commands[*].params` | MCP command_batch; engine command.batch/action.batch, using each step's command identity |

`targets` has no production request-key consumer in the audited schemas (the engine helper
named `targets` reads `id`/`ids`). It remains supported by the generic reference-record helper.
`objects`, `selected`, `selection`, `created` are reply/reference-record fields, not extra
object-selector tool arguments.

Explicitly preserved: text/name/label/labels/insert, file paths, group/style names, command
identifiers (including batch step ids), effect identifiers, plug-in identifiers and arbitrary
plug-in/effect params, swatch names and gradient data. `effect.apply`/`appearance.addEffect`
interpret `id` as an effect alias only without a string `effect`; plug-in commands interpret
`id` as a plug-in identifier, while plugin.run still translates its own `ids` selector.
Also audited: `target` appearance enums and slice HTML targets, `source` action-batch enum,
from/to coordinates/indexes/names, artboard/index/subpath/segment/anchor values, exportForWeb
`slice` displayed number and `slices` enum. These are not object references in those contexts.

## Reply audit: every production objects/selected use in engine and MCP

| Producers / consumers | Shape and treatment |
|---|---|
| engine inspect; fileinfo; paint.setFill/setStroke; stroke.set; fonts; docinfo page summaries; MCP set_paint | `objects` scalar count stays numeric; MCP apply_paint consumes numeric counts |
| engine docinfo | `objects` map of counts stays numeric |
| document.find (edit.rs) | `selected` count stays numeric |
| paint.freeform.get | `selected` point index/null stays literal |
| layer.rs | `selected` node-id arrays keep qualification |
| docmenu guide list; slice list | `selected` booleans stay literal |
| style.rs | `selected` style-name string/null stays literal |
| guide.select; object.slice.select; artboard tool options | Existing numeric-list qualification retained (guide/artboard lists are indexes; slice lists carry per-document ids) |

Also audited non-reply uses: `objects` transform boolean and perspective.plane.move enum;
`objects` plug-in output records; `selected` artboard tool-option lists. Nested records retain
id qualification without changing their name/text or count fields.

## Regression evidence

New test names (common prefix `board_refs_`):

- preserve_scalar_counts_and_qualify_id_lists
- leave_authored_strings_literal
- decode_only_id_leaves_in_nested_lists
- set_paint_keeps_its_object_count_numeric
- headless_add_text_preserves_reference_looking_content
- board_call_preserves_names_from_another_board
- command_context_preserves_literals_and_decodes_declared_ids
- overloaded_fields_and_opaque_params_stay_literal
- batches_keep_labels_and_reject_mixed_boards_before_dispatch
- full_paint_params_keep_selectors_and_literal_swatch_names

First three are varos-agents unit tests; the other seven use the MCP harness. The headless
JSON-RPC test creates text and reads the actual engine node back. Board-scoped tests use the
existing FakeVaros backend to inspect dispatch, counts, board targets and decorated results;
they do not claim native desktop acceptance. The context tables assert their command names
exist in the real engine registry. Existing wrong-board/resource/inference tests remain.

To prove regression sensitivity, the original boards.rs from HEAD and original MCP decoder
call were temporarily restored with the new tests retained; the fixed files were restored
byte-for-byte in a finally block. Command:

`cargo test --locked -j 4 -p varos-agents -p vectorcraft-mcp --no-fail-fast board_refs_ -- --test-threads=4`

Final old-code result: exit 101, **0 passed / 10 failed** (3 varos-agents, 7 MCP), all at
behavior assertions for the reported corruption; no compile failures. Log:
`/tmp/varos-board-refs-all-before.log`. The fixed focused run passed all 10; the complete
requested crate gate below rechecks them along with the existing suites.

## Local gates

All Cargo builds/tests ran sequentially with `-j 4`, test threads 4. `cargo fmt` does not accept
a jobs flag and does not compile. Crate names match the requested names.

| Exact command | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS, exit 0, no formatting differences (after formatting the changed Rust files) |
| `cargo test --locked -j 4 -p varos-agents -p vectorcraft-mcp -p vectorcraft-engine -- --test-threads=4` | FAIL, exit 101: 65 passed, 1 failed; Cargo stopped at varos-agents before running the other crates |
| `cargo test --locked -j 4 -p varos-agents -p vectorcraft-mcp -p vectorcraft-engine --no-fail-fast -- --test-threads=4` | Supplemental full collection: FAIL, exit 101; **3,816 passed, 24 failed, 2 ignored**, including integrations and doctests; all 10 new regressions pass |
| `cargo clippy --locked -j 4 --workspace --all-targets -- -D warnings` | PASS, exit 0, no Clippy findings |
| `git diff --check` | PASS, exit 0 |

Supplemental counts by crate, including integrations: varos-agents 65 passed / 1 failed;
vectorcraft-engine 3,588 passed / 1 failed / 2 ignored; vectorcraft-mcp 163 passed / 22 failed.

Failure analysis: 23 existing tests require a local TCP listener and fail at binding with
`PermissionDenied: Operation not permitted` (1 agents authentication test and 22 MCP remote/
attachment tests). The remaining engine test, `tests_history_mem::mac_reads_the_machine_memory`,
gets `None` because `/usr/sbin/sysctl -n hw.memsize` is blocked; a direct invocation confirms
`Operation not permitted`. None of those test/source files changed. No tests were disabled and
no sandbox bypass was attempted. The full test gate still needs a run in an environment that
permits local listeners and that sysctl; **this is not a passing test gate**.

Logs: `/tmp/varos-board-refs-test.log` (exact requested test command),
`/tmp/varos-board-refs-test-all.log` (full collection), `/tmp/varos-board-refs-fmt.log`,
`/tmp/varos-board-refs-clippy.log`. Cargo emitted three existing unused egui patch warnings;
the lockfile/dependency configuration was not changed.

## Files changed

- `crates/varos-agents/src/boards.rs`: shape-aware reply qualification and key-aware record decoder.
- `crates/varos-agents/src/boards/requests.rs`: audited tool/command request reference policy.
- `crates/varos-agents/src/tests.rs`: three unit regressions.
- `crates/mcp/src/varos_boards.rs`: one production call-site change, seven MCP regressions and fake-backend fixtures.
- `docs/varos/STATE.md`: provisional status with the actual gate limitation.
- This report: audit, before/after evidence, commands and limitations.

## Review and limits

Independent read-only review approved the final implementation, including nested paint
selectors; no remaining blocking finding. No reviewer Cargo commands were run.

This patch does not redesign the pre-existing qualification of guide/artboard index lists or
other reply fields. New tools/commands introducing object-reference fields require an explicit
entry in the request policy and regression coverage. No native acceptance, release build,
workspace test suite or full `cargo xtask ci` is claimed; the requested crate tests and workspace
Clippy gate are recorded individually.


## Moderator re-run (outside the sandbox)

`cargo test --locked -j 4 -p varos-agents -p vectorcraft-mcp -p vectorcraft-engine --no-fail-fast -- --test-threads=4`: 3,840 passed, 0 failed (the 24 sandbox-denied listener/sysctl tests pass on the host). Workspace Clippy `-D warnings` and fmt pass. Committed as `d8b420361`.
