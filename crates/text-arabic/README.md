# vectorcraft-text-arabic

Kashida (tatweel) justification policy for justified right-to-left Arabic area type. The policy
comes from Varos' `varos-text`. The measuring and re-shaping happen in `crates/text/src/kashida.rs`,
using VectorCraft's own HarfRust shaper and the paragraph's own face.

## Which fonts take kashida

HarfRust shapes a font with its AAT `morx` (or `mort`) tables whenever the font has them, for
horizontal text, and then ignores the font's `GSUB` (as HarfBuzz does). That decides the path:

- **OpenType Arabic fonts (no `morx`).** A tatweel goes only where HarfRust marks a join *safe to
  insert tatweel* (its Arabic shaper sets the mark). Checked on macOS: SF Arabic, Noto Naskh
  Arabic, Noto Sans Arabic, Arial, Tahoma, Times New Roman.
- **`morx` fonts.** HarfRust's `morx` shaping never sets the mark, so the joining rules of
  `varos-text` propose the boundaries (between two joining letters, never inside لا, never next
  to harakat, never after a letter that doesn't join to the left), and every trial re-shape is
  verified in the same face before it is kept (`crates/text/src/kashida.rs`, `morx_trial_ok`):
  one more glyph per tatweel and each tatweel its own glyph; every authored glyph unchanged (glyph
  id, cluster, offsets: the joining forms and the marks survive); no `.notdef`; the word grows by
  the tatweels' advances within a font unit each. A `morx` font whose letters don't join takes
  none. Checked on macOS 2026-10-19 (Force 100, 24 pt, 300 pt Justify All, Al-Fatiha): Geeza Pro,
  Al Nile, Baghdad, Damascus, KufiStandardGK, DecoType Naskh, Farah, Diwan Kufi, Al Bayan and
  Nadeem all take kashida; with full harakat Geeza Pro takes none (cause not investigated), the
  others still take some.
- **Damascus has both `GSUB` (with the `arab` script) and `morx`.** HarfRust uses its `morx`, so
  it is on the `morx` path (before this, it took no kashida at all).
- Fonts with `morx` and no `GSUB` Arabic on that Mac (the font-menu lane's survey of 781 Arabic
  families): Al Bayan, Al Nile, Al Tarikh, Baghdad, Beirut, DecoType Naskh, Diwan Kufi, Diwan
  Thuluth, Farah, Farisi, Geeza Pro, KufiStandardGK, Mishafi, Mishafi Gold, Muna, Nadeem, Sana,
  Waseem.
- Known, outside this code: HarfRust's mark attachment in Al Nile and Farah puts the kasra of the
  Allah ligature (اللَّهِ) about 1.5 em off, with or without kashida; kashida never moves a mark
  (tested).

The face must also have its own U+0640 glyph. A tatweel is never borrowed from another font.

## Kashida controls

Each paragraph has a kashida setting (`ParaStyle::kashida` in `crates/doc/src/text.rs`): **Off**,
**Auto** (default) or **Force**, and a **strength** from 0 up (default 50). The Paragraph panel
shows them for right-to-left paragraphs: its slider spans 0–100 and its number field takes any
value from 0 up; `text.setFormat` takes `kashida` and `kashidaStrength`. Documents saved before the
setting load as Auto at 50, which is the old fixed Balanced policy, and a paragraph at the default
saves nothing new. The strength is stored as a whole number (`u32`): files with a strength of 100
or less read and write exactly as before.

| Mode | Justified line (Justify / Justify All) | Last line of a Justify Left/Center/Right paragraph | Line that isn't justified (Left, Right, Center, Auto; point type) |
|---|---|---|---|
| Off | none | none | none |
| Auto | `policy(strength, false)`: a share of the slack | none | none |
| Force | `policy(strength, true)`: a deeper share | `policy(strength, true)`, kashida only (its spaces stay) | elongated by **strength %** of its natural width |

**Justified lines** (owner decision 2026-10-17, unchanged): `policy(strength, force)` in `src/lib.rs`
maps 0..100 onto `varos-text`'s policies: 0 is Minimal (1 tatweel per word, a quarter of the
slack), 50 Balanced (2, 60 %), 100 Display (4, all of it), interpolated in between; above 100
reads as 100. Force adds one tatweel per word (at most 5) and 0.4 of the slack (at most all of it).

**Lines that aren't justified** (owner decision 2026-10-19: «الرقم يكون لا نهائي، ويكون شغال في أي
مود»): Force elongates the line by `strength` percent of its natural width (30 → about 30 % wider,
within one tatweel), never past the box's width (a line that already overflows doesn't grow; point
type has no box); `growth_policy()` allows up to 8 tatweels a word. The line keeps its alignment:
a right-aligned line stays on its right edge, a centred one stays centred. Auto does nothing there.

**Where the tatweels go** (both kinds of line, owner decision 2026-10-19): `plan_with` spreads them
across the words of the line, one more per word a round, the words in reading order (right to
left), each word's boundaries in `varos-text`'s priority order (after a seen, then a sad, then the
rest), so Force no longer spends the whole slack on the first words (it replaced `varos-text`'s
`plan`, which walks the boundaries in priority order across the whole line). Never in the name of
God: `sacred_from` keeps every tatweel out of الله, لله and اللهم with any proclitic (والله، بالله،
فلله، تالله…), out of the sacred ligatures U+FDF0..U+FDFD (ﷲ, ﷺ…), and out of a longer word's
trailing الله (عبدالله). The joining rules already keep them out of لا and away from harakat.
A line costs at most 256 trial re-shapes (`plan_with`'s cap).
