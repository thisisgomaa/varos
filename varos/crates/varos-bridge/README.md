# Varos Bridge — attached design slice (API 1.0)

Ahmed: this lets Claude Code describe and change the board already open in Varos. A move + solid recolour in one `edit` request is one normal undo step. There is no new menu or panel. The app never calls an AI provider itself.

This slice implements `capabilities`, `list_boards`, `describe` (including explicit bounded geometry), `select`, `edit` (`move`, `set_paint`, `add_shape`, `resize`, `rotate`, `rename`, `delete`, `align`, `distribute`, `group`, `ungroup`, `order`, and — slice 3 — the page verbs `add_artboard`, `resize_artboard`, `rename_artboard`, `delete_artboard`, `set_active_artboard`), `snapshot` (whole board or one page), `history`, and `request_status`. Files, saving/export, arbitrary paths, rounded rectangles, headless hosting and Windows attachment are not enabled. The existing standalone `varos-cli describe/apply` API 0.x remains a separate provisional file workflow; attached API 1.0 is `varos-cli bridge …` and uses exactly the same service as MCP.

Slice 1 has landed and the owner verified live move + recolour followed by one human undo. Slice 3's live acceptance (the Instagram Story batch below on an open board, then one ⌘Z) is pending. Slice 2's live acceptance is still pending: an external agent builds the poster below on the active open board, asks for a snapshot, then the owner checks the objects and undoes the entire design once. Headless tests and CPU preview inspection do not establish live desktop acceptance.

## Try it on macOS — register once, then just say "use Varos"

**مرة واحدة بس:** تسجّل Varos عند Claude Code بأمر واحد. بعد كده أي جلسة جديدة في أي فولدر تقدر تقول "use Varos" وخلاص — من غير سكريبتات ولا توكن في أي ملف إعدادات. أول مرة الوكيل بيتصل، Varos بيطلب موافقتك أنت (من التيرمنال مؤقتاً لحد ما يتعمل شكل في البرنامج).

1. **Install Varos with its helpers** (the helpers live inside the app so their path never changes): `tools/mac/bundle.sh` now copies `varos-bridge` and `varos-cli` into `Varos.app/Contents/MacOS/`. An older installed Varos does not contain the C1 listener.
2. **Register once** (user scope — every folder, every new session):

   ```sh
   /Applications/Varos.app/Contents/MacOS/varos-cli bridge register claude
   ```

   This runs `claude mcp add --transport stdio --scope user varos -- /Applications/Varos.app/Contents/MacOS/varos-bridge mcp`. If `claude` is not on your PATH it prints that exact command instead (and, if Claude Code desktop's own CLI is found, how to use it: `… register claude --claude '<path>'`). `--replace` replaces only an existing `varos` entry; `--dry-run` only prints. Other clients: `register codex` (runs `codex mcp add varos -- <path> mcp`), `register cursor` / `register print` (print the `mcpServers` entry to merge into `~/.cursor/mcp.json`). The entry is only the executable and `mcp` — **no token, socket path or key**.
3. **Open Varos**, then open Claude Code in **any** folder and say **"use Varos"**. Varos's window never waits for the Keychain: its connection key loads in the background. If macOS asks whether Varos may use its Keychain item, choose **Always Allow**; if access is refused or the Keychain is locked, Varos keeps working and shows one notice that agents can't connect in this session. The agent's tools load immediately even if Varos is closed; a call then answers `host_not_running` ("open Varos") instead of hanging.
4. **First time only — approve the agent.** The first call answers `pairing_required` with a request id and a short match code (like `4F2-9A1`) — the agent shows you both. In **your own Terminal** (an agent cannot do this for you: the command needs an interactive terminal):

   ```sh
   /Applications/Varos.app/Contents/MacOS/varos-cli bridge pair            # see who is waiting (label is a claim; the key is verified)
   /Applications/Varos.app/Contents/MacOS/varos-cli bridge pair --approve <request-id>   # default scopes: read,edit
   ```

   `--approve` does **not** print the code: it asks you to type the match code the agent showed you, which proves you are approving that same agent. A wrong code approves nothing. Add `--scopes read,edit,destructive` or `…,history` only if you want that agent to be *allowed to ask* for delete/ungroup or shared undo (each still needs the exact confirmation described below). `--deny <request-id>` refuses. Requests expire after 10 minutes. **This terminal approval is temporary** — a pairing screen inside Varos needs saved mockups and owner approval first (ADR-0011 §3), so none is built.
5. Ask the agent again; it now works. Approval is remembered across Varos and Claude Code restarts. A fresh Claude Code session reuses the same approved profile.

**See / remove agents:** `varos-cli bridge agents` lists approved agents and scopes; `varos-cli bridge agents revoke <profile-id>` removes one (its next call — even one already queued — is refused; it would have to be paired again as a new agent); `varos-cli bridge agents audit` shows the recent audit log; `varos-cli bridge hosts` lists running Varos launches. To unregister from Claude Code: `claude mcp remove --scope user varos`.

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
- **Identity.** One per-user host key (loaded on a background thread at launch; a refusal is reported once through Varos's normal notice and the app runs without the paired listener) and one key per agent profile (Ed25519, `ed25519-dalek`), stored in the macOS login Keychain (service `com.varos.bridge`, accounts `host` / `agent:<profile-id>`) behind a `CredentialStore` trait. A locked/unavailable Keychain returns `credential_unavailable`; there is no plaintext fallback. Windows/Linux have no store yet (`credential_unavailable`; Windows is compile-only).
- **Handshake (connection 1.0).** Agent nonce → host signs (versions offered/chosen, API, instance, epoch, host key, both nonces) → agent checks the signature, the registry fingerprint and the epoch, then signs the same binding plus its profile/session (offered versions are length-prefixed in the signed transcript) over the host's fresh nonce → host verifies once, checks the trust store and either admits the call with the approved scopes or records a pairing request. Peer uid is still checked on every connection. If Varos's own key changed since an agent was approved (e.g. its Keychain item was reset), the agent is not stuck: it gets `pairing_required` saying the key changed, and one new approval fixes it. Nothing about boards is released before this succeeds.
- **Trust store.** `~/Library/Application Support/Varos/bridge/trust.json` (0600, folder 0700): approved profiles (public key, fingerprint, claimed label, scopes, pinned host fingerprint), a revocation list and a monotonic trust generation, changed under an `flock` transaction. `profiles.json` maps an MCP client name (a hint, never identity) to its profile id. The host re-reads trust on every connection and again on the owning thread right before the call runs; an unreadable store denies.
- **Scopes.** `read` (capabilities/list/describe/snapshot/status), `edit` (select and ordinary edits), `destructive` (eligibility to request delete/ungroup/delete_artboard) and `history` (eligibility to request shared undo/redo). Default approval is `read,edit`. Destructive and history still need the exact single-use owner grant below; a scope alone never confirms anything. Board policy in C1 is "all exposed open boards" (per-board grants are C2).
- **Audit.** `~/Library/Application Support/Varos/bridge/audit/audit.log` (0600, persistent): time, agent profile, session prefix, request id, verb, board handle, from/to revision, result code, plus pairing/revocation events — never names, paths, payloads or keys. Rotation (10 MiB; rotated file deleted after 30 days) and appends happen under one lock. A mutation is refused (`audit_unavailable`) if its audit line cannot be written first. Opt-in legacy token calls are audited as agent `legacy`. It is a diagnostic log, not tamper-proof against the account owner.
- **Legacy transition (deprecated, one slice).** The slice-1 per-launch token endpoint (`$TMPDIR/varos-bridge-<pid>-<launch>/endpoint.json`, `varos-cli bridge-endpoint`, `--attach <socket> --token …`) is **off by default** — it would make pairing optional. Only if you deliberately launch the desktop with `VAROS_BRIDGE_LEGACY=1` does it run, on its **own** socket, for old setups during this one transition slice. Its token then grants every scope without pairing, exactly as before (its calls are audited as agent `legacy`); the paired socket never accepts it. `tools/mac/bridge-connect.sh` no longer needs it: it only runs `varos-cli bridge register claude`. It is removed in the next connection slice.
- **Test override.** `VAROS_BRIDGE_HOME=<existing 0700 folder>` relocates discovery and trust files for tests. Debug builds only — a release build ignores it with a warning. It never relocates or weakens secrets.
- **Keychain prompts after rebuilds (owner decision).** The login Keychain trusts the *signing identity* of the binary that created each item. Ad-hoc signatures (every `cargo build`, and `bundle.sh` by default) change on each build, so macOS asks again after each rebuild. A stable self-signed code-signing identity, created once by the owner (Keychain Access ▸ Certificate Assistant ▸ Create a Certificate, type "Code Signing"), passed as `CODESIGN_ID="<name>" tools/mac/bundle.sh`, keeps the same identity across rebuilds and stops those prompts for the installed app. Nothing creates such a certificate automatically; `target/debug` builds stay ad-hoc.

## API details and refusal behavior

MCP is UTF-8 newline JSON-RPC 2.0 on stdio, with protocol revision **2025-06-18**. Stdout is protocol only. `initialize` and `notifications/initialized` precede tools. It returns both `structuredContent` and the shared compact text projection; consume one representation. Tool failures have `isError: true` and the same ADR error payload as CLI. JSON-RPC framing/method errors are separate; unknown tool names return `-32602`. Cancelled requests receive no MCP response. EOF requests cancellation and waits at most 500 ms for the worker; it does not reverse any published edit.

`capabilities {"api":"1.0"}` reports the launch epoch, proxy client identity, actual enabled verbs and limits. Different Bridge API versions are refused before editing. Unknown fields/variants are refused, including model-supplied `confirm`, `allow_history`, `allow_destructive`, filesystem destinations inside edit batches and unsupported edit verbs. MCP input schemas are complete for this slice. Wire DTOs do not expose `EditCommand` or `AppCommand`.

`describe {"board":"b2"}` returns a small header, counts, outline bounds, up to 20 artboards and a bounded selection preview. `fields` with `bounds`, `paint`, `parent`, `name`, `state`, `geometry` requests object details; omit `ids` for paint-order pagination. Explicit detail IDs are capped at 1,000, each page at 100 objects / 16 KiB readable text. Repeat the same query with its cursor; a changed revision/query returns `resync_required`. `fields:["metadata"]` reads board metadata only; `fields:["artboards"]` reads paginated page properties. Each page has a persistent `id` such as `artboard:7` (format 4: it survives edits, undo/redo and save/reopen; a new page never reuses a removed page's id) plus its current `index`; the summary and receipts name the `active_artboard`. The old revision-bound `ref` (`a0@12`, valid only at revision 12) is still returned and accepted as an `align` target for **one more slice** — it is deprecated; use `artboard:N`. `settings_digest` records settings changes without exposing an internal enum schema.

`describe {"board":"b2","since":12}` returns the exact net projection change, including human edits and history, or `resync_required` if the requested boundary was not observed/retained or the net diff exceeds the budget. Journal limits are 128 revision boundaries / 8 MiB per board. Large edit receipts remain committed successes and return counts plus a revision-pinned detail query/cursor instead of an unmarked partial dump. Geometry changes are marked, not dumped. Selection previews carry count/more fields. Idle event-loop observation runs only while an authenticated socket client is attached. It compares the editor revision, a fixed-size settings snapshot and dirty flag before fingerprinting; unchanged documents cost no serialization or SHA-256. Transient selection is sampled at request boundaries. Disconnected changes are reconciled on the next request; unobserved intermediate revisions require resync.

Mutation IDs are positive canonical `r1`, `r2`, …, increasing for each proxy client. After a successful mutation, the same ID and payload returns the original receipt, even after a human edit; a different payload is refused. Pre-commit failures (including `busy`, `cancelled`, `revision_conflict` and a worker deadline before publication) retain no receipt and do not advance the high-water sequence: retry the same ID with fresh `expected_rev` after resolving the cause. Successful IDs must still increase; a later successful ID retires earlier unused IDs. The cache retains 128 receipts/client and a high-water sequence, with at most 64 mutating clients per desktop launch. `request_status` queries the retained receipt for the same client. A new one-shot CLI normally creates a new client; use an explicit 64-hex `--client` identity to resume one across calls. A host restart changes the epoch and loses this in-memory journal/receipt history. A disconnect/timeout is not evidence of rollback: inspect the receipt/revision before retrying. Cancellation before publication changes nothing; cancellation after publication does not reverse a committed edit.

Reads leave pending fields open and return the last committed state; they never commit a field or create an undo step. Only mutations settle valid pending fields as a separate human undo step, then recheck `expected_rev`. Invalid fields or active gestures return `busy`; the bridge does not dismiss or finish a human gesture. A human settings change that bypassed the old editor revision path is observed and advances the Bridge document revision. A stale request returns `revision_conflict` with expected/actual values. Nothing implicitly switches tabs.

The runtime queue admits 32 pending socket requests; there are at most 8 socket workers, 6 MCP tool calls, 128 outstanding confirmation challenges and 64 cancellation entries (60-second expiry). The listener polls idle accept at 50 ms. On shutdown it cancels pending work and shuts down active sockets before joining workers (500 ms worker wait, at most 1 second in listener drop). Socket requests expire after 30 seconds awaiting the host; query the receipt after a timeout. Requests are capped at 1 MiB / 100 operations / 1,000 explicit and expanded targets. Staging checks each operation's preconditions, then validates/encodes the final document once. Only a final validation failure replays prefixes to identify the first invalid operation; failure identifies the zero-based operation index and publishes nothing. No latency or maximum-format-size responsiveness promise is made; large-board staging/projection measurements remain release work.

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

The destination is a CLI-local option, never a host wire field or a desktop filesystem grant. The `snapshot` MCP tool takes no destination. Slice-4 file tools have separately authorized host destinations. Use actual receipt revision and allocated IDs in both examples.

## Temporary owner destructive grant — optional

By default `delete` and `ungroup` return `confirmation_required` after validating/staging the entire batch, with exact affected IDs, `expected_rev`, `actual_rev` and a payload digest. A model-provided `confirm:true` or digest alone grants nothing. Nothing has been published at this point.

For this temporary slice, the owner may launch the **desktop** with `VAROS_BRIDGE_ALLOW_DESTRUCTIVE=1 target/debug/varos`. The listener reads this once at startup and issues exact, 60-second, single-use grants. Retry the exact `edit` envelope/operations/request_id/revision with its returned `digest`; the host binds the grant to client, board, epoch, source and staged result, rechecks revision, then consumes it before publication. Modified payloads, another client/board, human revision changes or expiration invalidate the grant. Resending an expired digest returns a fresh `confirmation_required` challenge with affected IDs/revisions/digest and, under the owner policy, renews the 60-second grant; retry that exact challenge. Completed retries use the existing cached receipt. Destructive edits in a mixed batch still publish only once and undo once. This grant does not authorize shared undo/redo; the separate history policy below remains required for those requests. Clients have no grant-enabling flag/Hello field. Restart the desktop without this environment variable for default refusal. No confirmation UI is added.

## Temporary owner history grant — optional

The sample exercise uses human ⌘Z and does **not** need this grant. By default agent `history` requests return `confirmation_required` with affected IDs, expected revision and a digest; a model-supplied boolean never authorizes history.

For this temporary slice only, Ahmed may deliberately launch the **desktop** with `VAROS_BRIDGE_ALLOW_HISTORY=1 target/debug/varos`. The desktop reads this owner-set environment variable once when its listener starts; unset it (or use any value other than `1`) for default refusal. This is an owner-established policy to issue exact, 60-second, single-use grants for shared undo/redo requests. It can undo a **human** history entry. Even with the desktop grant, the first request must receive the challenge; repeat the exact board/action/revision/request_id with the returned `digest`. The host binds it to the actual history entry and epoch, rechecks revision, consumes it once, and caches the completed receipt. Changes invalidate it. The client has no `--allow-history` option and the Hello DTO rejects `allow_history` as an unknown field. No tool or token-bearing client can enable this host grant, and no confirmation UI or blanket `confirm:true` bypass exists. Restart the desktop without the environment variable when not testing history.

## CLI through the same implementation

```sh
# JSON arguments on stdin, or --request-file request.json (same tool argument object).
# Same discovery, Keychain identity and pairing as MCP (the CLI is its own agent profile).
printf '%s\n' '{"api":"1.0"}' | target/debug/varos-bridge capabilities --json

printf '%s\n' '{"board":"b2","fields":["bounds","paint","parent"]}' | \
  target/debug/varos-cli bridge describe --pid <varos-pid>
```

CLI defaults to the compact text; `--json` returns the same structured receipt as MCP. Failures exit nonzero. The CLI and MCP bindings only decode/encode, authenticate and call the one service. They never reload/overwrite the file behind the live editor.

## Verification boundary

Tests construct no GPU renderer or event loop. Frozen API 1.0 describe/edit/error/poster fixtures, actual MCP binding with fake-host CLI parity, each design verb, indexed rollback, request-local identity, confirmation binding, bounded geometry, valid PNG dimensions, explicit CLI output, one-step undo/redo, revisions, busy/inactive/scope/auth refusal, cancellation, journals, monotonic receipts and allocator branching run headlessly. Four real Unix-socket integration tests (legacy token path, real-binary MCP, the C1 paired path: pairing → typed match code → scopes → revoke-while-queued → audit incl. `legacy` → two launches → registry cleanup, and host-key change → `session_reset` → `pairing_required` → re-approve with a background-loaded key) are ignored by default because some sandboxes reject socket bind with `EPERM`. Run them from a normal macOS terminal:

```sh
cargo test -p varos-bridge --test contracts -- --ignored
```

C1 headless tests (`tests/connection.rs`) cover the registry (round trip, permissions, stale/pid-reuse cleanup, forged and symlinked entries), deterministic selection, the handshake (happy path, wrong host/agent key, replayed nonce, epoch mismatch, downgrade), pairing → approve → revoke → refused, bounded pending requests, scope parsing, trust-store read failure, the owning-thread recheck, locked credential store, per-client profiles, a changed host key asking again, one profile per client under concurrent first use (approved profile preferred), the deferred host-key load (never blocks start; refusal reported once), FIFO-safe private reads, the `--attach auto` state machine on a fake registry, and the real `varos-bridge mcp` binary initializing and answering `host_not_running` with no Varos. The macOS Keychain itself is **not** exercised by the automatic tests (they use an in-memory store); `cargo test -p varos-bridge --lib keychain -- --ignored` writes and deletes one throwaway Keychain item. Keychain prompts after rebuilds/re-signing and behaviour across app upgrades are the open C1 signing gate.

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

File tools need an owner-granted `files` scope (default OFF, old profiles remain OFF):

```text
varos-cli bridge pair --approve <request-id> --scopes read,edit,files
```

The owner launches the desktop with `VAROS_BRIDGE_FILE_ROOTS=/absolute/output/folder:/another/folder`.
This is a temporary launch setting, not a request argument or a UI feature. Each directory must
already exist. `save` uses only the CURRENT `.vrs` backing file and refuses a changed/unreadable
external fingerprint. `save_as {path}` writes a fresh `.vrs` destination; `export_pdf {path,scope}`
writes pure PDF. Export scopes: `all_visible_artboards`, visible `artboard:N`, or `artwork_bounds`
for a free canvas. Both explicit-path tools require an absolute path inside an owner root, refuse
backing-file aliases and existing destinations, and prevent symlink/changed-parent escapes. There
is no overwrite confirmation route in this slice: choose a fresh filename. macOS writers also
refuse non-local volumes. Windows hosting remains unsupported; the code is compilation-tested.

These are standalone requests with `api`, `board`, `expected_rev`, and monotonic `request_id`.
They cannot be mixed into an edit batch. All filesystem inspection, encoding and durable writing
use the desktop I/O worker; no inline fallback. An acceptance receipt contains `accepted:true`
and `ticket`; it does not prove a write. Poll `request_status {request_id}` for pending/completed
and the completion receipt. Path-policy refusals arrive in that completion. A completed save says
`durable:true`, or `durable:false` when replacement succeeded but durability was unconfirmed;
the latter keeps the board dirty. Later human edits remain dirty after the captured snapshot saves.
File completion never opens a dialog. Accepted request retries reuse the ticket instead of writing twice.

### Second example: logo mark, rounded label, then export to a granted folder

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
the owner's launch roots, the filename fresh, and the agent must have `files` scope.

Slice 4 review amendment: save_as writes a copy; the board stays on its current file (owner may widen later). Its dirty state and Recent entries stay unchanged. Destinations require an absolute filename, `.vrs` for save_as or `.pdf` for export_pdf (case-insensitive), and no dot-prefixed components. Protected system roots, ~/Library and the running app bundle are refused. Roots are logged to the owner at startup; capabilities exposes only `files_roots_granted: bool`.

File completion codes: `save_conflict` means a fingerprint mismatch or an existing destination (including a publication race); other IO failures use `io_error`; denied grants/destinations use `scope_refused`. request_status returns the failed completion receipt. Missing or expired results return `not_found`, never indefinite pending. Completion audit entries contain verb, board, ticket and result code, without paths.
