Frozen Live Corners fixtures (Lane C). Integration w2 (2026-10-09) assigned the lane's provisional
format 6 its final number **9** (images 6, gradients 7, text 8, Live Corners + embedded preview 9):
`next_corners.json`, `next_live_round.json` and `refused_negative_radius.json` were restamped 6 → 9
(stamp only). `refused_corners_on_v5.json` stays 5; `refused_corners_on_v8.json` is the same body
stamped 8 (corners are refused before format 9); `refused_future.json` is format 10. `SHA256SUMS`
covers every file (`shasum -a 256 -c SHA256SUMS` from this folder).

Integration w3 (2026-10-10): wave 3 rechained the formats to v9→v10 appearance→v11 effects→v12 colour→v13 live→v14 typography (writer 14). Every refused-future fixture now claims format 15 (restamped in place; only the `"varos"`/`/VAROS_SchemaVersion` stamp changed) and `SHA256SUMS` was regenerated.
