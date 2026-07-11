# ADR-0002: Two-level command boundary

- **Date:** 2026-07-11
- **Decision owner:** Product owner
- **Supersedes:** None
- **Superseded by:** None

## Context

The current UI collects a private `Op` enum and dispatches it inside `apply_ops` (`varos/crates/varos-app/src/ui.rs:91-145`, `varos/crates/varos-app/src/ui.rs:5372-5454`). Some branches directly mutate document state, including ruler origin, snapping, and stroke width (`varos/crates/varos-app/src/ui.rs:5437-5463`). This couples UI rendering and interaction code to edit semantics, history behavior, and model mutation.

The accepted foundation design resolves the earlier core-versus-app question with two boundaries, not one (`docs/foundation/FOUNDATION_CHARTER.md:62`, `docs/foundation/FOUNDATION_CHARTER.md:81`, `docs/foundation/FOUNDATION_CHARTER.md:127`).

## Decision

Varos will use two command levels:

1. `EditCommand` lives in `varos-core`. It represents deterministic `Document`/`Editor` edits, owns their history and undo semantics, and can execute without UI, window, renderer, or file-dialog dependencies.
2. `AppCommand` lives in `varos-app`. It represents application effects such as New, Open, Save, Export, panel changes, window operations, and dialogs. When an application command performs an edit, it delegates that edit to `EditCommand`.

UI code may read view state, but after F4 it may not directly mutate `Document` or `Editor`; edits flow through `EditCommand`, and application effects flow through `AppCommand`.

These closed command enums are internal boundaries, not the plugin or AI API. Plugin/AI adapters, command versioning, query contracts, permissions, and compatibility are deferred and require later decisions.

## Consequences

- F3 characterization tests must pin current dispatch, shortcut/menu, gesture, history, and round-trip behavior before F4 changes production dispatch.
- Headless callers gain one deterministic edit path without importing the app crate.
- File, dialog, window, and panel side effects remain out of `varos-core`.
- F4 is complete only when the zero-direct-mutation gate in the charter is measurable and green (`docs/foundation/FOUNDATION_CHARTER.md:62`).
- Future plugin or AI work must use explicit adapters instead of treating either enum as a stable public protocol.

## Status

Proposed.
