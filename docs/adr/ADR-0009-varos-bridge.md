> **Status:** accepted — owner (Ahmed) 2026-10-06 («ماشي»); first slice commissioned the same day
# ADR-0009: Varos Bridge — one local command API for agents and people

- **Date:** 2026-10-06
- **Decision owner:** Product owner (Ahmed)
- **Supersedes:** On acceptance only, ADR-0004's deferral of a documented AI command/query API; not its persisted-model rule (see §7).
- **Superseded by:** None

**ملخص بالمصري**

الـBridge باب محلي يخلي Claude Code أو Codex أو أي وكيل فاهم البروتوكول يقرا ملخص الرسمة ويعدّلها بأوامر واضحة. نفس منطق Varos ونفس التراجع؛ مجموعة تعديلات تنجح كلها أو تترفض كلها بسبب مفهوم. يشتغل مع النافذة المفتوحة، وبعدها من غير نافذة. ده اقتراح، مش ميزة اتبنت ولا موافقة على التنفيذ. الشات جوه البرنامج ييجي بعد إثبات الباب الخارجي، وتصميمه يتعرض كصور قبل أي كود. استخدام اشتراك المستخدم هدف مش ضمان: شروط الشركات وحدود الحساب ممكن تقيّد التكامل.

## Context

[The owner's vision](../VISION_AI_NATIVE.md) asks for cheap, semantic agent control of the same editable boards that humans use. This record makes the proposed contract concrete without presenting existing internal enums as a public API. The inspected baseline is `b0c2d23` in this worktree (2026-10-06).

Evidence (paths below are relative to the repository root):

- `varos/crates/varos-core/src/command.rs:13` defines `EditCommand`; `:244` defines `Editor::execute`, which returns `()` rather than a result. `editor.rs:1290` has a readable Pathfinder precondition, but `:1297` silently returns on failure. Several other edits silently skip invalid or inapplicable targets. A successful call to `execute` therefore does **not** prove the requested edit happened.
- `varos/crates/varos-core/src/editor.rs:3217` owns history: `begin` replaces `pending`, and individual operations call `begin`/`commit` themselves. Wrapping several existing `execute` calls in an outer `begin`/`commit` is **not** a batch implementation. History holds document snapshots; selection and other editor state require separate treatment.
- `varos/crates/varos-core/src/board.rs:1` defines a Board as a document, with metadata and presets. Artboards are pages **inside** that board. `model.rs:193` and `:305` contain path/node IDs; `model.rs:359` has no artboard ID. `model.rs:695` allocates IDs from a document counter, which is restored with history; it is not a lifetime non-reuse guarantee.
- `varos/crates/varos-core/src/format/mod.rs:1` and its `structure`, `validate`, `migrate`, `limits`, and `error` modules provide bounded decoding, structural checks, semantic validation and migrations. The writer is format 3. `.vrs` is PDF with embedded model JSON, not a bare JSON file (`varos/crates/varos-pdf/src/lib.rs:40`, `:136`).
- `varos/crates/varos-app/src/host.rs:1` explicitly documents one FIFO `ActionQueue` **and** its exception: pointer/panel edits act directly outside it. `main.rs:710` (`run_doc`) settles fields and protects field-local undo. `app_command.rs:50` supplies lifecycle commands, not an external control server.
- `varos/crates/varos-app/src/thumbs/raster.rs:32` already renders a document with `tiny-skia` and core `build_scene`, without a GPU. The containing thumbnail service and binary module are app-owned; they are not yet a headless library.

PLAN and STATUS still describe MCP/web implementation as parked; the newer vision calls Bridge the next direction. This **proposal does not silently resolve that scheduling conflict**. Owner acceptance and a bounded work order are required before implementation. This change adds only this record and one open-decision entry in PLAN.

## Decision

Propose a local, versioned Bridge adapter with one validated request/response implementation, used by an MCP stdio server and a CLI. Deterministic edit semantics and transactions stay in `varos-core`; file/session policy, serialization and transport stay outside it. Neither host imports GPU/window/UI dependencies into core. No visible UI is authorized here.

### 1. Vocabulary and mapping to today's code

The following 22 verb families are the v1 contract target, not a claim of availability. `edit` is the envelope for one or more edit verbs; MCP/CLI expose the same names and schemas. All mutating verbs require explicit board and revision, and object operations carry explicit IDs; they never accidentally consume the human's current selection. “Exists” below means the underlying operation exists, **not** that its present validation/error behavior meets the Bridge contract. Every edit needs the checked transaction work in §3.

Core paths in this table abbreviate `varos/crates/varos-core/src/`; app paths abbreviate `varos/crates/varos-app/src/`.

| Verb family | Existing implementation | New work / precise v1 choice |
|---|---|---|
| `open` | `format::decode_model`; `varos_pdf::load_vrs_with_notice`; `Editor::replace_doc`; app `AppCommand::OpenPaths` | Host adapter, bounded authorized path, migration notices, request/result correlation. Open adds/focuses a session; never replaces a dirty board. Not an `EditCommand`. |
| `new_board` | `board::new_board`, `new_board_with_preset`; app `NewBoard`, `NewWithPreset` | Host session creation and returned handle. Zero artboards by default. No new drawing algorithm. |
| `list_boards` | App `Workspace::tabs` and document sessions; `board::display_name` | New paginated host query, only authorized open sessions. Headless lists its own sessions. Not a disk/cloud scan and not a list of artboards. |
| `describe` | Readable `Editor.doc`, tree, bounds, `rev`, `is_editable`; `Document::outline_bbox` | New deterministic query projection, pagination, change journal (§2); no existing summary/diff API. Geometry queries remain pure core. |
| `select` | `Editor::layer_select_set`, `select_all`, `prune_inert_selection` | New checked selection API/command for explicit node/path IDs; current method silently filters. Bridge selection is a transient, deliberate session action, no document undo entry. |
| `add_shape` | `Document::build_shape`, `Editor::shape_anchors`, pointer placement | **New core `AddShape` command**, explicit bounds, parent layer and paint; return allocated IDs. v1 rectangle/ellipse only; no synthetic pointer events, no arbitrary Rust-model insertion. |
| `move` | `EditCommand::Nudge {x,y}` → `Editor::nudge`; absolute placement via `SetObjectBounds` | Checked explicit-target adapter. Relative world delta, no snapping; absolute placement uses world bounding-box anchor. No new transform algorithm. |
| `resize` | `SetObjectBounds` → `set_obj_bbox` | Checked positive dimensions and explicit anchor/proportions. A single rotated unit uses local dimensions, multiple units use world bounds, matching core; reject degenerate bounds, do not silently clamp requests. |
| `rotate` / `flip` | `SetObjectRotation`, `Flip(bool)` | Checked angle/axis and explicit targets. Rotation is absolute degrees as in core, not an implied delta. |
| `set_paint` | `ApplyPaint {target,color}`, `SetStrokeWidth`, `SetOpacity` | Fill/stroke solid RGBA or `null` (none), width ≥ 0, opacity 0–1. Multiple properties in one transaction. Explicit targets prevent empty selection from changing future drawing defaults. Gradients deferred. |
| `align` | `Align {mode,target}` → `align`; six `AlignMode` values | Require explicit selection or artboard reference, never `Auto`. Check reference exists; core currently falls back when artboard is absent. Left/center/right/top/middle/bottom. |
| `distribute` | `Distribute(DistAxis)` → `distribute`; `Editor::distribute_spacing` exists without a command variant | v1 equal centers, horizontal/vertical, ≥3 eligible leaf paths. Group distribution and explicit gap deferred; gap would need a new command wrapper. Reject unsupported mixed/group targets. |
| `pathfinder` | `Boolean(BoolOp)` → `pathfinder`; `pathfinder_enabled` | Unite, minus-front, intersect, exclude. Reject any ineligible input rather than skipping it; require ≥2 closed editable paths. Preserve core stacking-order semantics and bottom paint inheritance; return removed and new IDs, including a valid empty result. |
| `group` / `ungroup` | `GroupSelection`, `UngroupSelection` | Checked structural targets and parent rules; include affected descendants in validation and confirmation where required. No new grouping algorithm. |
| `order` | `Arrange(ZOrder)`; `MoveLayer {sources,target,position}` | Front/forward/backward/back in v1. Arbitrary reparenting is deferred despite the existing internal command; reject it explicitly. |
| `delete` | `DeleteSelected`, `DeleteLayerSelection` | Explicit whole-object/group deletion, with expanded target set and confirmation (§8). No anchor deletion, file removal, or recovery deletion in v1. |
| `rename` | `RenamePath`, `RenameNode` | Checked ID/type dispatch and existing name cleaning. Return actual cleaned name, reject invalid/empty input rather than an ambiguous success. |
| `set_board` | `SetBoardName/Description/Tags`; `try_set_board_*`; `board::check_*` | Reuse cleaning, Unicode bounds and tag normalization, return normalized values. New batch error propagation; no second metadata ruleset. |
| `undo` / `redo` | `EditCommand::Undo/Redo`; `Editor::undo/redo` | Standalone history request, exactly one shared history entry. Typed `nothing_to_undo/redo`. Can affect human edits; no separate agent history. Not allowed inside `edit`. |
| `snapshot` | app `thumbs::raster::rasterize`, `Raster::encode_png`; core `build_scene` | Extract shared CPU raster facility (§4); immutable revision-pinned board image, default 544×246, max 1024×1024. No window capture and no GPU construction. |
| `export_pdf` | `varos_pdf::plan_pdf_export`, `export_pdf_bytes`; app `ExportPdf(SessionId, ExportScope)` and `FileJobs` | New authorized explicit-destination host route (current app command opens a save dialog). Pure PDF, no editable payload; all visible artboards, one revision-bound artboard (mapped to `ActiveArtboard` on the export snapshot), or artwork bounds only for a free canvas. Reject hidden/missing pages; no arbitrary page ranges in v1. |
| `save` | `varos_pdf::write_pdf_checked` / `save_vrs`; app `Save`, `SaveAs`, `FileJobs` and durable storage | New explicit-path/result host route; reuse checked encoding and safe-write/durability policy, do not downgrade to a raw write. Capture revision, conflict-check disk identity/content, report completion/durability. Not a document undo step. |

Cross-cutting new core work: typed edit outcomes; isolated atomic batch execution; checked ID allocation with a session high-water mark; explicit target resolution/selection rules; safe creation; pure query helpers where the adapter would otherwise duplicate geometry. New host/adapter work: protocol DTOs (separate wire structs), authorization, session handles, revisions/journal, CPU extraction, local IPC, lifecycle completion and MCP/CLI bindings. Raw `EditCommand`, `AppCommand`, and `Document` serialization are **not** the protocol.

Coordinates are finite document points (1/72 inch), x right and y down, unaffected by zoom or ruler origin. Bounds are `[x,y,width,height]`; color is sRGB `#RRGGBBAA` on the wire, decoded to core RGBA. Core's current outline bounds are exposed as `bounds`; do not imply exact raster/ink bounds. Detail distinguishes world bounds and local size for rotated units. Hidden/locked targets and unsupported features return errors, never silent partial edits. Host adapters do not change Illustrator shortcuts, boxes, panel layout or tokens.

### 2. Token-cheap describe, identity and changes

**Summary first.** `describe` defaults to one board header: handle, revision, name, units, counts, world bounds, selection, dirty state and available detail sections. It does not dump paths, anchor arrays, the file, hidden metadata or a screenshot. `detail` accepts IDs, a subtree, artboards or named fields. Default page is 20 objects; maximum 100 and 16 KiB text per page. Opaque cursors bind the query and revision; an edit expires the cursor. Large geometry is explicitly requested and paginated. Truncation is marked with `more`/cursor, never silently omitted.

**Stable references, with honest limits.** Board handles such as `b7` identify a host session, never tab position or file name; the full session epoch accompanies the initial connection and every handle is scoped to it. `p42` means path ID 42 and `n8` node ID 8, so namespaces cannot collide. These underlying IDs survive normal edits and save/reopen; reopening creates a new board handle. Names and stacking indices are never identities. Boolean replacement returns new IDs and tombstones for removed ones.

Before offering stable IDs, fix the core allocator: maintain a checked lifetime high-water mark outside undo snapshots, reserve above it after undo/redo, and fail on exhaustion. Undo may restore the **same** removed entity, but new branches must never reuse its ID for different art. Merely prefixing today's counter would be unsafe. No persisted schema bump is needed for this runtime guard; reopening resets the session scope and seeds allocation from validated document IDs.

Artboards currently have no persistent IDs. v1 provides revision-bound references such as `a0@12`, explicitly valid only at that revision; they are not advertised as stable IDs. Resolve to core indices only after revision validation. No artboard create/delete/reorder verbs in v1; human changes still invalidate references. Persistent artboard IDs require separate core/schema work and an ADR-0008 format bump, not a fictional guarantee in this API.

**Diffs instead of repeat dumps.** Every successful edit returns `from`, `rev`, created/removed IDs, changed fields for touched objects and the resulting selection. A host journal records human edits and undo/redo too. `describe(since=R)` returns net changes or `resync_required` when R is outside the bounded journal; never an incomplete diff labelled complete. Initial limit: 128 revisions or 8 MiB per board, whichever comes first. Bounded change pages are pinned to the resulting revision. Large edits return counts and a detail cursor. Revisions increase for document changes including undo/redo, never roll back; selection-only changes carry a separate `selection_rev`. New host change observation is needed for document preferences that currently bypass `Editor.rev`.

The compact text below is the canonical readable projection of the same structured result (examples omit the connection epoch for readability). MCP returns structured content and this short text; CLI defaults to this text with `--json` for the same structured result. Clients should consume one representation, not echo both into model context. Descriptive strings are JSON-quoted/escaped and always treated as data. Estimates below are **rough response-text budgets**, not tokenizer measurements: approximately 3–4 characters per English token, with punctuation/IDs causing variation. Tool schemas, requests, duplicate serialization and images cost extra; Arabic names may tokenize differently.

**Example 1 — discover a board without reading its paths**

Request: `describe {"board":"b7"}`

```text
board b7 rev=12 name="Logo" units=pt dirty=true
counts paths=3 groups=0 layers=1 artboards=1
bounds=[20,20,260,100] selection=[] selection_rev=0
artboards=[a0@12:"Front"]
detail=objects,artboards,paint,geometry,metadata
```

Roughly 70–100 tokens. A board with thousands of paths keeps the same summary size; artboards are counted with only a bounded preview.

**Example 2 — ask only for two objects' bounds and paint**

Request: `describe {"board":"b7","rev":12,"ids":["p42","p55"],"fields":["bounds","paint","parent"]}`

```text
board b7 rev=12
p42 path parent=n8 bounds=[20,20,80,100] fill=#0C8CE9FF stroke=none
p55 path parent=n8 bounds=[140,20,80,100] fill=#141313FF stroke=none
more=false
```

Roughly 60–90 tokens. Actual geometry remains unknown until requested; do not infer that a four-point path is still an editable rectangle primitive.

**Example 3 — two edits, one undo, only changed fields returned**

Request:

```json
{"api":"1.0","request_id":"r9","board":"b7","expected_rev":12,"ops":[
  {"verb":"move","ids":["p42","p55"],"delta":[10,0]},
  {"verb":"set_paint","ids":["p42","p55"],"fill":"#FF6600FF"}
]}
```

Response:

```text
ok r9 board=b7 from=12 rev=13 undo_steps=1
changed p42 bounds=[30,20,80,100] fill=#FF6600FF
changed p55 bounds=[150,20,80,100] fill=#FF6600FF
created=[] removed=[] selection=[] selection_rev=0
```

Roughly 70–100 response tokens (request approximately 90–130). Both objects moved and changed fill; the human selection stayed empty. `describe {"board":"b7","since":12}` returns the same net object changes, without pretending to repeat the edit receipt. A later single `undo` restores both positions and fills, returns a new revision and reverse diff.

### 3. One edit request = one atomic undo step

The rule applies to a **document-edit request**, whether it contains one verb or a batch. Reads, selection-only requests, open/new/save/export/snapshot have zero document undo steps. Undo/redo move exactly one existing step and cannot be combined with edits. Reject mixed file/history/edit batches; filesystem effects cannot be rolled back by document undo. An unchanged valid batch returns `changed=false`, zero undo steps and the same document revision.

Proposed core API: `execute_checked(command) -> Result<EditOutcome, EditError>` and `execute_batch_checked(batch) -> Result<BatchOutcome, EditError>`. These names are design targets, not existing symbols. Repair the operations underneath: preconditions must return typed errors, and success must distinguish changed from no-op. Update desktop call sites to handle outcomes; retain a compatibility wrapper only during migration, never on the Bridge path. Metadata's `try_set_board_*` and Pathfinder's existing reason are starting points, not a complete error model. A wrapper that returns `Ok(())` after today's `execute` is unacceptable.

Transaction algorithm:

1. At the execution boundary, validate authorization, board/session epoch, request schema/size, revision, target existence, capabilities and any confirmation grant. Reject stale requests; do not rebase them silently. Read-only inspection can specify a revision too.
2. Stage against an isolated **core-owned working state**, including selection, active layer, defaults and allocator state. Validate all commands and execute them sequentially there, so later commands can reference results from earlier commands using request-local names such as `$shape1`. Resolve those names to actual IDs in the response. No staged state is visible to the live editor or published journal.
3. Existing per-operation begin/commit must be refactored into transaction-aware internals. Do not push intermediate history entries or mutate live `rev`. Validate semantic preconditions per operation and the resulting document using `format::check_structure` and `format::validate`; exercise save encoding limits on the staged result as well. Existing normalization is not permission to silently repair invalid authored input.
4. If anything fails, discard the stage. Live document, selection, active layer, paint defaults, ID allocation, undo/redo, dirty state and revision remain unchanged. This includes failure after an earlier successful staged command. No compensating series of Undo calls.
5. Commit once on the owning thread after rechecking the revision if work was staged elsewhere: push the original document once (retain current 200-step history cap), install the validated final state, clear redo once, increment document revision once, publish one diff, schedule redraw/recovery/dirty tracking through normal host paths. Preserve the human's selection, active layer and drawing defaults unless `select` was requested separately; prune references deleted by the edit. Undo follows existing core selection/navigation semantics, not a new promise to restore arbitrary transient UI state.

Starting request bounds: 1 MiB JSON, 100 operations, 1,000 explicit targets total, one board per edit. File-format caps still apply independently. Bound generated geometry too. Advertise actual limits in capabilities; lower them if measurement requires. Validation must reject wrong units/enums, unknown fields, non-finite numbers, missing or locked IDs, invalid dimensions, mixed unsupported targets and impossible operations **with reasons** rather than filtering or clamping. CPU work needs cancellation checkpoints and budgets; exact time/memory caps must be measured before release, particularly booleans and staging large documents.

Example failure, zero-based operation index:

```json
{"ok":false,"request_id":"r10","board":"b7","rev":13,"error":{
  "code":"locked_target","op_index":1,"ids":["p55"],
  "reason":"Path p55 is locked by layer n8. No edits were applied.",
  "retryable":false},"undo_steps":0}
```

`revision_conflict` includes expected/actual revisions and asks for a fresh describe; `invalid_argument` identifies the field; `unsupported`, `not_found`, `limit_exceeded`, `confirmation_required`, `busy`, `cancelled`, `io_error` and `save_conflict` are distinct. Core errors stay typed; the adapter maps them to stable external codes plus readable reasons. JSON-RPC parse/protocol errors are separate from tool failures. MCP tool failures use `isError` with the same error payload; CLI uses nonzero exit status and the same payload.

A request ID is a monotonically numbered idempotency key scoped to the authorized client/session (the examples use `r9`, `r10`): same ID + same payload returns the original receipt, different payload is rejected. Keep a bounded receipt cache plus the client high-water sequence; reject IDs older than its retained window rather than rerunning them. A new connection uses a new client identity unless it resumes the existing authorized session. A disconnect is not proof of rollback; the client queries the receipt/revision. No crash-durable exactly-once claim in v1: after host restart the epoch changes and the client must reopen/inspect saved state. Cancellation before commit changes nothing; after commit it reports the committed receipt, never a false rollback.

### 4. Two hosts, one set of semantics

**Attached mode (first).** The desktop owns the real `Editor`, history and save state. A local IPC reader validates framing, queues a typed request with an explicit `SessionId` and reply channel, and wakes the existing host. It never mutates the editor from its worker thread. Add an internal Bridge `AppCommand` route into the existing FIFO `ActionQueue`; document edits delegate to the checked core API through a shared settling boundary derived from `run_doc`. This is a new door adapter, not an existing MCP listener and not synthesized keys/menu clicks.

The host must not use “whatever tab is active when drained.” Bind the authorized session at request creation and recheck it on execution; a closed/replaced session returns `not_found`. For v1, edits of an inactive/Home board return `board_not_active`; the human activates it. Reads may inspect authorized inactive sessions. This avoids hidden edits and implicit tab switching. Open may focus a board under existing lifecycle rules.

Settle a pending valid field first as its own **human** undo step, then recheck the agent's revision (normally returning conflict if that field changed the document). An invalid field returns `busy`/`field_invalid`; do not hold the FIFO forever or dismiss the field. Bridge history requests never enter field-text undo; focused editing must finish first. Pointer gestures and picker previews must settle or return `busy`; because pointer/panel actions bypass FIFO today, queue order alone is insufficient. Once the stage commits on the UI thread, no human event may interleave inside it. An off-thread stage requires final revision comparison. No shortcut or K3 behavior is changed.

Reuse `OpenPaths`, lifecycle duplicate-file checks, workspace identity and background `FileJobs`. Add explicit-path save/export variants or equivalent typed payloads: current `SaveAs`/`ExportPdf` ask dialogs and do not accept an agent destination. A request receives `accepted` plus a ticket while I/O runs, then a final completion receipt tied to the captured revision; accepted never means saved. Preserve migration notices, save-conflict/durability outcomes and dirty state if the document changed after capture. Reads/snapshots observe a settled committed revision, not partially dragged art.

**Headless mode (second).** A long-lived local host owns an `Editor` per opened board, invokes the same checked core requests, loads/saves through the existing format/PDF pipeline and keeps history for that host session. It constructs no `Renderer`, `EventLoop`, window or egui context. Extract `thumbs/raster.rs` into a shared CPU-only facility outside core; both the desktop thumbnail service and Bridge use it. Do not import the app binary, copy the renderer, or add `tiny-skia`/transport dependencies to core. Native filesystem safety helpers also need a shared non-UI home rather than pulling in app dialogs. Crate placement is implementation work requiring the normal dependency review; this ADR does not authorize a workspace rewrite.

Snapshot v1 is the existing CPU board-preview appearance, fitted to visible scene bounds, not an exact screen grab or a promise of print proofing. It returns image dimensions, source revision and `renderer=cpu`; PNG bytes are an MCP image only on explicit request, or written to an authorized output path by CLI. Reuse the existing visual tokens where needed; no parallel token definitions. Crop/page modes and GPU/CPU parity claims wait for measured fixtures.

A one-shot CLI process must not pretend it retains undo across invocations. Provide `varos bridge host --headless` as an explicit user-started session service and have CLI calls connect to its local endpoint; MCP may own an in-process headless host for the life of its stdio process. No hidden auto-start daemon. Stopping a dirty host refuses until save or explicit discard authorization; unexpected termination may lose unsaved headless work in v1, which the host must state. Attached mode edits never get auto-saved by an agent disconnect.

Headless must not write a file open for editing in desktop or another Bridge host. Introduce a cooperative canonical-file writer lease shared by both hosts, plus an external-change fingerprint checked before safe replacement. If ownership cannot be established, open read-only or save a new path. Leases alone do not protect against unrelated editors; fingerprints alone do not close races. Filesystem replacement/identity must be checked at the write boundary, and limitations documented. Never “attach” by independently loading and overwriting the live file.

### 5. MCP + CLI from one implementation

Use a transport-neutral Rust service with explicit request/result DTOs, schema validation, authorization, target resolution and dispatch. The MCP binding and CLI binding only decode/encode and call it. Attached/headless hosts implement the same host port. Thin shell aliases may exist, but no second editing implementation or subprocess-per-verb Rust code path.

MCP uses local stdio JSON-RPC 2.0, UTF-8 newline-delimited messages. Stdout contains protocol only; diagnostics go to stderr. Implement initialization/version negotiation, `tools/list`, `tools/call` and cancellation; advertise only supported capabilities. No HTTP/SSE/TCP server in v1. This follows the [MCP stdio transport specification](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports). Pin and test a supported MCP protocol revision separately from Bridge API versioning.

Keep discovery small: expose `capabilities`, `open`, `new_board`, `list_boards`, `describe`, `select`, `edit`, `history`, `snapshot`, `export_pdf`, `save`, and `request_status`; the edit verbs are the discriminated operations of `edit`, and `history` chooses undo/redo. `capabilities` can return detailed schema for one verb on demand, but required MCP input schemas remain complete enough to validate requests. Large documentation is fetched on demand, not appended to every reply. `request_status` handles receipts and I/O tickets in either transport.

Illustrative future commands (not executable today):

```text
varos bridge mcp --attach <endpoint>
varos bridge mcp --headless --root <approved-directory>
varos bridge host --headless --root <approved-directory>
varos bridge describe --session <endpoint> --board b7 --json
varos bridge edit --session <endpoint> --request-file changes.json
```

For Mac-first attached/local-host IPC, choose Unix-domain sockets in an owner-only runtime directory, with a random per-launch capability and peer-user checks. These are local IPC, not network listeners. CLI and MCP proxies use the same framed request DTOs and limits. Reject stale endpoints, never fall back to TCP. Windows remains compile-only; unsupported native attachment returns a capability error until a separately reviewed local transport exists. The app FIFO is an in-memory ordering queue, **not** a filesystem named pipe transport.

### 6. Later in-app chat: the user's CLI remains the agent

External Claude Code/Codex control is the first proof. Later, a normal Varos box may launch the user's installed, unmodified CLI with `std::process::Command`, an argument array and piped streams (never shell interpolation). The CLI is the LLM client and MCP client; it launches/connects to Varos Bridge for document tools. Varos itself performs no provider inference request. Any other agent can use the same MCP tools or CLI without a provider-specific core path.

Proposed adapters use documented programmatic entry points: Claude Code print mode with streamed JSON; Codex `exec --json`, which reuses saved CLI authentication, with the Varos stdio MCP server configured for that session. These are distinct streams: agent progress/chat events to Varos, MCP requests from the agent to Bridge. Use documented resume/session IDs for continuation, bounded output, cancellation and timeouts. Feature-detect installed versions and report missing CLI/login/permissions/limits plainly. Do not scrape a terminal, read OAuth token files or automate sign-in. Users complete the vendor's own login flow.

Documented technical support is **not blanket permission to ship a subscription-backed wrapper**. Official references checked for this draft on 2026-10-06:

- [Codex non-interactive mode](https://developers.openai.com/codex/noninteractive), [MCP configuration](https://developers.openai.com/codex/mcp), and [authentication](https://developers.openai.com/codex/auth) document programmatic output, local MCP servers, saved login reuse and subscription/API-key authentication. This supports the proposed mechanism, not a guarantee of eligibility, unlimited usage or approval of Varos's future packaging.
- [Claude Code programmatic use](https://code.claude.com/docs/en/headless) and [MCP](https://code.claude.com/docs/en/mcp) document the technical mechanism. Its [legal and compliance page](https://code.claude.com/docs/en/legal-and-compliance) conditions running Claude Code in products on the applicable commercial terms, an unmodified binary and each user's own credentials; it prohibits intermediating Claude.ai credentials and distinguishes end-user login to Claude Code from third-party subscription routing. Recheck the exact distribution and orchestration design before shipping; seek vendor clarification if it falls between those cases. Do not claim that spawning a binary automatically settles the terms question.

No API key is required **when the installed CLI and the user's eligible account permit this mode**. API-key mode remains the user's optional vendor choice; Varos must neither require it silently nor disable vendor authentication alternatives. Subscriptions, usage caps, organizational restrictions, model availability and policies can change. “Zero project inference bill” means no Varos-operated inference service, not free or unlimited inference for the user. Rate-limit/login refusal stops the turn; never rotate accounts, extract credentials or bypass limits. If an integration cannot comply, keep external MCP control and disable that chat adapter.

The future launcher must allow only the user's chosen executable, restrict unrelated shell/file tools using supported CLI permissions, and expose approvals without auto-answering them. The Bridge cannot sandbox a separately launched agent's other tools. Agent prompts/describe results/images may reach the provider through the CLI even though **Bridge has no network**; the later design must make that data boundary clear.

No chat box, approval panel, progress UI, tokens or panel layout is built under this decision. Each visible addition stops at mockup images and an owner-reviewed plan before code; it must use the existing box system.

### 7. Versioning and relationship to earlier ADRs

Bridge starts with `api="1.0"`. Connection capabilities report supported Bridge major/minor, negotiated MCP revision, app build, readable/writable `.vrs` versions, host mode, enabled verbs and limits. Client requests a supported major/minor; otherwise refuse before executing. Handshake IPC uses the same compatibility checks, so an older CLI cannot accidentally issue newer requests to a desktop.

A major changes field meaning, units, defaults, required fields, ID semantics, removal, or error semantics. A minor adds optional fields/verbs/capabilities without changing existing behavior. Execute only fields known at the negotiated version; reject unknown request fields and variants. Clients tolerate additive response fields. Keep frozen request/response/describe/error fixtures per supported version and parity tests across both transports and hosts. Never equate MCP's date-based protocol revision, Bridge API `1.x`, app release number and persisted `FORMAT_VERSION=3`.

On owner acceptance, this ADR supersedes **only** ADR-0004's deferral of the AI command/query contract and external compatibility guarantees for this Bridge surface. Serde core models remain the persistence source of truth. This is an explicit adapter schema, not a universal introspectable property registry for inspectors/plugins/files. ADR-0008's file versioning, bounded validation and migration rules remain; persistent artboard IDs or other model additions must follow them. ADR-0003's PDF container, ADR-0001's native GPU desktop and ADR-0002's edit/app separation remain. ADR-0002 already requires explicit external adapters; this supplies that later decision rather than exposing its enums. The AI-schema deferral in ADR-0008 is narrowed accordingly; unrelated plugin/property-registry work stays deferred.

While status is proposed, **no accepted ADR is superseded** and no authority headers are rewritten. On acceptance, update ADR-0004's “Superseded by” cross-reference with this precise scope, reconcile the parked/next scheduling notes, and commission the first work order. Those are not part of this docs-only change.

### 8. Security and destructive operations

**Superseded for local trust by [ADR-0011 Amendment 3](ADR-0011-bridge-connection-and-trust.md#amendment-3--open-local-trust-owner-2026-10-08), owner 2026-10-08:** same-uid agents receive all scopes without pairing, confirmation or file-root grants. Undo + audit + future per-AI review history replace the gates described historically below. Uid checks, revision checks, mistake-guards and refusal to overwrite existing save_as/export destinations remain.

- Default deny. User grants read/write roots or individual files and authorized live board sessions at launch/attach. Separate read, edit and export/write capabilities. `list_boards` cannot leak other sessions, paths or Recent entries. A tool request cannot widen grants or enable its own write access.
- All open/save/export/snapshot paths are local and checked at use time. Canonicalize existing inputs and output parent directories, reject escape via `..`, symlinks, aliases or replaced parents; use directory-relative/identity-checked operations rather than trusting a string-prefix check. Handle hard-link aliases in writer ownership checks. Reject URL schemes, device paths and network-mounted locations in v1; if locality cannot be established, refuse. Do not enumerate the home directory or read credentials. Apply bounded file/PDF/model parsing before installing a document.
- Bridge has no outbound network, remote listener, telemetry, package install, generic shell tool or provider SDK. Its only IPC is stdio/local sockets. The later agent launcher belongs to the app layer; its network activity is a separate, user-selected vendor process (§6).
- Require trusted human confirmation for delete, Pathfinder replacement, ungroup (structure loss), overwrite/export onto an existing destination, and any explicit discard. Also confirm undo/redo because the shared top history entry may belong to the human. Ordinary reversible move/paint/group edits need only the existing edit grant. Saving to the current backing file may use a user-issued save grant, scoped to that file and an unchanged external fingerprint; an unrelated overwrite needs a new confirmation.
- Confirmation is a two-phase exchange: return `confirmation_required` with exact affected IDs/counts or destination, expected revision/file fingerprint and a payload digest. A trusted host/terminal owner path issues a short-lived, single-use grant bound to those values. Revalidate on use; changes expire the grant. `confirm:true` from the model is not authorization. Unattended callers without a pre-established exact grant get refusal. No blanket `--yes` bypass and no confirmation UI is implicitly authorized here; its eventual visible design requires mockups. The headless host can use its owner terminal approval channel, separate from agent stdio.
- Treat names, tags, imported metadata and agent text as untrusted data, never instructions; quote/escape control text. Bound requests, output, geometry, image allocations, pending work and logs. Keep audit receipts with verb/board/revision/result, not full documents or credentials. Revoke attachment on host shutdown; local same-user authorization is not protection against a compromised account.

## Consequences

Benefits: one edit implementation, ordinary human undo, small inspectable results, no paid Varos inference backend, and a headless path that preserves the pure-core boundary. Costs: this is more than wrapping an enum; error propagation, transactions, identity, authorization, lifecycle receipts and CPU extraction are release prerequisites.

### Risks and rejected shortcuts

| Risk / alternative | Decision and evidence needed |
|---|---|
| A “batch” partly changes the board | Stage and validate before one commit. Test failure at every operation boundary, including ID allocation, selection/defaults, redo and dirty state. Reject outer begin/execute-loop/commit. |
| UI edits race with agent edits | Explicit session/revision, field/gesture settling, final compare and single owning-thread commit. FIFO alone does not cover pointer edits today. Test field invalidity, held save, tab change/close, drag and concurrent human edit with pure host fakes. |
| IDs point to different art after undo | High-water allocation and namespace/session scope; test undo then new branch and boolean replacement. Artboard indices remain revision-bound until a format decision. |
| CPU preview differs from GPU/PDF | Reuse/extract existing raster; fixture tests for clip, holes, transforms, opacity and free-canvas bounds. Label preview; no pixel parity claim. Render tests stay CPU-only. |
| Cloning/validation freezes large boards | Bound requests and generated geometry; benchmark near format caps and booleans, track staging memory. Move staging off-thread only with safe revision compare. No unmeasured latency promise. |
| Concurrent writers or save completion lies | Cooperative ownership + disk fingerprint + safe replace; test stale worker completion, external modifications, symlink swaps, refusal and durability warnings. Accepted job is not durable save. |
| MCP/schema overhead consumes the token savings | Measure complete exchanges with actual supported client tokenizers, including discovery/duplicate content. Keep the three examples as budget fixtures; estimates are not measured savings. |
| CLI login/terms/limits block chat | External MCP remains useful without an embedded chat adapter. Revalidate vendor docs and permissions before shipping; no provider credential proxy. |
| “One universal schema” or direct Serde exposure | Rejected: internal model/command changes must not silently change public compatibility, and persistence is not a permission boundary. |
| UI scripting, remote HTTP, separate CLI logic | Rejected in v1: pixel/input automation is brittle, remote service expands security scope, duplicate implementations drift. |

### Delivery and acceptance boundaries

After owner acceptance, commission one narrow attached slice: MCP `list_boards`, `describe`, `select`, `edit` (move and solid paint only), and `history`, with the checked batch/error/revision/security foundations. An illustrative future exercise is: Ahmed opens a board, an external CLI describes two objects and requests one move+paint batch, then Ahmed sees it and presses ⌘Z once. No claim of success without the real owner-window check, separate from headless tests. Add shape/structural operations and safe file effects in later gated slices, then headless and CPU snapshots. The vocabulary table defines the v1 target; a partial slice advertises only what it implements.

Required implementation evidence: identical result/error fixtures through MCP and CLI; attached/headless semantic parity for supported verbs; rollback and one-step undo/redo; stale revisions and retry receipts; stable IDs; bounded describe/diff resync; scope/confirmation/path escape refusals; save/export round-trip and pure-PDF checks; CPU raster fixtures. No test constructs a GPU Renderer or EventLoop. Run workspace tests, native all-target clippy, fmt and Windows-target clippy; desktop acceptance remains a separate owner activity. Independent review precedes any merge under standing rules.

**Not in v1:** in-app chat UI; web/WASM/cloud drives; remote MCP/HTTP hosting; shared-network boards or CRDT co-editing; account/credential brokerage; autonomous background agents; general plugin execution or property registry; persistent artboard identities and artboard mutation verbs; arbitrary path/anchor editing; editable text/Arabic shaping; image generation; arbitrary reparenting; group/gap distribution; gradients; SVG import/export or production PNG export (small snapshot PNG is not that feature); cross-board batches; crash-durable headless history/recovery; full GPU/CPU equivalence; window/panel control; Windows runtime support. None of these is implemented or promised by this document.

## Amendment — 2026-10-07: persistent artboard ids (slice 3)

§2's deferral is resolved by the ADR-0008 format-4 amendment: every artboard carries a stable id, so
the Bridge exposes `artboard:N` (persistent across edits and save/reopen, like `path:N`/`node:N`
underlying ids) and the page verbs `add_artboard`, `resize_artboard`, `rename_artboard`,
`delete_artboard` (destructive confirmation, §8) and `set_active_artboard` inside `edit` — same atomic
batch, one undo step, typed errors, request-local names. The revision-bound `a0@12` reference remains
a **deprecated alias for one slice** (align target, describe `ref`). Active-artboard rule: page verbs
never change the human's active page as a side effect; `set_active_artboard` is an explicit navigation
change (no undo step on its own); deleting the active page while others remain is refused until the
batch sets another active page first; deleting the last page leaves a free canvas. `snapshot` accepts
`artboard` to render one page at its own aspect ratio and background. Reorder/duplicate/page paint
remain deferred. Details: `varos/crates/varos-bridge/README.md`.

## Status

accepted — owner 2026-10-06. Supersedes ADR-0004's deferral of the AI command/query API in the scope of §7 only.

## Amendment — 2026-10-07: slice 4 remaining creation, page and file operations

The commissioned slice adds `add_path` (2–1,000 explicit anchors with absolute point/handle
coordinates), rectangle `radius` (0 through half the shorter dimension), `reorder_artboard`
(zero-based position), `duplicate_artboard` (`with_art` required, optional relative offset),
`set_artboard_color` and `set_artboard_clip`. A rounded rectangle is an eight-anchor cubic path,
using the model's circle approximation constant; it has no live radius property. Duplication
inserts after the source and preserves active navigation. Artwork uses the desktop's overlap
membership/copy helper, including whole clipping groups, inherited transforms and layer membership.
Fresh page/path/node/anchor identities use the session high-water allocator. All document verbs
use the same isolated stage and one published undo step.

`save`, `save_as` and `export_pdf` are standalone tools, never edit operations. They require the
new C1 `files` scope, denied by default (including previously approved profiles). Owner approval
can include `--scopes read,edit,files`. `save` only targets the current `.vrs` backing file, with
the source fingerprint checked on the worker and immediately before atomic replacement.
`save_as`/`export_pdf` require an absolute explicit destination in a root granted by
`VAROS_BRIDGE_FILE_ROOTS=<dir:dir>` at desktop launch. This environment setting is temporary owner
policy, pending a reviewed UI. Roots are captured/canonicalized at launch. Existing destinations
are refused; this slice provides no exact overwrite-confirmation ceremony. All open backing files
and hard-link aliases are refused as explicit destinations. Parent canonicalization rejects escapes;
the Mac writer pins and rechecks directory identity, traverses without following symlinks, refuses
non-local volumes, and publishes new files without replacement. Windows file hosting remains
unsupported; Windows remains compilation-only. The safe encoding/durability writer is reused.

File requests return `accepted:true` and a ticket. `request_status` reports pending/completed and
the completion receipt, including durability or a typed refusal. Worker policy errors therefore
appear on completion, not in the acceptance receipt. Worker unavailability refuses before acceptance;
there is no UI-thread fallback. The original snapshot becomes a successful save's checkpoint, so
later edits stay dirty. File completions never open dialogs or settle a human field/gesture.
The worker rechecks the agent's current `files` grant before work and publication. Export scopes
are `all_visible_artboards`, `artboard:N` (visible page only), and `artwork_bounds` (free canvas only).
Existing v1 fixtures retain their bytes; the new logo request/result is frozen separately.

### Slice 4 independent review amendment (2026-10-07)

save_as writes a copy; the board stays on its current file (owner may widen later). No checkpoint, dirty-state or Recent change accompanies the copy. File fingerprint mismatches and destination-exists races use `save_conflict`; other IO uses `io_error`. Completion outcomes are audited with verb, board, ticket and result code, without paths.

Known widening for the owner until C2: the `files` grant lets an agent save the backing file of any board it can read (not per-file).

## Amendment — token economy (accepted 2026-10-08)

المقترح يخلي الوكيل يرسم نفس التصميم بكلام أقل: يكتب اللون والمكان المشترك مرة واحدة، ويسمّي العناصر المهمة بس، ويختصر الأشكال المتكررة. جرّبنا إعادة كتابة أوامر الموقع نفسه فنزل حجمها من ٢٨٬٠٩٢ إلى ٧٬٨٥٥ بايت، من غير تغيير الرسم؛ ده قياس لحجم الأوامر، مش إثبات لنفس النسبة في التوكنات أو تكلفة الصور. نبدأ بالاختصارات البسيطة والردود الصغيرة، وبعدها التكرار؛ النص الحقيقي يفضل هو الحل بدل شرائط النص المؤقتة. أحمد فوّض القرار للموديراتور يوم 2026-10-08؛ شرائح 1–2 هي نطاق التنفيذ المقبول.

**Amendment — token economy: accepted 2026-10-08 (owner delegated)** — «مش هعرف أقرر، قرر انت».
Slices 1–2 are commissioned in `feat/bridge-token-economy` from main `9db9a85`; slice 3 remains deferred.
The landing SHA is pending: this work order prohibits commit/push. No landed SHA is claimed.
Encoding experiment baseline: `243f9fd`, branch `docs/bridge-token-economy`.
This preserves PLAN A3’s measured-savings gate. Reproducer, assumptions and implementation gates: [work order](../foundation/work_orders/BRIDGE_TOKEN_ECONOMY.md).

### Evidence and measured encoding

The supplied Claude Code + Opus wireframe has 201 operations in batches of 72/91/38:
186 shapes, 13 paths, one page creation and one explicit page activation; page bounds 1440×3000.
Keys repeat: verb 201, name 200, fill/parent 199 each, bounds 187, kind 186, radius 138, stroke/stroke_width 25 each, anchors 13. The generator was inspected, not executed or changed. Names are already optional in `dto.rs`/`design.rs`; omitting them is usable today, not a new verb.
All numbers below are measured compact UTF-8 JSON bytes, not measured model tokens.

| Cumulative encoding | Bytes | Saved from preceding row | Reduction from baseline |
|---|---:|---:|---:|
| Original three ops arrays | 28,092 | — | — |
| Batch defaults (parent + modal fill) | 23,677 | 4,415 | 15.7% |
| Omit 199 shape/path names; keep page name and local | 18,627 | 5,050 | 33.7% |
| Compact arrays, retaining exceptional options | 9,910 | 8,717 | 64.7% |
| Exact translated repeats | 7,960 | 1,950 | 71.7% |
| Bars inside repeated footer columns | 7,855 | 105 | 72.0% |
| Inline group / future add_text: no equivalent substitution | 7,855 | 0 | 72.0% |
| Clone alternative to preceding encoding (not adopted) | 7,998 | −143 | 71.5% |

New rows include `{"defaults":…,"ops":…}` overhead; original files are bare ops arrays.
Unchanged board/request/revision envelopes are excluded from this microbenchmark and included in the end-to-end gate.
The best form retains 201 expanded operations and their ordering/paint/geometry; 135 top-level operations.
Only autogenerated object names differ. These numbers originated as an offline experiment; slices 1–2 now implement the API 1.1 forms, with deterministic document-equivalence gates in contracts.rs. Provider token/image accounting and owner live review remain separate.

### Request contract

`edit.defaults` accepts only `parent, fill, stroke, stroke_width, radius, opacity`.
Defaults apply to omitted applicable fields on creation leaves, including structural children, never existing-target edits.
Explicit values override; explicit null clears fill/stroke, never means “inherit”; numeric/parent null is invalid.
Radius applies only to rects (an explicitly supplied ellipse radius still errors); defaults are batch-local, never editor paint state.
Missing both default and leaf value retains today's behavior. Effective paint, bounds and parent must pass today's validation.
Optional `name` continues to work; API 1.1 omitted names use deterministic host labels such as “Rect 12”,
“Ellipse 13”, “Path 14”, scoped to allocated identity, cached on retry and restored on redo; names are never IDs.

Choose JSON arrays, freely mixed with objects: `["rect",[x,y,w,h],"#RRGGBBAA",r]`,
`["ellipse",[x,y,w,h],"#RRGGBBAA"]`, `["path",[[x,y],…],closed,"#RRGGBBAA"]`.
After required fields, fill is optional string/null; rect radius may follow a supplied fill; an optional final object holds remaining options.
To inherit fill while setting radius, use `["rect",[x,y,w,h],{"radius":8}]`; no null holes.
Duplicate positional/options fields, extra slots, unknown keys, wrong types and nonfinite/invalid numbers are errors.
Path points abbreviate plain anchors only; use object add_path for handles/smooth. Holes remain unsupported, never discarded.
Only these three creation forms get tuples; other verbs remain objects with identical validation and receipts. A line DSL might remove punctuation but requires another escaping/parser contract and opaque-string MCP validation.
Arrays reuse JSON and permit bounded tuple schemas; actual tokenizer advantage and lower error rate remain unproven.
Measure schema tokens and repair calls too; retain object fallback if a client cannot represent tuple schemas.

### Structural operations, without a layout language

`repeat {ops,count,dx,dy}`: count includes the first instance, integer 1–100; instance i translates all geometry/handles by i×(dx,dy).
Expand instance-major in source order; no layout inference. Nested repeats give grids; nesting ≤4.
Locals become `$name_0`, `$name_1`, etc. (nested suffixes concatenate); internal references remap within each instance.
Outside references must use a suffixed local; collisions/overlong locals/forward references fail. Existing IDs are not implicitly copied.
Repeat children are creation operations, bars, inline groups or repeats, not arbitrary mutations/history/pages.
Measured repeat reduction: nav 4→1, separators 2→1, five logos 10→1, checks 12→1,
pricing feature rows 24→3, footer columns 20→1, social circles 3→1: 66 top-level operations removed.
Whole feature cards differ in icon/colour; highlighted pricing differs in paint. Neither is falsely counted as an identical repeat.

`group {children:[…],local?,name?}` is exclusive with today's `ids` form:
create ≥2 children in the same layer, then group them in source order, using world coordinates and one atomic stage.
Children use the creation-only grammar above; no implicit extra parent, reparenting or transforms.
It removes local-binding/ID-list boilerplate when grouping was already intended. This sample contains no groups: measured fair saving is zero; inserting groups would change its editable tree.

`clone {ids,dx,dy,count}`: count is additional copies (1–100), copy i at (i+1)×delta; resolve sources once in staged order.
Accept complete editable units from one parent, including earlier batch locals; reject ancestor/descendant target overlap, partial groups, layers, pages and inert descendants.
Preserve groups/paint/geometry and internal order; append copies at parent top, allocate fresh node/path/anchor IDs.
Return created IDs in instance-major order. Copying Plan 1's 13 paths replaces Plan 3's 13 creates with one clone,
but 13 source bindings prevent other repeat savings: measured 7,998 bytes, 143 worse. Do not select it for this fixture.

`bars {rows,x,y,w,h,gap,fill?,widths?}` is explicitly a placeholder-text stopgap, never text.
Expand rows (1–100) into rects at y+i×(h+gap), radius h/2; widths defaults to w or has exactly rows positive entries ≥h.
Require h>0, w≥h, nonnegative gap, finite geometry; inherit applicable batch paint/parent; no shaping or line wrapping.
Footer links: four rects→one bars per column (16 rendered rects remain); only 105 extra bytes saved after repeat.
Future `add_text` belongs to ADR-0010: real content, fonts, shaping and editable text replace placeholders.
This sample supplies no text strings; equivalent add_text savings are unmeasured and counted as zero, not invented.

### Receipts, reads and bounds

Keep full receipts by default; API 1.1 `receipt:"ids"` returns only created/changed/removed object/page IDs, locals and revision,
inside the existing ok/request_id/board/rev/undo_steps envelope; errors retain reasons and operation location.
Stable ID ordering is documented; expansion failures also identify the nested op/instance. Large ID results use marked revision-pinned pagination.
No per-leaf paint/bounds/name echo. The journal remains complete; compact/object syntax yields the same selected receipt.
Current receipts are one diff per batch, not 201 independent receipts: `service.rs` projects created objects and paginates above about 12 KiB.
The work-order proxy estimates 39,644 bytes of created-object detail before pagination; first two batches cross that threshold.
Actual baseline is estimated at 9–11 KiB structured plus 6–9 KiB text; IDs-mode proxy is 2,959 structured bytes, about 5–6 KiB with text.
These are code-based estimates with assumed IDs/state; real receipts, PNGs and provider token usage were not supplied.
MCP currently sends both compact text and structuredContent; count both on wire, and measure which the client exposes to the model.

For additive compatibility, introduce opt-in API 1.1 `snapshot profile:"economy"`: omitted dimensions use ≤512×512
(page fit) or 512×232 (board preview); explicit dimensions can reach the existing 1024 cap. Existing requests keep their defaults.
Today defaults are page 1024×1024, board 544×246. Assuming three page-fit shots, 492×1024→246×512 each:
1,511,424→377,856 pixels total; raw RGBA 6,045,696→1,511,424 bytes. PNG bytes and image tokens are not pixel counts.
Transport base64 uses 4×ceil(PNG bytes/3); cap remains 783,360 encoded-source PNG bytes per image before framing.
API 1.1 `describe summary_budget:1024` bounds summary result UTF-8 bytes, with ≤1024 text bytes too;
retain revision/counts, bounded name/selection/page previews and explicit more/cursor; details retain 20/100 objects and 16 KiB limits.
The three failed describes and planning tokens are absent; do not credit the separately fixed fields-composition bug here.

Capabilities' single economy hint: “Use API 1.1 defaults and omit decorative names; compact rect/ellipse/path plus repeat for identical rows, bars only for placeholders; request IDs receipts, a budgeted summary or since-revision diff, and an economy snapshot only when needed; inspect specific IDs/fields for detail and ask for larger images explicitly.”

### Versioning, gate and slices

API 1.0, omitted api, all current defaults and frozen fixtures stay unchanged. New fields/forms require explicit `api:"1.1"`;
negotiate support and reject unavailable features before execution. New economy read profiles are opt-in, avoiding a minor-version default change.
MCP advertises complete version-discriminated object/tuple/structural schemas (bounded recursive alternatives), never `ops:any`;
CLI consumes the same decoder/service and returns the same results. No persisted model version change.
Retain 1 MiB requests, 100 top-level AND 100 expanded primitive operations, 1,000 expanded targets, 1,000 anchors/path;
count generated groups, clones' copied leaves and bars against expansion/work budgets before allocating; format limits still apply.
One batch remains one undo step, all-or-nothing rollback, revision check, cancellation and idempotent cached receipt.
Hash the original versioned request including defaults/receipt mode; same ID with a different spelling remains a conflict.
Add fixtures only. Gate: wireframe ops payload ≤9,364 bytes (one third), with exact expansion equivalence except omitted names.
Measure wireframe, frozen poster and frozen story before/after: request/receipt/schema/read text bytes and actual tokenizer counts,
PNG/base64 bytes and provider image usage, retries and planning separately; no aggregate token claim until the same-task runs exist.
Implement in savings/effort order: (1) defaults/names/compact plus terse reads/receipts; (2) repeat; (3) optional bars/group/clone.
Slice 1 reaches 9,910 ops bytes; slice 2 reaches 7,960 and passes the byte gate; slice 3 is not required for that gate.
All slices require parity/rollback/undo/idempotency/limit gates and same-task visual/editability checks; owner acceptance stays separate.

## Amendment — 1.2 progressive disclosure

Status: proposed — 2026-10-09; moderator acceptance pending.

API 1.2 `tools/list` publishes full core tool schemas (capabilities, describe,
snapshot, select, edit, history, request_status, list_boards and export tools).
Core edit operations, creation tuples, defaults and bounded repeat remain inline;
extended edits appear as a compact verb enum with one-line descriptions. Typed
execution and every existing tool/verb spelling remain unchanged. Read-only
`list_verbs {"api":"1.2"}` groups names and descriptions; `schema
{"api":"1.2","tool":"edit","verb":"stroke_style"}` resolves the full standalone
params schema (`stroke_style` discovers the existing `set_stroke_style` verb).
Other tool schemas can be requested by tool name. Discovery requires 1.2 and no
board, revision or mutation ID. The 24,000-byte cap applies to each supported
API's list; 1.0/1.1 projections and fixtures remain byte-identical. This implements
a bounded discovery projection in the direction of proposed ADR-0015, without a
free-form command dispatcher or accepting that ADR's broader registry proposal.
