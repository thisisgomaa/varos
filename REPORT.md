Lane D renderer capability — feat/w3-render; fix-round hand-back (no git writes).
- Renderer-only primitives, CPU reference and encoder-only RGBA16F GPU passes; producer hookup remains pending.
- New lane modules: raster/{layers,layer_tests,layer_scene}.rs; render-wgpu/{layer_gpu.rs,layer_pass.wgsl}; original shared lib.rs blocks remain unchanged.
- Nested layers, 16 blend modes, masks at LayerEnd, Gaussian blur, shadow/glow; capped subtrees flatten explicitly.
- Default depth 16 / 256 MiB; four surfaces per active layer, quarter-budget effect cache; CPU f32 / GPU f16 accounting.
- Budget covers offscreen/cache reservations; root, geometry, staging/uniforms and encoder-retained resources are outside measured accounting.
- Producer must provide RGBA16F geometry, bucket_zoom and complete-input revisions (including effects/pan); no model/format/API change.
- Attribution headers/NOTICE retained; provisional owner review status unchanged; no GUI launch or installation.

## Fix round
- Accepted all three P2 findings; no disagreements. Cross-lane merge risks are integration obligations, not resolved by this isolated lane.
- Reduced-budget admission trims retained pool against live scratch plus cache before allocating; cache insertion also trims surplus pool.
- CPU/GPU share bounded LRU policy: replace obsolete object revisions, promote hits, evict oldest entries before replacement allocation.
- CPU and WGSL Dodge/Burn use exact backdrop endpoints; independent near-endpoint goldens prevent inverted colours.
- Added deep-pool→85-byte regression, revision 1/2/3/3 blur+shadow hits, bounded LRU recency; independent coloured goldens cover all 16 modes/alpha cases.
- Numeric effect-refusal tests now enclose effects in valid layers and assert specific errors before rendering.
- Targeted headless gate: 39 passed, 0 failed, 2 timing probes ignored; shader parsed/validated without GPU.
- PASS fmt, dependency directions, workspace tests (2283 passed / 0 failed / 17 ignored), native + Windows Clippy -D warnings, core/UI ratchets, Bridge fixtures (6 passed; 1.0/1.1=23993 B frozen; 1.2=23879/24000 B). Evidence: /tmp/w3-render-fix-gates.json and /tmp/w3-render-fix-*.log.
- Integration: reconcile appearance traversal/types/masks/formats/aggregate budgets, effects/live input revisions, CMYK overprint and text outline settlement.
- GPU execution, renewed independent review, producer integration and owner hand-testing remain unverified; no commit/push/merge.
