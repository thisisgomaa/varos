> **Status:** proposed — Phase 7 import contract; docs only, parser/dependency and UI reviews pending.
# Interchange import: fidelity, firewall and atomic publication

- Date: 2026-10-09; baseline `7b48f2c`.
- Authority: [PLAN](../../PLAN.md) Phase 7; [ADR-0008](../../adr/ADR-0008-vrs-format-versioning.md), [ADR-0009](../../adr/ADR-0009-varos-bridge.md) and latest [ADR-0011](../../adr/ADR-0011-bridge-connection-and-trust.md) amendments.
- Prerequisites: stroke, image/blob and gradient capabilities from 2.1/3.1/5.1; supported Phase 6 masks may be used when delivered. No new `.vrs` fields or import-specific format bump.

## Baseline and borrowing boundary

The native path is `varos/crates/varos-pdf/src/lib.rs:136-143` → `load_vrs_checked` / `read.rs` → `varos-core::format::decode_model`. It is a bounded native decoder, not a general PDF importer.
`varos/crates/varos-core/src/export.rs:1-14` defines `ExportReport { notes: Vec<ExportNote> }`, each note `{kind, object_id: Option<u32>, message}`; report-bearing PDF/SVG export APIs already exist. Reports stay outside Document and undo snapshots. Bridge export reports are opt-in per-tool API 1.2 (`varos-bridge/src/mcp.rs:215`); this does not advertise imports.
IDEA/ADAPT reference: `~/Documents/AI workspace/reference/artcraft/vectorcraft/crates/svg/src/import.rs:31-82,803-809,859-867,897-906,975-1003`: usvg conversion plus warnings, including ignored filters, alpha→luminance masks, ignored nested masks and approximated gradients. Adopt explicit diagnostics; do not inherit those fallbacks silently. Review all lifted code/dependencies and add attribution/NOTICE at first implementation borrow.

## Firewall: separate crate, explicit operation

Create a proposed `varos-import` crate for parser adapters and conversion. It may depend on pure core/model types and bounded resource-decoding helpers; app/CLI/Bridge host adapters call it. Core, native format modules and `varos-pdf` native read must never depend on it or call its parsers. Keep usvg/hayro/DXF parser dependencies out of the native reader's dependency closure; verify with dependency tests.
Native `.vrs` open always uses the native decoder. Missing/corrupt/unsupported embedded model, future format, malformed PDF or native refusal **never** triggers visual PDF reconstruction, even if a plausible preview exists. Explicitly importing an external PDF is a different action and never claims to recover an editable `.vrs` model. A renamed `.vrs` selected for native open is still refused, not sniffed into import.
Open may route explicitly selected external SVG/SVGZ/PDF/AI/DXF to import; sniff signatures to validate the chosen format, not to reinterpret failed native reads. Place and clipboard call the same importer with insertion options. File extension alone never proves PDF-compatible AI or supported DXF.
Parsing and conversion run on a bounded worker with cancellation. Produce a staged candidate plus report; validate structure, semantic invariants, asset references and native encode limits before any editor publication. Do not attach foreign parser objects, serialized source XML/PDF or executable data to Document.
Open publishes a fresh pathless, dirty document requiring Save As `.vrs`; never overwrites/rebinds the source. Place/Paste allocates fresh native IDs and publishes one checked undo step with blobs pinned; reject locked target/stale revision/busy state without mutation. Remap candidate IDs, names and internal references atomically. Cancellation/refusal leaves tabs, selection, dirty state, IDs, assets and history unchanged.
Input options are typed: format, open/place/paste, page selection, sizing/units, destination and loss policy. Desktop uses a provisional report/choices sheet (owner Figma gate for 7.1/7.3). Bridge/CLI reuse typed DTOs and reports only when implemented under negotiated APIs; explicit targets/revisions and file-read boundaries remain, no generic command/JSON bypass.

## Per-format fidelity and loss policy

“Preserve” below is the implementation acceptance target, conditional on a shipped native representation. Appearance can be preserved while source editability is lost; report the latter. Default is staged refusal of lossy conversion. A desktop acceptance or explicit CLI/Bridge loss policy may permit only the listed, bounded conversions; it cannot waive malformed input or resource limits. Never return partial success without a report.

| Format / feature | Target mapping | Loss, refusal or explicit alternative |
|---|---|---|
| SVG/SVGZ paths, basic shapes, transforms, viewBox | Editable native paths, bake transforms; preserve winding, draw order and opacity | Primitive/transform editability lost when baked: report; convert CSS px at 96 dpi to 72-point world units, preserve physical lengths |
| SVG solid paint / strokes / supported linear-radial gradients | Native paint/stroke/gradient after prerequisite phases | Unsupported focal geometry, spread, dash/marker/non-scaling semantics: refuse or explicit outline/bake with tolerance; no average-colour or pad substitution |
| SVG groups, clips, opacity/alpha masks | Preserve groups, nested clips and representable mask semantics | Alpha and luminance are not interchangeable; unsupported mask, blend or filter refuses unless an explicit validated raster fallback exists; never drop it |
| SVG `<use>`, symbols, pattern fills | Expand bounded references into native paths/groups where appearance is exact | Report loss of symbol/pattern linkage; unsupported pattern/filter rasterization is optional and reported, not required for first slice |
| SVG embedded/linked images | Embed decoded assets by default through image/blob rules | No remote fetch; missing/blocked references refuse unless user explicitly selects omission. Optional local resources require bounded source-relative resolution; report embedding/link loss |
| SVG text and fonts | Already outlined paths accepted; live text may become outlines only with a verified shaping/font path | Text programme is later: missing fonts or unavailable faithful outlining refuses. Never silently substitute fonts, omit text or claim editable type |
| SVG script, animation, foreignObject, CSS/resources outside supported subset | Static supported vector subset only | Active/external content not executed; unsupported visual behavior produces blocking loss. No DTD/entity expansion or network access |
| PDF and PDF-compatible AI paths/images/pages | hayro-based adapter → native geometry/assets; selected pages → artboards; honour page box, rotation, clipping and units | Choose page range and page box explicitly (default CropBox, else MediaBox); no inference of Illustrator private editing data |
| PDF/AI text | Embedded/available glyph outlines through a validated conversion path | Always report loss of text editability; unresolved glyphs/fonts refuse, no font substitution. Does not implement native text objects |
| PDF/AI gradients, transparency, masks, groups | Preserve only mappings proven equivalent to delivered native features | Mesh/pattern/blend/overprint/spot/ICC/CMYK not yet representable: block; optional explicit baked/raster or RGB conversion reports colour/editability loss and method/profile; never pretend print fidelity |
| AI private data; non-PDF AI/EPS/PostScript | Only a valid PDF-compatible stream supported in 7.3 | Private layers/live effects/links not reconstructed: report. Legacy AI/EPS/PostScript and encrypted/password-protected PDF refused in first slice; no interpreter or password UI implied |
| PDF annotations/forms/actions/attachments/optional content | Static selected page appearance only | No actions/scripts/attachments executed or imported. Report omitted annotations/forms/interactivity; unsupported visible layer state blocks or requires explicit flattened appearance |
| DXF LINE/ARC/CIRCLE/LWPOLYLINE/POLYLINE/SPLINE | Supported 2D geometry → paths; preserve layers, units and colour where representable | Arcs/splines tessellated or cubic-approximated only within declared point tolerance; report approximation; test bulges, closed paths, axis orientation |
| DXF blocks/inserts and units | Expand supported 2D inserts with bounded transforms/references | Report lost block linkage; unitless/unknown units require an explicit unit choice; no guessed inch/mm scaling |
| DXF 3D, text/dimensions, hatches, xrefs, proprietary entities | Unsupported until separately implemented | Refuse affected content by default; explicit omission lists entities/counts. No 3D projection or font substitution silently. DWG always refused; binary DXF deferred unless separately tested |
| Clipboard Varos internal flavor | Validated native selection, remapped IDs; current in-app clipboard stays available | Not a whole-document read or trust bypass; enforce bounds/schema; malformed preferred flavor refuses by default |
| Clipboard SVG / `image/svg+xml` | Same SVG adapter and loss policy | External resource base absent: embedded data only; never network/ambient-file resolution; live text restrictions identical |
| Clipboard PDF / `application/pdf` | Same PDF adapter; platform UTI mapped by host | Multi-page requires explicit choice; private Illustrator/AICB payload not interpreted; losses identical to PDF |
| Clipboard PNG/TIFF and supported bitmap flavors | Embed one decoded image with orientation, alpha and explicit size policy | Report vector/editability loss if chosen instead of offered vectors; absent resolution uses declared 96 ppi; no tracing implied |
| Clipboard HTML, plain text, file URLs, unknown/private flavors | Do not treat as vector art | Text field paste remains native text editing. Canvas plain text awaits text programme; file URLs go through explicit file-import flow, never arbitrary path reads |

Clipboard host snapshots available formats/change count once; preference order: valid internal selection → SVG → PDF → bitmap. Choose the highest supported flavor and preflight it. A failed/lossy preferred vector flavor does not silently fall back to bitmap; show alternatives or require explicit `flavor`/loss policy in automation. Recheck clipboard generation before consumption; bytes/report identify the chosen flavor.
Within a declared non-lossless policy, omission or rasterization must name affected source items/features. Desktop approval binds the staged source/options/report; changed bytes/options require a new review. This is fidelity choice, not a reinstatement of ADR-0011 pairing/permission gates.

## Report shape: generalize vocabulary, preserve export contracts

Propose pure `FidelityReport` / `FidelityNote` in a core-owned diagnostics module, with no parser dependencies. Do not rename the exported/wire `ExportReport` in place. Existing export APIs and serialization remain an explicit compatibility projection with current `notes` fields; 1.0/1.1 and existing 1.2 export fixtures stay unchanged. A Rust alias is safe only if the shape remains exactly identical; a richer struct requires an adapter, not a blind alias.

| Proposed import report field | Meaning |
|---|---|
| `report_version`, `direction`, `format`, `status` | Version 1, import/export, detected format, lossless/converted/lossy/refused |
| `source_digest`, `options_digest`, `converter_version` | Bind diagnostics to exact input and decisions; no raw paths/content in audit |
| `notes[]` | Stable `code`, `severity` (info/warning/error), `effect` (baked/outlined/rasterized/approximated/omitted/colour_converted/refused), plain message |
| per-note `source_ref`, `target_ids`, `count`, `details` | Page/element/entity/flavor locator; staged IDs remapped on publication; tolerance/ppi/profile/replacement where relevant |
| `totals`, `more`, `cursor` | Full counts by effect/code; bounded detail pages; never silently truncate losses |
| `output_summary` | Pages/artboards, paths, images and dimensions; describes candidate even before acceptance, not a committed document |

Cap inline notes (proposed 100 / 16 KiB), aggregate repeats deterministically, retain bounded pageable details in the staged job. If complete diagnostics cannot be retained within job limits, refuse `limit_exceeded`; never claim a complete lossless result. Escape labels/messages as data. Missing target IDs are valid for omissions/refusals; source locators are not native IDs.
Return an import outcome containing report plus staged candidate/token, or typed parse/unsupported/resource/cancelled error plus available report. Successful publication returns committed IDs/revision; failed parse cannot masquerade as an empty imported board. No report goes into `.vrs` or undo snapshots. CLI JSON and desktop text render the same structured facts.

## Resource limits and required tests

Before implementation, review usvg/hayro/DXF crates, licenses/features, font/raster dependencies and cancellation behavior. Proposed starting caps: 64 MiB input, 128 MiB SVGZ expanded bytes, 256 nesting/reference depth, 100 selected pages, 100,000 source objects and 10-second parse budget. Enforce native format/asset limits too, whichever is tighter; measure/tune explicit caps before enabling formats.
Bound allocations before decompression/image decode (including aggregate decoded pixels/bytes), entity expansion, recursion and path subdivision. No unbounded XML entities, cyclic uses/blocks, linked-resource traversal or parser subprocess/network execution. Cooperative cancellation must be demonstrated; a parser that cannot meet it needs an isolated worker process or remains disabled. Source-relative resources reject traversal/symlink escapes and get aggregate budgets.

| Test family | Required evidence |
|---|---|
| Native firewall | Corrupt/missing embedded model, future `.vrs`, malformed PDF, plain PDF renamed `.vrs`: native refusal and zero importer calls; native golden reader/writer fixtures unchanged |
| Fidelity fixtures per table row | Expected geometry/order/units/IDs plus exact report codes/counts; SVGZ matches SVG; text/font, masks, gradient, AI-private, DXF-unit and clipboard fallback losses covered |
| Visual oracles | Reference input renders against imported CPU/GPU/PDF output for the supported subset; declared pixel/geometry tolerances, explicit colour assumptions; manual complex fixtures as well as automated checks |
| Round trip | Import → native validate/save → reopen → equality and stable subsequent native bytes; embedded images survive without source file; no parser invoked during reopen |
| Atomicity | Cancel/error/stale targets/locked insertion/encode failure leaves document and asset store unchanged; one paste/place undo step removes all inserted art; redo restores it |
| Malicious input | Fuzz parsers/converters; zip bombs, image bombs, deep/cyclic refs, huge paths/pages, NaN/overflow, DTD/entities, external URLs, traversal and malformed clipboard; bounded refusal, no I/O escape |
| Report compatibility | Existing ExportReport and Bridge fixtures unchanged; report paging/aggregation deterministic, acceptance bound to digest/options, no hidden losses after parser normalization |
| Host parity | Same options yield same candidate/report for desktop/CLI/Bridge; explicit fallback choice, source files untouched, pathless Open and no spontaneous Save/overwrite |

Delivery order: 7.1 SVG/SVGZ + report infrastructure; 7.2 clipboard using the same adapters; 7.3 PDF-compatible AI/PDF after hayro review; 7.4 DXF subset/DWG refusal. Any earlier flavor lacking an adapter stays unsupported. No text, colour-management or raster-effect programme pulled forward implicitly.
Implementation gates: dependency directions, Rust workspace/fmt/clippy/ratchets, independent review, owner-chosen report/page UI, then native owner tests from PLAN. This spec does not claim any importer or fallback is implemented.
