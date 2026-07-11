# Varos Full Project Audit

**Audit date:** 2026-07-11  
**Auditor:** Codex  
**Purpose:** Evidence package for Ahmed and an adversarial counter-review by Claude before any cleanup or implementation work.  
**Status:** Updated audit snapshot only. This document is not a specification, roadmap, source of truth, or authorization to change code.  
**Committed baseline:** `main` and `origin/main` at `1aff281` (`docs(pains): log round-2 feedback - footer clip root cause, parked artboard-chrome items`)  
**Update note:** The original audit observed concurrent edits. They were subsequently committed as `d6bd3c7`, `5c47f96`, and `1aff281`; Codex then reran the gates and release-build runtime checks on the stable committed baseline. At update time, tracked project files were clean and only this untracked audit directory remained.

## 1. Executive verdict

Varos does **not** need a rewrite. Its core architecture is substantially better than its current product and project operations.

The strongest part is the real separation between pure document/interaction logic, renderer-neutral scene construction, wgpu rendering, PDF-container persistence, and the native app. The project also has unusually strong behavior-level tests for its age.

The weakest part is trust at the project boundary:

- visible UI controls promise behavior that does not exist;
- README and planning claims do not consistently match the code;
- several documents independently call themselves the source of truth;
- CI does not protect `main` on GitHub;
- the dependency audit is red;
- GitHub, legal governance, product decisions, and release operations depend on one person;
- important intent is encoded in conversations, personal paths, dated comments, and local-only branches rather than public issues and accepted decisions.

The central recommendation is therefore:

> Freeze new feature fronts, restore project truth and automated gates, make operations survivable without Ahmed or any AI agent, then reduce the cost of change in `ui.rs` and `editor.rs`. Continue product features only after those foundations are measurable.

## 2. Approximate scorecard

These scores are directional, not scientific.

| Area | Score | Verdict |
|---|---:|---|
| Pure core architecture | 8/10 | Real and worth preserving. |
| Renderer/model separation | 8/10 | A strong long-term seam. |
| Core behavioral tests | 8/10 | Broad for a pre-alpha; still missing fuzz/stress/UI layers. |
| File durability | 6/10 | Thoughtful v1 format and atomic write; migration policy and corpus are too small. |
| Application architecture | 5/10 | Works, but ownership is concentrated in very large files. |
| Visual identity | 7/10 | Coherent and distinctive in the running app. |
| UX completeness and honesty | 3/10 | Dead and fake controls materially damage trust. |
| Documentation governance | 3/10 | Rich history, fractured authority. |
| Security and dependency hygiene | 3/10 | Three advisories; no enforced audit gate or security policy. |
| Release engineering | 2/10 | No tags, releases, installer/signing pipeline, or automatic checks. |
| Contributor readiness | 2/10 | Public repository, but no public work queue or multi-maintainer process. |
| Independence from individuals | 1/10 | One account, one contributor, personal legal/communication dependencies. |

## 3. Scope and method

The audit covered:

- all tracked file types and repository layout;
- active Rust workspace and crate boundaries;
- source metrics and concentration;
- save/load/PDF container design;
- test, build, lint, formatting, and dependency audit commands;
- unsafe Win32 surfaces and vendored code;
- the running Windows application and a real shape/artboard workflow;
- custom egui accessibility exposure;
- README, contribution, legal, architecture, UI, roadmap, pain-log, mask, transform, and launch documents;
- `docs/plan.html`, including its embedded data and render logic;
- historical HTML prototypes and BStudio/Pivot design references;
- visual reference PNGs;
- Git history, tags, branches, contributors, remotes, and GitHub repository metadata;
- ignored local Adobe cursor/reference assets and fallback behavior.

The in-app browser refused `file://` navigation by policy, so `plan.html` could not be rendered through that browser. Its full HTML, CSS, JavaScript data arrays, status block, build rail, decisions, systems, and Arabic gloss were read directly. Existing visual references and the real native app were inspected visually.

## 4. Evidence snapshot

### 4.1 Repository and GitHub

| Evidence | Result |
|---|---|
| Public repository | `thisisgomaa/varos` |
| Owner type | Personal user account, not an organization |
| Default branch | `main` |
| Remote branches | `main` only |
| Local unmerged branch | `codex/p6-header` |
| Open GitHub issues | 0 |
| Open GitHub pull requests | 0 |
| Status checks on `1aff281` | None |
| Workflow runs associated with `1aff281` | None returned |
| Tags on remote | None |
| Commit count | 225 |
| Contributors in local history | One identity for all 225 commits |
| Tracked files | 160 |

Branch-protection, private-vulnerability-reporting, account recovery, and repository-admin redundancy could not be proven from the available repository API. They must be checked explicitly in GitHub settings.

### 4.2 Code size and concentration

| File | Lines | Approx. bytes | Risk |
|---|---:|---:|---|
| `varos-app/src/ui.rs` | 5,563 | 269,762 | Largest application concentration; little direct behavioral coverage. |
| `varos-core/src/editor.rs` | 4,533 | 202,711 | Interaction, history, tools, selection, transforms, and operations are heavily concentrated. |
| `varos-core/src/model.rs` | 1,937 | 87,942 | Central schema and tree behavior; high compatibility blast radius. |
| `varos-render-wgpu/src/lib.rs` | 1,063 | 55,542 | GPU lifecycle and compositing concentration. |
| `varos-app/src/main.rs` | 1,066 | 54,912 | Window, event routing, save/open, crash handling, and app lifecycle. |
| `varos-app/src/shell/boxtree.rs` | 1,042 | 51,081 | Vendor-wrapper seam is useful but has grown broad. |

Documentation concentration is also material:

- `DETAILED_ROADMAP.md`: 2,459 lines / 251 KB.
- `ELEMENTS_CATALOG.md`: 1,652 lines / 213 KB.
- `plan.html`: 584 lines / 47 KB, with roadmap data duplicated in JavaScript.
- `PAINS_LOG.md`: 240 physical lines / 45 KB, including current and stale states in the same document.

There are 94 references to “Ahmed” and 103 date-like references in tracked Rust source. The history is valuable, but much of it belongs in issues/ADRs rather than permanent implementation comments.

### 4.3 Verification commands

The stable committed baseline `1aff281` produced:

| Command | Result |
|---|---|
| `cargo test --workspace` | Passed, 223 tests. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed. |
| `cargo build --workspace` | Passed. |
| `cargo build --release -p varos-app` | Passed. |
| `cargo fmt --all --check` | Passed. |
| `cargo audit` | Failed: 3 vulnerabilities and 3 unmaintained warnings. |

The earlier moving snapshot had 215 tests and a formatting failure; those observations are superseded by the stable results above. The exact count remains volatile and should not be used as timeless marketing or documentation text.

### 4.4 Dependency findings

`cargo audit` scanned 498 lockfile dependencies, including target-specific packages.

| Package | Finding | Path / qualification |
|---|---|---|
| `crossbeam-epoch 0.9.18` | `RUSTSEC-2026-0204` | Through `lopdf -> rayon`; upgrade to `>=0.9.20`. |
| `quick-xml 0.39.4` | `RUSTSEC-2026-0194`, High | Through Wayland scanner dependencies in the all-target lockfile. |
| `quick-xml 0.39.4` | `RUSTSEC-2026-0195`, High | Same target-specific path; Windows runtime does not use this path, but repository audit remains red. |
| `proc-macro-error` | Unmaintained | Through `flo_curves -> ouroboros`. |
| `proc-macro-error2` | Unmaintained | Present in lockfile; exact active target path requires dependency cleanup. |
| `ttf-parser 0.25.1` | Unmaintained | Through `resvg/usvg`, `lopdf`, and text/font dependencies. |

CI currently treats `cargo audit` as `continue-on-error`, so even a manual run cannot block a release.

## 5. What is genuinely strong

### 5.1 The hard core seam is real

`varos-core` contains model, geometry, editor interaction, scene construction, tools, and units without wgpu, winit, egui, or platform UI dependencies. The compiler enforces the most important architectural promise.

### 5.2 The scene boundary is useful

The core emits renderer-neutral `Prim` and `Group` values, including isolated, knockout, and clip semantics. The GPU renderer consumes meaning rather than owning the document. This supports headless tests and protects against renderer replacement.

### 5.3 File persistence contains good defensive choices

- `VRS_VERSION` rejects unknown newer schemas rather than guessing.
- Atomic temp-and-rename writes reduce corruption risk.
- Legacy raw JSON remains loadable.
- The current `.vrs` is a valid PDF with an associated embedded JSON model.
- Serde defaults have supported several additive migrations.
- Three legacy/golden fixtures already exist.

### 5.4 Interactive bugs are often converted into tests

The test suite covers artboards, groups, layers, rotation, transforms, snapping, occlusion, opacity, masks model behavior, PDF round trips, legacy migration, and tessellation steps. Recent live-transform group regressions received focused tests rather than only patching symptoms.

### 5.5 Failure presentation improved

GPU initialization returns a readable error; top-level failures and panics create `%APPDATA%/Varos/crash.txt` and show a dialog. Single-instance behavior and file forwarding are implemented with documented Win32 assumptions.

### 5.6 Proprietary cursor references are not tracked

`Cursors/`, `assets/cursors-ai/`, and `logo.ai` are ignored. The application has legal built-in cursor fallbacks, so a clean checkout does not require proprietary Adobe material. This is the correct build property.

## 6. Critical findings: product truth and user trust

### P0.1 Visible commands have no behavior

In `varos-app/src/ui.rs`:

- `Search` and `Ctrl+K` have no command implementation.
- `Share` is a visual mirror only.
- the top-level `Export` button is a visual mirror only.
- burger-menu rows for `New`, `Open`, `Save`, and `Export` call `menu_row(...)` but ignore its returned click.
- `Ctrl+O`, `Ctrl+S`, and `Ctrl+Shift+S` work in `main.rs`, creating a contradiction between visible menus and shortcuts.
- `Ctrl+N` is displayed but not implemented.

This was confirmed both in code and in the running app. The menu remained open after clicking `New`, and no action occurred.

**Impact:** a user cannot distinguish unfinished decoration from a defect. Every visible command must work, be visibly disabled with a reason, or be absent.

### P0.2 Document tabs are fake state

The app stores tab labels as `Vec<String>` and an active index. Clicking `+` adds `Untitled-N`; clicking and closing tabs changes only those labels. The application still owns one `Editor`, one `Document`, and one current file.

`set_doc_tab` updates tab zero regardless of the visually active tab. There is no per-tab document state.

**Impact:** this is more dangerous than a disabled feature because it implies multi-document isolation while edits still affect the same document.

### P0.3 README claims SVG/PNG export is working, but no exporter exists

README places “SVG/PNG export” in “Working today.” Code search found no document SVG or PNG export pipeline. PNG code is limited to icon/cursor debugging. PDF output exists through `.vrs`/`.pdf` save.

**Impact:** public capability claims are false at the audited commit.

### P0.4 The README’s CI and test claims are stale

- README references 101 tests.
- CI comments reference 90 tests.
- plans reference 79, 89, 101, 199, and other dated totals.
- the stable audited baseline ran 223 tests.
- CONTRIBUTING says CI enforces four gates on every PR, but workflow triggers are manual-only and GitHub returned no checks on `main`.

Use generated badges or “200+” with a dated evidence link. Never make a volatile count a timeless product fact.

### P0.5 `.vrs` is a valid PDF, but “opens anywhere” needs qualification

The bytes are a valid PDF, which is valuable. However, a `.vrs` extension is not universally associated with PDF viewers. A client or print shop may not open it by double-click without choosing an application or renaming/saving as `.pdf`.

Marketing should distinguish:

- technical PDF validity;
- OS file association;
- viewer auto-detection;
- the explicit `.pdf` save path.

## 7. Critical findings: documentation truth fracture

Varos has more documentation than many mature projects, but no enforceable authority model. The same decision is repeated and amended inside large narratives. Drift is inevitable because updates are manual and duplicated.

### 7.1 Contradiction matrix

| Subject | Conflicting claims | Actual audited state |
|---|---|---|
| UI architecture | Constitution/old plans: Tauri + web panels. Current rules: native GPU UI. | Native winit + egui + wgpu. No Tauri shell. |
| Schema strategy | Constitution: single schema deferred. `CLAUDE.md`: single schema is hard law. Roadmap: RNA-style goal. | Serde document structs exist; no introspectable schema/property registry. |
| Native extension | Many plans/catalog entries: `.varos`. Constitution and code: `.vrs`. | `.vrs`. There are 51 `.varos` references and 36 `.vrs` references in docs/root text. |
| Container | Detailed roadmap: ZIP/OPC. Save plan: CBOR in PDF. | JSON model embedded in a PDF container. |
| Masks | `plan.html`, roadmap, and layers vision: deferred. New mask plan/master plan: next. | Stages 1-2 merged; gestures and full semantics incomplete. |
| License | Master plan contains AGPL recommendation and later GPL correction. | Repository license is GPL-3.0. |
| UI source of truth | BStudio design system says Figma wins. `UI_FIGMA_SPEC` says it supersedes prior colors. `UI_DIRECTION` says it is law. Tokens contain later Ahmed changes. | Running native UI and `UI_DIRECTION` are closest to current, but no formal precedence record exists. |
| Current phase | `plan.html`: Layers active, Color Stage 2 next, masks deferred. Pain log/master plan: pain sweep, A7, masks, visual polish. | A7 and its group/layer-move follow-up are merged; masks stages 1-2 are merged; P6 remains local; P4/P5 remain manual-retest items. |
| CPU fallback | Constitution promises CPU fallback. | GPU initialization can fail gracefully, but no CPU renderer fallback exists. |
| Auto-update | Constitution locks silent incremental updates. | No updater or release channel exists. |

### 7.2 `plan.html` findings

`docs/plan.html` is not a passive illustration. It declares itself “the whole roadmap” and names `DETAILED_ROADMAP.md` as source of truth.

Problems:

- visible update label says 2026-07-02 while the status blob contains changes through 2026-07-06;
- it says masks are deferred, contradicted by the 2026-07-09 merge;
- its build rail points to Layers/Color Stage 2 rather than the current work;
- it references `.varos`, not `.vrs`;
- it repeats the roadmap as JavaScript arrays, requiring manual dual maintenance;
- it marks the command/Op pattern “done,” but the meaningful `Op` enum is local to `ui.rs`, not a stable core command API;
- it contains long milestone prose that is difficult to diff and impossible to validate automatically;
- its visual CSS uses large radii and a box shadow, while current UI law says near-sharp and no shadows. This is harmless for an archive, dangerous for an authority document.

Conclusion: `plan.html` is a valuable historical dashboard, but a false current source of truth.

### 7.3 Historical design references are mixed with current law

Tracked `design-reference` content uses three product identities: Pivot, BStudio, and Varos. `design-system.md` describes a different React/Skia codebase, references missing `COLORS.md` and `v0/web/...` paths, and says a BStudio Figma file is canonical.

The references are visually useful, but a future maintainer cannot know which details are inherited inspiration and which are Varos requirements. They should eventually be clearly archived and mapped to the accepted Varos decisions.

### 7.4 Personal and temporary paths remain in durable docs

Examples include:

- a personal Claude memory path in `VAROS_START_HERE.md`;
- a temporary Claude scratchpad path in `SAVE_EXPORT_PLAN.md`;
- absolute `D:\VAROS` worktree commands in box-system plans;
- missing promised documents such as `VRS_FORMAT.md` and `VENDOR_PATCHES.md`.

These are direct continuity failures: a successor cannot access the cited evidence.

### 7.5 Pain-log and work-order reconciliation

`PAINS_WORK_ORDER.md` and `PAINS_WORK_ORDER_2.md` were read completely during the update. They are temporal execution contracts, not current product truth:

- work order 1 defines the original five waves and explicitly forbids P5, P6, A30, masks, and new feature fronts for that session;
- work order 2 defines T1-T11, admits A30 and P6, and still forbids masks for that session;
- later history merged masks stages 1-2 anyway, proving that the execution sequence changed after those orders were written.

Reconciled state at `1aff281`:

| Work | Evidence-backed state |
|---|---|
| T1-T5, T7-T10 and related A-items | Mostly merged according to the top status table and referenced commits in `PAINS_LOG.md`. A30's property toggle exists, but its requested right-click item is still explicitly missing. |
| A7 live transform | Merged through stages 1-7; group, ungroup, and cross-group layer-move regressions were then fixed in `d6bd3c7` with 9 added regression tests. `live_transform` now passes 31/31. |
| Layer footer/icon clipping | Raster size was reduced, then the actual layout clipping cause was fixed in `5c47f96`. Codex visually confirmed the footer icons visible and unclipped in the release build at 1920x1032, both empty and with three layer rows. Other icon locations still need a deliberate visual sweep. |
| P4 layer-row rename | Fallback double-click detection was added, but remains unresolved. In the release-build automation, two clicks within 150 ms selected the row but exposed no obvious inline editor. Automation may classify native egui clicks differently, so this is a manual-retest requirement, not proof of either success or failure. |
| P5 single instance | Code is merged, but the same top section records both “Ahmed confirmed it works” and a later “not working at all.” The exact launch scenario must be captured and retested before closure. Multi-document opening remains deferred. |
| P6 box header | Green on local branch `codex/p6-header`, not merged, and explicitly waiting for mouse/UX approval. |
| Masks | Stages 1-2 are merged; stages 3-6 remain incomplete. The older “in flight” text is stale. |
| Artboard canvas chrome | Artboard-name double-click rename and the size-chip redesign are explicitly parked for later judgment. |

The log still contains stale sections below the current table: A7 is both merged and described as undecided, P5 appears in contradictory states, and older queue rows list completed work as pending. It is excellent raw history, but unsafe as a live queue without a compact authoritative status block generated from issues or accepted decisions.

## 8. Architecture and maintainability findings

### P1.1 `ui.rs` is the highest refactor risk

At 5,563 lines, it owns UI state, snapshots, icon loading, color modal, top bar, document tabs, rail, control bar, layers, properties, align/pathfinder, artboard chrome, rulers, menus, and operation dispatch.

The file has multiple `clippy::too_many_arguments` allowances whose comments explicitly defer splitting. This is controlled debt, but it is now large enough to increase regression probability.

Do not split it by arbitrary line count. First add characterization around command routing and panel state, then split by ownership.

### P1.2 `editor.rs` is the second concentration risk

At 4,533 lines, `Editor` owns transient interaction state, tool behavior, selection, drag variants, transforms, snapping, groups, artboards, history, color sessions, and masks operations.

The facade is useful, but implementation domains should eventually move behind it. A direct “split everything” refactor without tests for commands and gestures would be unsafe.

### P1.3 Undo stores up to 200 full `Document` clones

Every `begin()` clones the document; commit pushes the snapshot; undo/redo clone again. This is simple and correct for current vector-only documents, but memory grows roughly with document size x history depth.

Before embedded images, fonts, or large assets enter `Document`, heavy immutable bytes must be shared (`Arc` or content-addressed storage), or history must evolve. Benchmarks must measure real memory before redesigning the command log.

### P1.4 Plugin/AI readiness is low despite roadmap language

- `ToolKind` is a closed enum.
- UI `Op` is private to `ui.rs`.
- mutations are many direct `Editor` methods, not a versioned public command protocol.
- the model is serializable but not introspectable as a Blender-RNA-style property schema.
- no plugin host, permission model, ABI, or compatibility contract exists.

This is not a failure at pre-alpha. The failure would be claiming plugin readiness or exposing an unstable API too early.

### P1.5 Local `egui_tiles` fork lacks a durable patch contract

The fork is usefully isolated behind `shell/boxtree.rs`, but it includes several Varos changes and no `VENDOR_PATCHES.md`, upstream base commit, automated diff check, or update procedure.

The workspace comment saying “the only edit” is already too narrow relative to changes visible across `lib.rs`, `behavior.rs`, and `tree.rs`.

### P1.6 Deprecated winit lifecycle is intentionally allowed

`main.rs` uses `#![allow(deprecated)]` for the closure-based event loop. This preserves current behavior, but winit’s `ApplicationHandler/run_app` path should become a planned migration before the deprecated API is removed.

### P1.7 Crate metadata and toolchain policy are incomplete

- no `rust-toolchain.toml` or declared MSRV;
- crate manifests lack license, repository, description, and `rust-version` metadata;
- package metadata is duplicated rather than centralized under `[workspace.package]`;
- no `.gitattributes`, and Git warns about LF/CRLF conversion;
- no `cargo-deny`/license/advisory policy file.

Cargo.lock is tracked, which is good for an application.

### P1.8 Source comments overfit one person and one moment

Comments frequently record Ahmed’s exact report, date, pain ID, or session decision. Some are excellent regression explanations. Others make the source read like a session transcript and become stale when the decision changes.

Long-term rule: code comments explain the invariant and failure mode; issues/ADRs preserve who, when, and the decision history.

## 9. File-format durability study

### What is good

- clear `VRS_VERSION = 1` header;
- additive serde defaults;
- refusal of newer versions;
- legacy raw-JSON load;
- atomic writes;
- embedded editable source in a valid PDF;
- round-trip tests and three legacy fixtures.

### What is missing

- a public, exact `VRS_FORMAT.md`;
- explicit policy for rename/removal/semantic changes;
- migration functions for v2+;
- fixture provenance and expected behavior documentation;
- corruption, truncation, zip-bomb-style/resource-limit, and fuzz tests;
- file-size and parse-time limits for hostile PDFs/JSON;
- stable ID and unknown-field policy for third-party tools;
- ICC/output intent for dependable cross-viewer color;
- mask semantics in PDF export;
- text/font embedding and fallback policy;
- a “view-only when model is too new” path (currently newer schemas are refused).

The Save plan proposed CBOR while the code embeds JSON. The detailed roadmap proposed ZIP/OPC while the code uses PDF. The implemented format may be the better decision; it simply needs a formal accepted record.

## 10. Security and unsafe-code study

Threat surface is currently limited because Varos is offline and has no updater, plugin host, or cloud service. The primary untrusted input is a `.vrs`/PDF file.

Risks:

- RustSec advisories described above;
- `lopdf` parsing hostile PDF structures without a project fuzz corpus;
- embedded model allocation and parsing without explicit limits;
- 27 unsafe occurrences across `cursors.rs` and `single_instance.rs` at the stable baseline;
- `single_instance.rs` has strong `SAFETY` documentation, while many cursor/window unsafe blocks do not;
- no `#![deny(unsafe_op_in_unsafe_fn)]` policy;
- no `SECURITY.md` or private-reporting procedure documented in the repository;
- no automated dependency update service;
- no license inventory for Rust dependencies; `THIRD_PARTY_NOTICES.md` currently covers icon assets only.

No claim is made here that an advisory is directly exploitable through Varos. The requirement is explicit triage with target path, reachable code, upgrade status, and accepted residual risk.

## 11. Test and quality-system study

### Strong coverage

- pure model and interaction behavior;
- transforms and group invariants;
- artboard membership and clipping behavior;
- opacity/knockout semantics;
- PDF container round trips;
- old-file migration;
- mask model/tessellation foundations;
- headless shell rendering.

### Missing coverage layers

- real menu and shortcut command parity;
- multi-document behavior (because it does not exist);
- UI screenshots at multiple window sizes and DPI scales;
- accessibility tree expectations;
- GPU integration smoke tests on supported adapters;
- deterministic large-document benchmarks;
- memory budget for 200 document snapshots;
- fuzzing for `.vrs`/PDF/model parsing and geometry operations;
- installer, file association, signing, update, and uninstall smoke tests;
- release artifact reproducibility;
- Linux/macOS compilation gates if cross-platform remains a promise.

CI being manual-only means these tests currently protect only when a human remembers to run them.

## 12. Running UI and UX study

The native application was first launched from `target/debug/varos.exe` and inspected at approximately 1338x1008. After the repair commits, Codex built and launched `target/release/varos.exe` and repeated focused checks at approximately 1920x1032.

Successful workflow:

- app launched and GPU initialized;
- rectangle and ellipse were created;
- an artboard was created around them;
- object selection and properties display worked;
- Window menu showed the panel set;
- the visual system looked coherent, professional, and clearly more mature than an ordinary prototype.
- the Layers footer folder/group and delete controls remained visible and unclipped in the release build with zero and three layer rows, independently confirming the `5c47f96` layout fix at that viewport.

Observed UX risks:

- application opens with no artboard and no meaningful new-document/empty-state flow;
- dead Search/Share/Export/file commands;
- fake document tabs;
- the right properties box can remain much taller than its content;
- custom egui controls are almost absent from Windows accessibility/UI Automation;
- the UI is English-only despite the Arabic-first identity;
- no text system exists, so the Arabic differentiator is still a plan rather than product behavior;
- the cursor artwork seen in local testing has a strong glow/halo and needs precision testing by hand; local proprietary assets may alter this appearance;
- no evidence was collected for small windows, 125/150/200% DPI, keyboard-only use, high contrast, or low-end GPUs.
- layer-row rename still needs a human mouse recheck: automated rapid clicks selected the row but did not visibly enter rename mode.

Visual judgment remains Ahmed’s gate, but product truth and accessibility are engineering gates, not taste.

## 13. Product completeness study

Varos is currently a vector drawing engine and editor foundation, not a complete Illustrator alternative.

High-impact missing capabilities:

1. Text creation/editing and font system.
2. Arabic shaping, BiDi, fallback, OpenType, export, then kashida.
3. SVG import.
4. Actual SVG/PNG export.
5. Complete stroke caps, joins, alignment, dashes, and arrowheads.
6. Gradients and reusable swatches.
7. Reachable and complete masks.
8. New-document flow, real multi-document tabs, recent files, autosave, and recovery.
9. Installer, file association, signing, version display, changelog, and release channel.
10. Performance validation and large-file handling.

Do not start all ten. Product sequencing must follow stabilization and user-value gates.

## 14. Continuity and governance study

This is the largest existential risk relative to Ahmed’s goal that the project survive every individual.

### 14.1 Current single-person dependencies

- GitHub repository is owned by one personal account.
- all 225 commits are attributed to one contributor identity;
- no public issues or PRs carry the work queue or review history;
- only `main` exists remotely; P6 exists only as a local branch;
- no releases or tags establish recoverable milestones;
- no CODEOWNERS or maintainer team exists;
- Code of Conduct reports go to one personal Gmail address;
- trademark is personally owned by Ahmed Gomaa;
- CONTRIBUTING grants additional relicensing rights specifically to Ahmed Gomaa;
- manual visual gates repeatedly require Ahmed by name;
- the canonical Figma source is outside the repository;
- editable vector logo source is intentionally local/ignored;
- many decisions exist only as dated conversation summaries.

GPL protects the public code’s continued availability. It does not automatically solve repository administration, trademarks, signing keys, Store accounts, domains, private reports, or relicensing authority.

This section is not legal advice. Trademark succession and the person-specific relicensing grant require qualified legal review. The durable direction is an entity or governance structure that survives an individual, not silent assumptions.

### 14.2 Missing operational assets

- maintainer/onboarding guide;
- architecture document describing the current native stack;
- accepted-decision record system;
- security policy and reporting channel;
- release checklist and rollback procedure;
- secrets/service registry kept securely outside Git but referenced by role;
- organization with at least two recovery-capable administrators;
- signing-key custody and rotation policy;
- issue/PR templates and public work taxonomy;
- backup/mirror plan;
- contributor promotion and maintainer-removal process;
- inactivity/succession procedure.

### 14.3 Definition of “independent of everyone”

A successor with only the public repository should be able to:

1. understand current status without reading chats;
2. build with a declared toolchain;
3. run every deterministic gate;
4. know which UI/product claims are real;
5. change the model without corrupting old files;
6. update or remove the vendored fork;
7. publish a test build without private undocumented steps;
8. report and fix a vulnerability;
9. make a decision through a public process;
10. continue under the license even if the original accounts are inaccessible.

Varos does not yet satisfy this definition.

## 15. Recommended remediation program

This is a proposed sequence for review, not authorization to implement.

### Phase 0 - establish a stable review baseline

- preserve `1aff281` as the current reviewed baseline and avoid mixing the audit with new feature commits;
- record the green fmt/clippy/test/debug-build/release-build evidence and the still-red dependency audit;
- create a tag or immutable audit commit;
- have Claude counter-review this report;
- let Ahmed resolve disputed priorities.

Exit gate: one stable commit and one agreed list of facts.

### Phase 1 - restore trust before features

- wire/remove/disable every dead command;
- remove fake tabs or implement true per-document state;
- correct README capabilities and CI language;
- resolve dependency advisories;
- enable automatic CI on push/PR and make required checks visible;
- decide and preserve or close local-only P6 work.

Exit gate: no visible lie, clean required checks, no unexplained security finding.

### Phase 2 - make the project self-describing

- create one current-status source with precedence rules;
- classify every existing document as current, historical, superseded, or reference;
- create current architecture and maintainer guides;
- create ADRs for native UI, `.vrs` PDF+JSON, schema policy, command boundary, and vendor fork;
- archive BStudio/Pivot references clearly;
- replace personal/temp-path citations with repository evidence;
- turn pain items into public issues with acceptance criteria.

Exit gate: a new maintainer can explain the system and next work without a chat transcript.

### Phase 3 - continuity and release operations

- move to a GitHub organization or establish equivalent multi-admin recovery;
- define maintainer roles and succession;
- obtain legal advice on trademark and person-specific relicensing continuity;
- add SECURITY, issue/PR templates, changelog, release checklist, and version policy;
- establish signing, Store, secrets, backup, and recovery custody by role;
- create the first reproducible tagged pre-alpha release.

Exit gate: loss of one person/account does not stop development or releases.

### Phase 4 - reduce code-change cost

- characterize UI command routing before splitting `ui.rs`;
- introduce one core command contract for menus, shortcuts, automation, undo, and future plugins;
- split UI and Editor by domain behind stable facades;
- migrate deprecated winit lifecycle;
- formalize vendor patch tracking;
- narrow/document unsafe blocks;
- pin toolchain/MSRV and normalize line endings.

Exit gate: common changes touch owned modules and tests, not one central file.

### Phase 5 - durability and performance

- publish `.vrs` format and migration policy;
- expand golden corpus and add hostile/corrupt cases;
- add fuzz targets;
- add standard stress documents and CPU/RAM/VRAM/time budgets;
- address full-clone undo before heavy embedded assets;
- test supported Windows/GPU/DPI matrix.

Exit gate: measurable limits and a credible old-file guarantee.

### Phase 6 - product sequence

Recommended value order after stabilization:

1. SVG import.
2. Real SVG/PNG export and honest export UI.
3. Finish masks stages 3-6.
4. Gradients/swatches and complete stroke specification.
5. Packaging/signing/controlled beta.
6. Text architecture, Latin editing, Arabic shaping/BiDi/fallback/export, then kashida.
7. Plugin/AI interfaces only after command and schema contracts are stable.

## 16. What not to do

- Do not rewrite the core or switch frameworks to “clean things up.”
- Do not split giant files before characterization tests and ownership boundaries exist.
- Do not expose a plugin API over `ToolKind` or private UI `Op`.
- Do not add a second document model for inspector/plugins/AI.
- Do not promise CPU fallback, cross-platform support, SVG/PNG export, multi-doc, or Arabic until evidence exists.
- Do not delete historical documents; classify and supersede them.
- Do not make the founder’s memory the acceptance test.
- Do not let two agents edit the same ownership area concurrently without branches and handoff.
- Do not make `cargo audit` advisory-only for a public release.

## 17. Questions requiring Ahmed’s decision after counter-review

1. Should fake tabs be removed immediately or is true multi-document support a near-term requirement?
2. Should dead Search/Share commands disappear, or be visibly disabled with a roadmap reason?
3. Is `.vrs` permanently the public extension? If yes, all `.varos` documentation must become historical.
4. Is PDF+embedded JSON the accepted permanent v1 container direction?
5. Is single-schema introspection a current law or a future extraction goal?
6. Does Varos promise a CPU fallback, or only a respectful GPU failure?
7. Is Figma still authoritative, and if so, how will a successor access/export it?
8. What is the intended legal owner of trademark, signing identities, and relicensing authority in a succession scenario?
9. Is the project ready to move from a personal repository to an organization with additional administrators?
10. Which product gate matters first after stabilization: import/export completeness, masks, or text?

## 18. Claude counter-review protocol

Claude should not implement fixes while reviewing this report.

Required response format:

| Audit claim | Agree / disagree / uncertain | File:line, command, or runtime evidence | Corrected severity | Recommended action |
|---|---|---|---|---|

Claude must:

1. Read this report completely.
2. Inspect `README.md`, `CONTRIBUTING.md`, `CLAUDE.md`, `docs/plan.html`, `DETAILED_ROADMAP.md`, `MASTER_PLAN_V1_LAUNCH.md`, `PAINS_LOG.md`, `PAINS_WORK_ORDER.md`, `PAINS_WORK_ORDER_2.md`, `MASKS_PLAN.md`, `UI_DIRECTION.md`, and the current Rust ownership files.
3. Re-run quality and dependency commands against a stable tree.
4. Identify every factual error in this report before discussing style or priority.
5. Explicitly verify or refute the dead-command, fake-tab, and absent SVG/PNG-export findings.
6. State the actual current mask stage and actual source-of-truth precedence.
7. Trace each RustSec advisory to target and reachable code, then propose an upgrade or documented triage.
8. Identify risks Codex missed, especially data-loss, geometry, PDF, Win32, and concurrent-edit risks.
9. Challenge the proposed sequence and provide a better sequence if evidence supports it.
10. Separate “Ahmed must judge feel” from deterministic engineering acceptance criteria.

Questions Claude must answer directly:

- Is any rewrite justified? If yes, name the exact boundary and evidence.
- Are `ui.rs` and `editor.rs` the correct first modularization targets?
- Is the current `Op` model sufficient to call the command pattern complete?
- Does the current `.vrs` design have an unrecognized compatibility or security flaw?
- Is the clone-snapshot undo risk urgent now or only before embedded assets?
- Which audit P0 item should be first, and why?
- Which recommendation would Claude reject entirely?
- What did Codex fail to inspect or misunderstand?

Claude should finish with one of these verdicts:

- **Accept audit as baseline**
- **Accept with listed corrections**
- **Reject as baseline**, with a replacement evidence set

## 19. Audit limitations

- The original working tree changed during review; the update gates were rerun on stable `1aff281`, but this still is not a signed release audit.
- Branch-protection and account-recovery settings were not directly available.
- No installer or signed release artifact existed to test.
- No low-end GPU, Intel iGPU, high-DPI matrix, screen reader, or non-Windows machine was available.
- The local browser could not render `file://docs/plan.html`; source was read completely instead.
- No destructive, publishing, legal, settings, or implementation changes were made.
- Legal observations are continuity risks, not legal advice.

## 20. Bottom line

Varos already has the beginning of a durable editor core. The project will not survive two hundred years because the code is clever; it can survive because its contracts, decisions, files, tests, release powers, and legal/operational ownership stop depending on the memory or account of any one person.

The next milestone should not be “more features.” It should be: **a stranger can clone Varos, know what is true, run the gates, choose the next issue, and continue safely without asking who Ahmed, Claude, or Codex were.**
