# Quick Look integration — Lane F

Native `.vrs` remains a PDF with the unchanged editable model. The desktop save worker prepares a
544 × 246 CPU PNG in the existing thumbs cache, tagged with the editable model's SHA-256, then reads
that cache entry into an unfiltered PDF `/VAROS_Preview` EmbeddedFile stream. The catalog carries
`/VAROS_PreviewVersion 1`. `varos-pdf::quicklook::embed_preview_next` is the named pure container
migration; `preview` is its bounded reader. No JSON document keys or FORMAT_VERSION changed.
Repeated migration replaces the preview stream; missing previews are accepted for existing files.
Headless tests verify model preservation, invalid/oversized input refusal, cache and preview roundtrip.

The macOS extension is deliberately **not packaged, signed or installed by this lane**. To finish it:

1. Add a Quick Look Thumbnail Extension target in the signed macOS packaging project, with
   `QLThumbnailProvider`, App Sandbox and the app's existing `.vrs` UTI in `QLSupportedContentTypes`.
2. On the provider's background queue, open the PDF with `CGPDFDocument`, read the catalog's
   `VAROS_PreviewVersion` and `VAROS_Preview` stream using `CGPDFStreamCopyData`, enforce the same
   2 MiB limit, decode with ImageIO, and return a `QLThumbnailReply` drawing that image.
3. For older files without a preview, draw the first PDF page with CoreGraphics. Do not instantiate
   the editor, renderer, EventLoop or an agent connection in the extension.
4. Embed/sign the extension with the app, then owner-test Finder thumbnails, multi-page documents,
   old files, corrupt files and cache refresh on a installed build. This remains unverified here.

Offline CLI Actions example (selection rebound explicitly; created locals are remapped):
`varos-cli apply board.vrs --batch move.vrs-actions --ids 7,12 --out moved.vrs`
Attached API 1.2: use `list_verbs`, then `schema` for `preferences`, `shortcuts`, `command_index`,
`history_list`, `history_jump`, `actions` or `help`; attached CLI exposes the same typed tools.
Actions v1 records committed moves, paint, opacity, stroke width, rotation, delete, group and ungroup. Replay also supports creating rectangles
and selecting their symbolic local IDs. Unsupported committed document commands stop recording with a reason. File, UI and history commands are excluded.
