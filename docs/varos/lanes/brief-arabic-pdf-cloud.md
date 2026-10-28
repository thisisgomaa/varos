# Task — Arabic PDF copy-back bugs (repo thisisgomaa/varos-next, branch off `main`)

Create branch `lane-arabic-pdf-cloud` from `main`; when done push it and open a PR to `main` titled
"fix(pdf): Arabic copy-back keeps ligature words and stroked Force-kashida words intact". Do not merge.

Two known bugs (see `docs/varos/BACKLOG.md` lines ~61 and ~79) in the PDF writer's text layer for Arabic
(`crates/pdf/`, `crates/text-arabic/`; read `docs/varos/` notes on kashida first — Off/Auto/Force +
strength, OpenType and AAT/morx fonts):
1. **Ligature words read back reversed**: a word containing a ligature (the Allah ligature) is copied from
   the exported PDF (Preview/Acrobat/pdftotext) in the wrong order. Ignored test
   `a_word_with_a_ligature_reads_back_in_order` in `crates/pdf/tests/arabic_copy_back.rs` — un-ignore and
   make it pass. Likely cause: the ActualText / ToUnicode mapping for multi-codepoint glyphs and the
   logical-vs-visual order of the run; fix at the writer so the authored string is what copy-back returns.
2. **Stroked Force-kashida words read back with spaces** at the unioned outline joins. Ignored test
   `stroked_force_kashida_keeps_authored_searchable_text` (same file) — un-ignore and make it pass. The
   searchable text must be the authored text (no inserted tatweel, no spaces), while the painted
   stroked outline stays as it is.
Add fixtures for both (an OpenType Arabic font from the repo's test fonts and an AAT one if available),
verify with `pdftotext` (install poppler in the environment) and with the crate's own reader.
Scope: `crates/pdf`, `crates/text-arabic` (and `crates/text` only if a shaping fact is wrong). Do NOT touch
`crates/ui-egui`. Gates (exact lines in the PR): `cargo test -p vectorcraft-pdf -p varos-text-arabic`
(or the crates' real names) all green incl. the two un-ignored tests, `cargo test --workspace --release`
(known pre-existing release-only ui failures may be ignored: 9 `panels::gradient::tests::*` +
`tests_rotateview::escape_after_closing_a_popup…`), `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo fmt --check`, `git diff --check`. In the PR: the root cause of each bug and a before/after
`pdftotext` sample.
