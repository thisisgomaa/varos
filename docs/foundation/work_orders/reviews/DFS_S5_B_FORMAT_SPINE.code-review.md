> **Status:** reference — independent code review, 2026-09-24.
# DFS S5-B — format spine: code review

Piece `DFS_S5_B_FORMAT_SPINE` · branch `feat/dfs-s5-b-format-spine` (`91ccde1` + merge `380ef35`) · diff base `130bc85` · contract: `DFS_S5_FORMAT_V2.md` §3–§4 (amended), ADR-0008 (Proposed).

## Verdict: APPROVE WITH NITS
1. No data loss: all 16 frozen S5-E v1 fixtures + the 3 legacy fixtures load, equal the pre-S5 reader's result exactly, and pass the round-trip law (raw and PDF, `content_eq` and full `==`, A == B bytes).
2. The version gate, strict decode, iterative structure check, id headroom and v1→v2 migration are correct. A 100k-deep chain and 3-node cycles are refused on a 64 KiB stack.
3. No P1. Two P2s are about the user-facing text: serde internals leak into `Display`, and the app replaces the new reasons with generic text. One P2 is a design gap for the moderator: an over-limit document cannot be saved or recovered.
4. `varos-app`, the `varos-pdf` source and the `file.rs` signatures are unchanged. The two PDF fixtures differ in exactly 2 bytes each.
5. ADR-0008 is still Proposed, so nothing reaches `main` until Ahmed accepts it (work order R1).

## Gates I measured (worktree, `CARGO_TARGET_DIR=/home/user/varos/target-review-s5b`, sources touched first)
- `cargo test --workspace -j 4`: **554 passed, 0 failed, 1 ignored** (44 binaries). Matches the report.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `cargo fmt --all -- --check`: clean.
- `cargo clippy -p varos-app --all-targets --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin` with `-D warnings`: both clean.
- Scratch probe crate (path deps on this worktree, nothing in the worktree edited), `scratchpad/s5b/probe`:
  - **Fixtures (check 1):** 19/19 load with `migrated = true`, including the 8 PDF files through `varos_pdf::load_vrs`. Each result `==` the old reader (`serde` + `sync_tree`). Raw: save A → reload (`content_eq` and `==`) → save B, A == B. PDF: `save_vrs` A → `load_vrs` → `save_vrs` B, byte-identical.
  - **Old-reader harness** (S5-E's `old_reader_harness.rs`, both `#[ignore]` tests run with `--include-ignored`): **2/2 PASS** on S5-B. Both the JSON and the PDF output are refused as "newer Varos".
  - **Release timing:**

    | case | model | `encode_model` | of which `sync_tree` | of which read-back | `decode_model` |
    |---|---|---|---|---|---|
    | 40k paths × 1 anchor | 15.2 MiB | 1.22 s | 1.16 s | 38 ms | 1.30 s |
    | 40k × 4 anchors | 24.3 MiB | 1.48 s | 1.28 s | 82 ms | 1.30 s |
    | 10k × 25 anchors | 21.9 MiB | 177 ms | 28 ms | 65 ms | 139 ms |

    I could not reproduce the report's "69 MiB" figure. Its meaning is not stated, and the model blob at the 40k cap is 15.2 MiB.
  - **`cmp -l` (check 7):** `native_demo.pdf` differs at offsets 1293 and 3711 only; `native_rich.pdf` at 2642 and 8664 only. Every change is `061→062` ('1'→'2'), one in `{"varos":2` and one in `/VAROS_SchemaVersion 2`.

## Checks from the brief
- **(2) `deny_unknown_fields` and in-memory serde:** only the file format uses serde on these types. `clipboard.rs` has no serde, undo history uses `clone()`, and `varos-app` never (de)serializes model types. The flag is compatible with `SnapConfig`'s struct-level `#[serde(default)]`, and no struct uses `flatten`. Safe. S3-C's future recovery snapshots will use `doc_to_blob`/`doc_from_blob` and will inherit the save-side refusals (see P2-3).
- **(4) Id headroom:** stale counter + tree-less path → node ids `[1,2,4]`. The pre-S5 reader would have handed out node id 1 a second time; on my probe that duplicate made `sync_tree` overflow the stack. A legacy registry with counter 0 and anchor ids up to 8 gives nodes `[1,9,10,11]`, next id 12. Raising the counter before `sync_tree` works, and `check_structure` runs again after migration, so a duplicate id cannot survive. The `id_exhaustion_refused` boundary test (`u32::MAX-6` refused, `-7` accepted) catches an off-by-one.
- **(5) Iterative:** 100k-deep node chain → `TooLarge{TreeDepth, 100001}` in 98 ms, run on a 64 KiB thread stack. With `max_tree_depth = usize::MAX` it passes, so the walk itself is iterative. A 100k legacy `groups[].parent` chain is refused the same way. 3-node cycles: detached → `Cycle{10}`; hanging from a root → `BadParentage` ("listed in more than one place"); legacy → `Cycle{5}`.
- **(8)** `VRS_VERSION` is an alias of `FORMAT_VERSION` = 2, and `write.rs:286` picks it up. The `doc_to_blob`, `doc_from_blob`, `load_vrs` and `write_atomic` signatures are unchanged. `91ccde1` touches no `varos-app` file and no `varos-pdf/src` file.
- **(10) The 40,000 limit:** it is honest for V1. Astra's test file (`docs/audits/2026-09-24-ASTRA_USER_TEST.md`) is a hand-drawn logo with a handful of paths, and a logo sheet has hundreds. Only traced or imported art reaches 40k, and import is out of scope. Load time at the cap is 1.3 s, and 90% of it is `sync_tree`'s quadratic cost, not the new code. The numbers are declared once, in `Limits::DEFAULT` (`limits.rs:37-48`); grep finds no second copy.

## Findings

**P2-1 — Serde internals leak into the user-facing text.** `error.rs:89` puts the raw serde error inside `Malformed`'s `Display`, and the save backstop at `mod.rs:180` does the same with `{e}`. What I measured:
- `…(invalid type: integer \`42\`, expected struct Document at line 1 column 19).`
- `…(unknown field \`bogus\`, expected one of \`paths\`, \`groups\`, \`group_of\`, … \`move_art_with_ab\` …)`
- `…expected u32…` and `…expected f32…`
- Save: `This document can't be saved: a number in this document (the saved data: invalid type: null, expected f32 at line 1 column 289) has a value that is not a finite number.`

The work order says `Display` is the user-readable reason (§3 Public API).
**Fix:**
- `Malformed`'s `Display` shows only "This file is damaged or not a valid Varos document (line L, column C)." Use `serde_json::Error::line()/column()`, and keep the raw text in the payload for `Debug` and logs.
- The backstop becomes `Invalid::NonFinite { what: "a number in this document".into() }`, without `{e}`.
- Add a test that no `Display` contains `struct`, `u32`, `f32`, `expected one of` or `serde`.

**P2-2 — Cross-piece, for the moderator: the new reasons do not reach Ahmed yet.** `varos-app/src/file_ports.rs:171-195` (`plain_reason`, from S1) keeps only the "newer Varos" text. Every other refusal is shown as generic text:
- On open (`LoadError`): "It isn't a Varos document, or it is damaged."
- On save (`SaveRefused`): "Varos couldn't write the document."

Hand test 2 ("readable refusal") and R5 ("a refused save leaves the document dirty **with the reason**") are therefore not met in the real window. S5-D will make it worse: once `varos_pdf::load_vrs` returns `LoadError`'s text, I/O errors read "The file could not be read (…)", and the `"read failed: "` prefix matching in `plain_reason` stops recognising them.
**Fix:** in the S1/S5 integration piece, pass `LoadError`/`SaveRefused` `Display` through as-is, since it is already plain once P2-1 is fixed. Map only `LoadError::Io` through `io_reason`. Record this in STATUS.

**P2-3 — Design gap, for the moderator (R5): an over-limit document cannot be saved or recovered.** `encode_model` refuses more than 40,000 paths or nodes (and, until S5-C lands, a NaN). Two things go through the same `doc_to_blob`: the save, and S3's recovery snapshot (work order §7). Once a document goes over the limit, neither works, so the only copy is in memory. The editor gives no warning near the cap. R5 assumed "S3 recovery still holds it", and with this design it does not.
**Fix (outside B):**
- S3-C's snapshot should fall back to a raw serde dump marked unvalidated, or the editor should warn at about 90% of a count limit.
- Write the decision into R5.

**P3-1 — `mask_child` is not in the dangling check.** Missing from `structure.rs:41-62`. Probe: a Layer with `role: Clip` and `mask_child: 203` (no such node), plus one tree-less path. The migration hands out node 203 to the adopted path, so the clip silently binds to an arbitrary shape. Both load and save accept it.
**Fix:** in the node loop, `if let Some(m) = n.mask_child { if !index.contains_key(&m) { return Err(Invalid::Dangling{from:"node", id:n.id, missing:m}.into()) } }`. The "mask shape is missing" branch at `migrate.rs:61` then cannot be reached, so drop it. S5-C's Clip-only-on-Group rule closes the Layer variant too, but this check is B's own "no dangling references" rule.

**P3-2 — A mixed-era v1 file loads with a duplicated leaf.** Probe: a tree plus a legacy registry → path 1 ends up with 2 leaves and is drawn twice. The cause is `migrate_legacy`, which creates a leaf for every path. No writer ever produced such a file, so only a hostile file hits this. S5-C's "every path has exactly one leaf" rule refuses it. Note it in S5-C's PR, not here.

**P3-3 — What the read-back actually proves (check 3).** The typed decode of the bytes just written (`mod.rs:176-181`) proves only that serde can read them back. In practice that catches non-finite floats. It does **not** prove the file reopens: it skips the structure, canonical and limit checks. What guarantees reopen is this: the same `check_structure` and `validate` run on the normalized clone, and `sync_tree` is idempotent. `every_saved_doc_reopens` tests that on 9 documents.

Cost: 38–82 ms. That is 3–6% of a save at 40k paths, but 30–37% for anchor-heavy art, paid by every S3 snapshot.
**Recommendation:**
- Keep it while `validate` is a stub, because it is the only NaN guard today.
- Delete it when S5-C's finiteness rule lands. That rule names the object, which the backstop cannot.
- Do **not** replace it with a full `decode_model`, which would add another ~1.2 s of `sync_tree` at 40k.
- The implementer's report ("reads its own output back" ⇒ reopen guaranteed) slightly overclaims this.

**P3-4 — Repeated `check_structure` calls.** It runs 3× on a v1 load (`mod.rs:116`, `migrate.rs:36`, `mod.rs:120`) and 2× on a save (`mod.rs:163,165`). Each run is linear and takes milliseconds.
**Fix:** drop the call at `migrate.rs:36`. `decode_model` already ran it, and the doc comment can say "requires a structure-checked document".

**P3-5 — The limits doc is out of step.** `limits.rs:33` says the numbers are "mirrored in `docs/reference/VRS_FORMAT.md`", but S5-E's §8 still reads "(to be filled by S5-B)" with 100,000 nodes and paths. B does not own that file, so the moderator fills in the enforced column (40,000) at merge. `max_anchors = 1,000,000` can never be reached: the 32 MiB model cap binds first, at about 350k anchors (~92 bytes per anchor with handles, measured). State that in §8.

**P3-6 — Waste check (check 9): nothing to remove.**
- `read_bounded` has a consumer (`file::load_vrs`), and S5-D will use it.
- `MIGRATIONS`/`Step`, `MigrationFailed` and the PDF `LimitKind` variants are API the contract requires, and C/D consume them.
- `human_bytes`/`thousands` do not duplicate any helper in the workspace (grep).
- The tests match the 26 named in the contract, plus the ignored timing test. `new_saves_write_format_2`'s constant asserts and `limits_default_values_match_documented_numbers` only detect changes, but the contract asks for them.
- Mutation spot-checks would be caught: removing the counter raise (`migrate.rs:43`), removing the allocation term in the headroom check, and removing the canonical node comparison.

## Not verified
- The Mac hand tests. Hand test 0 (the corpus check) still gates `main`.
- The "69 MiB" figure in the report.
- Peak memory at the caps.
