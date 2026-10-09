# w2-images — Phase 3 resume
Branch `feat/w2-images`; resumed WIP `a205728`; changes remain uncommitted.
Status: 3.1–3.8 implemented (provisional UI, owner design review pending).
Format: provisional next writer v6; integrator assigns final number; pure `migrate_v5_to_next_images`.
Wire additions: Document `images`, `assets`, `raster_effects_ppi`; `NodeKind::Image(id)`; image affine/opacity/placement/replacement/link metadata; bounded binary original/proxy streams outside JSON.
Resources: sibling immutable BlobStore, shared history/clipboard/save/export/recovery pins, retirement on dropped history; no-raster-model guard passes.
Consumers: wgpu textures/mips/budget/cache; CPU raster, fitted snapshots/thumbnails, PDF XObjects/SMask/PPI downsampling, portable SVG images, Print and OS clipboard output.
Import: PNG/JPEG/GIF/WebP/TIFF/BMP, all eight EXIF orientations, non-square file PPI, bounded first frame/page; unavailable gif 0.14.2 replaced by cached 0.13.3 adapter.
Place: embed default + Link toggle, ⇧⌘P sheet, click/drag bounds, Finder drop, OS bitmap paste; worker decode with cancellation/tab/revision/gesture checks and one undo.
Edit: image translation/affine/rotation/opacity, cancellable gestures, Layers select/toggle, crop as Clip group and existing release command; overflow/protected-object refusals.
Links: real PanelId, source/status/PPI fields, relink/update/go-to/embed/unembed; accepted pixels survive source changes; relative relocation retained.
Package: staged fresh folder, content-key collision handling, Links/, Document.vrs, Report.txt, fonts “—”; refuses omissions/modified sources/existing destinations.
Rasterize: one image/path/group, PPI and transparent/white background; document raster effects PPI. Trace: four presets to editable paths, preserved clipping, one undo.
Automation: API 1.2 add_image/image_action, bytes-free describe, list_verbs/schema; matching headless CLI workflows and resource-aware existing export/apply commands.
Frozen v1–v5 and all 16 Bridge fixtures unchanged; new v6 container/model/SVG and malformed/future/refusal fixtures added.
Gates: `cargo fmt --all --check` + `python3 ../tools/check_dep_directions.py` PASS; `cargo test --offline --workspace -j 3 --no-fail-fast`: 1,916 passed / 0 failed / 15 ignored.
Native + Windows x86_64-pc-windows-msvc Clippy `--workspace --all-targets --offline -j 3 -- -D warnings`: PASS on final source.
Ratchets: 3 shell + 6 Bridge PASS; source/caps unchanged; ui.rs 811 ≤843 lines; API 1.0/1.1 tables 23,993 B; API 1.2 22,828 ≤24,000 B.
CLI smoke: 11/11 PASS (describe, snapshot, SVG/raster/PDF export, rasterize, trace, crop, linked Place, Package, reopen); legacy CLI 31/31 PASS.
Memory corpus: 2048² PNG (86,134 B), 200 transforms, then 200 distinct 256² PNG replacements; undo/redo resources remain resolvable.
200 transforms: resource bytes 17,125,494 →17,125,494; model 1,573 B. 200 replacements: resources 122,365,284 B; originals 468,324 B; CPU charge 121,896,960 B.
Measured peak RSS (fresh child getrusage, Darwin): undo/replacements 172.66 MiB; decode 50.56 MiB; native save/reopen 107.20 MiB; PDF export 58.98 MiB.
Evidence: docs/reference/W2_IMAGES_EVIDENCE.json; headless probe example varos-pdf/examples/measure_images.rs; gate logs /tmp/w2-handoff-{tests,native,windows}.log.
Known export limits: RGB interpretation; production output refuses proxy-only originals; image PDF custom printer marks/boxes currently refused explicitly.
Unverified: real GUI, Finder/OS clipboard interaction, 8 GB Mac GPU residency/eviction/idle pacing and CPU/GPU visual parity; independent review and owner design approval pending.
Optional 3.9 HEIC/PSD deferred. No commit, push, GUI launch or install performed; all Cargo work offline -j 3 from varos/.
