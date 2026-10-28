# Arabic PDF copy-back — review follow-ups (worktree lane-arabic-pdf-2, branch off main)

You are an implementer in this git worktree (already on branch lane-arabic-pdf-2). Do NOT commit; the
orchestrator commits. Keep one task: test coverage + any writer fix it reveals, in `crates/pdf` only (and
`crates/text-arabic` only if a shaping fact is wrong).

Context: PR #4 (merged) fixed two bugs in the PDF writer's text layer for Arabic: ligature words (Allah) read
back reversed, and stroked Force-kashida words read back with spaces. Read `crates/pdf/tests/arabic_copy_back.rs`,
`crates/pdf/tests/fixtures/arabic-copy-back.md`, and the fix in `crates/pdf/src/export.rs` (visual-order
ToUnicode/ActualText mapping for RTL clusters; invisible-glyph matrix expansion over virtual kashida).
Also read docs/varos/BACKLOG.md "2026-10-28 — cloud batch notes" for the review follow-ups.

Add fixtures + tests, each verified with the crate's own reader AND with `pdftotext` when installed
(`which pdftotext`; if absent, the pdftotext assertions must be skipped with a visible note, not faked):
1. Auto kashida that actually elongates: a justified AREA text (not point text) wide enough that Auto inserts
   virtual kashida; assert that elongation happened (glyph advances grew) and that copy-back equals the
   authored string exactly.
2. Mixed scripts in one paragraph: Arabic + Latin words with punctuation (",", ".", "(", ")"), digits in both
   Arabic-Indic and ASCII sets, and a font change mid-line; copy-back order and characters exact.
3. Other RTL clusters: a Hebrew word with a multi-codepoint cluster (niqqud) and a Persian word with ZWNJ;
   copy-back exact (the mapping change applies to all RTL scripts). Use fonts already in the repo's test
   fixtures; if no Hebrew-capable font exists, say so and skip that case visibly.
4. Byte-identity: a Latin-only document's PDF bytes are identical with and without the Arabic mapping code
   path (if the writer has a switch) — otherwise assert a frozen hash of a Latin-only export with a
   documented regeneration command in the test.
5. If any case fails, fix the writer in `crates/pdf/src/export.rs` at the same layer (no shaping hacks) and
   explain the root cause in your final report.

HEAT RULE (hard, this Mac): another cargo lane is running. Write all fixtures/tests FIRST without cargo.
Before your FIRST cargo command, wait until the file
/private/tmp/claude-501/-Users-gomaa-Documents-AI-workspace-varos/90bf49a1-794a-43e8-b7df-7f72ceea7227/scratchpad/go-arabic2-cargo
exists (`test -f`; if absent `sleep 60`, up to 180 times). Then run ONE cargo command at a time, always with
`-j 2`, `--offline`, tests with `-- --test-threads=2`, and
`CARGO_TARGET_DIR="~/Documents/AI workspace/lanes/_target-shared"`.
Gates: `cargo test -p vectorcraft-pdf -p vectorcraft-text-arabic --release` (0 failures),
`cargo clippy -p vectorcraft-pdf -p vectorcraft-text-arabic --all-targets -- -D warnings`, `cargo fmt --check -p vectorcraft-pdf`,
`git diff --check`.

Report contract (final message): for each of the 5 cases → DONE / SKIPPED (why); any writer fix with root
cause and file:line; the exact `test result:` lines of each gate; files touched; anything left.
