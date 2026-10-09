> **Status:** proposed — Phase 9.2 design; no implementation or acceptance implied.
# ADR-0015: One command identity across Varos surfaces

- Date: 2026-10-09; baseline: `main` / `feat/spec-registry` at `7b48f2c`.
- Authority: [PLAN](../PLAN.md) 9.2–9.5; preserves [ADR-0009](ADR-0009-varos-bridge.md) typed verbs and the latest [ADR-0011](ADR-0011-bridge-connection-and-trust.md) amendments.
- Supersedes: none. Registry/schema versioning is independent of `.vrs`; no document-format bump.

## Context and evidence

Paths below abbreviate `varos/crates/varos-app/src/` as `a/`, `varos-core/src/` as `c/`, and `varos-bridge/src/` as `b/`.

| Observed boundary | Evidence at baseline |
|---|---|
| App lifecycle and deterministic edits are separate internal vocabularies | `a/app_command.rs:1-12`; `c/command.rs:1-16`, `Editor::try_execute` / `execute` |
| Menu IDs exist, but many edits dispatch synthetic keys; File rows already dispatch commands | `a/menus/mod.rs:96-128`; `a/menus/{file,edit,object,view,window}.rs` |
| Native state is partly document-wide and partly File-specific | `a/mac_menu.rs:151-199`; `a/menus/mod.rs:65-93` |
| Keyboard and Windows burger duplicate labels/routing | `a/main.rs:270-351`; `a/ui/bar.rs:547-578` |
| Wire DTOs, schemas and capability lists are separate tables | `b/dto.rs:12-29`; `b/service.rs:384-397`; `b/mcp.rs:28-45,212-227` |
| Offline CLI has another verb table and core batch adapter | `varos/crates/varos-cli/src/main.rs:15-16` and its `apply` branch |

[GAP_2 §E](../reference/gap/GAP_2_TOOLS_PAINT_LAYERS_APP.md#e-application) identifies the registry/shortcut gap. [Study A](../reference/study/APPEARANCE_STUDY_A_vectorcraft.md) supplies typed-parameter and loss cautions; it has no command-registry section at this baseline.
IDEA prior art: `~/Documents/AI workspace/reference/artcraft/vectorcraft/crates/engine/src/cmd/mod.rs:94-139,177-186` (`CommandSpec`, resolved info, `cmd!`); `mcp/src/tools.rs:77-100` (`list_commands` / `run_command`). Borrow the index and reason model, not arbitrary JSON execution or their UI framework. No source copied here.

## Decision: identity indexes typed behavior

Introduce a UI-independent catalog of stable, namespaced ASCII `CommandId`s. One behavior has one canonical ID across menus, keys, Bridge and CLI; an ID is not a label, key chord, enum discriminant or translated menu path. Different fixed parameters are presets of one behavior, not duplicate handlers. Materially different effects require different IDs.
The catalog is an index over typed handlers, **not** a new universal `run_command(id, Value)` wire endpoint. Never deserialize external data directly into `AppCommand` or `EditCommand`.

| Proposed descriptor field | Contract |
|---|---|
| `id`, `schema_version`, aliases | Permanent identity; aliases only for identical semantics; retired IDs never reused |
| `label_key`, `description_key`, category | Localizable metadata; names never used for dispatch |
| `args_schema`, `result_schema`, typed binding | Reference the typed DTO/operation definition; unknown/malformed arguments refuse |
| `scope`, `effects`, `undo_policy` | App/document/view/selection/field/native; document mutation, I/O, query or UI; one-step/none/history |
| surface support and API range | Desktop, attached Bridge, offline CLI; unsupported is explicit, not inferred from enum existence |
| availability/check-state binding | Pure predicate over resolved context and arguments; checked state independent of enabled state |
| default shortcuts and focus context | Platform-specific parity data; no display-only accelerator that invokes another behavior |
| placements/presets | Ordered menu/burger paths and fixed typed arguments; separators and native roles are presentation data |
| recordability/replay policy | Explicit opt-in; result-ID bindings; reject nonportable/UI-only effects |

Keep pure command identity/metadata and edit predicates below app/transport dependencies (a small shared catalog module/crate, placement settled by dependency review). Core owns edit eligibility and execution. App owns lifecycle, field, view and native handlers. Bridge owns versioned DTO validation and service adapters; CLI selects the proper host adapter. No core dependency on app, AppKit, egui, GPU, parser or transport.

| Example canonical ID | Desktop lowering | External typed binding / limit |
|---|---|---|
| `object.move` | Nudge resolves selection and point delta → `EditCommand::Nudge` | `edit` / `move`, explicit IDs and delta; keyboard increment only affects desktop argument construction |
| `object.group` | `GroupSelection` | `edit` / `group`; same checked domain operation |
| `object.order` | Arrange presets front/forward/backward/back | `edit` / `order` with typed mode |
| `history.undo` | Document Undo after focus resolution | `history` undo; shared history, revision check |
| `file.save` | Save current file; pathless desktop prompts | `save` requires backing file; no external dialog fallback |
| `file.save_copy` | `AppCommand::SaveCopy` | Existing Bridge `save_as` writes a copy; preserve wire name |
| `file.save_as` | `AppCommand::SaveAs`, changes backing association | No equivalent attached Bridge verb today; must not alias copy |
| `file.export_pdf` | Sheet collects scope/destination before execution | `export_pdf`, revision-pinned ticket/completion |
| `ui.export_options` | `ShowExport` / `ShowExportSelection` presets | UI-only; distinct from the export that writes bytes |

## Availability, focus and dispatch

Resolve `enabled: bool` and `disabled_reason: {code, message_key, args}?` from a read-only context: host/API support, editor/document and selection revision, explicit targets, history, hidden/locked state, active gesture/transaction, field focus, session/lifecycle and file-job state. Enabled implies no reason; disabled requires one. Examples: `no_document`, `no_selection`, `locked_target`, `nothing_to_undo`, `busy`, `needs_arguments`, `unsupported_host`.
Discovery without required arguments returns `needs_arguments`, not a guess based on the human selection. A supplied typed invocation is checked against its own targets; unselected valid Bridge targets can be eligible while the desktop selection action is disabled. State responses identify epoch/board/revision and selection revision where relevant; they are advisory snapshots, never grants.
Use the same domain predicates at discovery and execution; recheck on the owning thread immediately before mutation/publication. Core checked validation remains authoritative; no `Ok` inferred merely because `execute()` returned. Stale revision, busy state and invalid targets return existing typed errors without partial edits.
Physical keys resolve field versus canvas context first. Text Cut/Copy/Paste/Select All/Undo stay native field operations with distinct field IDs and history; object deletion must not run while typing. Lifecycle Save remains reachable through field focus using the established settle boundary. Invalid fields refuse settlement; active human gestures are not cancelled to serve agent work.
Each accepted invocation enters the existing host FIFO/lifecycle or core batch boundary once. Native accelerators must not also execute a second keyboard event. Preserve Bridge explicit targets/revisions, atomic batch, receipts, file tickets and error semantics.
ADR-0011 Amendment 3 governs: local same-uid trust is open; registry discovery adds no pairing, scope approval or confirmation ceremony. Existing uid, bounds, file mistake guards and overwrite refusals remain. Availability never bypasses them.

## Menu, shortcut and automation consumers

`menus/*.rs` become declarative placements of IDs/presets, without separate labels, shortcuts or enablement logic. `mac_menu.rs` translates the resulting tree into native items and native roles, synchronizes changed states, and sends IDs plus arguments. Windows burger renders the same catalog subset and availability; it may group rows differently without redefining behavior.
Recent files and panels are dynamic instances of an ID plus typed path/panel arguments, not IDs derived from filenames or translated panel titles. Keep native Services/Hide/window roles native; do not advertise them as headless capabilities.
Target menu map: **File, Edit, Object, Type, Select, Effect, View, Window, Help**, plus the macOS application menu. Move Select rows when that menu lands; Type/Effect families appear with delivered features, not enabled placeholders. Menu movement and translation do not rename IDs. Preferences uses the macOS application menu and the appropriate Windows Edit placement.
9.3 shortcut editor (⌥⇧⌘K) reads bindable IDs, localized labels, defaults, effective bindings and focus contexts from the catalog. Store versioned overrides by ID separately from general settings; support explicit unbind and reset-to-parity defaults. Validate platform-normalized chords, OS-reserved keys and overlaps within simultaneously active contexts; report both actions and require an explicit resolution before saving. Field and canvas contexts may reuse keys without stealing text input. Menu accelerators and shortcut hints update from effective bindings together. UI awaits owner Figma choice.
Bridge `capabilities` stays compact. Propose a negotiated read-only command-index query (filter/category/ID, default 20/max 100 entries, bounded pages, detailed schema on demand), with typed availability requests for target-sensitive checks. Add only under an explicitly opted-in API revision; current 1.0/1.1 fixtures and current per-tool 1.2 export report behavior stay frozen. This ADR does not claim 1.2 already supports registry queries or all edits.
Each supported entry points to an existing typed tool/verb/schema; unsupported desktop-only entries say why. MCP tools remain typed; no `run_command` escape hatch. Generate common metadata/projections and verify schema-to-handler coverage, without replacing hand-checked DTO validation with free-form strings.
9.5 Actions records **successful committed semantic invocations**, not pointer events, field previews or arbitrary enum serialization. Capture explicit typed arguments, units in points and local symbolic references for created objects; bind source targets at replay, never reuse session IDs blindly. Record no-op/cancelled/failed operations as no action step.
Replay compiles supported document steps into existing typed Bridge/core atomic batches; CLI `apply` keeps its current format until an explicit versioned Actions adapter is added. Fail preflight for unknown versions/IDs, unavailable handlers or unbound targets; never silently skip a step. Multi-document/file actions cannot claim global atomicity. Queries, UI, OS clipboard, lifecycle I/O and history are initially nonrecordable; document-only batch = one undo step. Actions is not the per-AI history review model.

## Incremental migration and acceptance

1. Inventory menu IDs, all key paths, burger rows, `AppCommand`, `EditCommand`, Bridge DTOs/capabilities and CLI verbs; assign canonical IDs and typed mappings. Classify internal events (`FileDone`, picker previews, Bridge envelope) as plumbing, not user commands.
2. Add catalog and pure availability adapters alongside existing dispatch; parity tests cover every existing route and current quirks. Preserve current menu IDs as aliases where semantics match; publish explicit migrations for renamed IDs.
3. Move File/lifecycle first, then core edits, view/tools and panel rows. Keep `AppCommand` for app ownership and `EditCommand` for core ownership: the registry unifies identity, not their enum responsibilities. Remove synthetic-key edit routes only after field-focus tests pass.
4. Derive native/burger/keyboard metadata; remove duplicate tables once coverage proves no orphan/duplicate dispatch. Add Bridge/CLI projections under negotiated versions; unsupported handlers stay unadvertised as executable.
5. Build 9.3 and 9.5 on the stable catalog after their design/implementation gates; no persisted raw Rust enum discriminants or changed `.vrs` bytes.

Required tests: unique IDs/valid aliases; every placement and shortcut resolves; every advertised typed verb has a handler; version/host filtering; menu–key–burger identical outcomes and exactly-once dispatch; field Undo/Delete/Save; no-document/locked/hidden/busy/stale states; explicit-target Bridge parity; atomic failure and no-op history; file copy versus Save As; shortcut conflicts/migration; Actions created-ID remapping and rollback; frozen wire fixtures and dependency directions. Cache invalidation covers selection, document, focus, jobs and settings; no idle polling/redraw introduced.
Implementation still requires repository gates, independent review and native owner checks. Tradeoff: descriptor/adaptor work is substantial, but preserves typed contracts and makes drift testable without turning all internal commands into a public protocol.
