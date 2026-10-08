# Varos Bridge — attached design slice (API 1.0)

Ahmed: this lets Claude Code describe and change the board already open in Varos. A move + solid recolour in one `edit` request is one normal undo step. There is no new menu or panel. The app never calls an AI provider itself.

This slice implements `capabilities`, `list_boards`, `describe` (including explicit bounded geometry), `select`, `edit` (`move`, `set_paint`, `add_shape`, `resize`, `rotate`, `rename`, `delete`, `align`, `distribute`, `group`, `ungroup`, `order`, and — slice 3 — the page verbs `add_artboard`, `resize_artboard`, `rename_artboard`, `delete_artboard`, `set_active_artboard`), `snapshot` (whole board or one page), `history`, and `request_status`. File saving/export and rounded rectangles are enabled; headless hosting and Windows attachment remain unavailable. The existing standalone `varos-cli describe/apply` API 0.x remains a separate provisional file workflow; attached API 1.0 is `varos-cli bridge …` and uses exactly the same service as MCP.

Slice 1 has landed and the owner verified live move + recolour followed by one human undo. Slice 3's live acceptance (the Instagram Story batch below on an open board, then one ⌘Z) is pending. Slice 2's live acceptance is still pending: an external agent builds the poster below on the active open board, asks for a snapshot, then the owner checks the objects and undoes the entire design once. Headless tests and CPU preview inspection do not establish live desktop acceptance.

## Cheapest way (API 1.1)

Negotiate with `capabilities {"api":"1.1"}`. The cheapest way to create decorative art is an explicit
API 1.1 edit with batch-local `defaults`, omitted decorative names, compact rect/ellipse/path tuples,
and `repeat` for identical translated rows. Use `receipt:"ids"`, `describe summary_budget:1024`
(or a since-revision diff), and `snapshot profile:"economy"` only when an image helps; request specific
IDs/fields and larger dimensions explicitly. API 1.0 and unprofiled reads keep their existing behavior.
These are measured JSON-byte savings, not a measured tokenizer or provider-cost claim.

```json
{"api":"1.1","board":"b1","request_id":"r1","expected_rev":0,
 "defaults":{"parent":"node:1","fill":"#112233FF","radius":4},"receipt":"ids",
 "ops":[{"verb":"repeat","count":3,"dx":24,"dy":0,
         "ops":[["rect",[0,0,20,12],{"local":"$tile"}]]}]}
```

Defaults accept only parent, fill, stroke, stroke_width, radius and opacity; explicit leaf values win.
Null removes fill/stroke; parent/numeric null is invalid. Radius defaults apply only to rectangles;
an explicit ellipse radius is refused. Defaults never change the human's drawing paint state.
Omitted creation names become `Rect <path-id>`, `Ellipse <path-id>` or `Path <path-id>`; retry returns
the cached receipt and redo restores the same labels. Explicit names retain the existing cleaning rules.

Tuples may freely mix with existing object verbs: `["rect",[x,y,w,h],fill,radius]`,
`["ellipse",[x,y,w,h],fill]`, `["path",[[x,y],…],closed,fill]`. Fill is optional color/null; radius
may follow a supplied rect fill. Use a trailing options object to inherit fill while supplying radius,
e.g. `["rect",[0,0,20,12],{"radius":4}]`. Duplicate/unknown keys, extra slots and invalid values fail.
Path tuples contain plain points; use object add_path for handles/smooth. Holes are refused.

Repeat count includes the first instance (1–100), expands instance-major in source order, translates
points and handles, and nests at most four levels. Children are shapes, paths or repeats. Locals acquire
instance suffixes (`$tile_0`, `$tile_1`; nested suffixes concatenate); external references must use those
suffixed names. Collisions, overlong names and forward references fail. Both top-level and expanded
operations are capped at 100; expanded creation/target work is capped at 1,000 before ID allocation.
Errors retain the top-level op_index and a nested location of instance/op entries. One batch is one undo step.

IDs receipts contain only created/changed/removed object IDs, created/removed page IDs, locals and revision,
plus the existing reply envelope. Object ID lists use canonical lexicographic order; page IDs use page order;
locals use key order. Large receipts mark `more` and supply a revision-pinned cursor and `detail_request`
for API 1.1 request_status. Each page includes only its IDs/locals; collect all pages. A document edit expires
a continuation cursor; retrying the original request still returns the identical cached first page.

Budgeted summaries accept 256–1024 UTF-8 bytes and retain revision/counts, bounded name/selection/page
previews, and an explicit more/cursor/detail_request. The structured result and readable text both fit the
requested budget; a budget too small for the mandatory header/cursor is refused. This option is summary-only.
Economy snapshots default to 512×232 for a board or an aspect fit within 512×512 for a page. Explicit dimensions
may still reach 1024. MCP and CLI use the same decoder/service, including defaults, tuples and repeat.
Bars, inline group creation and clone remain deferred to token-economy slice 3 (the general capabilities hint
mentions bars, but they are absent from enabled edit_verbs/schemas).

Replay the frozen wireframe encoding with `python3 tools/bridge_token_economy.py
varos/crates/varos-bridge/tests/fixtures/wireframe-source-1.1.json` from the repository root.
It measures 3,364 + 3,410 + 1,186 = **7,960 bytes**, including each defaults/ops wrapper;
the contract test applies both encodings and compares the complete document except omitted path names.
The three original transaction boundaries are preserved. Poster/story compact payloads are 508/618 bytes.

## Try it on macOS — register once, then just say "use Varos"

Run `varos-cli bridge register claude`, then say **“use Varos”**. Open Varos before calling its tools. Same-uid local agents are trusted automatically with every scope; no pairing or save permission is needed (owner decision 2026-10-08).

Registration for `codex`, `cursor`, and `print`, including `--dry-run`/`--replace`, is unchanged. `pair`, `pair --approve`/`--deny`, and `agents`/`agents revoke` are compatibility no-ops: they print “not needed: local agents are trusted (owner decision 2026-10-08)” and exit 0. `bridge status` says “trust: local user”; `agents audit` shows recent audit entries and `hosts` lists launches.

**Two Varos launches at once:** the agent gets `ambiguous_target` with a short list (instance, pid, mode, build — never board names or paths) and must ask you. Close the extra Varos, or pass `--pid <pid>` from that list (instance ids change every launch, so don't register them). If the attached Varos restarts, the next call answers `session_reset` once (board handles changed; the agent lists boards again).

From a source checkout the same works with `target/debug/varos` (start it from a terminal) and `target/debug/varos-cli bridge register claude`; `register` warns that a build-folder path is not stable. `tools/mac/bridge-connect.sh` is deprecated and now only runs `varos-cli bridge register claude`.

Ask, for example:

> Use Varos. Call capabilities with api 1.0. List the boards. Describe the active board, then request a page of objects with fields bounds, paint and parent. Choose two unlocked visible path IDs. Move them 10 points right and recolour their fills to #FF6600FF in ONE edit request with two operations, using the board's expected_rev and a new monotonic request_id. Keep my selection. Report the receipt. Do not save or request history.

Use IDs returned by the bridge, such as `path:42` / `node:8`, not names or stacking positions. The concrete edit is:

```json
{
  "api": "1.0", "request_id": "r1", "board": "b2", "expected_rev": 12,
  "ops": [
    {"verb": "move", "ids": ["path:42", "path:55"], "delta": [10, 0]},
    {"verb": "set_paint", "ids": ["path:42", "path:55"], "fill": "#FF6600FF"}
  ]
}
```

The sample board, revision and IDs are placeholders; use live values. Coordinates are document points, x right, y down, independent of zoom/ruler origin. Fill/stroke are `#RRGGBBAA`; `null` removes paint; absent paint stays unchanged. `stroke_width` is nonnegative points; `opacity` is 0–1. Node targets expand to all descendant paths and refuse if any is hidden/locked. A partial rotated group move changes only the requested leaf paths.

Look at Varos: both objects should move/recolour together. Press **⌘Z once** with the canvas focused: both changes should undo together. This real window check is the acceptance step; headless tests are not evidence that Ahmed has seen it. No auto-save happens on agent disconnect.

## Connection and trust (ADR-0011 C1)

- **Discovery.** Each running Varos publishes `hosts/<instance-id>/endpoint.json` (0600) plus socket `b.sock` (0600) in an owner-only (0700) `varos-bridge/` folder inside the macOS per-user temp directory, resolved with `confstr(_CS_DARWIN_USER_TEMP_DIR)`, not `$TMPDIR`. The record holds `discovery_version`, random `instance_id`, `epoch`, pid + process start time, `mode`, socket path, connection version range, app build and the host key fingerprint — no token, key, board name or file path. Records are published by atomic rename; the host removes only its own identity-checked entry on exit and cleans at most 16 stale (dead-pid / pid-reused) entries at start, never recursively. Socket paths are checked against the 103-byte macOS limit. The record only *locates* a host; it never authorizes one.
- **Identity.** `FileKeyStore` on every OS holds raw 32-byte Ed25519 seeds in `~/Library/Application Support/Varos/bridge/keys/{host.key,agent-<profile-id>.key}` on macOS; Windows uses `%APPDATA%\Varos\bridge\keys\`, Linux `~/.config/varos/bridge/keys/`. Unix directories/files are 0700/0600; unsafe modes or symlink parents return `credential_unavailable` with a repair instruction. Keys are created atomically and reused across rebuilds. No Keychain access or prompts. These permissions protect against other users, not same-uid processes; Windows relies on user-profile ACLs and attachment remains compile-only.
- **Handshake and trust (connection 1.0).** Both Ed25519 proofs still bind nonces, version, profile/session, instance and epoch. The OS peer uid must equal the host uid; then every scope (read/edit/destructive/history/files) is admitted automatically. Identity attributes audit and future per-AI history; it is not an approval gate or vendor attestation. No trust store or pending requests are read/written. Any same-uid process has full edit + save access; this was already possible in practice with readable file identities. Undo + review is the owner's safety model. Cross-uid peers remain refused.
- **Profiles.** profiles.json remembers the stable client profile id. A missing explicit --identity returns identity_invalid; remove the old reference and retry. No re-pair is needed. Core undo entries have no metadata slot; separate per-AI review history is the next PLAN track A piece, not implemented here.
- **Audit.** `~/Library/Application Support/Varos/bridge/audit/audit.log` (0600, persistent): time, agent profile, client label (claimed), session prefix, request id, verb, board handle, from/to revision, result code — never names, paths, payloads or keys. Rotation (10 MiB; rotated file deleted after 30 days) and appends happen under one lock. A mutation is refused (`audit_unavailable`) if its audit line cannot be written first. Opt-in legacy token calls are audited as agent `legacy`. It is a diagnostic log, not tamper-proof against the account owner.
- **Legacy transition (deprecated, one slice).** The slice-1 per-launch token endpoint (`$TMPDIR/varos-bridge-<pid>-<launch>/endpoint.json`, `varos-cli bridge-endpoint`, `--attach <socket> --token …`) is **off by default**. Only if you deliberately launch the desktop with `VAROS_BRIDGE_LEGACY=1` does it run, on its **own** socket, for old setups during this one transition slice. Its token then grants every scope without pairing, exactly as before (its calls are audited as agent `legacy`); the paired socket never accepts it. `tools/mac/bridge-connect.sh` no longer needs it: it only runs `varos-cli bridge register claude`. It is removed in the next connection slice.
- **Test override.** `VAROS_BRIDGE_HOME=<existing 0700 folder>` relocates discovery and profile files for tests. Debug builds only — a release build ignores it with a warning. It never relocates or weakens secrets.


## API details and refusal behavior

MCP is UTF-8 newline JSON-RPC 2.0 on stdio, with protocol revision **2025-06-18**. Stdout is protocol only. `initialize` and `notifications/initialized` precede tools. It returns both `structuredContent` and the shared compact text projection; consume one representation. Tool failures have `isError: true` and the same ADR error payload as CLI. JSON-RPC framing/method errors are separate; unknown tool names return `-32602`. Cancelled requests receive no MCP response. EOF requests cancellation and waits at most 500 ms for the worker; it does not reverse any published edit.

`capabilities {"api":"1.0"}` reports the launch epoch, proxy client identity, actual enabled verbs and limits. Different Bridge API versions are refused before editing. Unknown fields/variants are refused, including model-supplied `confirm`, `allow_history`, `allow_destructive`, filesystem destinations inside edit batches and unsupported edit verbs. MCP input schemas are complete for this slice. Wire DTOs do not expose `EditCommand` or `AppCommand`.

`describe {"board":"b2"}` returns a small header, counts, outline bounds, up to 20 artboards and a bounded selection preview. `fields` with `bounds`, `paint`, `parent`, `name`, `state`, `geometry` requests object details; omit `ids` for paint-order pagination. Explicit detail IDs are capped at 1,000, each page at 100 objects / 16 KiB readable text. Repeat the same query with its cursor; a changed revision/query returns `resync_required`. `fields:["metadata"]` reads board metadata only; `fields:["artboards"]` reads paginated page properties. Each page has a persistent `id` such as `artboard:7` (format 4: it survives edits, undo/redo and save/reopen; a new page never reuses a removed page's id) plus its current `index`; the summary and receipts name the `active_artboard`. The old revision-bound `ref` (`a0@12`, valid only at revision 12) is still returned and accepted as an `align` target for **one more slice** — it is deprecated; use `artboard:N`. `settings_digest` records settings changes without exposing an internal enum schema.

Fields may be combined freely: board sections `metadata`, `artboards`, `selection`, `state` and object fields `bounds`, `paint`, `parent`, `name`, `geometry`. For example, `{"board":"b2","fields":["artboards","selection","bounds","name","paint","parent"]}` returns the board sections and paginated object details together. `ids` scopes only objects; with explicit IDs and only board fields, default object details are included. In combined replies, `limit`/`cursor` page objects only; board sections are complete and repeated on each object page, within the same 16 KiB budget. Dedicated metadata/artboard and selection queries retain their existing pagination and shapes. Alongside `metadata`, `artboards` or `selection`, `state` reports `dirty` and `active_artboard` and retains object `hidden`/`locked` flags when objects are included. Alone or with object fields (without `since`), `state` keeps its API 1.0 object meaning. `since` may accompany sections and object details and keeps its exact net-diff/resync semantics. Unknown field names return `invalid_argument`.

CLI parity: `printf '%s' '{"board":"b2"}' | varos-cli bridge describe --fields artboards,selection,bounds,name,paint,parent --json` (a JSON array is also accepted for `--fields`). The flag adds `fields` to stdin or `--request-file` arguments; supplying both is refused.

`describe {"board":"b2","since":12}` returns the exact net projection change, including human edits and history, or `resync_required` if the requested boundary was not observed/retained or the net diff exceeds the budget. Journal limits are 128 revision boundaries / 8 MiB per board. Large edit receipts remain committed successes and return counts plus a revision-pinned detail query/cursor instead of an unmarked partial dump. Geometry changes are marked, not dumped. Selection previews carry count/more fields. Idle event-loop observation runs only while an authenticated socket client is attached. It compares the editor revision, a fixed-size settings snapshot and dirty flag before fingerprinting; unchanged documents cost no serialization or SHA-256. Transient selection is sampled at request boundaries. Disconnected changes are reconciled on the next request; unobserved intermediate revisions require resync.

Mutation IDs are positive canonical `r1`, `r2`, …, increasing for each proxy client. After a successful mutation, the same ID and payload returns the original receipt, even after a human edit; a different payload is refused. Pre-commit failures (including `busy`, `cancelled`, `revision_conflict` and a worker deadline before publication) retain no receipt and do not advance the high-water sequence: retry the same ID with fresh `expected_rev` after resolving the cause. Successful IDs must still increase; a later successful ID retires earlier unused IDs. The cache retains 128 receipts/client and a high-water sequence, with at most 64 mutating clients per desktop launch. `request_status` queries the retained receipt for the same client. A new one-shot CLI normally creates a new client; use an explicit 64-hex `--client` identity to resume one across calls. A host restart changes the epoch and loses this in-memory journal/receipt history. A disconnect/timeout is not evidence of rollback: inspect the receipt/revision before retrying. Cancellation before publication changes nothing; cancellation after publication does not reverse a committed edit.

Reads leave pending fields open and return the last committed state; they never commit a field or create an undo step. Only mutations settle valid pending fields as a separate human undo step, then recheck `expected_rev`. Invalid fields or active gestures return `busy`; the bridge does not dismiss or finish a human gesture. A human settings change that bypassed the old editor revision path is observed and advances the Bridge document revision. A stale request returns `revision_conflict` with expected/actual values. Nothing implicitly switches tabs.

The runtime queue admits 32 pending socket requests; there are at most 8 socket workers, 6 MCP tool calls, 64 cancellation entries (60-second expiry). The listener polls idle accept at 50 ms. On shutdown it cancels pending work and shuts down active sockets before joining workers (500 ms worker wait, at most 1 second in listener drop). Socket requests expire after 30 seconds awaiting the host; query the receipt after a timeout. Requests are capped at 1 MiB / 100 operations / 1,000 explicit and expanded targets. Staging checks each operation's preconditions, then validates/encodes the final document once. Only a final validation failure replays prefixes to identify the first invalid operation; failure identifies the zero-based operation index and publishes nothing. No latency or maximum-format-size responsiveness promise is made; large-board staging/projection measurements remain release work.

## Design verbs and request-local identities

`edit` keeps the existing revision, receipt, scope, cancellation and one-step undo contracts. Each operation resolves and checks its targets against the isolated staged document, then the final document is checked/encoded before one publication. A later failure returns its zero-based `op_index` and leaves the live document, allocator, dirty state, undo/redo, selection, tool and drawing defaults unchanged. Successful deletion prunes references to removed entities from human selection; it never selects the new artwork. All explicit and expanded targets remain bounded to 1,000 across the batch.

Creation requires an explicit non-null fill or stroke; omitted/null paint on both channels returns `invalid_argument: paint required`. This follows ADR-0009 explicit paint rather than inheriting human drawing defaults. Stroke width defaults to 0 and opacity to 1. A missing parent uses the human's active layer. `parent` must be an editable visible `node:N` layer. `insert` defaults to `"top"`, the only supported position, at the front of that layer. Existing siblings keep their relative order. Name input is bounded to 256 characters, cleaned by the core's existing name cleaner; an empty cleaned name is refused. Receipts carry the actual cleaned name.

A creation may supply `"local":"$a"`; `group` may supply `"local":"$poster"`. These aliases are ASCII `$` + a letter + letters/digits/underscores (maximum 64 bytes), unique within that request. Later `ids` may reference them; forward/unknown/duplicate aliases are refused. Successful receipts return `result.locals`, mapping aliases to actual allocated `path:N` / `node:N` IDs. Aliases expire after the request and cannot be used in another request, `select`, or a read. A mapped identity removed later in the same batch is no longer a live object; even a successful create-then-delete batch with no net undo entry reserves its allocated IDs against reuse. This is separate from the visible `name` property. Shape allocation uses the core `EditCommand::AddShape` outcome and the existing session high-water allocator, including undo-then-create branches.

| Verb | Operation fields and behavior |
|---|---|
| `add_shape` | `kind: "rect"` or `"ellipse"`, `bounds: [x,y,width,height]` with positive finite dimensions; optional `parent`, `insert`, `local`, `name`, `fill`, `stroke`, `stroke_width`, `opacity`. Ellipses use the existing four cubic anchors/handles. Plain rectangles use four corner anchors. Optional rect `radius` creates an eight-anchor cubic path; the model has no live corner-radius primitive. With stroke as the only paint, `stroke_width` must be > 0; omitted or zero width returns `invalid_argument: stroke_width must be > 0 when stroke is the only paint`. |
| `resize` | `ids`, `bounds: [x,y,width,height]`. Explicit top-left anchor, unconstrained proportions. For one rotated unit, width/height are local dimensions and x/y locate the world bounding-box top-left; several units use world bounds, matching the core. Degenerate bounds and requests that would trigger the core's minimum-size/scale clamp are refused. |
| `rotate` | `ids`, `degrees` (absolute finite degrees for each complete object/group, including mixed rotations; partial-group targets are refused with `unsupported` and the alternative `rotate the whole group node:N`). |
| `rename` | `ids`, `name`; dispatches path/leaf-node/container identities to the correct core command. |
| `delete` | `ids`; deletes complete objects/groups, including their descendants. Layer deletion and partial-group deletion are deferred. |
| `align` | `ids`, `mode: left|center|right|top|middle|bottom`, `target: "selection"` (at least two complete units), `"artboard:N"`, a request-local bound to a page (`"$story"`, including one added earlier in the same batch), or the deprecated revision-bound `"a0@12"` (refused with `invalid_argument` in any batch that also contains a page verb, because page verbs can shift indices; use `artboard:N` there). Units/groups remain rigid. Missing, hidden, locked or stale artboards are refused; no Auto/fallback; the active page is not changed. |
| `distribute` | `ids`, `axis: "h"` or `"v"`; equal centers, at least three distinct editable leaf paths. Groups/mixed grouped targets and explicit gaps are refused. |
| `group` | `ids`, optional `local`; at least two complete units sharing a parent. Existing groups can be nested, with all affected descendants checked. Returns the new group ID through `created` and optional `locals`. |
| `ungroup` | `ids`; explicit complete top-level `node:N` groups only, dissolves one level. |
| `order` | `ids`, `order: front|forward|backward|back`; within the existing parent, never arbitrary reparenting. |
| `set_paint` | Existing `ids`, `fill`, `stroke`, `stroke_width`, `opacity`; opacity is already folded into this verb (0–1). |
| `add_artboard` | Exactly one of `bounds: [x,y,width,height]` or `preset: "square"|"portrait"|"story"|"a4"` (1080×1080, 1080×1350, 1080×1920 px = pt at 72 ppi; A4 595×842 pt — the Start presets). A preset may take `origin: [x,y]`; without it the page goes right of the right-most page (60 pt gap, top-aligned with the active page) or at the origin on a free canvas. Optional `name` (default "Artboard N", cleaned like object names) and `local`. Sides must be ≥ 1 pt and finite; ≤ 1,000 pages. Returns `artboard:N` via `locals` and `artboards_created`. Does **not** change the active page. White page, clip on (the editor's default). |
| `resize_artboard` | `id`, `bounds: [x,y,width,height]` set exactly (same checks). Artwork does not move with the page (the panel's X/Y/W/H behaviour). A locked page is refused (`locked_target`). |
| `rename_artboard` | `id`, `name` (bounded and cleaned; empty refused). Allowed on a locked page. |
| `delete_artboard` | `id`. Removes the page only; its artwork stays (a floater, or on the other pages it overlaps). **Active rule:** deleting the active page while other pages remain is refused — put `set_active_artboard` for another page earlier in the same batch; deleting the last page leaves a free canvas. Locked pages are refused. |
| `set_active_artboard` | `id`. Makes the page the active one (where the human's panel/export "active artboard" points). Navigation, not content: alone it creates no undo step and no new revision; the receipt and `describe` show `active_artboard`. The human's artboard multi-selection is re-pointed to it. |

Page verbs address pages only by `artboard:N` or a request-local bound to one; an `artboard:` id in an object `ids` list is refused (pages are not objects). No page verb changes the active page as a side effect, and the human's artboard multi-selection is carried across insertions/removals by id. Reorder, duplicate and page colour/clip edits are supported. Page bleed edits remain unsupported (`artboard_bleed` in `capabilities.unsupported`).

Resize and move accept explicit leaf paths inside a group and leave siblings unchanged; group outline bounds follow the changed leaves. Partial resize of a rotated group is refused because core has no independent local size frame for the leaf: use `move` for placement, or `resize` with the whole group node ID. Partial-group rotate is refused because core has no independent absolute leaf angle; rotate the whole group `node:N` instead, matching the desktop Rotate tool's whole-group behavior. Whole-group transforms retain their existing live absolute-angle semantics. Align/group/order and deletion still require complete units. Layer IDs remain supported for ordinary move/paint/rename and describe; structural layer mutations are refused. Slice 4 adds `add_path` and radius geometry (documented below). `capabilities.unsupported` lists only remaining deferred operations.

## Moderator poster batch

This is the exact checked fixture [poster-request-1.0.json](tests/fixtures/poster-request-1.0.json), also exercised through the actual MCP stdio binding and CLI decoding against the same service. Read `list_boards`/`describe` first and replace **only** the envelope's `board`, `expected_rev`, and monotonic `request_id` with live values. The example assumes `b1`, revision 1, and a fresh client. The design is 360×480 document points at the origin, on the active layer; the three circles become evenly spaced. It creates five paths and a group, touches no existing art, and needs no destructive grant.

```json
{
  "api": "1.0",
  "request_id": "r1",
  "board": "b1",
  "expected_rev": 1,
  "ops": [
    {"verb": "add_shape", "kind": "rect", "bounds": [0, 0, 360, 480], "fill": "#172B4DFF", "local": "$background", "name": "Background"},
    {"verb": "add_shape", "kind": "ellipse", "bounds": [40, 160, 60, 60], "fill": "#F4BD50FF", "local": "$a"},
    {"verb": "add_shape", "kind": "ellipse", "bounds": [120, 160, 60, 60], "fill": "#E66C4FFF", "local": "$b"},
    {"verb": "add_shape", "kind": "ellipse", "bounds": [260, 160, 60, 60], "fill": "#B5D8CCFF", "local": "$c"},
    {"verb": "distribute", "ids": ["$a", "$b", "$c"], "axis": "h"},
    {"verb": "add_shape", "kind": "rect", "bounds": [40, 340, 280, 36], "fill": "#F4EAD5FF", "local": "$title", "name": "Title bar"},
    {"verb": "group", "ids": ["$background", "$a", "$b", "$c", "$title"], "local": "$poster"},
    {"verb": "rename", "ids": ["$poster"], "name": "Poster"}
  ]
}
```

MCP: call `edit` with that argument object. CLI (from `varos/`), after adapting the envelope in a copy:

```sh
target/debug/varos-cli bridge edit --attach "$BRIDGE_SOCKET" --token "$BRIDGE_TOKEN" \
  --request-file /explicit/path/poster-request.json --json
```

The receipt's `$poster` mapping identifies the group for subsequent reads/edits. The title is a rectangle, not editable text. The owner should see the poster and press ⌘Z once to remove all five shapes and the group, then ⌘⇧Z to restore them.

## Instagram Story batch (slice 3)

The owner's ask — *"make an Instagram Story artboard and design on it"* — is one request: the exact checked fixture [story-request-1.0.json](tests/fixtures/story-request-1.0.json), exercised through the actual MCP stdio binding and the CLI decoder against the same service ([frozen receipt](tests/fixtures/story-result-1.0.json)). Replace only `board`, `expected_rev` and `request_id` with live values. Pick an `origin` clear of existing pages (or omit it to place the page right of the right-most one; then align by `$story` as below, because bounds are absolute document points).

```json
{
  "api": "1.0", "request_id": "r1", "board": "b1", "expected_rev": 1,
  "ops": [
    {"verb": "add_artboard", "preset": "story", "origin": [0, 0], "name": "Instagram Story", "local": "$story"},
    {"verb": "set_active_artboard", "id": "$story"},
    {"verb": "add_shape", "kind": "rect", "bounds": [0, 0, 1080, 1920], "fill": "#172B4DFF", "local": "$bg", "name": "Background"},
    {"verb": "add_shape", "kind": "ellipse", "bounds": [0, 0, 640, 640], "fill": "#F4BD50FF", "local": "$sun", "name": "Sun"},
    {"verb": "align", "ids": ["$sun"], "mode": "center", "target": "$story"},
    {"verb": "align", "ids": ["$sun"], "mode": "middle", "target": "$story"},
    {"verb": "add_shape", "kind": "rect", "bounds": [120, 1500, 840, 160], "fill": "#F4EAD5FF", "local": "$title", "name": "Title bar"},
    {"verb": "group", "ids": ["$bg", "$sun", "$title"], "local": "$design"},
    {"verb": "rename", "ids": ["$design"], "name": "Story design"}
  ]
}
```

The receipt maps `$story` to the new `artboard:N`, lists it in `artboards_created`, names it `active_artboard`, and maps `$design` to the group. Then look at that page alone:

```json
{"board": "b1", "rev": 2, "artboard": "artboard:2"}
```

`snapshot` with `artboard` renders exactly that page — its bounds, its background colour (transparent pages stay transparent), at its own aspect ratio inside `width`×`height` (each default **1024** for a page; a story comes back 576×1024). The owner should see the page and the design, and one ⌘Z removes the page and everything on it; ⌘⇧Z brings back the same page id. CLI: `varos-cli bridge edit --request-file … --json`, and for files on disk `varos-cli export-pdf <file.vrs> --out page.pdf --artboard artboard:N` exports just that page (the id is mapped to the active-artboard export scope on an in-memory copy; the file is not changed). `export-pdf` refuses an `--out` that resolves to its input (same path, `..`, symlink or hard link): an export is a model-free PDF and would destroy the editable document; it has no `--in-place`.

## See the staged result through CPU snapshots and geometry

`describe {"board":"b1","rev":2,"ids":["path:8"],"fields":["geometry"]}` explicitly requests path `closed`, local `anchors` (`point`, optional `hin`/`hout`), `holes`, and `world_transform`. Containers have `geometry: null`; use bounds/parent for them. Geometry stays off default summaries, diffs and receipts. Object pagination/cursors apply, with an absolute cap of 1,000 anchors (outer + holes) per object and a **16 KiB page budget**, typically only **about 300 anchors** (fewer with handles, long coordinates or other fields). Capabilities report both limits and `geometry_anchor_pagination:false`; 1,000 anchors is not a guaranteed readable page. An individual object exceeding either budget returns `limit_exceeded` with no unmarked partial geometry, even at `limit:1`; omit geometry or request fewer objects when the page contains several. Finer within-path geometry pagination is deferred.

`snapshot {"board":"b1","rev":2}` explicitly requests an immutable revision-pinned CPU preview via `varos-raster`, default **544×246**, with optional integer `width`/`height` each 1–1024. Adding `"artboard":"artboard:N"` renders only that page instead (see the story section). A stale revision is refused. It constructs no window/GPU, creates no history entry and changes no selection/tool. This is the thumbnail-style fitted preview, including the dark dotted well; it is not production PNG export or GPU pixel parity. Desktop clones the revision-pinned document on the owning thread, then rasterizes and encodes on a worker and replies through the existing channel. Cancellation is checked before rasterization, before encoding and before delivery. Headless service tests retain synchronous rendering. Encoded PNGs exceeding the existing transport-frame budget are refused.

MCP returns an `image/png` image content block only for this requested tool; structured/text content contains dimensions/revision/preview metadata, never a duplicate base64 dump. CLI writes bytes only to its explicit local destination (a new file; overwrite/symlinks at the final filename are refused), and prints metadata instead of image bytes:

```sh
printf '%s\n' '{"board":"b1","rev":2,"width":544,"height":246}' | \
  target/debug/varos-cli bridge snapshot --attach "$BRIDGE_SOCKET" --token "$BRIDGE_TOKEN" \
  --output /explicit/path/poster-preview.png --json
```

The destination is a CLI-local option, never a host wire field or a desktop filesystem grant. The `snapshot` MCP tool takes no destination. File tools use revision-pinned host destinations with mistake-guards. Use actual receipt revision and allocated IDs in both examples.

## Local edits and shared history

Delete, ungroup, page deletion and shared undo/redo need no confirmation or digest. Optional legacy digest input fields remain accepted for API 1.0 compatibility and have no authorization effect. Shared history may undo a human edit. Every batch remains one undo step; expected revision, locked/hidden targets, active-page rules and idempotent receipts still apply. Saving is explicit and never happens just because an agent disconnects.

## CLI through the same implementation

```sh
# JSON arguments on stdin, or --request-file request.json (same tool argument object).
# Same discovery, file profile identity and local-user trust as MCP (the CLI is its own agent profile).
printf '%s\n' '{"api":"1.0"}' | target/debug/varos-bridge capabilities --json

printf '%s\n' '{"board":"b2","fields":["bounds","paint","parent"]}' | \
  target/debug/varos-cli bridge describe --pid <varos-pid>
```

CLI defaults to the compact text; `--json` returns the same structured receipt as MCP. Failures exit nonzero. The CLI and MCP bindings only decode/encode, authenticate and call the one service. They never reload/overwrite the file behind the live editor.

## Verification boundary

Tests construct no GPU renderer or event loop. Existing frozen fixtures are unchanged. Connection tests cover stable profiles, uid refusal, signature/nonce/epoch/version checks and compatibility no-op commands. Real-socket contracts cover immediate all-scope local admission with no pairing state, audit profile + label, legacy isolation, registry cleanup and host-key restart without approval. They remain ignored by default because sandboxes may reject bind with EPERM. Run:

```sh
cargo test -p varos-bridge --test contracts -- --ignored
```

Current gate evidence and unverified runtime boundaries are in VERIFICATION.md. No real Keychain access is part of these tests.

Then perform Ahmed's window exercise above. Kernel peer-uid checks, real socket lifecycle and the owner window remain separate from the passing fake-host proof. No merge, installation or owner acceptance is claimed.

## Slice 4: paths, rounded rectangles, page ordering/copies and files

`add_path` accepts `anchors:[{p:[x,y],hin?:[x,y],hout?:[x,y],smooth?:bool}]`, `closed`,
and the same explicit paint/parent/name/local options as `add_shape`. Handles are absolute document
points. All coordinates must be finite; 2–1,000 anchors per path. `add_shape` rect accepts optional
`radius` from 0 through half the shorter dimension. This creates editable cubic path geometry
(eight tangent anchors), not a rectangle with a live radius property. Ellipse radius is refused.

Page verbs also include `reorder_artboard {id,position}` (zero-based),
`duplicate_artboard {id,with_art,offset?,local?}` (relative world delta; default right by page width
plus desktop gap), `set_artboard_color {id,color}` (`#RRGGBBAA` or null), and
`set_artboard_clip {id,clip}`. Duplication inserts after the source, returns a fresh page ID and
keeps the active page. `with_art:true` uses desktop overlap membership and preserves copy groups,
clipping masks, transforms and layers; locked/hidden artwork is treated exactly as desktop duplication.
Copies share no path/node/anchor identities with their sources. Page edits are in the atomic edit batch.

`describe {fields:["selection"]}` paginates deliberate current selection. `list_boards` includes
`dirty` and `backing_file` (filename only, null for untitled sessions).

File tools need no owner grant or environment setting. save uses the current backing file with an unchanged external fingerprint. File destinations use the passwd home, never $HOME: allowed are fresh absolute .vrs/.pdf filenames under that home or /Volumes/<volume>/, including the explicit cloud exceptions ~/Library/Mobile Documents (iCloud Drive) and ~/Library/CloudStorage/<provider>/ (Dropbox, Google Drive and OneDrive File Provider roots). Refused: /tmp, /private/var, other ~/Library locations (including Application Support), system roots, the running app bundle, dot components and existing files (including symlink/hard-link aliases). Home / is refused. Parents must exist; containment is checked at acceptance and again after canonical parent resolution at use time, with directory identity pinned. Network volumes are unsupported (scope_refused: network volume not supported). Fresh publication tries linkat first; on macOS ENOTSUP/EPERM/EXDEV falls back to renameatx_np(RENAME_EXCL), never overwriting. FAT32/exFAT use this fallback; real FAT32/exFAT volumes remain unverified. Windows hosting is compilation-tested only. PDF scopes are all_visible_artboards, visible artboard:N, or artwork_bounds.

These are standalone requests with `api`, `board`, `expected_rev`, and monotonic `request_id`.
They cannot be mixed into an edit batch. All filesystem inspection, encoding and durable writing
use the desktop I/O worker; no inline fallback. An acceptance receipt contains `accepted:true`
and `ticket`; it does not prove a write. Poll `request_status {request_id}` for pending/completed
and the completion receipt. Path-policy refusals arrive in that completion. A completed save says
`durable:true`, or `durable:false` when replacement succeeded but durability was unconfirmed;
the latter keeps the board dirty. Later human edits remain dirty after the captured snapshot saves.
File completion never opens a dialog. Accepted request retries reuse the ticket instead of writing twice.

### Second example: logo mark, rounded label, then export to a new filename

Use the returned live board/revision; the creation request below is the new frozen logo fixture
(`tests/fixtures/logo-request-1.0.json`, result alongside it). For a fresh rev-0 board:

```json
{"api":"1.0","request_id":"r1","board":"b1","expected_rev":0,"ops":[
  {"verb":"add_path","anchors":[{"p":[20,20]},{"p":[60,20],"hout":[80,20]},{"p":[80,60],"hin":[80,40]},{"p":[20,60]}],"closed":true,"fill":"#141313FF","local":"$mark"},
  {"verb":"add_shape","kind":"rect","bounds":[100,20,100,40],"radius":10,"fill":"#FF6600FF","local":"$label"}
]}
```

Send that via `edit` (MCP) or `varos-cli bridge edit --request-file logo.json --json`. Creation is
one undo step and returns both path IDs. Then, on the returned revision, send `export_pdf`:

```json
{"api":"1.0","request_id":"r2","board":"b1","expected_rev":1,"path":"/absolute/output/folder/logo.pdf","scope":"artwork_bounds"}
```

After acceptance, send `request_status {"request_id":"r2"}` until completed and check the receipt's
`ok` and `exported`. Export is a separate file effect with zero undo steps. The folder must be in
the user's home or an external volume, and the filename must be fresh.

Slice 4 review amendment: save_as writes a copy; the board stays on its current file (owner may widen later). Its dirty state and Recent entries stay unchanged. Destinations require an absolute filename, `.vrs` for save_as or `.pdf` for export_pdf (case-insensitive), and no dot-prefixed components. Destination containment and cloud exceptions follow the file policy above. capabilities reports trust: local user and the file guard list; files_roots_granted is removed.

File completion codes: `save_conflict` means a fingerprint mismatch or an existing destination (including a publication race); other IO failures use `io_error`; protected or escaping destinations use `scope_refused`. request_status returns the failed completion receipt. Missing or expired results return `not_found`, never indefinite pending. Completion audit entries contain verb, board, ticket and result code, without paths.

The opt-in legacy token listener (`VAROS_BRIDGE_LEGACY=1`) also receives all scopes: read, edit, destructive, history and files. `VAROS_BRIDGE_HOME` relocates discovery, state and keys together in debug builds; release builds ignore it.

## What the human sees

Owner design: 2026-10-08, canvas presence replaces the chip proposals.
Azure marks the human's selection and focus.
AGENT orange marks observed agent editing activity.
The host uses the existing client label and profile identity.
Labels are sanitized, capped at 24 characters, and self-declared.
A committed edit lights the target artboard, or the active page.
Its flat orange 1.5-point Outside outline sits flush on the page edge.
A small title-style label sits at the page's top-right.
Agents on the same page have stacked labels.
Agents on different pages have independent outlines.
The page stays lit after each committed edit (4-s hold).
Accept/handle/complete are synchronous on the UI thread; in flight is never visible.
The whole batch commits immediately, as one undo step.
Created and changed objects then get 1-point bounds outlines.
Feedback appears every min(25 ms, 1500 ms / object count).
Each object outline fades over 900 ms; removed objects draw nothing.
New work collapses old stagger and finishes it within 120 ms.
Human selection wins; repaints use the host's existing pacing.
No extra agent calls or model tokens; no wire, panel or band changes.
