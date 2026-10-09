# w2-images — Phase 3
3.1–3.8 implemented; provisional UI and native owner acceptance remain pending.
Writer remains provisional v6: images/assets/raster_effects_ppi; integrator assigns combined versions.

## Fix round
Astra's eight findings and Opus image findings 1–5 addressed; no disagreement with their reproductions.
PDF: removed parallel scene writer; mixed leaves share write.rs exact cubic/stroke/knockout writer.
Images: Flate RGB/SMask, upright three-component JPEG DCT passthrough; CMYK uses decoded RGB.
Save: pinned hash/header/metadata checks + bounded PDF preflight; no second decoded resource store.
Open: original-budget failure retains original bytes and proxy pixels, with a visible load notice.
Boundary: if even proxy residency cannot fit, return TooLarge rather than MalformedPdf; hard cap retained.
Zero opacity: appearance omitted consistently, while native original/proxy streams remain.
Scene: one ordered path lookup; debug 200/400/800 empty paths measured 5.98/14.92/34.78 ms.
Interaction: live image signature, mixed Select All/frame/move/scale/rotate/numeric edits/marquee/copies.
Boards: image membership/visibility/lock and clipping shared; GPU scissors + CPU masks preserve crop.
CPU image exports now use artwork-only scenes, excluding artboard chrome; SVG shares vector dispatch.
Bridge API 1.2 writable capability derives FORMAT_VERSION; account-home lookup and initial Links badge fixed.
Tests: restored envelope-normalized v5 JSON/PDF byte goldens, specific refusal errors, frozen v5→v6 gate.
Original frozen fixtures unchanged; corrected image-writer golden added separately; PDFium 16 pixels PASS.
Gates: fmt/dep directions PASS; cargo test --offline --workspace -j 3 --no-fail-fast: 1,928 passed / 0 failed / 15 ignored.
Native + Windows Clippy -D warnings PASS; shell 3 + Bridge 6 ratchets, panic guards and Bridge fixtures PASS.
Ratchets/fixture sources unchanged; ui.rs 811/843; logs /tmp/w2-fix-{workspace-green,native-green,windows-green,bridge-fixtures}.log.
Integration: stroke::canvas_seam(ed,p,ppu) marks main 9f14e1e cache/cap/backoff/fallback routing; no main merge.
Opus gradients 7–10 absent here; gradients lane must fix them. Mixed v7 golden/v6→v7 gate remain integration work. Low Bridge link hashing/description compaction unchanged.
No git writes, commit/push/GUI/install; fixes uncommitted. Native/GPU behavior and adversarial allocation peaks unverified.
