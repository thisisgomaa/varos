> **Status:** proposed — decision pending; no implementation or format number reserved.
# ADR-0014: Raster object and links

- Date: 2026-10-09
- Scope: [PLAN](../PLAN.md) Phase 3, slices 3.0–3.6; first image writer is 3.1.
- Governing decisions: [ADR-0004](ADR-0004-v1-schema-policy.md), [ADR-0008](ADR-0008-vrs-format-versioning.md), [ADR-0009](ADR-0009-varos-bridge.md), [ADR-0011](ADR-0011-bridge-connection-and-trust.md), including its latest amendments.
- Owner decision: Place embeds by default, with an explicit Link option.

## Context and evidence

`varos-core/src/model.rs` currently has Layer/Group/Path nodes and None/Solid Paint; an image is a new leaf, not a paint or a vector path containing pixels.
[Gap 1 §B/§E](../reference/gap/GAP_1_FILES_IO_CANVAS.md) identifies the missing import, links and renderer paths; its historical v5 label is superseded by PLAN's schedule.
[Appearance study C risk table](../reference/study/APPEARANCE_STUDY_C_varos_fit.md#d-risk-table) records 200 whole-document undo clones, texture pressure and idle-heat risks.
The slice 0.1 guard `varos-core/tests/no_raster_model.rs` recursively walks field types from Document/Node/Path, including absent optional payloads; wrapping bytes in Arc does not satisfy it.

Prior art read under `~/Documents/AI workspace/reference/artcraft/`:
- VectorCraft `crates/doc/src/node.rs:326–340` (ImageObject), `lib.rs:512–520` (ImageBlob), `links.rs:22` (LinkInfo).
- VectorCraft `crates/engine/src/cmd/links.rs:1–56`, `place/mod.rs:1–8`, `package.rs:1–16` (operations and packaging).
- PhotoCraft `crates/codecs/src/lib.rs:39–55`, `orientation.rs:161–198` (bounded decoder entry and EXIF normalization).
Adapt those contracts with attribution and dependency/license review; do not import their UI or CPU renderer. VectorCraft's document-relative paths are distinct from the home-relative locator required here.

## Model and ownership

Proposed leaf: `NodeKind::Image { blob: BlobKey, px_w: u32, px_h: u32, ppi, xform, placement }`.
`BlobKey` is an opaque immutable asset identity, represented without byte arrays (for example a validated string); duplicate content may share it after hash and byte verification.
`px_w/px_h` are positive, oriented source dimensions; `ppi` is a finite positive x/y pair, preserving non-square source resolution.
`xform` maps the oriented pixel rectangle into document coordinates, including translation, scale and rotation; it must be finite and invertible.
Today's `Xform` is rotation/pivot only: specify a separate image affine value in this bump, leaving existing vector serialization unchanged. Apply ancestor node transforms once; keep the image leaf's existing node transform at identity.
`placement` records embed/link mode, optional LinkInfo, and replacement policy (default keep displayed bounds; explicit keep transform). Crop uses an existing Clip group, never destructive pixel edits.
Reuse scene opacity/compositing semantics; pin image opacity's stored location in the v6 wire table before the writer, with default 1 and parity across consumers.

The **logical document blob table** is `{ key → { mime, bytes: Arc<[u8]>, proxy?: small RGBA } }`, owned by a sibling `BlobStore` in the document session, outside `Document`, `Node`, `Path` and undo/redo snapshots.
`Document` carries only the key/metadata table and image references. No raster bytes, proxy pixels, encoded base64, renderer caches or store pointers reachable through those structs.
History clones copy keys and small metadata only. A session resource owner counts references from current document, undo, redo, clipboard, checkpoints and in-flight save/export/recovery jobs; each pins the required immutable store entries outside the clone stacks.
Update/relink creates a new key, never overwrites old bytes; undo must show the previous image even if its source file has changed. GC releases entries only after the last pin disappears, including redo truncation and tab closure.
Missing linked originals have an explicit unavailable state: retain metadata and proxy, never relabel proxy bytes as a full-resolution original.

Keep and extend the slice 0.1 type-walk guard: classify new key/metadata/affine types and their defining modules, not a blanket Arc/String/payload exemption. Add structural coverage against hiding base64 in metadata and pointer-identity/allocation tests proving shared assets across history.

## Persistence and the scheduled v6 bump

Retain the PDF + embedded JSON `.vrs` container. Core owns the metadata/asset manifest schema; the container writer joins a captured Document with its pinned sibling store and writes bounded binary asset streams, referenced by keys from the manifest.
Original embedded bytes and lossless proxy streams live outside model JSON; read them into a validated sibling store before publishing the document/store pair atomically. Recovery and clipboard must carry this pair too; today's model-only recovery JSON cannot recover embedded images by itself.
Embedded entries persist full originals plus optional proxies. Linked-only entries persist link metadata and a proxy, with full source bytes only a session cache; a key shared by embedded and linked placements persists the full original once.
Native save includes all referenced assets, even hidden/off-page images; export PDF contains appearance only and no editable payload. Never serialize undo-only assets into the current document.
Missing linked source is a valid degraded state if its proxy and manifest are valid; a missing embedded stream is corruption and refuses the load/save.

PLAN schedules v5 StrokeStyle then **v6 images/blobs/links**. The first merged writer takes the next number; rebase this proposal if ordering changes. One bump carries every Phase 3 stored field, with working render/export/Bridge behavior, not dormant keys.
Before that writer ships: update ADR-0008's amendment and `docs/reference/VRS_FORMAT.md`; implement pure named `migrate_v5_to_v6` (empty image table, no invented content), sequential migrations and strict pre-decode version/catalog checks.
Use `#[serde(default, skip_serializing_if = ...)]` for new optional/default metadata fields; required Image payload fields remain required. Plain-object payloads remain byte-identical apart from the envelope version; no downgrade writer.
Freeze old/new round trips, malformed/refusal files, future-version rejection and old-reader harness fixtures. Reject unknown keys/variants and dangling/duplicate asset references; preserve `format/{migrate,validate,limits}.rs` bounded-load/symmetric-save policy.
Neither opening nor migration rewrites the source file. The normal migration notice precedes any later manual save or enabled autosave into the newer format.

## Links, Place and Package

LinkInfo records absolute path, optional path relative to the OS account home (never shell `$HOME` expansion), last accepted mtime, byte size, content hash, and embedded-or-linked mode through `placement`.
Also permit a separately typed document-relative locator for portable packages; do not confuse its base with home-relative paths. Bound path lengths; resolve/canonicalize at use time with no URL/network fetch or directory scan.
Resolve package-relative, then absolute, then home-relative candidates; require matching content identity before accepting a relocated candidate. Different bytes mean modified, even if mtime/size match. Hash on bounded open/refresh; watches are hints, not continuous polling.
Persisted fingerprints describe the last accepted source. Missing/modified status is runtime state. Keep the last accepted pixels/proxy until explicit Update/Relink; never silently replace artwork on a watcher event.
Proxy: oriented RGBA, longest side ≤256 px, with dimensions; serialize losslessly, account for its bytes, and visibly label proxy-only display. A missing proxy uses a labelled placeholder, never an invisible object.

Place ⇧⌘P: Embed selected initially; Link toggle applies to that placement. Click uses physical size; drag specifies bounds. In point units, natural size is `px × 72 / file_ppi`; convert through document units. Missing/invalid ppi falls back to 72 with an import note.
Apply EXIF orientations 1–8 once at import, including mirror cases and swapped dimensions/ppi axes. Normalize orientation metadata if pixels are re-encoded; retaining original encoded bytes requires recording the orientation normalization recipe in asset metadata for every consumer.
PNG/JPEG/GIF/WebP/TIFF/BMP arrive in 3.2; 3.1 must already support a bounded PNG path end to end. Animated/multipage inputs use an explicitly reported first frame/page, not silent flattening. HEIC/PSD wait for 3.9.
Finder drop uses the same embed default; bitmap paste embeds because it has no stable file path. Place/drop/paste commits one undo step after bounded decode; cancellation/refusal leaves no node or leaked resource.

Links panel lists mode, source, missing/modified/proxy status, pixel dimensions, source ppi and effective ppi on both transformed axes.
- **Relink:** select replacement, validate/decode, preserve placement policy, commit metadata/key once.
- **Update:** reload explicitly selected modified sources; report failures, commit successful replacements as one undo step.
- **Go To:** select/reveal the image on canvas, without modifying it.
- **Embed:** persist the accepted full original; refuse when only a proxy is available.
- **Unembed:** durably write a full image to an explicit destination before switching to linked; failure leaves the document embedded. Undo restores mode but does not delete the exported file.

Package writes a copy of the `.vrs`, `Links/` with collision-safe filenames and a report; rewritten document-relative links refer to copied, verified source bytes. The open document/path/dirty state remain unchanged.
Stage output before publication; cancellation cleans staging, never overwrites unrelated files. Report missing/modified links and proxy-only assets; no success-labelled complete package with omissions. Fonts are added only when text exists and licensing permits.

## Bounds, rendering and export

Proposed starting limits (measurement may lower them; increases require the ADR-0008 owner process):
| Resource | Bound / action |
|---|---|
| Source dimensions | ≤16,384 per axis and ≤32 million pixels; reject before pixel allocation |
| Encoded original | ≤16 MiB per blob; ≤32 MiB unique originals per live document |
| Proxy | ≤256² RGBA pixels each; ≤8 MiB aggregate |
| CPU decoded cache | ≤256 MiB process-wide; reserve decode scratch separately, ≤256 MiB; queue decodes |
| Pinned original/proxy history store | ≤256 MiB per session across all 200 undo/redo steps; refuse a new import/update if admission cannot fit, never silently discard undo |
| Image GPU residency | ≤256 MiB process-wide including mips/staging, reduced by available renderer budget |

These are additional limits, not exemptions from current 256 MiB file, 32 MiB JSON, 64 MiB total decoded PDF streams, object/tree/count bounds. Codec-expanded RGBA and PDF stream decompression are separately accounted; the checked writer must refuse any combined container exceeding existing limits, even when each image fits.
Sniff headers and dimensions before decode/allocation (never fully decode to learn size); use checked width×height×channels arithmetic, decoder allocation limits, bounded metadata/frame counts and decompression limits. Verify decoded dimensions/MIME/hash against manifest; reserve peak scratch plus destination before starting.

wgpu: cache textures by immutable key/orientation/color interpretation; build mips once, budget roughly 4/3 base texture size plus staging, evict non-visible LRU entries and release GPU resources after submitted work completes.
Honor adapter dimension limits: tile or refuse with a clear reason; never silently drop a large image. Prioritize visible mip levels, allow labelled preview while loading, no full-resolution upload per pan/zoom/frame and no idle uploads or continuous redraw requests. Idle stays `ControlFlow::Wait`.
CPU raster consumes the same image scene primitive, transform, clipping, alpha and sampling rules through a bounded store resolver; snapshots/thumbnails must depict the image. Cached decode is outside Document. Exercise CPU/GPU parity with rotated, cropped, transparent and missing-link fixtures.
PDF uses image XObjects, pooled by asset and export sampling parameters; alpha uses an SMask. Downsample only above the ppi chosen in PDF options, using displayed physical size (both axes); never upsample or mutate the stored original. Native editable data retains full embedded originals regardless of preview sampling.
SVG emits `<image>` with transform/clip/opacity; embedded images use escaped MIME/base64 data URIs, linked export may use a properly escaped href. Portable SVG mode embeds resolved linked originals; external href mode reports dependency paths.
No silent export loss: `ExportReport` notes cover proxy fallback, missing/modified links, external SVG dependencies, selected downsampling and unsupported color/profile behavior. Missing full originals default to refusal for production output; an explicit preview export may use proxies with a loss note. Never claim CMYK/ICC proofing before Phase 12.

## Bridge 1.2

Opt-in API 1.2 adds `add_image` with exactly one local path or opaque host-issued, bytes-free asset handle, plus placement mode (embed default), bounds/transform and ppi override. No base64/source-byte field in edit requests.
Resolve paths/decode off the owning thread with bounds; stage immutable resources, then recheck board, expected revision, active gesture and handle ownership/lifetime at atomic batch publication. A stale/expired handle refuses; no cross-board lookup by guessed key. A batch is one undo step; staged failures release pins.
`describe` exposes image ID, key, dimensions, source/effective ppi, transform, placement/link status and proxy availability, never pixels. Metadata/diffs/snapshots exclude original and proxy payloads; the explicitly requested rendered snapshot image remains permitted under existing bounds.
Advertise only implemented capability fields. Freeze 1.0/1.1 fixtures; a legacy caller encountering unsupported image detail receives a clear unsupported-version result rather than a fabricated path or dropped object.
Follow ADR-0011's current open-local-trust amendment: same-uid/local transport, revision/receipt/batch and file-use checks remain; do not reintroduce superseded pairing approvals. Imported bytes/metadata are data, never agent instructions.

## Acceptance and open decisions

Gate 3.1 on measured RSS, peak decode/export/save memory and retained allocations after 200 transform steps and 200 distinct image replacements; verify undo/redo after delete/update/GC, clipboard and recovery reopen. Report corpus and numbers, not only passing tests.
Measure GPU residency and memory pressure on an 8 GB Mac with several large images and existing offscreen layers; image budget is not the whole renderer budget. Exercise eviction/pan/zoom, idle uploads/frame counts and responsiveness. Structural sharing is deferred unless measurements demand it.
Test external-link edits with unchanged size/mtime, missing proxy/full stream, all eight orientations, ppi fallback, package relocation, downsampling, Bridge fixture compatibility and refusal atomicity. Require visible PDF/SVG/CPU/GPU comparison and owner Place→Crop→Relink→Package hand test.
Open before implementation: accept this ADR; finalize exact image opacity/affine/asset-stream wire fields and bounded decoder budgets in the bump work order. Owner Figma choices for Place/Crop/Links/Package remain later slice gates; this proposal approves no UI.
