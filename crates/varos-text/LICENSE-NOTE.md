# Licence of `varos-text` (and the v1 cursor set)

Owner decision 2026-10-18: `varos-text`, `varos-app/assets/cursors/v1` and the original Varos icons in `varos-app/assets/icons/varos` are licensed
**MIT OR Apache-2.0** (dual, at the user's option) so they can be reused inside permissively
licensed projects, including the Arabic-first fork of VectorCraft that Varos is moving to.
The rest of this repository stays under its existing licence. The vendored, patched
`cosmic-text` keeps its own upstream licence (MIT OR Apache-2.0) — see `docs/VENDOR_PATCHES.md`.

## Code adapted from BStudio (2026-11-05)

Parts of `varos-text` adapt the owner's own earlier BStudio code, which was published under
MPL-2.0 — notably the directional-edge caret geometry in `src/engine_support.rs` (from BStudio
`caret.rs`) and the joining / mark / lam-alef guards described in `README.md`. The owner is the
sole author and copyright holder of that BStudio code and, on 2026-11-05, licensed those parts
under **MIT OR Apache-2.0** as part of `varos-text`. No MPL-2.0 obligation applies to this crate.
