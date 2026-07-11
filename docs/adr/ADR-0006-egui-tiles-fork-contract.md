# ADR-0006: Vendored egui_tiles patch contract

- **Date:** 2026-07-11
- **Decision owner:** Product owner
- **Supersedes:** The stale "only edit" manifest comment at `varos/Cargo.toml:10-13`
- **Superseded by:** None

## Context

Varos patches `egui_tiles` 0.16.0 through the workspace manifest (`varos/Cargo.toml:10-15`). F1 compared the vendor tree against upstream VCS SHA `62ac74717ebe284749a0066adf9566bbbab9ee42` and found five behaviorally modified source files, not the single narrow hook claimed by the manifest comment (`docs/foundation/DEPENDENCY_MAP.md:98-115`):

- `src/behavior.rs`
- `src/container/linear.rs`
- `src/container/tabs.rs`
- `src/lib.rs`
- `src/tree.rs`

Application use is intentionally localized to `varos-app/src/shell/boxtree.rs` (`varos/crates/varos-app/src/lib.rs:4-7`, `varos/crates/varos-app/src/shell/boxtree.rs:1-22`).

## Decision

The vendored fork is based on `egui_tiles` 0.16.0 at upstream SHA `62ac74717ebe284749a0066adf9566bbbab9ee42`. Its supported Varos delta is exactly the five source files listed above and the behavior ledger in `docs/foundation/DEPENDENCY_MAP.md:98-115`.

F2a.4 will promote that ledger into `VENDOR_PATCHES.md`, document the rebase procedure, and add a machine check that compares the vendor tree with the recorded upstream base. Direct use of the fork remains confined to `shell/boxtree.rs`.

## Consequences

- A fork update is reviewable as an upstream-base change plus an explicit Varos patch delta.
- An undocumented sixth modified source file, a missing documented patch, or use outside `shell/boxtree.rs` fails the vendor contract.
- Replacing, rebasing, or removing the fork must preserve the behavior described in the patch ledger or explicitly supersede this ADR.
- The stale manifest comment must be corrected in the authorized vendor-contract work order; F2a.1 does not modify source or manifests.

## Status

Accepted — product owner, 2026-07-11.
