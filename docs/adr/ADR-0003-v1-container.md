# ADR-0003: V1 document container

- **Date:** 2026-07-11
- **Decision owner:** Product owner
- **Supersedes:** Earlier `.varos` extension proposals in historical documents
- **Superseded by:** None

## Context

The product owner selected `.vrs` as the official V1 extension and PDF plus embedded JSON as the V1 container (`docs/foundation/FOUNDATION_CHARTER.md:119-120`). The current persistence path writes a valid PDF and embeds the editable, versioned model blob (`varos/crates/varos-pdf/src/lib.rs:1-16`, `varos/crates/varos-pdf/src/lib.rs:252-266`). The blob wrapper carries `VRS_VERSION` and the `Document` (`varos/crates/varos-core/src/file.rs:11-23`). The loader also recognizes legacy raw-JSON `.vrs` files (`varos/crates/varos-pdf/src/lib.rs:29-42`).

## Decision

The Varos V1 document extension is `.vrs`. A V1 `.vrs` file is a PDF container with the editable Varos model embedded as versioned JSON.

The PDF representation and the editable model serve different purposes: ordinary PDF readers can consume the rendered pages, while Varos recovers its model from the embedded payload. The existing raw-JSON `.vrs` load path remains a legacy compatibility input; new V1 saves use the PDF container.

This is a V1 decision, not an eternal container promise. The detailed compatibility and hostile-input contract will be locked in `VRS_FORMAT.md` after the scheduled hardening work, as required by the charter (`docs/foundation/FOUNDATION_CHARTER.md:120`).

## Consequences

- Persistence code keeps PDF semantics in `varos-pdf` and model serialization in `varos-core`.
- Historical `.varos` references remain historical; current documentation and new files use `.vrs`.
- Container changes after V1 require explicit migration and a superseding decision rather than silently changing `.vrs` meaning.
- This ADR does not claim that every PDF editor can preserve the embedded Varos model after modifying the file.

## Status

Accepted — product owner, 2026-07-11.
