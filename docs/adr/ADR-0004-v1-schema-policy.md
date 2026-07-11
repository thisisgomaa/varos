> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# ADR-0004: V1 schema policy

- **Date:** 2026-07-11
- **Decision owner:** Product owner
- **Supersedes:** The unqualified introspectable "one schema" promise in `CLAUDE.md:10`
- **Superseded by:** None

## Context

Varos currently has one implemented persistence model, but not a general schema registry. `Document`, `Node`, and their nested model types are Rust structs with Serde implementations (`varos/crates/varos-core/src/model.rs:283-336`, `varos/crates/varos-core/src/model.rs:498-548`). `doc_to_blob` and `doc_from_blob` serialize that model through the versioned V1 wrapper (`varos/crates/varos-core/src/file.rs:11-39`). Compatibility for existing fields is handled locally through Serde defaults and selected custom serialization, not through an introspectable property system (`varos/crates/varos-core/src/model.rs:308-328`, `varos/crates/varos-core/src/model.rs:499-548`).

The charter requires the ADR to record this honest state instead of treating the older file + inspector + plugins/AI slogan as already implemented (`docs/foundation/FOUNDATION_CHARTER.md:84`).

## Decision

For V1, the persisted editable schema is the versioned JSON representation produced from the Serde model types owned by `varos-core`. Those Rust model types and their explicit Serde implementations are the source of truth for persisted document data.

V1 does not claim an introspectable property registry shared automatically by persistence, inspectors, plugins, and AI. Inspector bindings may consume the core model through reviewed application APIs, but they do not create a second persisted schema.

A discoverable plugin/AI schema, stable property identifiers, migrations, validation metadata, query contracts, and external compatibility guarantees are future work. They require a separately reviewed ADR and must not be inferred from the V1 Serde representation.

## Consequences

- Persisted fields must be defined and compatibility-reviewed in `varos-core`; UI-only state remains outside the `Document` schema.
- Current Serde defaults and custom encodings remain compatibility mechanisms, but this ADR does not expand them into a complete long-term format specification.
- `VRS_FORMAT.md` will document the formal V1 wire contract after hardening; until then, code and tests are the executable evidence.
- Documentation must distinguish the implemented persistence schema from the deferred extensibility schema.

## Status

Accepted — product owner, 2026-07-11.
