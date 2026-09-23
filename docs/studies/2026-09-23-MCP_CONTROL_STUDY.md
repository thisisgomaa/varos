> **Status:** reference — study/proposal for Ahmed's decision (charter §3, level 5); not an accepted decision.
# Study: Letting AI agents drive Varos through MCP

- **Date:** 2026-09-23
- **Author:** Claude (study session). **Decision owner:** Ahmed (product owner).
- **Authority:** none. This is a level-5 document under `docs/foundation/FOUNDATION_CHARTER.md` §3 (lines 29-39). It proposes; it decides nothing. Anything here that conflicts with an ADR loses, and the conflicts are named in §6.1.
- **Baseline read:** working tree at `61786a0` (clean). All `file:line` citations below were checked against that tree on the date above.

---

## 1. الملخص (بالمصري، من غير كلام تقني)

1. **الفكرة:** نخلّي أي مساعد ذكي (زي Claude) يقدر "يمسك" ملف Varos: يقرالك إيه اللي في الملف، يرسم أشكال، يلوّن، يحرّك، يجمّع، يعمل دمج أشكال، ويطلّع PDF — زي ما Figma بيسمح دلوقتي.
2. **ليه ده رخيص عندنا:** من شهرين خلّصنا شغلانة إن **كل تعديل على التصميم بيعدّي من باب واحد بس** في قلب البرنامج (اسمه `EditCommand`). يعني المساعد مش محتاج باب جديد — هيخبّط على نفس الباب اللي الماوس والكيبورد بيخبّطوا عليه، ونفس الـ Undo بيشتغل.
3. **الناقص:** مفيش لسه أمر "ارسم شكل بأرقام" (الرسم دلوقتي بالماوس بس)، ومفيش تصدير PNG، ومفيش "Undo واحد لطلب المساعد كله"، ومفيش نصوص عربي في البرنامج أصلاً لسه.
4. **الطريق اللي بنرشّحه:** نبدأ بنسخة **بتشتغل على الملف من برّه البرنامج** (أأمن وأسهل تتجرّب)، تقرا الأول بس، وبعدين تعدّل على نسخة من الملف وانت تفتحها وتشوف. آخر مرحلة: المساعد يعدّل **قدامك وانت فاتح البرنامج** — بس ده بعد ما نخلّص شغل الأساس (F4.2).
5. **الأمان:** كله على جهازك بس، مفيش إنترنت، ومقفول لحد ما انت تفتحه بإيدك.
6. **القرار المطلوب منك (قرار واحد):** توافق إننا نكتب **قرار معماري جديد (ADR)** يسمح بجزء جديد في المشروع مخصوص للمساعدين، ويقول صراحةً إن "اللغة" اللي المساعد بيكلّم بيها البرنامج **تجريبية وممكن تتغيّر** — وتحدد إمتى نبدأ (اقتراحنا: بعد ما شغل الأساس الحالي يخلص).

---

## 2. Reference: how Figma's MCP works (high level)

Evidence class: general knowledge plus one observation. Items marked **(uncertain)** are from memory and must be re-verified against Figma's current docs before anyone cites them as fact.

**Observed (2026-09-23):** the session that wrote this study had a Figma MCP server attached. Its tool catalogue listed read tools `get_metadata`, `get_design_context`, `get_screenshot`, `get_variable_defs`, and write tools `use_figma`, `create_new_file`, `upload_assets`, `generate_diagram`. The server's own description says `use_figma` executes JavaScript "in the Figma file context", which means the Plugin API. That catalogue is data the server supplied. It is not Figma documentation.

How it works in outline:

- **Two deployments (uncertain on details).** Figma first shipped a *local* MCP server that runs inside the Figma desktop app and listens on a loopback HTTP port (reportedly `127.0.0.1:3845`). A *remote* server followed, hosted by Figma and reached with OAuth. The local one is "Figma is open and the agent talks to the live app". The remote one is "the agent talks to Figma's cloud copy of the file".
- **Addressing by node id.** Every layer has a stable id (such as `1:2`, which appears in URLs as `node-id=1-2`). Read tools take an id or fall back to the user's current selection in the desktop app. `get_metadata` returns a sparse outline of ids, names, types, positions, and sizes, so the agent can pick targets cheaply before it asks for heavy context.
- **Selection is shared context.** "Implement what I have selected" works because the server can see the live selection. This is the feature designers experience most directly.
- **Screenshots close the loop.** `get_screenshot` lets the agent check what it did. Without it the agent works blind.
- **Writes ride on an existing public API.** `use_figma` runs Plugin-API code. Figma did not have to design a new mutation protocol for MCP, because it already had a stable, versioned, documented plugin API that it maintains for third parties. **This is the decisive difference from Varos.** Varos has no such public API and has explicitly refused to promise one (ADR-0002 line 24, ADR-0004 lines 19-21).
- **Undo (uncertain).** In our understanding, plugin edits land in the user's normal undo history as ordinary edits. We have not verified how Figma groups a multi-step agent run into undo steps.

**Takeaway for Varos:** copy the *shape*: node ids, a cheap tree read, selection as context, a screenshot for self-checking, and a live mode where the user watches. Do **not** copy the *mechanism* of executing arbitrary script against a public API. That mechanism would force exactly the schema promise that ADR-0004 forbids.

---

## 3. Architecture options

### 3.1 What already exists that makes this cheap

- **One edit door.** `EditCommand` is a closed enum in core (`varos/crates/varos-core/src/command.rs:12-117`). `Editor::execute` is the single entry point (`command.rs:191-193`). F4.1 measured zero direct document writes from the UI (`docs/foundation/F4_DESIGN.md:159`), and keyboard edits use the same door (`varos/crates/varos-app/src/main.rs:153-220`). An agent adapter that calls `execute` gets the same semantics and history as a human click.
- **Headless by construction.** ADR-0002 requires that `EditCommand` "can execute without UI, window, renderer, or file-dialog dependencies" (`docs/adr/ADR-0002-two-level-command-boundary.md:19`). It also states that "Headless callers gain one deterministic edit path without importing the app crate" (line 29). Existing core tests already drive it with no GPU: `varos/crates/varos-core/tests/edit_command.rs:9-45` builds an `Editor`, sets a selection, executes, and undoes.
- **Undo is automatic.** Each semantic method brackets itself with `begin()`/`commit()` (`varos/crates/varos-core/src/editor.rs:2870-2888`). `commit` bumps `rev` (line 2883), and `rev != saved_rev` is how the app knows the file is unsaved (`editor.rs:364-366`, `varos-app/src/main.rs:391-392`). Agent edits would show up as unsaved changes with no extra work.
- **Ids exist and persist.** Every path, anchor, and node carries a `u32` id drawn from the document counter `Document.ids` (`varos/crates/varos-core/src/model.rs:516`, `nid()` at `model.rs:588-591`). The counter is serialized with the file, so ids survive save and load. The tree is `nodes` plus `roots` (`model.rs:510-512`), with `NodeKind::{Layer, Group, Path(u32)}` (`model.rs:241-246`).
- **A file format that is already a PDF.** Saving writes a valid PDF with the model embedded (`varos/crates/varos-pdf/src/lib.rs:1-16`; `save_vrs` at `lib.rs:30`, `write_pdf` at `lib.rs:72`, `load_vrs` at `lib.rs:36`). "Export PDF" for an agent is therefore nearly free.
- **A precedent for talking to the running app.** The single-instance module already forwards file paths from a second process into the running window through `WM_COPYDATA` (`varos/crates/varos-app/src/single_instance.rs:88`, `:128-137`, `:221`). It queues them in a mutex (`:71`) and drains them between frames on `AboutToWait` (`varos-app/src/main.rs:658-661`). An agent bridge would follow the same "queue, then drain when idle" pattern.

### 3.2 A correction to the brief: "core only" is not enough to read Ahmed's files

The app saves and opens through `varos_pdf` (`main.rs:420`, `main.rs:884`), so real `.vrs` files are PDF containers. `varos_core::file::load_vrs` reads only the legacy raw-JSON form (`varos-core/src/file.rs:56-62`). A headless tool built on `varos-core` alone could not open the files Ahmed actually has. It needs `varos-pdf` too. That is still GPU-free and still allowed, because `varos-pdf` depends only on core (`docs/adr/ADR-0005-crate-dependency-directions.md:23`).

### 3.3 The three options

| | (a) Embedded in the running app | (b) Headless `varos-mcp` binary | (c) Both, sharing one adapter crate |
|---|---|---|---|
| **What the agent touches** | The live `Editor` Ahmed has open. He watches edits appear. | A `.vrs` file on disk. Ahmed opens the result. | Whichever is available. The same tools and code run in both. |
| **Transport** | The app process must expose a local endpoint (loopback HTTP or a Windows named pipe), or a small stdio shim must relay to it. | MCP stdio: the agent launches the binary. Nothing listens on any port. | stdio front end. It relays to the app when the app is running and live mode is on, and otherwise edits the file directly. |
| **Core purity (CLAUDE.md hard law 1; ADR-0005 line 26)** | Safe if the tool logic lives outside `varos-app`. At risk if it is written inside `ui.rs`/`main.rs`. | Naturally safe. The crate depends on core + pdf only. | Safe. The adapter crate is pure. Only a thin transport lives in the app. |
| **"No test constructs a Renderer or EventLoop" (hard law 2)** | Transport tests would need the event loop, so they must stay thin and untested-by-CI, or be mocked. | All tool tests are plain `Editor` tests, like `tests/edit_command.rs`. | Same as (b) for 95% of the code. |
| **Concurrency with Ahmed's hands** | Real problem (§6.3). The agent can collide with a live drag, an open picker, or the current tool. | None while the app does not have the same file open. Last writer wins if it does (§6.3). | Both sets of problems, but each is solved once. |
| **Undo** | The agent's steps enter Ahmed's live undo stack. That is good (Ctrl+Z works) and risky (a 200-step cap, `editor.rs:2879-2881`). | Undo lives inside the tool session only. The file on disk has no history. | Both. |
| **Needs first** | F4.2 `AppCommand` (Open/Save/Export are still inline in `main.rs`; `STATUS.md` "F4.2 queued"; `F4_DESIGN.md:143`), a wake-up path into a `ControlFlow::Wait` loop (`main.rs:656`), and an opt-in UI toggle. | Only a new crate (needs an ADR, §6.1) and the missing core commands in §4. | Both lists, in stages. |
| **What Ahmed feels** | The Figma experience: "I see it happen." | "Give me a minute, then open the file." | Starts as (b), becomes (a). |
| **Platform** | Windows-first pipe/HWND plumbing. The single-instance code already has non-Windows stubs (`single_instance.rs:272-282`). | Pure Rust, cross-platform. It runs on this Mac today. | Both. |

**Which one the hard laws favour.** The laws favour **(b) as the base and (c) as the destination.**

- The laws say core stays pure and no test may touch the GPU or event loop. All real logic belongs in a crate that depends only on `varos-core` (+ `varos-pdf`), so it can be tested exactly like `tests/edit_command.rs`: translating agent requests into edits, building the tree view, reporting results.
- Option (b) *is* that crate plus a stdio loop. Option (a) is that crate plus a transport inside the app. Starting with (a) invites tool logic into `main.rs`/`ui.rs`, the files the foundation program is trying to shrink (`FOUNDATION_CHARTER.md:71`, F5).

**Why `EditCommand` is the natural mapping.**

1. It is already the only legal way to change a `Document` from outside core (ADR-0002 line 22).
2. Each variant already owns its history policy (`F4_DESIGN.md:80`), so the adapter adds no undo logic of its own.
3. It is deliberately **not** `Serialize`/`Deserialize` (`command.rs:11-12`, no derive). That protects us: the adapter must define its own small JSON arguments and translate them. ADR-0002 asks for exactly this, "Future plugin or AI work must use explicit adapters instead of treating either enum as a stable public protocol" (line 32). The enum can keep changing freely, and only the adapter's thin JSON layer faces agents.

**Recommendation:** option (c), built in the order (b) then (a). One new crate, provisionally `varos-agent` (depends on `varos-core` + `varos-pdf`), holds all tool logic and the agent-facing JSON. It ships first as a stdio binary. Later the app gains a thin, opt-in, local-only bridge into the same crate.

---

## 4. First tool set (15 tools)

Legend: ✅ exists and is callable today · 🟡 exists but needs adapter work or has a sharp edge · ❌ **does not exist yet**.

Two findings apply to every write tool:

- **`execute` returns nothing** (`command.rs:191`, `-> ()`), and many methods silently do nothing on bad input. Examples: grouping fewer than 2 objects (`editor.rs:1540-1543`) and booleans on fewer than 2 closed paths (`editor.rs:1019-1030`). The adapter must compare `rev` before and after (`editor.rs:366`) to report "changed" or "nothing happened". It must diff node ids to report newly created ids. ❌ No result or error value exists in core. The adapter can infer one. A proper result is a later core decision.
- **Almost every command acts on the current selection**, not on ids passed in. The adapter therefore sets the selection first (tool 4) and then executes. Tools take node ids, like Figma, and translate them to selection internally.

| # | MCP tool | What it does | Maps to (file:line) | State |
|---|---|---|---|---|
| 1 | `get_document` | Artboards, units, counts, active artboard, unsaved flag | `Editor.doc` is public (`editor.rs:323`); `artboards`/`active` (`model.rs:526`, `:530`); `units` (`model.rs:520`); `rev` (`editor.rs:366`) | 🟡 data exists; the agent-facing summary shape is new (§6.1) |
| 2 | `get_tree` | Layers, groups, and paths with ids, names, hidden/locked flags, bounding boxes (Figma's `get_metadata`) | `nodes`/`roots` (`model.rs:510-512`); `Node` (`model.rs:284-330`); `node_paths` (`model.rs:1687`); `outline_bbox` (`model.rs:868`) | 🟡 same as #1 |
| 3 | `get_selection` | Selected ids plus the selection box | `objsel` (`editor.rs:328`), anchor `selected` (`editor.rs:327`), `obj_bbox` (`editor.rs:549`) | ✅ |
| 4 | `set_selection` | Select nodes by id | `Editor::layer_select_set` (`editor.rs:4231`), a declared transient interface, not an `EditCommand` (`F4_DESIGN.md:102`); path id to node id via `node_of_path` (`model.rs:1011`) | 🟡 side effects: switches the tool to Object (`editor.rs:4232`) and changes the active layer (`editor.rs:4244-4246`) |
| 5 | `create_shape` | Rect / ellipse / triangle / polygon from numbers | Parts exist: `Document::build_shape` (`model.rs:924`), `Path::new` (`model.rs:198`), `nid` (`model.rs:588`); `commit()` adopts new paths into the active layer via `sync_tree` (`editor.rs:2875`). Today creation happens only through a pointer gesture (`tools/shapes.rs:8-18`). | ❌ needs a new `EditCommand::CreateShape`. The polygon is fixed at 6 sides (`model.rs:943`). |
| 6 | `create_path` | Free path from anchor list (points + handles), open/closed | `Anchor` (`model.rs:14-20`), `Path::new` (`model.rs:198`) | ❌ needs a new `EditCommand::CreatePath`, which must mint anchor ids from `doc.ids` |
| 7 | `set_style` | Fill, stroke colour, stroke width, opacity on given ids | `ApplyPaint` (`command.rs:25-28` to `:129-132` to `editor.rs:3905`), `SetStrokeWidth` (`command.rs:23`, `:216-230`), `SetOpacity` (`command.rs:22` to `editor.rs:4072`) | 🟡 `apply_paint` also overwrites Ahmed's *current* swatch (`editor.rs:3906-3909`) and the paint target (`command.rs:130`). With an empty selection, `SetStrokeWidth` changes the default width instead (`command.rs:218-221`). The adapter must never call it with an empty selection. |
| 8 | `transform` | Set x/y/w/h, rotation, flip | `SetObjectBounds` (`command.rs:13-20` to `editor.rs:1394`), `SetObjectRotation` (absolute degrees; `command.rs:21` to `editor.rs:1517`), `Flip` (`command.rs:69` to `editor.rs:1364`) | 🟡 there is no "move objects by dx,dy": `Nudge` moves *anchor* selection only and returns early when that is empty (`editor.rs:3815-3818`). The adapter computes an absolute x/y from `obj_bbox` instead, or core gains a `MoveSelection` ❌. |
| 9 | `group` / `ungroup` | Group or ungroup ids | `GroupSelection`/`UngroupSelection` (`command.rs:51-52` to `editor.rs:1540`, `:1574`) | ✅ (grouping needs 2 or more, `editor.rs:1541`) |
| 10 | `boolean` | Unite / minus-front / intersect / exclude | `Boolean(BoolOp)` (`command.rs:75` to `editor.rs:1019`); `BoolOp` (`boolean.rs:20-25`) | ✅ needs 2 or more closed paths with 3 or more anchors (`editor.rs:1020-1030`); the result gets new ids, so the agent must re-read the tree |
| 11 | `arrange` | Align, distribute, z-order | `Align`, `Distribute`, `Arrange` (`command.rs:70-76`) | ✅ |
| 12 | `delete` | Delete ids | `DeleteSelected` (`command.rs:78` to `editor.rs:3846`) | 🟡 **tool-sensitive:** if the active tool is Artboard, it deletes the *artboard* (`editor.rs:3847-3849`). The adapter must use `DeleteLayerSelection` (`command.rs:53`) or force the Object tool. |
| 13 | `set_node_props` | Rename, hide, lock | `RenameNode`, `ToggleNodeHidden`, `ToggleNodeLocked` (`command.rs:45-50`) | 🟡 toggles are not idempotent. An agent needs "set hidden = true", so the adapter reads the current state and then toggles. |
| 14 | `undo` / `redo` | Step history | `Undo`/`Redo` (`command.rs:115-116`, `:183-184`) | 🟡 one agent request that runs N commands creates N undo steps (§6.2) |
| 15 | `export` | PDF (all visible artboards); PNG | PDF: `varos_pdf::write_pdf` / `save_vrs` (`varos-pdf/src/lib.rs:72`, `:30`). PNG: nothing. The only rasterizer is the GPU renderer; PNG code in the app is cursor/icon tooling (`varos-app/src/main.rs:272-354`). | PDF ✅ · **PNG ❌** (PNG export is a parked product item, `FOUNDATION_CHARTER.md:116`) |

Also needed but outside the 15:

- **`open` / `save`.** Headless mode uses `varos_pdf::load_vrs` + `Editor::replace_doc` (`editor.rs:2933-2949`). Embedded mode would use `AppCommand` ❌, which is F4.2 and not built yet (`F4_DESIGN.md:143`).
- **`get_screenshot`**, the agent's eyes. ❌ It needs a rasterizer (§5, stage 6). The render-agnostic `build_scene` (`varos-core/src/scene.rs:197`) is the natural input for one.
- **Artboard tools.** `AddArtboard`, `SetArtboardRect` (`command.rs:84-103`) exist ✅. They are left out of the first set to keep it small.

---

## 5. Staged plan (gated pieces Ahmed can test by hand)

Every stage is its own branch and gate, per charter §4 (lines 41-49). Every stage keeps `cargo test --workspace`, `clippy -D warnings`, `fmt --check`, the golden round-trip, and `tools/check_dep_directions.ps1` green. Each stage also has one plain-language hand test.

| Stage | What is built | "Done" looks like (Ahmed's hand test) |
|---|---|---|
| **0: Decide** | Ahmed's decision (§1 item 6) plus a drafted **ADR-0008 "Agent adapter (experimental)"** (content in §6.1). No code. | ADR accepted or rejected. STATUS records when the track may start. |
| **1: Read-only, headless** | `varos-agent` crate + stdio binary. Tools 1-3 on a `.vrs` path. | Ahmed points Claude Desktop/Claude Code at a copy of one of his files and asks "what's in this file?" The answer matches his Layers panel, and Arabic layer names come back intact. The file's bytes are unchanged afterwards. |
| **2: Edits on a copy** | Tools 4, 7-14. Results always saved to a **new** file, never over the original. | Five scripted prompts ("make the red square blue and 50% transparent", "group these", "unite the two circles", …). Ahmed opens each output in Varos, and it looks right and undoes correctly inside the tool session. |
| **3: Creation + PDF** | New core commands `CreateShape`, `CreatePath` (core change with its own headless tests and red-proof, like `F4_DESIGN.md:162-170`), plus tool 15 (PDF). | "Draw three circles in a row, unite them, export a PDF." The PDF opens in any viewer, and the `.vrs` opens in Varos with editable paths. |
| **4: One request = one undo** | A core "history group" so N commands from one agent call become one undo step (§6.2). | After a 10-step agent request, a single Ctrl+Z in Varos (after opening the result) reverts all of it. |
| **5: Live mode** | After F4.2. An opt-in toggle in the app (off by default), a local-only bridge, and a queue drained only when Ahmed is idle (§6.3). The stdio binary relays to the app when live mode is on. | Ahmed has a file open, asks the agent for a change, and **watches it appear**. Ctrl+Z undoes it. Turning the toggle off means no endpoint exists (verified by a tool, not assumed). |
| **6: Eyes** | `get_screenshot`: an offscreen GPU render in live mode, and in headless mode either nothing or a CPU rasterizer, which is a separate ADR because ADR-0001 line 19 treats a CPU renderer as a future option, not a promise. | The agent can check its own work: "does it look centred?" |

**Scheduling honesty.** This is a *feature*. The accepted charter says the current program is "not features" (`FOUNDATION_CHARTER.md:16`). The active work order is P11.2, with F4.2 queued next (`docs/foundation/STATUS.md`, "Now"). Stages 1-4 touch no app files and could run alongside, but only if Ahmed explicitly opens the track. Stage 5 must wait for F4.2.

---

## 6. Risks

### 6.1 Schema exposure versus ADR-0004 (the main risk)

ADR-0004 says the V1 persisted schema is the Serde JSON of the core model (lines 17-19). It says that "a discoverable plugin/AI schema, stable property identifiers, migrations, validation metadata, query contracts, and external compatibility guarantees are future work" requiring "a separately reviewed ADR" and "must not be inferred from the V1 Serde representation" (line 21). CLAUDE.md repeats this as a hard law. Any MCP server necessarily publishes tool names, argument shapes, and a tree view, and agents and users will start relying on them.

What this study therefore does **not** do: it does not propose exposing `doc_to_blob` output (`file.rs:22`) or the `EditCommand` enum to agents, and it promises no stability.

What a future **ADR-0008** would have to decide explicitly. Nothing here is decided by this study.

1. **A new crate.** ADR-0005 fixes the workspace at four crates, "a fifth crate or a new edge requires a superseding ADR" (line 34). The charter's target architecture also says "No new crates" (`FOUNDATION_CHARTER.md:80`). ADR-0008 must supersede ADR-0005's edge table with one added row: `varos-agent` may depend on `varos-core` and `varos-pdf`, and nothing may depend on it except `varos-app` (for live mode). `tools/check_dep_directions.ps1` must be updated in the same order.
2. **An *experimental* agent contract, carved out of ADR-0004.** The ADR should state that the tool names, arguments, and tree view are an **unversioned, experimental adapter surface**, hand-written in `varos-agent`, with no compatibility guarantee and free to break between builds. This does not change ADR-0004. It makes use of ADR-0004's "future work" clause while keeping the promise that the persisted schema and the agent surface are different things. **What would require amending or superseding ADR-0004:** any promise of stable property identifiers, a discoverable or introspectable schema (for example, generating tool schemas from the model), versioned compatibility for third-party agents, or validation metadata. ADR-0008 must say plainly that it makes none of these promises.
3. **Node ids as handles.** Ids are persisted `u32`s (`model.rs:516`), but they are not a compatibility promise. Booleans and duplication mint new ids, and hostile-file `ids` re-derivation is still parked (`FOUNDATION_CHARTER.md:113`). The ADR should say ids are valid "for this document, until the next structural edit, re-read after writes".
4. **Adapters, permissions, and query contracts**, which ADR-0002 defers (line 24). ADR-0008 answers them only for the local, single-user, experimental case.

### 6.2 Undo and history semantics for agent edits

- **Granularity.** Each command is one undo step (`editor.rs:2874-2888`). A 30-step agent request equals 30 Ctrl+Z presses.
- **Grouping is not possible today.** `begin()` unconditionally overwrites `pending` (`editor.rs:2870-2873`), and each inner method commits on its own. An outer bracket therefore cannot merge steps, and `execute` deliberately adds no wrapper (`F4_DESIGN.md:80`). Stage 4 needs a real core design (a history group with a depth counter, or snapshot-once-and-suppress-inner-commits) plus headless tests.
- **Cap.** History holds 200 snapshots (`editor.rs:2879-2881`). A chatty agent in live mode could push Ahmed's own history off the end. Mitigation: history groups (Stage 4) and a per-request command budget.
- **Replacing the document wipes history.** `replace_doc` clears undo and redo (`editor.rs:2933-2938`). If option (b) is ever wired to "reload in the running app", Ahmed's undo history is lost, and `confirm_discard_unsaved` (`main.rs:391-399`) would prompt over any unsaved work. This is one more reason live mode must edit the in-memory `Editor` and never reload.
- **Labelling.** Undo steps carry no labels today (`undo: Vec<Document>`, `editor.rs:369`), so Ahmed cannot see which steps were the agent's. Nice to have, not required.

### 6.3 Concurrency with the live UI (option (a) only)

- **Mid-gesture corruption.** During a drag, `pointer_down` has already called `begin()` (`editor.rs:3227`). An agent command arriving mid-drag would call `begin()` again and overwrite the gesture's undo snapshot. The picker session has the same problem (`PickerBegin` … `PickerCommit`, `command.rs:31-44`). **Rule:** the queue drains only when `drag` is `Drag::None` and no picker session is open. Mid-gesture undo is already a parked risk (`FOUNDATION_CHARTER.md:115`). Live mode must not make it worse.
- **Shared transient state.** Agent calls change Ahmed's selection (tool 4), current swatch (tool 7), active tool (`editor.rs:4232`), and active layer. Options: (i) Figma-style, where the agent visibly shares the selection; or (ii) save and restore selection, tool, and swatch around each agent request. Recommend (ii) as the default and (i) as a later choice for Ahmed.
- **Tool-sensitive commands.** `DeleteSelected` deletes an artboard when the Artboard tool is active (`editor.rs:3847-3849`). Every adapter call must pin the tool state it assumes.
- **Waking the loop.** The loop sleeps on `ControlFlow::Wait` (`main.rs:656`) and has no user-event channel (`EventLoop::new()`, `main.rs:504`). The bridge needs a wake-up: an event-loop proxy, or a window message like the existing `WM_COPYDATA` path.
- **Headless file races (option (b)).** Last writer wins. `write_atomic` uses a fixed temp name `*.vrs.tmp` (`file.rs:51`), so two writers saving the same file at the same moment can collide on that temp file. Mitigation: headless mode writes new files only (Stage 2 rule).

### 6.4 Security (local only)

- **stdio mode** opens no port. The agent host launches the binary as the user. This is the safest possible start.
- **Live mode:** off by default and visibly on when enabled. Use a Windows named pipe restricted to the current user, or loopback-only HTTP with a random per-session token. Never bind a non-loopback address. The spirit is the same as ADR-0007's "visible and user-controlled" rule, though that ADR is about updates, not this.
- **Hostile input.** Clamp and reject NaN, infinite, and absurd numbers in the adapter before they reach core. Opening agent-chosen `.vrs` files goes through the parked hostile-file work (recursion guards, decompress cap, `ids` re-derivation, `FOUNDATION_CHARTER.md:113`). That hardening should land before agents can open arbitrary paths. Until then, allow only a user-chosen folder.
- **File writes.** Export and save paths are limited to that same folder. No overwrite of the source file in Stages 1-3.
- **Prompt injection through documents.** Layer and artboard names are free text (`model.rs:289`, `command.rs:47-50`) and are returned to the agent. A shared file could carry instructions in a layer name. The tool descriptions should tell the agent that names are data, and the server must never act on them.

### 6.5 Arabic text

- **There is no text object in Varos yet.** `NodeKind` is only Layer, Group, or Path (`model.rs:241-246`), so no MCP tool can create or edit text today, Arabic or otherwise. A `create_text` tool must wait for the text engine.
- **What works now:** Arabic *names* for layers and artboards are plain UTF-8 strings (`Node.name`, `model.rs:289`; `RenameNode`, `command.rs:47-50`). JSON carries them unchanged. Stage 1's hand test includes an Arabic layer name round-trip.
- **Rule for later:** when text lands, agents send logical Unicode strings plus paragraph direction. Shaping, bidi, and joining stay inside Varos's engine. Agents must never send pre-shaped glyphs or visual-order strings. This keeps the "Arabic-first typography" moat inside the product, not in the agent.

---

## 7. What this means for the "100% online" idea

This study is **local-first**: stdio, and later a local pipe. A separate study covers online Varos. The two meet at one point. The `varos-agent` crate is designed to depend only on pure crates (`varos-core`, `varos-pdf`), so the same tool logic could in principle sit behind a remote MCP endpoint (MCP's HTTP transport) or run next to a hosted document. However, nothing in this study makes Varos online-ready.

- History is whole-document snapshots per step (`undo: Vec<Document>`, `editor.rs:369-371`). That model suits one user and does not suit multi-user merging.
- There is no authentication, account, or conflict model.
- Whether `varos-core`/`varos-pdf` compile to WebAssembly has **not been tested**.
- The embedded live-mode endpoint must never be exposed beyond loopback, whatever the online study concludes.

---

## Appendix: evidence index (quick lookup)

| Claim | Evidence |
|---|---|
| Single edit door | `varos-core/src/command.rs:12`, `:191`; `docs/foundation/F4_DESIGN.md:159` |
| Enum is not serializable | `command.rs:11-12` (no derive) |
| Enum is not the AI API | `command.rs:1-4`; `ADR-0002:24`, `:32` |
| No introspectable schema | `ADR-0004:19-21`; `CLAUDE.md` hard laws |
| Fifth crate needs ADR | `ADR-0005:34`; charter `:80` |
| Real files are PDF containers | `varos-app/src/main.rs:420`, `:884`; `varos-pdf/src/lib.rs:30-45` |
| History mechanics | `varos-core/src/editor.rs:2870-2904`, `:369-371` |
| No creation command | `command.rs:12-117` (no create variant); `tools/shapes.rs:8-18` |
| No PNG export | `varos-app/src/main.rs:272-354` (cursor/icon PNG only); charter `:116` |
| No text model | `model.rs:241-246` |
| Local IPC precedent | `varos-app/src/single_instance.rs:71`, `:88`, `:128-137`; `main.rs:658-661` |
