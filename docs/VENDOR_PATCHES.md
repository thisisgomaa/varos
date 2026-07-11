> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Vendored dependency patch contract

This document is the durable contract for Varos' local `egui_tiles` fork. [ADR-0006](adr/ADR-0006-egui-tiles-fork-contract.md) is the decision authority; this file promotes the verified F1 ledger in [DEPENDENCY_MAP.md](foundation/DEPENDENCY_MAP.md) into an operational identity, patch list, checker, and rebase procedure.

## Upstream identity

| Property | Contract value | Evidence |
|---|---|---|
| Package | crates.io `egui_tiles 0.16.0` | `varos/vendor/egui_tiles/Cargo.toml:12-17` |
| Upstream VCS commit | `62ac74717ebe284749a0066adf9566bbbab9ee42` | Packaged `.cargo_vcs_info.json`; independently recorded in `docs/foundation/DEPENDENCY_MAP.md:101` |
| crates.io archive SHA-256 | `9EB8FEF6130BD04FCB7BB3584845605E57C56FED249BC3CA5A568E696CC0A174` | `Get-FileHash -Algorithm SHA256 egui_tiles-0.16.0.crate`, verified 2026-07-11 |
| Vendored location | `varos/vendor/egui_tiles` | Workspace patch at `varos/Cargo.toml:14-15` |

The crates.io extraction may contain `.cargo-ok`, `.cargo_vcs_info.json`, and `Cargo.toml.orig`. They are registry/package metadata, are absent from the vendored tree, and are excluded from content-delta counting. After those exclusions, upstream and vendor must have the same 17-file set.

## Machine check

From the repository root on Windows:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/check_vendor_patches.ps1
```

The checker locates the immutable `.crate` archive in Cargo's cache or downloads it from crates.io when absent, verifies the archive SHA-256, extracts it to the system temporary directory, verifies the packaged VCS SHA, normalizes line endings, compares the complete file set, and exits non-zero unless the modified-file set is exactly the five files below. An audited extracted tree can be supplied with `-UpstreamPath <path>`; its package version and VCS SHA are still checked.

Verified output on 2026-07-11:

```text
check_vendor_patches: PASS
upstream: egui_tiles 0.16.0 (62ac74717ebe284749a0066adf9566bbbab9ee42)
archive SHA-256: 9EB8FEF6130BD04FCB7BB3584845605E57C56FED249BC3CA5A568E696CC0A174
comparable files: 17; modified files: 5
```

## Patch ledger

| Vendored file | Delta from upstream | Required Varos behavior | Evidence |
|---|---|---|---|
| `src/behavior.rs` | `paint_drag_preview` receives `DropPreview`; adds pane edge/tab target hooks. | The canvas can reject tab targets independently from edge docking, and Varos can paint direction-aware previews. | `varos/vendor/egui_tiles/src/behavior.rs:389-451`; consumer `varos/crates/varos-app/src/shell/boxtree.rs:680-729` |
| `src/container/linear.rs` | Disables between-child drop zones and removes `linear_drop_zones`. | Docking targets an individual box edge instead of an ambiguous whole-container seam. | `varos/vendor/egui_tiles/src/container/linear.rs:244-246,315-316,473-475`; upstream diff ledger `docs/foundation/DEPENDENCY_MAP.md:108` |
| `src/container/tabs.rs` | Returns before upstream tab-bar layout when configured height is below one pixel. | Varos can draw its own chip tabs without Epaint receiving a zero pixel scale. | `varos/vendor/egui_tiles/src/container/tabs.rs:225-232` |
| `src/lib.rs` | Adds `DropSide`/`DropPreview` and direction-aware target selection with pane opt-outs. | Implements the edge-dock/center-tab model consumed by the shell. | `varos/vendor/egui_tiles/src/lib.rs:223-258,374-433`; consumer `varos/crates/varos-app/src/shell/boxtree.rs:680-729` |
| `src/tree.rs` | Carries target side and neighbor geometry into previews; adds neighbor lookup and a short post-drop glide. | Adjacent boxes preview and move coherently when a dock operation changes layout. | `varos/vendor/egui_tiles/src/tree.rs:334-337,467-590,856-858`; consumer `varos/crates/varos-app/src/shell/boxtree.rs:98-100,699-729` |

## Isolation contract

Direct application use of `egui_tiles` is confined to `varos/crates/varos-app/src/shell/boxtree.rs`; the crate boundary documents the same rule at `varos/crates/varos-app/src/lib.rs:4-7`. Other application modules use Varos' shell API and must not import the fork directly. The comparison script protects the upstream delta; normal review and the dependency-direction gate protect this consumer boundary.

## Rebase or replacement procedure

1. Treat a new upstream version or VCS base as an architecture decision. Accept a superseding ADR before changing the identity constants in this contract or checker.
2. Acquire the exact crates.io archive, record and independently verify its SHA-256 and packaged VCS SHA, and retain the command output in the gate review.
3. Start from that clean upstream tree and reapply only the behaviors in the five ledger rows. Do not copy an old vendor tree over the new package.
4. Run `tools/check_vendor_patches.ps1`. Any sixth modified file, missing expected patch, or file-set difference is a failed contract, not an automatic documentation update.
5. Review the full upstream diff and confirm that direct use remains isolated to `shell/boxtree.rs`.
6. Run `cargo fmt --all -- --check`, `cargo test --workspace -j 4`, and `cargo clippy --workspace --all-targets -j 4 -- -D warnings` before gate review.
