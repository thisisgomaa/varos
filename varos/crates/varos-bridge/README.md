# Varos Bridge — attached design slice (API 1.0)

Ahmed: this lets Claude Code describe and change the board already open in Varos. A move + solid recolour in one `edit` request is one normal undo step. There is no new menu or panel. The app never calls an AI provider itself.

This slice implements `capabilities`, `list_boards`, `describe` (including explicit bounded geometry), `select`, `edit` (`move`, `set_paint`, `add_shape`, `resize`, `rotate`, `rename`, `delete`, `align`, `distribute`, `group`, `ungroup`, `order`, and — slice 3 — the page verbs `add_artboard`, `resize_artboard`, `rename_artboard`, `delete_artboard`, `set_active_artboard`), `snapshot` (whole board or one page), `history`, and `request_status`. Files, saving/export, arbitrary paths, rounded rectangles, headless hosting and Windows attachment are not enabled. The existing standalone `varos-cli describe/apply` API 0.x remains a separate provisional file workflow; attached API 1.0 is `varos-cli bridge …` and uses exactly the same service as MCP.

Slice 1 has landed and the owner verified live move + recolour followed by one human undo. Slice 3's live acceptance (the Instagram Story batch below on an open board, then one ⌘Z) is pending. Slice 2's live acceptance is still pending: an external agent builds the poster below on the active open board, asks for a snapshot, then the owner checks the objects and undoes the entire design once. Headless tests and CPU preview inspection do not establish live desktop acceptance.

## Try it on macOS, without installing anything

From the repository's `varos/` directory:

```sh
cargo build -p varos-app --bin varos
cargo build -p varos-cli -p varos-bridge
```

1. Start **this build**, `target/debug/varos`, from a terminal. Open a board with two editable objects. Keep that board active, finish any text/number field, close the colour picker and release any drag. A previously installed Varos does not contain this listener.
2. In another terminal, run `target/debug/varos-cli bridge-endpoint`. It prints endpoint **file paths**, never tokens. The desktop also prints the path to stderr. Copy the path for the running build. The location is `$TMPDIR/varos-bridge-<pid>-<launch>/endpoint.json` (normally under the macOS user temporary directory). The app creates the directory with mode 0700, the socket and endpoint file with mode 0600. Each launch gets a new token and epoch; old connections never fall back to TCP. On normal shutdown the directory is removed. After a crash a stale file may remain, but connection fails; use the new launch's path.
3. Read the chosen file explicitly, and register the stdio server:

```sh
# Replace this with the exact path printed above; choose one endpoint, not a directory scan.
BRIDGE_ENDPOINT='/path/from/bridge-endpoint/endpoint.json'
BRIDGE_SOCKET="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["socket"])' "$BRIDGE_ENDPOINT")"
BRIDGE_TOKEN="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["token"])' "$BRIDGE_ENDPOINT")"

claude mcp add --transport stdio varos -- "$PWD/target/debug/varos-bridge" mcp \
  --attach "$BRIDGE_SOCKET" --token "$BRIDGE_TOKEN"
```

The endpoint file is a capability: choosing it and passing its token explicitly grants this proxy read/edit access to this desktop launch's exposed open boards, including boards subsequently opened. It grants no file/root/shell access. Do not share it. Claude Code stores the server arguments in its local MCP configuration. `list_boards` exposes session handles, names, revisions and dirty/active state, never backing paths, the private Home placeholder or Recent entries. No tool can widen these grants. An inactive/Home board can be read but cannot be edited/selected/undone; activate it yourself. Reopening produces a different board handle. Same-user capability protection does not protect against a compromised macOS account.

**Command syntax checked live on 2026-10-06:** the official [Claude Code MCP page, “Option 3: Add a local stdio server”](https://code.claude.com/docs/en/mcp#option-3-add-a-local-stdio-server) documents `claude mcp add [options] <name> -- <command> [args...]`, `--transport stdio`, and passing the server's flags after `--`. That is the syntax used above. This was a documentation check; no Claude registration/login/provider call was performed during implementation.

4. Start/restart Claude Code in the chosen project and inspect `/mcp` to check Varos is connected. Ask:

> Call Varos capabilities with api 1.0. List the boards. Describe the active board, then request a page of objects with fields bounds, paint and parent. Choose two unlocked visible path IDs. Move them 10 points right and recolour their fills to #FF6600FF in ONE edit request with two operations, using the board's expected_rev and a new monotonic request_id. Keep my selection. Report the receipt. Do not save or request history.

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

5. Look at Varos: both objects should move/recolour together. Press **⌘Z once** with the canvas focused: both changes should undo together. Save through the existing human save command if desired. This real window check is the acceptance step; headless tests are not evidence that Ahmed has seen it.

Disconnect/removal: `claude mcp remove varos`. After restarting Varos, repeat registration with its new endpoint/token. No auto-save happens on agent disconnect.

## API details and refusal behavior

MCP is UTF-8 newline JSON-RPC 2.0 on stdio, with protocol revision **2025-06-18**. Stdout is protocol only. `initialize` and `notifications/initialized` precede tools. It returns both `structuredContent` and the shared compact text projection; consume one representation. Tool failures have `isError: true` and the same ADR error payload as CLI. JSON-RPC framing/method errors are separate; unknown tool names return `-32602`. Cancelled requests receive no MCP response. EOF requests cancellation and waits at most 500 ms for the worker; it does not reverse any published edit.

`capabilities {"api":"1.0"}` reports the launch epoch, proxy client identity, actual enabled verbs and limits. Different Bridge API versions are refused before editing. Unknown fields/variants are refused, including model-supplied `confirm`, `allow_history`, `allow_destructive`, filesystem destinations and unsupported edit verbs. MCP input schemas are complete for this slice. Wire DTOs do not expose `EditCommand` or `AppCommand`.

`describe {"board":"b2"}` returns a small header, counts, outline bounds, up to 20 artboards and a bounded selection preview. `fields` with `bounds`, `paint`, `parent`, `name`, `state`, `geometry` requests object details; omit `ids` for paint-order pagination. Explicit detail IDs are capped at 1,000, each page at 100 objects / 16 KiB readable text. Repeat the same query with its cursor; a changed revision/query returns `resync_required`. `fields:["metadata"]` reads board metadata only; `fields:["artboards"]` reads paginated page properties. Each page has a persistent `id` such as `artboard:7` (format 4: it survives edits, undo/redo and save/reopen; a new page never reuses a removed page's id) plus its current `index`; the summary and receipts name the `active_artboard`. The old revision-bound `ref` (`a0@12`, valid only at revision 12) is still returned and accepted as an `align` target for **one more slice** — it is deprecated; use `artboard:N`. `settings_digest` records settings changes without exposing an internal enum schema.

`describe {"board":"b2","since":12}` returns the exact net projection change, including human edits and history, or `resync_required` if the requested boundary was not observed/retained or the net diff exceeds the budget. Journal limits are 128 revision boundaries / 8 MiB per board. Large edit receipts remain committed successes and return counts plus a revision-pinned detail query/cursor instead of an unmarked partial dump. Geometry changes are marked, not dumped. Selection previews carry count/more fields. Idle event-loop observation runs only while an authenticated socket client is attached. It compares the editor revision, a fixed-size settings snapshot and dirty flag before fingerprinting; unchanged documents cost no serialization or SHA-256. Transient selection is sampled at request boundaries. Disconnected changes are reconciled on the next request; unobserved intermediate revisions require resync.

Mutation IDs are positive canonical `r1`, `r2`, …, increasing for each proxy client. After a successful mutation, the same ID and payload returns the original receipt, even after a human edit; a different payload is refused. Pre-commit failures (including `busy`, `cancelled`, `revision_conflict` and a worker deadline before publication) retain no receipt and do not advance the high-water sequence: retry the same ID with fresh `expected_rev` after resolving the cause. Successful IDs must still increase; a later successful ID retires earlier unused IDs. The cache retains 128 receipts/client and a high-water sequence, with at most 64 mutating clients per desktop launch. `request_status` queries the retained receipt for the same client. A new one-shot CLI normally creates a new client; use an explicit 64-hex `--client` identity to resume one across calls. A host restart changes the epoch and loses this in-memory journal/receipt history. A disconnect/timeout is not evidence of rollback: inspect the receipt/revision before retrying. Cancellation before publication changes nothing; cancellation after publication does not reverse a committed edit.

Reads leave pending fields open and return the last committed state; they never commit a field or create an undo step. Only mutations settle valid pending fields as a separate human undo step, then recheck `expected_rev`. Invalid fields or active gestures return `busy`; the bridge does not dismiss or finish a human gesture. A human settings change that bypassed the old editor revision path is observed and advances the Bridge document revision. A stale request returns `revision_conflict` with expected/actual values. Nothing implicitly switches tabs.

The runtime queue admits 32 pending socket requests; there are at most 8 socket workers, 6 MCP tool calls, 128 outstanding confirmation challenges and 64 cancellation entries (60-second expiry). The listener polls idle accept at 50 ms. On shutdown it cancels pending work and shuts down active sockets before joining workers (500 ms worker wait, at most 1 second in listener drop). Socket requests expire after 30 seconds awaiting the host; query the receipt after a timeout. Requests are capped at 1 MiB / 100 operations / 1,000 explicit and expanded targets. Staging checks each operation's preconditions, then validates/encodes the final document once. Only a final validation failure replays prefixes to identify the first invalid operation; failure identifies the zero-based operation index and publishes nothing. No latency or maximum-format-size responsiveness promise is made; large-board staging/projection measurements remain release work.

## Design verbs and request-local identities

`edit` keeps the existing revision, receipt, scope, cancellation and one-step undo contracts. Each operation resolves and checks its targets against the isolated staged document, then the final document is checked/encoded before one publication. A later failure returns its zero-based `op_index` and leaves the live document, allocator, dirty state, undo/redo, selection, tool and drawing defaults unchanged. Successful deletion prunes references to removed entities from human selection; it never selects the new artwork. All explicit and expanded targets remain bounded to 1,000 across the batch.

Creation defaults: absent fill/stroke mean no paint; stroke width is 0 and opacity is 1. Supply a fill or stroke for visible artwork. A missing parent uses the human's active layer. `parent` must be an editable visible `node:N` layer. `insert` defaults to `"top"`, the only supported position, at the front of that layer. Existing siblings keep their relative order. Name input is bounded to 256 characters, cleaned by the core's existing name cleaner; an empty cleaned name is refused. Receipts carry the actual cleaned name.

A creation may supply `"local":"$a"`; `group` may supply `"local":"$poster"`. These aliases are ASCII `$` + a letter + letters/digits/underscores (maximum 64 bytes), unique within that request. Later `ids` may reference them; forward/unknown/duplicate aliases are refused. Successful receipts return `result.locals`, mapping aliases to actual allocated `path:N` / `node:N` IDs. Aliases expire after the request and cannot be used in another request, `select`, or a read. A mapped identity removed later in the same batch is no longer a live object; even a successful create-then-delete batch with no net undo entry reserves its allocated IDs against reuse. This is separate from the visible `name` property. Shape allocation uses the core `EditCommand::AddShape` outcome and the existing session high-water allocator, including undo-then-create branches.

| Verb | Operation fields and behavior |
|---|---|
| `add_shape` | `kind: "rect"` or `"ellipse"`, `bounds: [x,y,width,height]` with positive finite dimensions; optional `parent`, `insert`, `local`, `name`, `fill`, `stroke`, `stroke_width`, `opacity`. Ellipses use the existing four cubic anchors/handles. Plain rectangles use four corner anchors. The current model has no corner radius. |
| `resize` | `ids`, `bounds: [x,y,width,height]`. Explicit top-left anchor, unconstrained proportions. For one rotated unit, width/height are local dimensions and x/y locate the world bounding-box top-left; several units use world bounds, matching the core. Degenerate bounds and requests that would trigger the core's minimum-size/scale clamp are refused. |
| `rotate` | `ids`, `degrees` (absolute finite degrees for each complete object/group, including mixed rotations). |
| `rename` | `ids`, `name`; dispatches path/leaf-node/container identities to the correct core command. |
| `delete` | `ids`; deletes complete objects/groups, including their descendants. Requires the destructive confirmation below. Layer deletion and partial-group deletion are deferred. |
| `align` | `ids`, `mode: left|center|right|top|middle|bottom`, `target: "selection"` (at least two complete units), `"artboard:N"`, a request-local bound to a page (`"$story"`, including one added earlier in the same batch), or the deprecated revision-bound `"a0@12"` (refused with `invalid_argument` in any batch that also contains a page verb, because page verbs can shift indices; use `artboard:N` there). Units/groups remain rigid. Missing, hidden, locked or stale artboards are refused; no Auto/fallback; the active page is not changed. |
| `distribute` | `ids`, `axis: "h"` or `"v"`; equal centers, at least three distinct editable leaf paths. Groups/mixed grouped targets and explicit gaps are refused. |
| `group` | `ids`, optional `local`; at least two complete units sharing a parent. Existing groups can be nested, with all affected descendants checked. Returns the new group ID through `created` and optional `locals`. |
| `ungroup` | `ids`; explicit complete top-level `node:N` groups only, dissolves one level. Requires destructive confirmation. |
| `order` | `ids`, `order: front|forward|backward|back`; within the existing parent, never arbitrary reparenting. |
| `set_paint` | Existing `ids`, `fill`, `stroke`, `stroke_width`, `opacity`; opacity is already folded into this verb (0–1). |
| `add_artboard` | Exactly one of `bounds: [x,y,width,height]` or `preset: "square"|"portrait"|"story"|"a4"` (1080×1080, 1080×1350, 1080×1920 px = pt at 72 ppi; A4 595×842 pt — the Start presets). A preset may take `origin: [x,y]`; without it the page goes right of the right-most page (60 pt gap, top-aligned with the active page) or at the origin on a free canvas. Optional `name` (default "Artboard N", cleaned like object names) and `local`. Sides must be ≥ 1 pt and finite; ≤ 1,000 pages. Returns `artboard:N` via `locals` and `artboards_created`. Does **not** change the active page. White page, clip on (the editor's default). |
| `resize_artboard` | `id`, `bounds: [x,y,width,height]` set exactly (same checks). Artwork does not move with the page (the panel's X/Y/W/H behaviour). A locked page is refused (`locked_target`). |
| `rename_artboard` | `id`, `name` (bounded and cleaned; empty refused). Allowed on a locked page. |
| `delete_artboard` | `id`. Removes the page only; its artwork stays (a floater, or on the other pages it overlaps). Destructive: needs the confirmation below. **Active rule:** deleting the active page while other pages remain is refused — put `set_active_artboard` for another page earlier in the same batch; deleting the last page leaves a free canvas. Locked pages are refused. |
| `set_active_artboard` | `id`. Makes the page the active one (where the human's panel/export "active artboard" points). Navigation, not content: alone it creates no undo step and no new revision; the receipt and `describe` show `active_artboard`. The human's artboard multi-selection is re-pointed to it. |

Page verbs address pages only by `artboard:N` or a request-local bound to one; an `artboard:` id in an object `ids` list is refused (pages are not objects). No page verb changes the active page as a side effect, and the human's artboard multi-selection is carried across insertions/removals by id. Reorder, duplicate and page colour/clip/bleed edits are not in this slice (`capabilities.unsupported`).

Resize/rotate/align/group/order require complete units. A bare leaf inside a group cannot cause implicit sibling changes: supply the group ID or its complete descendant target set. Layer IDs remain supported for ordinary move/paint/rename and describe; structural layer mutations are refused. No `add_path` is included in this slice: safe anchor allocation/creation and its additional geometry schema are deferred. `capabilities.unsupported` lists this and the other deferred operations.

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

`describe {"board":"b1","rev":2,"ids":["path:8"],"fields":["geometry"]}` explicitly requests path `closed`, local `anchors` (`point`, optional `hin`/`hout`), `holes`, and `world_transform`. Containers have `geometry: null`; use bounds/parent for them. Geometry stays off default summaries, diffs and receipts. Object pagination/cursors apply, with at most 1,000 anchors (outer + holes) per object and the existing 16 KiB text budget. An individual object exceeding either budget returns `limit_exceeded` with no unmarked partial geometry. Finer within-path geometry pagination is deferred.

`snapshot {"board":"b1","rev":2}` explicitly requests an immutable revision-pinned CPU preview via `varos-raster`, default **544×246**, with optional integer `width`/`height` each 1–1024. Adding `"artboard":"artboard:N"` renders only that page instead (see the story section). A stale revision is refused. It constructs no window/GPU, creates no history entry and changes no selection/tool. This is the thumbnail-style fitted preview, including the dark dotted well; it is not production PNG export or GPU pixel parity. Encoded PNGs exceeding the existing transport-frame budget are refused.

MCP returns an `image/png` image content block only for this requested tool; structured/text content contains dimensions/revision/preview metadata, never a duplicate base64 dump. CLI writes bytes only to its explicit local destination (a new file; overwrite/symlinks at the final filename are refused), and prints metadata instead of image bytes:

```sh
printf '%s\n' '{"board":"b1","rev":2,"width":544,"height":246}' | \
  target/debug/varos-cli bridge snapshot --attach "$BRIDGE_SOCKET" --token "$BRIDGE_TOKEN" \
  --output /explicit/path/poster-preview.png --json
```

The destination is a CLI-local option, never a host wire field or a desktop filesystem grant. MCP takes no destination. Use actual receipt revision and allocated IDs in both examples.

## Temporary owner destructive grant — optional

By default `delete` and `ungroup` return `confirmation_required` after validating/staging the entire batch, with exact affected IDs, `expected_rev`, `actual_rev` and a payload digest. A model-provided `confirm:true` or digest alone grants nothing. Nothing has been published at this point.

For this temporary slice, the owner may launch the **desktop** with `VAROS_BRIDGE_ALLOW_DESTRUCTIVE=1 target/debug/varos`. The listener reads this once at startup and issues exact, 60-second, single-use grants. Retry the exact `edit` envelope/operations/request_id/revision with its returned `digest`; the host binds the grant to client, board, epoch, source and staged result, rechecks revision, then consumes it before publication. Modified payloads, another client/board, human revision changes or expiration invalidate the grant. Completed retries use the existing cached receipt. Destructive edits in a mixed batch still publish only once and undo once. This grant does not authorize shared undo/redo; the separate history policy below remains required for those requests. Clients have no grant-enabling flag/Hello field. Restart the desktop without this environment variable for default refusal. No confirmation UI is added.

## Temporary owner history grant — optional

The sample exercise uses human ⌘Z and does **not** need this grant. By default agent `history` requests return `confirmation_required` with affected IDs, expected revision and a digest; a model-supplied boolean never authorizes history.

For this temporary slice only, Ahmed may deliberately launch the **desktop** with `VAROS_BRIDGE_ALLOW_HISTORY=1 target/debug/varos`. The desktop reads this owner-set environment variable once when its listener starts; unset it (or use any value other than `1`) for default refusal. This is an owner-established policy to issue exact, 60-second, single-use grants for shared undo/redo requests. It can undo a **human** history entry. Even with the desktop grant, the first request must receive the challenge; repeat the exact board/action/revision/request_id with the returned `digest`. The host binds it to the actual history entry and epoch, rechecks revision, consumes it once, and caches the completed receipt. Changes invalidate it. The client has no `--allow-history` option and the Hello DTO rejects `allow_history` as an unknown field. No tool or token-bearing client can enable this host grant, and no confirmation UI or blanket `confirm:true` bypass exists. Restart the desktop without the environment variable when not testing history.

## CLI through the same implementation

```sh
# JSON arguments on stdin, or --request-file request.json (same tool argument object).
printf '%s\n' '{"api":"1.0"}' | target/debug/varos-cli bridge capabilities \
  --attach "$BRIDGE_SOCKET" --token "$BRIDGE_TOKEN" --json

printf '%s\n' '{"board":"b2","fields":["bounds","paint","parent"]}' | \
  target/debug/varos-bridge describe --attach "$BRIDGE_SOCKET" --token "$BRIDGE_TOKEN"
```

CLI defaults to the compact text; `--json` returns the same structured receipt as MCP. Failures exit nonzero. The CLI and MCP bindings only decode/encode, authenticate and call the one service. They never reload/overwrite the file behind the live editor.

## Verification boundary

Tests construct no GPU renderer or event loop. Frozen API 1.0 describe/edit/error/poster fixtures, actual MCP binding with fake-host CLI parity, each design verb, indexed rollback, request-local identity, confirmation binding, bounded geometry, valid PNG dimensions, explicit CLI output, one-step undo/redo, revisions, busy/inactive/scope/auth refusal, cancellation, journals, monotonic receipts and allocator branching run headlessly. Two real Unix-socket integration tests are explicitly ignored by default because the implementation sandbox rejects socket bind with `EPERM`; this is **unverified native IPC**, not a pass. Run them from a normal macOS terminal:

```sh
cargo test -p varos-bridge --test contracts -- --ignored
```

Then perform Ahmed's window exercise above. Kernel peer-uid checks, real socket lifecycle and the owner window remain separate from the passing fake-host proof. No merge, installation or owner acceptance is claimed.
