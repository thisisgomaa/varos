> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# ADR-0005: Enforced crate dependency directions

- **Date:** 2026-07-11
- **Decision owner:** Product owner
- **Supersedes:** Informal dependency comments that are not machine-enforced
- **Superseded by:** None

## Context

The workspace contains four first-party crates plus a vendored `egui_tiles` fork (`varos/Cargo.toml:3-15`). F1 verified the current dependency graph and the allowed and forbidden directions in `docs/foundation/DEPENDENCY_MAP.md:6-40`. The current manifests show `varos-render-wgpu -> varos-core`, `varos-pdf -> varos-core`, and `varos-app -> varos-core + varos-render-wgpu + varos-pdf` (`varos/crates/varos-render-wgpu/Cargo.toml:8-13`, `varos/crates/varos-pdf/Cargo.toml:10-13`, `varos/crates/varos-app/Cargo.toml:11-14`).

Today those directions are convention, not an automated gate. The charter schedules an enforcement check immediately after this ADR is accepted (`docs/foundation/FOUNDATION_CHARTER.md:61`, `docs/foundation/FOUNDATION_CHARTER.md:80`).

## Decision

The workspace remains four crates plus the vendored fork. Allowed first-party crate edges are:

| From | May depend on |
|---|---|
| `varos-core` | No other Varos crate |
| `varos-render-wgpu` | `varos-core` |
| `varos-pdf` | `varos-core` |
| `varos-app` | `varos-core`, `varos-render-wgpu`, `varos-pdf`, and the vendored `egui_tiles` package |

All reverse, renderer-to-PDF, and PDF-to-renderer edges are forbidden. `varos-core` remains headless and free of UI, GPU, windowing, persistence-container, and platform dependencies. `varos-render-wgpu` does not own Winit or application event-loop policy. `varos-pdf` does not import application or renderer behavior.

Inside `varos-app`, direct `egui_tiles` use remains confined to `shell/boxtree.rs`, as recorded by F1 (`docs/foundation/DEPENDENCY_MAP.md:28-40`).

## Consequences

- F2c adds a CI assertion for these edges and demonstrates that a wrong-edge patch fails before the checker is accepted.
- New cross-crate responsibilities must follow the allowed graph; a fifth crate or a new edge requires a superseding ADR.
- External dependency additions remain subject to normal review; this ADR controls architectural direction, not a permanent package allowlist.
- The command boundary in ADR-0002 must preserve core's headless position.

## Status

Accepted — product owner, 2026-07-11.
