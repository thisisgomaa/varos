# Lane H implementation decision — 2026-10-09

Owner instruction authorizes P5–P8 with provisional existing-kit controls. Typography
extensions live in a document sidecar: live path identity, flow links, OpenType
features and named style assignments. Logical Unicode stays in TextBox runs.
Core owns validation/commands; the existing text adapter owns geometry and shaping.
Format 14 is reserved for these keys. This isolated v9 checkout bridges directly
to v14; the integrator must insert the other lanes' named migrations in order.
PDF embedding uses shaped glyph identities and source clusters, with explicit
outline fallback for unsupported font programs/licence restrictions. No GUI,
installation, commits or pushes are authorized in this worktree.

Implementation surface:
- `typography` document sidecar (v14): frame binding (`Area`/`Path`), next-frame
  identity, named character byte ranges/paragraph style, feature overrides.
- `EditCommand::Typography(Action)`; Bridge API 1.2 `typography` is discoverable
  through `list_verbs` and full `schema`, without an inline tools/list summary.
- Headless CLI: `varos apply INPUT --batch ACTIONS.json --out OUTPUT`, where the
  batch is `{"api":"1.2","commands":[{"Typography":{"action":"features",
  "text":200,"features":{"liga":1,"calt":1}}}]}`. Other Actions use the same path.
- Core has no text-engine/UI dependency. PDF consumes `text-layout::font_export`.
- Fonts are identified by their existing immutable hashes. Named styles resolve
  at composition; style updates invalidate the bounded advanced-layout cache.
- CFF subsets are reencoded unhinted cubic outlines in a CID-keyed CFF program.
  TrueType subsets retain original glyph IDs, composite closure and font hints.
  Duplicate cluster auxiliaries stay vector outlines; the selectable glyph carries
  the whole source cluster exactly once. Curved/translucent or disallowed font
  programs explicitly fall back to outlines. SVG remains outline-first.
- Package includes only known bundled OFL font bytes, exact hashes, licence text,
  face/axis identity and a deterministic manifest. Other font redistribution is
  refused; no implicit licence grant is inferred from embedding permissions.
- No native appearance/IME/PDF-viewer print acceptance is claimed by headless tests.

Review boundaries:
- Frame allocation applies the existing two-line widow/orphan policy at thread
  boundaries; empty/too-small frames retain overset source for later frames.
- Curved caret/pointer maps use the same arc-length baseline and flip as glyphs;
  marks sharing a shaped cluster use its base glyph tangent.
- P7's broader programme exit is not complete: variable font axes are not exposed
  by the shipped engine. This lane adds no synthetic weights or axis emulation.
  The existing immutable static-face weights and Arabic-zero tracking remain.
- Named-style inheritance replaces a whole character/paragraph definition;
  field-by-field cascading and typography-aware rich clipboard transfer are not
  implemented. Source edits remap named ranges through Unicode-safe replacement
  boundaries so subsequent named-style updates still apply.
- External PDF viewer copy/print and native IME/path-edit UX need owner review.

Integration seam (paths relative to `varos/crates/`):
- `varos-core/src/format/migrate.rs`: replace the temporary v9-to-v14 branch
  with the combined v9→v10→v11→v12→v13→v14 table entries. Remove the temporary
  extra `v.push(9)` once v9 is a table key; the final step calls
  `typography_format::migrate_v13_to_v14`.
- `varos-core/src/format/mod.rs`: remove the isolated-lane rejection of versions
  10–13 when their readers/migrations land; keep the >14 future-version refusal.
- `varos-core/src/model.rs`: the optional empty-skipping typography sidecar is
  additive. Merge the other lanes' optional data before exercising the combined
  fixtures; this checkout cannot validate schemas it does not contain.
- Keep API 1.0/1.1 fixtures frozen. The extra progressive-disclosure verb leaves
  only 81 bytes in the current API 1.2 summary, so combined-lane growth must be
  checked again by the integrator.
