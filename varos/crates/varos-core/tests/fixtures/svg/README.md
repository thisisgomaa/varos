# SVG export golden files

Captured 2026-10-06 from the existing frozen raw v1/v2/v3 `.vrs` fixtures after their normal migration.
The source fixtures are untouched. Each `NAME-N.svg` is page N of the default export scope (visible
artboards, or whole-board bounds for boardless documents). Core tests compare bytes, validate XML
and unique ids, and rasterize with resvg. PDF tests load the frozen binary twins and compare against
the same SVG snapshots; all 25 raw/binary accepted containers also export whole-board bounds.

Intentional snapshot updates only:
`VAROS_BLESS_SVG_FIXTURES=1 cargo test -p varos-core --test svg_export frozen_raw_fixtures`

Pixel comparisons against the app's existing CPU scene raster live in `thumbs/svg_tests.rs`.
