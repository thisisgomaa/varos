# Lane H — Text P5–P8 / FORMAT 14

- P5/P6/P8 implemented; provisional UI, owner design review pending; original work committed before this fix round.
- P7 partial: named styles/OpenType/static faces delivered; variable axes and per-field cascading remain OPEN in the programme.

## Fix round

- Reviewed the independent FIX-THEN-MERGE findings; addressed all seven findings within the lane scope.
- PDF: per-glyph page/clip eligibility prevents off-page extraction; regression covers page and persisted clip-tree exports.
- Editing: composition, hit testing and scene snapshots remap named spans; deletion/empty/Arabic replacement + undo regression.
- Clipboard: capture connected stories, boundaries, features and referenced style ancestry; remap IDs/names; cross-document and Cut/Paste/Undo tests.
- Movement: binding origin + shared boundary-to-text conversion; text/joint translation, live rotation and path-caret tests. Disagree that flattening omitted live transforms: it already used world geometry.
- Properties: resolved values, staged editable overrides and named-style update from the assigned range; headless regression.
- Frozen v9: compare every legacy appearance object to the original oracle (only stamp/model normalization), plus real-writer image/shading resources; fixtures untouched.
- Wire contract updated with v14 keys, limits, coordinate semantics, migrations/refusals; P7 completion claim explicitly narrowed.
- Integration: per brief, retain isolated v9→14 bridge until integrator supplies real sibling v10–13 chain; sibling CMYK/appearance/effects reconciliation remains integration work.
- Gates: fmt, dependency directions, whitespace PASS; final workspace: 2,274 passed / 0 failed / 15 ignored, 150 targets.
- Native + Windows all-target Clippy -D warnings PASS; varos-text wasm32 check PASS; offline builds used -j 3.
- UI ratchets 3/3 PASS (unchanged); ui.rs 805/843. Bridge 6/6 PASS: 1.0/1.1 frozen 23,993 B; 1.2 23,919/24,000 B.
- No git writes, push, merge, GUI or installation; independent re-review and owner acceptance pending. Final suite evidence: /tmp/w3-text2-workspace-final.log; other gates: /tmp/w3-text2-*.log.
