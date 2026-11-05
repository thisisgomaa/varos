# Varos app icon

Varos app icon — original work, © 2026 the owner, MIT OR Apache-2.0

Source of truth: the owner-supplied installed app's
`/Applications/Varos.app/Contents/Resources/Varos.icns` (read-only).
`vectorcraft.icns` is a byte-for-byte copy. All other icon files are raster
renders of that icon, preserving its transparent canvas and original inset.
The SVG files are self-contained PNG wrappers, not invented vector artwork.
The in-window title bar, Home and About use the cached 128 px PNG (at least 2×
the largest drawn size, 44 pt). No engraved dragon is drawn.

Existing paths/names stay compatible with upstream consumers. The macOS bundle
expects `Contents/Resources/VectorCraft.icns`: `CFBundleIconFile` is `VectorCraft`
and `package.sh` copies `vectorcraft.icns` to that exact name.
`CFBundleIconName` is absent; there is no asset catalog override.
The 512 px runtime macOS icon preserves the source's inset without adding another.

Regenerate from the installed app:
`python3 crates/ui-egui/src/varos/generate_icons.py`

Regenerate from the checked-in original (no installed app needed):
`packaging/icons.sh`

Requires Python 3 and Pillow. Writes only inside this worktree, including the
DMG backdrop containing the same icon. See LICENSE.txt, LICENSE-MIT and LICENSE-APACHE.
