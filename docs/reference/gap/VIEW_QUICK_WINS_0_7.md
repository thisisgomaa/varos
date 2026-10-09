# Slice 0.7 view quick wins (2026-10-09)

Implemented after full slice 0.8 gates passed. Worktree only, uncommitted.
Paths below are relative to `varos/crates/`.

| Behaviour | Inventory evidence | Implementation |
|---|---|---|
| Fit All, typed zoom, Zoom Z and Hand H | `GAP_1_FILES_IO_CANVAS.md:84-85` | `varos-app/src/main.rs`, `gestures.rs`, `ui/bar.rs`, `ui/rail.rs`; pure view math and kit numeric field |
| Make/Release/Clear Guides, typed ruler-guide position | `GAP_1_FILES_IO_CANVAS.md:88` | `varos-core/src/view_commands.rs`, `varos-app/src/ui/guide_field.rs`; path geometry/paint retained, double-click ruler guide opens kit field |
| Show Grid and document spacing/subdivisions | `GAP_1_FILES_IO_CANVAS.md:91` | `EditCommand::View`, grid section in document panel; Scene/renderer grid and snapping share the document spacing |
| Reorder, fit artwork/selection, convert selection | `GAP_1_FILES_IO_CANVAS.md:101` | `varos-app/src/ui/panels/artboard.rs`; earlier/later arrows, core commands and atomic undo |

Bridge `edit` verb `view` is API 1.2 opt-in. Targeted actions require explicit
artwork ids; global actions require empty ids. The existing CLI apply/call
adapters expose the same document routes. View zoom/pan remain transient.
New optional guide/grid fields skip default serialization; no format bump.
Hand and Zoom glyphs come from pinned lucide@1.8.0, with provenance headers.
Artboard fit/conversion attribution is in the module header and `varos/NOTICE`.

Core, Bridge, pure gesture and CPU kit-action tests cover the new behaviours.
No test constructs a GPU renderer or event loop. Review and owner hand testing
remain pending: GUI launch and installation were explicitly prohibited.
Final combined gate results follow.

## Final combined gates

All commands ran offline with `-j 2` where applicable. Workspace: exit 0,
1,558 passed / 0 failed / 15 ignored across 86 unfiltered test summaries;
three additional child-process test runs passed (1,561 total pass reports).
49 new named tests: core 32, Bridge 8, app 9; existing kit/menu checks extended.
Native and x86_64-pc-windows-msvc all-target clippy: exit 0, zero warnings.
Formatting, dependency directions and diff whitespace checks: PASS.
Ratchet source byte-identical; ui.rs 826 / unchanged 843 cap.
14 tracked Bridge fixtures byte-identical; tools/list 23,993 / unchanged 24,000 bytes.
Logs and machine-readable counts: `varos/target/lane-b-gates/`.
No commit, push, merge, GUI launch or installation; independent review, native
runtime acceptance and owner hand testing remain pending.
