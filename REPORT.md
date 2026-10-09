# Lane F — Phase 9 Preferences, registry, shortcuts, History, Actions, Help/Quick Look
Branch feat/w2-app; resumed WIP 48830d0 on base b3d39ee; completion remains uncommitted.
9.1: typed validated Preferences draft, pure v1→v2 migration, additive keys, evidence-preserving repair/reset.
Worker publication serializes legacy toggles/preferences/shortcuts; generation and bounded source checks, explicit reconciliation.
Effective settings publish only after confirmed durability; failures preserve effective state and draft.
Keys: keyboard_increment_pt, default_units, gpu_preference, history_depth, language, canvas_colour; existing toggle keys retained.
Requested GPU applies at launch; actual adapter displayed; unavailable language identifiers retain English fallback.
9.2: registry projects incumbent menu/parity tables with canonical IDs/aliases and enabled/reason; ADR-0015 remains proposed.
Native/burger rows and accelerators use registry; typed API 1.2 application/history/actions/help schemas and discovery wired.
9.3: searchable shortcut draft, full physical-key rebind, conflict detection, unbind/reset, additive durable overrides.
Held Space remains a fixed gesture, explicitly disabled in the editor; unknown override IDs survive.
9.4: real dockable History panel, labelled human/agent steps, semantic counts/verbs, retained-state jump and top-agent undo.
History metadata stays session-only; cap/undo/redo/branch/replace discipline tested.
9.5: real Actions panel/Home sheet; successful supported committed commands record; no-ops omitted, unsupported edits abort visibly.
Version-1 .vrs-actions load/save and atomic explicit-selection replay; CLI apply verified as a real process.
9.6: Help/docs/shortcut list/crash-folder commands; CPU thumbnail cache embedded into .vrs PDF preview.
Pure migration embed_preview_next; optional PDF catalog VAROS_Preview/VAROS_PreviewVersion=1; editable format version unchanged.
Quick Look signed extension packaging deferred as permitted; instructions provided; Finder integration untested/uninstalled.
Tests: settings bounds/migration/frozen refusals/write faults/external replacement; shortcut persistence/conflicts/menu registry.
Tests: two-agent History/idempotence/frozen list, 10k counts summary (~10 ms debug sample), Actions/CLI, Help/preview refusals.
PASS: fmt, dependency directions, workspace tests (1916 passed / 0 failed / 15 ignored), native + Windows clippy -D warnings.
PASS: app ratchets 3/3, Bridge ratchets 6/6; tools/list 1.0/1.1=23,993 B frozen, 1.2=23,671 B ≤24,000.
Evidence: /tmp/w2-complete-{tests,clippy-native,clippy-windows}.log; /tmp/w2-final-ratchet.log.
Ratchets/legacy Bridge fixtures unchanged; ui.rs 772 lines (cap 843); no new dependency or prior-art lift.
See docs/foundation/work_orders/PHASE9_IMPLEMENTATION.md for behavior and migration details.
PLAN marks 9.1–9.6 implemented (provisional UI, owner design review pending).
No commit/push/merge/GUI/install; independent review, moderator integration and owner native acceptance pending.
