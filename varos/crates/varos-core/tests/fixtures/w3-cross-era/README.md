Cross-era refusal fixtures (integration w3, 2026-10-10). Each file claims an older wave-3 format but
carries a key that only a later format writes; the reader must refuse it before any typed decode with
a specific `FieldNotInFormat` error (`varos-core/tests/integration_w3.rs`).

| File | Claims | Carries | Refused as |
|---|---|---|---|
| `v10_stack_width_profile.json` | 10 | `width_profile` inside an appearance stroke entry (v11) | `effects/width_profile`, v10 |
| `v11_managed_colour.json` | 11 | managed CMYK swatch paint (v12) — the mixed v14 body restamped | `managed colour`, v11 |
| `v11_live_node.json` | 11 | `NodeKind::Live` (v13) — the frozen v13 live body restamped | `nodes[].kind.Live`, v11 |
| `v13_typography.json` | 13 | `doc.typography` (v14) — the mixed v14 body restamped | `typography`, v13 |

Derived once from `v14-mixed/mixed.json`, `v13/live.json` and `v10/multiple.json` (stamps / one key
only); tests never regenerate them. `SHA256SUMS` covers every file.
