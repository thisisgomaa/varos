# Export sheet v2 — Lane C implementation handoff

State: implemented, pending independent review and owner native hand test. Uncommitted work on `feat/p0-export`.

## Surface and execution

- `export_ui.rs` owns the existing ticket/cancel/Done lifecycle. `ui/export/{model,paint,mod}.rs` holds the Minimal checklist, grid/list cards, immutable snapshots, format/scale controls, K3 folder/ppi/JPEG quality fields, PNG transparency and preset/keyboard glue.
- File Export… uses ⌥⌘E; PDF… opens the same sheet with PDF and 1×. Selection opens the Selection tab; empty selection has a disabled reason. Advanced remains disabled with `coming` (slice 1.8).
- `file_jobs.rs` submits one Screen job per checked card/format through the existing IO worker. Fresh publication never silently replaces a destination; sheet collisions use ` 2`, ` 3`, etc. The shared cancel flag reaches the native durable rename boundary. Batch Done waits for every ticket result and reports files already written if another file fails/cancels.
- `varos-core/src/svg.rs` now plans a narrowed Selection snapshot while retaining mask geometry. `varos-raster/src/export.rs` shares SVG/raster planning and bytes between app, CLI and Bridge. PDF bytes remain in `varos-pdf`; raster's PDF dependency remains test-only.
- Offline `image` codecs available: PNG, JPEG, WebP, TIFF. JPEG quality is 0–100 (default 90); JPEG reports the opaque page-colour/white background. Raster allocation is limited to 64 million pixels and 16384 pixels/side; scale/ppi is validated. Vector formats ignore raster scale.
- Existing `thumbs` bounded worker/cache supplies cards. Artboards use `rasterize_artboard` including page colour; Selection uses its narrowed bounds. Pixels are cached; rendering never runs per frame.
- CLI: `export-svg` and `export-raster`; `--out`, `--artboard`, plus raster `--format`, `--scale` or `--ppi`, `--transparent`, `--quality`. For multiple boards, CLI `--out` is an existing folder. Existing export-pdf is retained.
- Bridge: `export_svg` / `export_raster` require explicit API 1.2. New MCP discovery uses `tools/list` with `params.api = "1.2"`; default discovery and 1.0/1.1 fixtures remain unchanged. Additional artboards are written beside the requested first path under their own names; destination conflicts are refused before writing. Reports survive completion receipts.

## Persistence and integration choices

- `layout.json` gets an additive `exports` map; old layouts deserialize without it. Checklists retain stable artboard identities, independent tab state, folder, format, scale and options. Advanced stays false in this slice.
- The document model has no persistent document UUID and phase 0 permits no format bump: saved documents use their canonical FileKey path as identity; untitled/recovered documents use the recovery identity. Moving a saved file changes that preference key.
- Narrow required hooks also touch `app_command`, `host`, `lifecycle`, `bridge_host`, `file_ports`, `menus/mod`, `ui/layout` and the existing sheet-open arm in `main.rs`. The main shortcut table, other lanes' core command/editor files, renderer, box/control kit and ratchet ceilings are untouched. Icon registry additions are additive; controls use existing kit and tokens.
- `ui.rs` is 828 lines, below the unchanged 843 cap. No commit, push, GUI run or installation performed.

## Verification

Headless coverage includes click/range/double-click, disabled Selection, scope→page fixture mapping, names/suffix/collisions, app/CLI/Bridge SVG and PNG byte parity on the same nested-mask fixture, report text painted in Done, batch ticket completion, persistence roundtrip and old-layout compatibility, untitled identity isolation, cancellation, vector scale independence and JPEG quality bounds. Existing SVG fidelity, durable cancellation, ratchets and Bridge frozen-contract tests remain in the workspace suite.

Final gates: fmt PASS; dependency directions PASS; offline workspace tests PASS (1521 passed, 0 failed, 15 ignored); native and x86_64-pc-windows-msvc all-target clippy with warnings denied PASS. Ratchet ceilings and Bridge fixture files are unchanged; git diff --check PASS. Native visual geometry, folder picker/Finder interaction and Windows runtime behaviour remain unverified because this lane is explicitly headless and must not launch/install the GUI.
