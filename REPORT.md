# Lane G — accessibility / visible updates / crash viewer
Implemented (provisional UI, owner design review pending); fix changes are uncommitted.
9.9: AccessKit 0.24.1 semantics + offline objc2 macOS NSAccessibility adapter; accesskit_winit unavailable.
9.7: signed manual update checks, explicit browser Download, bounded read-only crash viewer/folder actions.
Configure HTTPS update_manifest_url + base64 Ed25519 update_public_key in settings.json; no endpoint/key bundled.
API 1.2 release verbs use list_verbs/schema progressive disclosure + CLI; no format/writer or token changes.
Implementation notes: docs/reference/LANE_G_ACCESSIBILITY_UPDATES.md.

## Fix round
Accepted all five reviewer findings; no disagreements or additional low findings were listed.
P1 repaint: explicit immediate-request signal; delayed tooltip/caret deadlines stay with gui.repaint_at.
P1 Home accessibility: drain before mode dispatch; shared publication replaces trees in Home/document/presentation.
P2 release sheets: shared frame draws/processes updates and crash sheets in all three modes.
P2 native bounds: preserve top-left coordinates in winit's flipped NSView; AppKit converts to screen space.
P2 selection: transfer AXSelected for rows/tabs and numeric radio/tab AXValue ahead of count text.
Seven new headless regressions pass: repaint scheduling, mode tree/activation, completion/sheets, bounds and selection.
PASS: fmt, dependency directions, native + Windows Clippy (-D warnings), shell ratchets, Bridge fixtures/ratchets.
PASS workspace + doctests: 2273 passed, 0 failed, 15 ignored; API 1.2 tools/list 23,879/24,000 B; API 1.0/1.1 byte-frozen; ui.rs 822/843 lines.
Evidence: /var/folders/bq/2st8236n5xn501l970c3ny0w0000gn/T/w3-a11y-fix-f9adef0-qnjh2cwd (workspace.log + gate logs).
No git writes, merge, GUI launch, install or live fetch; native VoiceOver/heat, owner review and independent re-review pending.
