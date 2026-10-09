# COSMIC patch base

cosmic-text 0.19.0; MIT OR Apache-2.0 (notices retained).
Pristine .crate SHA-256: `be17b688510d934ce13f48a2beba700e11583e281e0fda99c22bb256a14eda73`.
Pristine directory digest: `f7787ee3330565390213ce1f4e76d6425daae3876c24bd0df375fa8f575bb548`.
Directory digest = SHA-256 of sorted UTF-8 `sha256  relative-path\n` entries in pristine.sha256, including registry metadata.
Promoted in T1; maintenance owner and independent review remain pending. Removal: upstream release containing fixes or owner-reviewed replacement. See docs/VENDOR_PATCHES.md.

Vendor file-set exclusions: registry metadata `.cargo-ok` / `.cargo_vcs_info.json`,
`Cargo.lock` (workspace lock is authoritative), and `.gitattributes` (nested LFS
filters would replace bundled TTF bytes with pointers). Pristine hashes retain
these upstream files; the patch and pristine ledger identities are unchanged.

Lane T (2026-10-09): apply `cosmic-text-0.19.0-tatweel.patch` AFTER the frozen
P1b patch. Requests HarfRust PRODUCE_SAFE_TO_INSERT_TATWEEL, retains the flag in
ShapeGlyph (also cloned by shape-run cache), false for basic unshaped glyphs.
No extra shaper or layout policy in vendor. `check_vendor_patches.py` pins both
patch hashes and reconstructs the complete vendor tree. Remove this supplemental
patch when upstream exposes the same flag. Maintenance/rebase review remains
with the moderator lane; independent review and owner proof review still open.
