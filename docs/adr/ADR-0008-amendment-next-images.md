> **Integration w2 (2026-10-09):** the image writer is final format **6**. The named migration is
> `migrate_v5_to_v6` (identity; formerly `migrate_v5_to_next_images`). Gradients took 7, text 8,
> Live Corners + preview 9; the `v6-images` fixtures stay stamped 6 (they load as migrated and re-save
> byte-identical apart from the stamp), the refused-future fixture is 10. See VRS_FORMAT "Wave-2 formats 6–9".

# ADR-0008 amendment: next image writer (proposed)

Image lane implements the proposed ADR-0014. Version **6 is provisional**: the integrator assigns the actual next writer version in merge order, before gradients/text. Existing v1–v5 fixtures and Bridge 1.0/1.1 wire fixtures remain frozen.

Pure migration `migrate_v5_to_next_images` changes the envelope version and leaves old documents unchanged. Old documents omit default image fields. New metadata keys on Document: `images`, `assets`, `raster_effects_ppi`; NodeKind adds `Image(id)`. Image metadata stores `id`, `blob`, `px_w`, `px_h`, `ppi`, `xform`, `opacity`, `placement`, `link`, `replacement`. Asset metadata stores immutable identity, MIME, dimensions, encoded/proxy lengths. Link metadata distinguishes absolute, home-relative, document-relative, accepted mtime/size/hash. Affine is `a,b,c,d,e,f`.

Raster bytes never enter Document, paths, nodes, history snapshots or JSON. The PDF container references bounded original/proxy binary streams by asset key. Embedded originals are mandatory; links persist accepted proxies and may hydrate matching originals by hash on open. The session sibling store holds immutable Arc resources shared by snapshots/clipboard/jobs. Dropped history resources retire without touching staged batch keys.

Frozen `varos-core/tests/fixtures/v6-images` pins an embedded transparent/cropped container, model JSON and SVG, plus malformed/missing/singular/future refusals. Regenerator: headless `varos-pdf/examples/freeze_images.rs`. Reopen validates the manifest and streams before document/store publication. Native save retains hidden current assets, omits undo-only assets, and refuses missing embedded resources and oversized containers.

Bounds remain ADR-0014's 16 MiB original, 32 MiB live unique originals, 256² proxy / 8 MiB live proxies, 256 MiB session, process CPU and GPU budgets, 16,384 axes / 32 million pixels. No ratchet or format limit is raised.
