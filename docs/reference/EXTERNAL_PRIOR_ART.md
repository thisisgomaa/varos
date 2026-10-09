> **Status:** current — owner decision 2026-10-09 («خليه معانا هو والفوتوشوب… ممكن نقتبس أو ناخدها عندنا»).

# External prior art: the ArtCraft "Crafting Apps" (VectorCraft, PhotoCraft)

Two agent-built, open-source Rust reimplementations surfaced by the owner on 2026-10-09. We keep local
clones as a **reference corpus** and may borrow ideas and, under the rules below, code.

| Repo | What | License | Local clone (outside this repo, never committed) | Pinned |
|---|---|---|---|---|
| https://github.com/storytold/vectorcraft | Illustrator-class vector editor, eframe/egui + `vello_cpu`, 21 crates, MCP + JSON control channel | MIT OR Apache-2.0 | `~/Documents/AI workspace/reference/artcraft/vectorcraft` | `a469568` (2026-10-09) |
| https://github.com/storytold/photocraft | Photoshop-class raster editor, 24 crates (gpu, raster, cms, codecs, raw, psd, tablet…) | Apache-2.0 | `~/Documents/AI workspace/reference/artcraft/photocraft` | `4cb7cf3` (2026-10-09) |

Siblings not cloned: `pdfcraft` (Acrobat-class), `craft-fonts`.

## License compatibility (why borrowing is allowed)

Varos is **GPL-3.0**. MIT and Apache-2.0 code may be incorporated into a GPL-3.0 work (Apache-2.0 is
one-way compatible with GPLv3; MIT is permissive). The reverse is not true, which is their problem, not
ours. Conditions we must meet for any copied or adapted code:

1. Keep their copyright line and license text: add the file's origin to `varos/NOTICE` (create it with the
   first borrow) and keep `LICENSE-MIT` / `LICENSE-APACHE` copies of theirs next to it.
2. Mark every borrowed or adapted block at the top of the file or function:
   `// Adapted from VectorCraft <path>@<commit> (MIT OR Apache-2.0), ArtCraft Team 2026.`
3. Their `docs/brand/` logos are trademarks, not open source: never copy them.
4. Their assets follow `ASSETS.md` rows; a borrowed asset keeps its row's author/source/license in our own
   provenance header (icons law stays: Lucide first, originals under `assets/icons/varos/`).
5. Never bring a whole crate in without an ADR (architecture laws: `varos-core` stays pure, no eframe, no
   CPU renderer promise). Prefer **ideas and algorithms** over wholesale files; our shell, tokens and box
   system are not negotiable.
6. Their code is agent-written and self-graded; treat it as a starting point to test, not as proven.
   Anything borrowed gets our headless tests before it lands.

## What they do that we should look at first (moderator read, 2026-10-09)

- **Stroke engine** (`vectorcraft/crates/geom`, `pathops`, `cmd/stroke.rs`): caps, joins, dashes, arrowheads,
  width profiles — our open piece B2.
- **Gradients** (`cmd/gradient.rs`, `render`): linear/radial/freeform + gradient mesh — our Gradient tab
  is disabled until an engine exists.
- **Pathfinder / Shape Builder** (`pathops`, `cmd/shaper.rs`): compare against our i_overlay results.
- **Blends, Repeat, Envelope Distort, Perspective grid** (`cmd/live.rs`, `distortcmds.rs`, `perspgrid.rs`):
  later, as live effects.
- **Command registry** (`crates/engine/src/cmd/*`, 80 modules): "every behaviour is a command" reachable
  from UI, control channel and MCP. Pattern worth adopting for the Bridge beyond API 1.1.
- **Robustness**: workspace no-panic clippy lints, `guard` at every entry point, import fuzzing,
  `ASSETS.md` + `cargo xtask assets`, a dated "honest assessment" in the roadmap.
- **PhotoCraft**: `gpu` and `raster` crates (effects pipeline), `cms` (colour management), `codecs`, `psd`,
  `tablet` (pen pressure) — relevant when Varos gets raster effects and tablet input.
- Not for us: eframe, CPU-only rendering, the 1:1 Illustrator chrome, 200 KB source files.

A per-feature survey with concrete file paths lives beside this page as it is produced
(`EXTERNAL_PRIOR_ART_SURVEY.md`).
