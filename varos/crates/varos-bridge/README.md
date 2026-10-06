# Varos Bridge — attached slice (API 1.0)

Ahmed: this lets Claude Code describe and change the board already open in Varos. A move + solid recolour in one `edit` request is one normal undo step. There is no new menu or panel. The app never calls an AI provider itself.

This slice implements `capabilities`, `list_boards`, `describe`, `select`, `edit` (`move`, `set_paint`), `history`, and `request_status`. Files, saving/export, snapshots, geometry dumps, shape creation, structural edits, headless hosting and Windows attachment are not enabled. The existing standalone `varos-cli describe/apply` API 0.x remains a separate provisional file workflow; attached API 1.0 is `varos-cli bridge …` and uses exactly the same service as MCP.

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

`capabilities {"api":"1.0"}` reports the launch epoch, proxy client identity, actual enabled verbs and limits. Different Bridge API versions are refused before editing. Unknown fields/variants are refused, including model-supplied `confirm`, `allow_history`, filesystem destinations and unsupported edit verbs. MCP input schemas are complete for this slice. Wire DTOs do not expose `EditCommand` or `AppCommand`.

`describe {"board":"b2"}` returns a small header, counts, outline bounds, up to 20 artboards and a bounded selection preview. `fields` with `bounds`, `paint`, `parent`, `name`, `state` requests object details; omit `ids` for paint-order pagination. Explicit detail IDs are capped at 1,000, each page at 100 objects / 16 KiB readable text. Repeat the same query with its cursor; a changed revision/query returns `resync_required`. `fields:["metadata"]` reads board metadata only; `fields:["artboards"]` reads paginated page properties. Artboard references are `a0@12`, tied to revision 12; no artboard mutation verbs exist. `settings_digest` records settings changes without exposing an internal enum schema.

`describe {"board":"b2","since":12}` returns the exact net projection change, including human edits and history, or `resync_required` if the requested boundary was not observed/retained or the net diff exceeds the budget. Journal limits are 128 revision boundaries / 8 MiB per board. Large edit receipts remain committed successes and return counts plus a revision-pinned detail query/cursor instead of an unmarked partial dump. Geometry changes are marked, not dumped. Selection previews carry count/more fields. Idle event-loop observation runs only while an authenticated socket client is attached. It compares the editor revision, a fixed-size settings snapshot and dirty flag before fingerprinting; unchanged documents cost no serialization or SHA-256. Transient selection is sampled at request boundaries. Disconnected changes are reconciled on the next request; unobserved intermediate revisions require resync.

Mutation IDs are positive canonical `r1`, `r2`, …, increasing for each proxy client. After a successful mutation, the same ID and payload returns the original receipt, even after a human edit; a different payload is refused. Pre-commit failures (including `busy`, `cancelled`, `revision_conflict` and a worker deadline before publication) retain no receipt and do not advance the high-water sequence: retry the same ID with fresh `expected_rev` after resolving the cause. Successful IDs must still increase; a later successful ID retires earlier unused IDs. The cache retains 128 receipts/client and a high-water sequence, with at most 64 mutating clients per desktop launch. `request_status` queries the retained receipt for the same client. A new one-shot CLI normally creates a new client; use an explicit 64-hex `--client` identity to resume one across calls. A host restart changes the epoch and loses this in-memory journal/receipt history. A disconnect/timeout is not evidence of rollback: inspect the receipt/revision before retrying. Cancellation before publication changes nothing; cancellation after publication does not reverse a committed edit.

Reads leave pending fields open and return the last committed state; they never commit a field or create an undo step. Only mutations settle valid pending fields as a separate human undo step, then recheck `expected_rev`. Invalid fields or active gestures return `busy`; the bridge does not dismiss or finish a human gesture. A human settings change that bypassed the old editor revision path is observed and advances the Bridge document revision. A stale request returns `revision_conflict` with expected/actual values. Nothing implicitly switches tabs.

The runtime queue admits 32 pending socket requests; there are at most 8 socket workers, 6 MCP tool calls, 128 outstanding confirmation challenges and 64 cancellation entries (60-second expiry). The listener polls idle accept at 50 ms. On shutdown it cancels pending work and shuts down active sockets before joining workers (500 ms worker wait, at most 1 second in listener drop). Socket requests expire after 30 seconds awaiting the host; query the receipt after a timeout. Requests are capped at 1 MiB / 100 operations / 1,000 explicit and expanded targets. Staging checks each operation's preconditions, then validates/encodes the final document once. Only a final validation failure replays prefixes to identify the first invalid operation; failure identifies the zero-based operation index and publishes nothing. No latency or maximum-format-size responsiveness promise is made; large-board staging/projection measurements remain release work.

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

Tests construct no GPU renderer or event loop. Frozen API 1.0 describe/edit/error fixtures, fake-host MCP/CLI parity, indexed rollback, one-step undo, revisions, busy/inactive/scope/auth refusal, cancellation, journals, monotonic receipts and allocator branching run headlessly. Two real Unix-socket integration tests are explicitly ignored by default because the implementation sandbox rejects socket bind with `EPERM`; this is **unverified native IPC**, not a pass. Run them from a normal macOS terminal:

```sh
cargo test -p varos-bridge --test contracts -- --ignored
```

Then perform Ahmed's window exercise above. Kernel peer-uid checks, real socket lifecycle and the owner window remain separate from the passing fake-host proof. No merge, installation or owner acceptance is claimed.
