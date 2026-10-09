# ADR-0008 amendment — pre-assigned wave-3 v13 live nodes

2026-10-09 · Lane E · owner-authorized provisional UI, integration and design review pending.

The writer emits format **13**, its lane-assigned number. `nodes[].kind.Live` adds Blend,
Repeat and Envelope source containers; evaluated geometry never serializes. The pure
`migrate_v12_to_v13` returns the input unchanged, because older nodes retain their exact
meaning and absent keys do not invent objects. Other wave-3 fields are optional/absent.

The worktree's three preceding identity rows reserve v10–v12 only so local v1–v9 files can
load. The integrator replaces those rows with the sibling pure steps and rechains in the
assigned order; no claim that those sibling formats are implemented here.

A ≤12 stamp with the new Live enum is refused before typed decode. Frozen v13 JSON/native
PDF fixtures, malformed parameter and future-version refusals, and an adapted v12 header
gate demonstrate round-trip durability and older-reader refusal. Historical fixtures and
API 1.0/1.1 bytes remain frozen. There is no downgrade save.
