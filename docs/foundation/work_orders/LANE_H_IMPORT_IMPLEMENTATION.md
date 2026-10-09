# Lane H implementation boundary (2026-10-09)

Owner authorized provisional UI before the final design review. Existing native report dialogs
are reused; File Place uses Illustrator ⇧⌘P. No tokens, kit or persisted schema keys change.

`varos-import` owns foreign conversion. Native `.vrs` reads retain their own decoder; `.pdf`
routing is chosen before native decoding, with Varos markers pinned to the native path. A native
failure never calls a foreign parser. Foreign Open creates a dirty, pathless document; Place and
canvas drops retain named layers as groups and use the existing checked `PlaceArtwork` edit.

Locally cached lopdf 0.43.0 (MIT) is used because hayro/hayro-interpret are absent. The supported
PDF profile is classic xref, static DeviceRGB/Gray paths, transforms, fill/stroke styles, dashes,
CropBox/MediaBox and representable clipping. One page is imported; multi-page input needs the typed
one-based `page` option in CLI/Bridge. Rotation/UserUnit, compressed object/xref streams, encryption,
nonzero compound fills/clips, Form XObjects, CMYK/ICC/spot/pattern/transparency operators refuse.
Text has no verified glyph-outline API in lopdf: omission is reported and requires acceptance.
PDF images are reported omissions because this checkout has no native image/blob node.
Bitmap-only paste refuses with that prerequisite message; no vector-to-bitmap fallback or tracing.

ASCII DXF supports LINE, ARC, CIRCLE, LWPOLYLINE bulges, POLYLINE/VERTEX/SEQEND and nonrational
clamped cubic Bezier SPLINE. Layer names/basic colour/hidden/locked state and declared physical
units survive. Unknown/unitless units require `points_per_unit`; general NURBS, 3D, INSERT,
text/hatches and unsupported line styles/colours refuse. Arc radial tolerance is 0.01 point.

Clipboard preference is trusted current Varos bytes, SVG, PDF, then bitmap. Unknown/malformed
internal data refuses; other Varos sessions' detached selection decoding is deferred. Snapshot
and publication recheck the NSPasteboard generation; losses are confirmed before mutation.
CLI `import`, `import-pdf`, `import-ai`, `import-dxf` require a fresh `.vrs` output; `--allow-loss`
is explicit. Bridge 1.2 `import_file` and `import_clipboard` expose typed options through schema
and list_verbs; native file scope and existing revision/idempotency receipts remain in force.

Desktop foreign Open/Place/drop uses the existing bounded IO worker. PDF/AI parsing re-enters the
host executable with `--varos-import-worker` before GUI startup, with bounded pipes, cancellation
and a hard ten-second process deadline. SVG/DXF use cooperative conversion checkpoints; desktop
clipboard and Bridge imports currently wait for staging synchronously (bounded), then publish.
Native GUI/OS interoperability and complex visual oracles require integration and owner review.
