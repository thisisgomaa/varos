# vectorcraft-text-arabic

Kashida (tatweel) justification policy for justified right-to-left Arabic area type. The policy
comes from Varos' `varos-text`. The measuring and re-shaping happen in `crates/text/src/kashida.rs`,
using VectorCraft's own HarfRust shaper and the paragraph's own face.

## Which fonts take kashida

A tatweel goes only where HarfRust marks a join *safe to insert tatweel*. HarfRust's Arabic
OpenType shaper sets that mark. Fonts that HarfRust shapes with AAT `morx` tables go through its
simple shaper instead, which never sets the mark, so those fonts get no kashida. Lines in those
fonts are still justified, with word spacing only.

Checked on macOS (2026-10-19, 12 pt, a 120 pt justified frame):

- Kashida works with these OpenType (GSUB) Arabic fonts: SF Arabic, Noto Naskh Arabic, Noto Sans
  Arabic, Arial (the system fallback under the default Source Sans 3), Tahoma, Times New Roman.
- No kashida with these AAT (`morx`) fonts: Geeza Pro, Al Nile, Baghdad, Damascus, KufiStandardGK.

The face must also have its own U+0640 glyph. A tatweel is never borrowed from another font.

## Later: kashida controls

The strength is the fixed `POLICY` (Balanced) in `src/lib.rs`. On/off buttons and a strength
slider would become a `ParaStyle` attribute (`crates/doc/src/text.rs`) read in place of `POLICY`
by `stretch_line`, and would be set from the Paragraph panel
(`crates/ui-egui/src/panels/paragraph.rs`, next to the justification buttons).
