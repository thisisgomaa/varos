> **Status:** reference — merged to `main` in `2cb072a` (2026-10-04, after the batch review, fixes `e772f16`). The E2 Start page has since been superseded by [Start v2 — Boards](START_V2_BOARDS.md) (`6405dc9`); the one open path, Home and Recent wiring described here remain. Current state: [PLAN](../../PLAN.md).
# DFS S2-E2 — Start, Home and recent-file integration

2026-09-27. Baseline `c27b8a4`. Implements the E2 slice of [S2/S3](DFS_S2_S3_START_RECENTS_RECOVERY.md) after [minimum U0-B/C](UI_U0_BC_KIT.md). Owner requested continued implementation with independent review batched at session end.

## Contract and implementation

- Launch without a file shows Start immediately after graphics initialization; the timed splash and its drawing code are removed. GPU startup failure still uses the existing readable fatal dialog. A command-line file goes through the same host/lifecycle handler before the initial UI frame; failure leaves Start available.
- `Workspace::start_page` keeps S1's internal nonempty session invariant with a hidden pristine placeholder. New reveals a boardless document. Closing the last tab returns to Start. Home preserves open sessions, view, selection and dirty state; selecting the active tab also leaves Home.
- `Ui::run_home` does not build a document Snap, panels or overlays. Canvas input and document commands have no target on Home, including commands queued after a Home click in the same FIFO batch. Mac uses a Home icon beside the tabs; native menus remain. Windows keeps its menu with a Home entry.
- `start_ui.rs` owns presentation and the E1 keyboard model. The single `host::start_command` adapter emits path-based commands. New/Open, Recent, missing-file Locate/Remove and Clear are wired. Tab/Shift-Tab, arrows, Enter/Space and Delete use the model; pointer-only kit controls avoid duplicate keyboard activation. Recovery rows/actions are exercised with fake data only; F2 supplies real recovery.
- `RecentStore` wraps the existing document store. Successful open/focus/save records through the existing 20-entry deduplicating model and durable metadata writer, using the one app-data resolver. Painting performs no filesystem work. Startup, lifecycle completion and Home focus refresh the cached presentation.
- Open, Recent and Locate share candidate loading before replacing/focusing a session. Failed opens and canceled Locate leave documents and Recent unchanged. Successful Locate replaces the old entry in its list position and removes duplicate path/file identity entries. Remove/Clear modify metadata only.
- Unknown-version/read-error Recent data is preserved; unavailable persistence remains usable in memory with a notice. Metadata write failure never reports document failure. Real document Save still uses the existing writer: F1 owns its durable replacement.
- Native File → Open Recent mirrors the first ten entries plus Clear Menu. Save, Save As, Close and document actions are disabled on Home.

## Repaint correction found during native verification

The existing non-Windows event adapter requested a new frame for `RedrawRequested` itself, keeping the window busy while idle. Exclude that event; honor egui's delayed repaint deadline in the event loop so tooltips still appear. An unsuccessful surface presentation gets at most three short retries instead of spinning. Home renders an empty scene and invalidates the document scene cache.

## Evidence and limits

- Workspace: **772 passed, 0 failed, 5 intentionally ignored**, including twelve new tests. The CPU tests cover Start/lifecycle transitions, Home/FIFO isolation, successful-only Recent updates, missing/cancel/Locate outcomes, persistence reload and metadata-only removal, the native ten-item projection, 1×/2× focus/accent/activation and fake recovery actions. No test creates a Renderer or EventLoop.
- macOS build, macOS and Windows-target all-target clippy with `-D warnings`, format, architecture checker and seven checker tests pass. Exact run evidence is in [GATE_LOG](../GATE_LOG.md).
- Native Mac checks use a temporary app bundle and isolated `VAROS_DATA_DIR`, with a copy of a repository fixture. The installed application and source fixtures are unchanged. Before the final splash-removal change, Start/New/Home, native Open, Recent persistence and native Recent reopen were exercised; Home's Save/Save As/Close menu items were disabled. Idle process sampling after the repaint fix reached 0% CPU; this is a spot observation, not a performance benchmark.
- Initial splash-free build inspection was blocked: the native window accepted New and updated its title, but screenshots showed only the window background. Temporary instrumentation identified `wgpu::CurrentSurfaceTexture::Occluded` on every attempted presentation, including after Raise/zoom; there was no successful final-build GPU presentation to inspect. Instrumentation was removed. The owner was asked to wake/show the display. At the next turn the same build rendered its Start surface, confirming that presentation resumed when visible; this did not require another rendering change.
- Windows is compile-only. Automated checks are not independent approval or owner acceptance. Arabic bidi/caret and screen-reader coverage remain limited as recorded in U0. Real recovery, durable document Save and Finder association remain F1/F2/S4 work.

## Next

F1: wire the durable real-save path and background recovery writer, then F2: scan and recover as a separate copy. Preserve this Start surface and the shared lifecycle pipeline. Independent review/merge and owner batch acceptance remain pending.

## Owner-requested Recent refinement — 2026-09-27

Owner feedback: the Recent surface is visually weak. The visible splash-free build now renders correctly, clearing the previous occlusion-only inspection blocker. The initial screenshot shows a small undifferentiated cluster at the upper-left of a large empty panel.

Refinement direction: preserve the warm-black palette, bundled type, neutral controls and focus-only azure. Center a bounded workspace; separate New/Open into a compact launch column on wide windows and stack them on narrow windows. Give Recent a clear title, aligned file/path and last-opened information, document symbols and visible row menus. Keep empty/missing states useful and Clear subordinate. No fabricated previews, new persistence, search/filter framework, host/lifecycle changes or recovery implementation. Verify pointer/keyboard actions and long/missing rows at narrow/wide widths, then inspect the native Mac surface.

Owner also identified the duplicate top-bar plus on Home. It is hidden on Home, where New document already owns creation; it remains beside document tabs while editing. The plus creates a new document, not another artboard inside the current document.

Refinement verification: **773 workspace tests passed**, 0 failed, 5 ignored; Mac build/format and Mac/Windows all-target clippy pass. The added CPU scenario checks 440/800/1460 logical-point widths at 1×/2×: long/missing file rows and menu targets stay within the viewport, row click opens once, and the separate menu exposes Locate/Remove without opening the file. Existing focus/accent/recovery/control tests remain green. The preceding E2 surface was visibly inspected, but the new build again remained occluded during native capture; final visual confirmation remains pending. Synthetic Recent rows were confined to the isolated temporary app-data folder and its original Recent list was restored. Independent review/merge and owner visual acceptance remain pending.
