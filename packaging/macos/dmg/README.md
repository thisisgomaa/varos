# Varos temporary DMG background

Original geometric V, matching the temporary app icon. No fonts or third-party
artwork. MIT OR Apache-2.0. Raster background includes 660x400 at 72 dpi and
1320x800 at 144 dpi. The light area keeps Finder icon labels readable.

Regenerate with `python3 crates/ui-egui/src/varos/generate_icons.py`.
The upstream `DS_Store` and `generate.py` are retained as historical source,
but packaging no longer copies their old bundle/volume-specific layout.
Finder uses its default layout; a custom Varos DMG layout is a later task.
