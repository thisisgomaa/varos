# Lane G — accessibility / visible updates / crash viewer
Branch: `feat/w3-a11y-updates`; worktree-only, uncommitted.
Status: implemented (provisional UI, owner design review pending).
9.9: cached AccessKit 0.24.1 + in-house objc2 NSAccessibility adapter on winit's macOS NSView.
`accesskit_winit` unavailable offline; no download attempted.
Kit emits roles/names/values for controls, fields, rows, menus, panels, headings and static chips.
Native press/focus returns through egui; traversal and logical Arabic varos-text label sources tested.
9.7: Varos ▸ Check for Updates…; strict Ed25519 manifest verification and SemVer precedence.
Manual worker fetch via system curl; explicit Download opens the browser; no installer/auto-install.
Help ▸ Show Crash Logs / View Crash Report…; bounded read-only provisional viewer.
Configure `update_manifest_url` (HTTPS) + `update_public_key` (base64 Ed25519) in settings.json.
No production endpoint/key bundled; absent settings produce a visible error.
API 1.2 `release`: check/download/crash_logs/crash_report/close/verify_manifest.
Detailed discovery only (`list_verbs` + `schema`); CLI `bridge release` and offline `verify-update`.
New modules: `varos-app/src/{release_ui,accessibility_macos}.rs`, `varos-app/src/shell/accessibility.rs`;
`varos-bridge/src/release.rs`, `varos-cli/src/release.rs`; shared wiring uses Lane G blocks.
No document format/writer change or migration; no prior-art lifts; tokens/ratchet limits unchanged.
Gates PASS: fmt check; dependency directions; native + x86_64-pc-windows-msvc clippy (-D warnings).
Gates PASS: shell ratchets; Bridge fixtures/cap; varos-text wasm32 check; native numeric-value test.
Full offline workspace tests + doctests PASS: 2265 passed, 15 ignored; new native-value test also PASS.
API 1.2 tools/list: 23,879 / 24,000 B; 121 B headroom; list_verbs: 15,788 B.
ui.rs: 825 / 843 lines. API 1.0/1.1 fixture bytes unchanged.
PLAN Progress updated; details: docs/reference/LANE_G_ACCESSIBILITY_UPDATES.md.
Pending: owner design review, installed-app VoiceOver/keyboard test and independent integration review.
No commit, push, GUI launch, install, merge or live manifest fetch performed.
Evidence: /tmp/lane-g-gates-tests.log; /tmp/lane-g-native-last.log; /tmp/lane-g-windows-complete.log.
Evidence: /tmp/lane-g-number-test.log; /tmp/lane-g-wasm.log.
