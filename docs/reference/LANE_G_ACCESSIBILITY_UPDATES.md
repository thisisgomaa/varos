# Lane G — accessibility, updates and crash reports

Implemented UI is provisional; owner design review and installed-app VoiceOver testing are pending.
No document writer or format version changes. No code lifted from prior art.

The offline cache contains AccessKit 0.24.1 but no accesskit_winit. egui emits AccessKit nodes;
a minimal objc2 NSAccessibilityElement adapter attaches the hierarchy to winit's NSView on macOS.
Kit roles cover buttons, rail tools, toggles, text/numeric fields, dropdowns, menus, rows, panels,
headings and static chips. Existing keyboard traversal remains authoritative. Native press/focus
requests become egui AccessKit actions. Source labels retain logical Unicode order; Arabic labels
from varos-text LabelCache are tested without reversing shaped glyphs. Arabic translation/catalog
rollout remains Phase 9.8. Windows keeps compiling; this lane's native accessibility adapter is macOS only.

Varos ▸ Check for Updates… is manual. Help ▸ Show Crash Logs opens `<app-data>/Logs`;
Help ▸ View Crash Report… reads `Logs/crash.txt` into a bounded, read-only kit sheet.
Checks run on a worker thread and wake the existing Wait loop on completion.

Configure two additive keys in the existing `settings.json` (preserved by Preferences saves):
`update_manifest_url`: HTTPS manifest URL; `update_public_key`: base64 Ed25519 public key.
Neither a production endpoint nor a production key is bundled; absent settings produce a visible error.
The envelope is JSON `{"payload":"<JSON string>","signature":"<base64 signature>"}`.
Sign the exact UTF-8 payload bytes. Payload keys: `version` (SemVer), `notes`, `download_url` (HTTPS).
Signature validation is strict; malformed/oversized manifests and untrusted keys fail closed.
Transport uses system curl, HTTPS only, no redirects, 10 s connect/30 s total timeout, 256 KiB cap.
Only a newer verified version offers Download. Clicking it opens the browser; no installer exists.

API 1.2: discover `release` using `list_verbs`, then `schema {api:"1.2",tool:"release"}`.
Actions: check, download, crash_logs, crash_report, close, verify_manifest.
The tool is excluded from the capped inline tools/list; API 1.0/1.1 remain fixture-frozen.
Attached CLI: `varos-cli bridge release` with JSON arguments on stdin.
Offline CLI: `varos-cli verify-update <manifest.json> <base64-key> <current-semver>`.
