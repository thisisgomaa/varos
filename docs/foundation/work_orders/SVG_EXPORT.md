> Status: pure implementation and headless gates complete, 2026-10-06; uncommitted, UI excluded.

# Pure SVG export

Add dependency-free `varos_core::svg`: plan first; one standalone SVG 1.1 per visible artboard,
or transparent whole-board artwork bounds (`--all` equivalent, including floaters). Return bytes;
file naming and disk I/O belong to the host. No import or UI changes.

Mirror PDF transforms, paint order, visibility, round strokes, even-odd rings and nearest clip runs.
Preserve names using stable XML ids and titles; round numbers to three decimal places. Hidden mask
sources still define clips. SVG knockout uses a luminance mask to remove fill beneath stroke.

Opacity follows the scene: object opacity below 0.999 isolates fill/stroke before fading; otherwise
translucent strokes knock out fill. Existing PDF differs: knockout takes precedence over isolation
there, even at low object opacity. Changing PDF is outside this task.

Existing Export sheet is PDF-only, without a format choice. A new choice and multi-file destination
flow need visible design; leave command/menu/FileJob wiring out under the owner's design-first rule.

Acceptance: golden SVGs from frozen v1/v2/v3 fixtures; all accepted clip fixtures export and rasterize;
resvg (dev-only) comparison against existing CPU raster; geometry/scope/visibility/error tests;
workspace tests, macOS clippy, fmt and Windows-target clippy. No Renderer or EventLoop in tests.

## API and boundaries

The immutable snapshot is `Document` (the persisted Board model). Call
`plan_svg_export(&doc, default_scope(&doc))`, then
`export_svg_files(&doc, &plan, &AtomicBool::new(false))`. The result is `Vec<SvgFile>`;
each carries its `PageSpec` and UTF-8 bytes. Use `ExportScope::WholeBoard` for a single transparent
artwork-bounds export, including floaters; `ActiveArtboard` for only the active page. No CLI parser
is added in this task: a future headless host maps `--all` to `WholeBoard`.

The host must choose safe destination names and write atomically. The library does not derive
filenames from untrusted names. XML ids have kind + model id + encoded name; titles preserve Unicode.
Masks contain one compound path, because separate SVG clipPath children union rather than even-odd
XOR. Container wrappers preserve names/ordering, but do not invent group opacity (nodes have none).
Nearest clip semantics match the existing canvas/PDF; this does not introduce recursive mask support.
Invalid tree/numbers/overflowing paint bounds are refused before traversal or byte output.

Test raster comparisons use the existing CPU `draw_groups` (the real thumbnail raster), excluding
editing overlays and thumbnail fit/grid. At 256×256, mean channel error must stay below 0.25 of 255
and fewer than 0.5% of pixels may differ by summed RGBA error >32. Twelve cases cover five frozen
fixtures plus opaque paint, isolated opacity, translucent/zero-alpha knockout, rotated cubic compound
paths and clipping. Largest observed mean error: 0.0092; largest differing-pixel fraction: 0.0002.
Golden coverage: 13 raw inputs → 17 files; accepted JSON/PDF corpus: 25 containers, both default and
whole-board scopes. Additional tests cover hidden/locked names, open fills, exact closing cubics,
empty masks, nearest nested clip runs, cancellation and malformed input. Original fixtures unchanged.

## Import considerations — 2026-10-06

- Rotation is baked into world-space coordinates; consider `transform="rotate()"` for
  future export/import that preserves editable rotation.
- Container groups split at nearest clip-run boundaries; only the first wrapper retains
  the model id/name. Import cannot reconstruct the original group tree from wrappers alone.
- Mask source geometry is anonymous inside the compound clip path; source ids/names are lost.
- Anchor smooth flags are dropped; cubic handles survive, but future import must infer
  smoothness or use explicit metadata.

SVG viewport width/height are unitless and numerically match the viewBox extent (Varos
1 pt = 1 px at 72 ppi). RGB paint uses `#rrggbb` with separate alpha attributes. Id names
retain XML-safe Unicode letters/digits and escape other characters, including underscores,
as `_xHH_` to preserve injectivity. Numbers quantize to 1e-4 before three-decimal formatting
to stabilize rotated goldens near decimal midpoints across platform sin/cos implementations.
The open PDF knockout difference is tracked in [PAINS_LOG](../../PAINS_LOG.md).

## Final local verification — 2026-10-06

Implementation and independent-review fixes validated, uncommitted; UI intentionally excluded.
All required commands run from `varos/` on the final source tree:

- `cargo test --workspace -j 4`: PASS, 1106 passed / 0 failed / 8 ignored (19 added tests, including three review regressions).
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS.
- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings`: PASS.
- `python3 tools/check_dep_directions.py` from repository root: PASS; normal core dependency tree
  contains no resvg, roxmltree, GPU/window/UI dependencies. resvg/roxmltree additions are dev-only.
- `git diff --check`: PASS; original frozen fixture hashes checked by the workspace suite.

Local logs: `varos/target/svg-export-review-gates/{tests,clippy-mac,clippy-windows,fmt,raster}.log` (ignored build output).
No Renderer/EventLoop in new tests. No GUI launch, owner hand test, Windows runtime test, install,
commit, push or merge. File destination handling and the designed format choice remain host work.
