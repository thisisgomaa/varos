# ADR-0008 amendment — wave-3 formats 10–14 (integration, 2026-10-10)

2026-10-10 · integration `integ/w3` · provisional UI everywhere; owner design review and an
independent review of this amendment pending.

**What changed.** Five wave-3 lanes each implemented their pre-assigned number as a migration from
v9 in isolation. Integration rechained them in the binding order and stamped them:

| from → to | step (named, pure) | era key gate (before typed decode) | lane contract |
|---|---|---|---|
| 9 → 10 | `format::migrate_v9_to_v10` | `paths[].stack`, `nodes[].look`, role `MaskAlpha` | appearance (Lane A, `VRS_FORMAT.md` "Lane A format 10") |
| 10 → 11 | `format::migrate_v10_to_v11` | `paths[].effects`, `stroke_style.width_profile` (also inside an appearance stroke entry) | live effects (ADR-0016) |
| 11 → 12 | `format::migrate_v11_to_v12` | `colour_mode`, `output_profile`, any managed paint | colour sources (ADR-0017) |
| 12 → 13 | `format::migrate_v12_to_v13` | `nodes[].kind.Live` | live objects (`ADR-0008-live-nodes-v13.md`) |
| 13 → 14 | `format::migrate_v13_to_v14` | `typography` | typography (text P5–P8) |

`FORMAT_VERSION = 14`, pinned literally together with every era constant. Every step returns its
input unchanged — no step validates (the loader validates once after the chain) and no step invents
data; the text lane's in-migration typography check moved entirely to its pre-decode key gate. The
lanes' reservations (identity rows for sibling eras, the v9→v14 bridge, the "10–13 are newer"
version guard) were removed, so `readable_versions()` is exactly 1..=14 and Bridge 1.2
`readable_vrs` advertises no number that has no reader.

**Rule 3 still holds per era.** Each era's keys are refused under every older stamp by a keys-only
scan before typed decode (cross-era fixtures `fixtures/w3-cross-era/`, plus each lane's own), so an
older build — whose frozen header gate refuses the newer stamp — and this build agree on what each
number means. All future-version refusal fixtures now claim 15.

**Durability evidence.** Frozen per-era corpora (`v10/`, `w3-effects/`, `v12/`, `v13/`, `v14/`), one
mixed v14 document (`v14-mixed/`) that pins the combined key order and round-trips byte-for-byte in
raw JSON and in the native container, and frozen v9–v13 header gates that refuse v14 JSON, embedded
model and catalog stamp. Precisely: canonical v9 bodies (the compact JSON Varos itself writes, e.g. `v9/mixed.json`) re-save byte-identical apart from the stamp; the pretty-printed lane inputs `lane_c/next_corners.json`, `lane_c/next_live_round.json` and `w3-effects/v9-plain.json` are canonicalised on save (same decoded document, compact bytes — kept frozen as decode inputs); and rewriting a native v9 container changes its PDF text representation (text is embedded as real text since v14 instead of outlines), so only the outlined appearance and the non-text resources are compared to the v9 oracle. Older-era PDF goldens without text compare
byte-for-byte with the stamp normalised (`varos-pdf/tests/support/native_era.rs`).

**Consequences.** Builds that write 9 (wave 2) refuse every file this build saves; no downgrade save.
Deferred, recorded rather than hidden: effects stay Path-level while appearance owns stack entries
(effects are not yet per stack entry); text nodes carry no appearance stack.
