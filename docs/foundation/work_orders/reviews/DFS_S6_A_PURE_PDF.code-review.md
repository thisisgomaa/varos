> **Status:** reference — independent code review, 2026-09-24.
# Code review — S6-A pure PDF export writer + page-scope planner (`DFS_S6_A_PURE_PDF`)

Worktree `agent-a6f3ef5b2ed82d21b`, branch `feat/dfs-s6-a-pure-pdf`, `git diff c9067d4...HEAD` (3 commits, varos-pdf only).
Contract: `DFS_S4_S6_ASSOCIATION_EXPORT.md` §3.3 + S6-A (amended for review F1–F3).

## Verdict: REQUEST CHANGES
1. The refactor is clean and faithful. `native_demo.pdf` is byte-identical to the pre-refactor writer, and the clip is correct: even-odd `W* n`, nearest clip group only, `q`/`Q` balanced everywhere.
2. **P1:** the mask path emits *every* mask ring for each member, so a hidden path standing only on a hidden board leaks its exact coordinates into the "no hidden data" export when it is part of a mask group (reproduced).
3. **P2:** the same per-member re-emission bloats files: 50 members under one 64-anchor mask gave 157 KB instead of about 12 KB (13.6×).
4. **P2:** no test covers a mask with several paths or holes, a nested clip, or a knockout member inside a clip. A mutation that emits only the first mask path's outer ring passes all 27 tests.
5. The rest is small: one duplicated bbox helper, an unused error variant, and three tests that prove little.

## Gates (measured by me, in the worktree `varos/`)
- `cargo test --workspace -j 4`: **373 passed, 0 failed, 0 ignored** (`export_pdf.rs`: 27 passed).
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean · `cargo fmt --all -- --check`: clean.
- `cargo clippy -p varos-app --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin` with `-D warnings`: both clean.
- **Byte identity:** I built the untouched `c9067d4` sources in scratch and ran `write_pdf(demo_doc())`. The result compared equal to `tests/fixtures/native_demo.pdf` (`cmp`: identical). The old `native_rich.pdf` (c91b634) also equals the c9067d4 output.
- **`native_rich.pdf` object diff (old vs new):** 16 vs 16 objects. Only obj 15 (board B's content stream) differs: `/Length 203→315`, plus `q <mask rect: m + 4 c + h> W* n`, then the member's own `q … Q`, then the closing `Q`. Beyond that, only the xref offsets and `startxref` change. The trailer is `/Size /Root` only, with **no /Info and no /ID**.
- **q/Q audit (scratch test, every stream):** balanced, with depth never negative. Covered cases: the rich doc, a rotated clip group, a knockout member inside a clip, a mask with a hole plus a second mask path, and nested clips (inner clip only: 1 `W*`, the outer mask ring absent). This matches `scene.rs` `clip_group_of`.

## Findings

1. **P1 — Hidden geometry leaks through mask rings.** `write.rs:127-135` gathers *all* paths under `node_mask_child(c)`, and `write.rs:199-201` emits them all for every member. Nothing checks that a ring can affect that member or page.
   - *Repro (scratch):* a mask group of {m1 on the visible board, m2 with `hidden = true`, standing only on a hidden board at x = 1077.625}, and one member on the visible board. The export contains `1077.625 266.625 m … 1097.625 …`, which is m2's exact outline. The contract says "no hidden or editable data", and `export_leaks_no_hidden_coordinates_or_names` pins exactly this class (a path on a hidden board) for ordinary paths.
   - *Fix (exact, not a heuristic):* per member (or per run, see 2), keep only the mask paths whose `world_bbox` intersects the member's padded `world_bbox` ∩ the page rect. Under even-odd, a ring whose box misses a point cannot contain that point, so dropping it leaves the clip unchanged everywhere the member can paint. A hidden mask path that *does* overlap still clips, as on the canvas (`scene.rs:528-548` ignores the eye).
   - *Test:* add a hidden-board mask path with a unique coordinate to `secret_doc`'s pattern, and assert that the coordinate is absent from the raw bytes and from every stream.
2. **P2 — The mask is re-emitted once per member.** `write.rs:197-203` and `:263-265` wrap each member separately. *Measured:* 50 member rects plus a 64-anchor circle mask give 156,889 B clipped against 11,538 B unclipped.
   - The canvas already treats a clip group's members as one contiguous run in `paint_list` (`scene.rs:618-661`, `Group::Clip { mask_rings, members }`).
   - *Fix:* keep a `cur_clip: Option<u32>` in the page loop. When `clip_group_of` changes, close the previous `Q` and open one `q <rings> W* n` for the run. Cull rings against the run's union bbox, as in 1. Close any open scope before the page ends. That is about 10 lines, and knockout XObjects stay inside the scope unchanged.
   - This deviates from the literal "per path" wording of review F1, but not from its intent (MASKS_PLAN §2.4 run structure). Re-bless `native_rich.pdf` and say so in the PR.
3. **P2 — The "ALL rings" claim is untested.** Every clip test uses a single-rect mask (`export_pdf.rs:104-116`, `:612-625`).
   - *Mutation:* I replaced `for (mp, mxf) in mask { emit_rings(..) }` with `mask.iter().take(1)` + `emit_ring(outer only)`, and **all 27 tests still pass**.
   - *Fix:* add `clip_uses_every_mask_ring_even_odd`: a mask group of 2 paths, one with a hole. Assert 3 `m` subpaths precede `W*` and none follows `f`/`B`.
   - Add `nested_clip_uses_nearest_mask_only` (inner mask ring present, outer absent).
   - Assert the knockout path: `Do` inside the clip scope (`clip_state[do_idx]`).
4. **P3 — Duplicated geometry helper.** `write.rs:388-400` `world_bbox` is line for line `varos_core::flatten::control_bbox_with` (`flatten.rs:95-107`; `control_bbox(doc, pi)` is public). It moved verbatim, so this is not new, but the new mask cull (`write.rs:136-140`) adds callers.
   - *Fix (follow-up is fine):* use `flatten::control_bbox(doc, pi)` ± pad. `drawable` already has `pidx`. The XObject /BBox values are identical, so the bytes do not move.
5. **P3 — `ExportError::Write(String)` (`export.rs:85`) is never constructed.** `write_pages` has no failure path, and `write_pdf` maps through `to_string`. Keep it only if S6-B maps its write errors into it, and note that in S6-B. Otherwise delete it.
6. **P3 — `ExportPlan::page_count()` (`export.rs:47-52`)** is `pages.len()` on a public field, with one test caller. Inline it, or keep it for S6-B's page count copy.
7. **P3 — `drawable` builds the mask `Vec` before the page cull** (`write.rs:90-93`). On N pages, every member builds it N times, even when the member is off the page. Swap the order: `bbox_hits` first, then the mask. This is cheap to fix together with 2.
8. **P3 — Tests that prove little or duplicate each other.**
   - `native_save_is_unchanged_by_the_export_path` (`:700-715`): `write_pdf` is a pure function over `&Document` with no shared state, so before == after always holds, and the round-trip half duplicates `container.rs`. Delete it, or keep only the round-trip of a *clipped* doc (MASKS_PLAN Stage 5 asked for that; it is the one useful half).
   - `export_is_deterministic` (`:672-678`) makes the same assertion twice.
   - `find_op` (`:577-583`) re-implements `has_op` (`:192-198`). Use `find_op(..).is_some()`.
   - `export_has_no_embedded_file…` checks `/Info` but not `/ID`. Add `!pdf.trailer.has(b"ID")`.
9. **P3 — `has_embedded_model` costs about 0.4 s at the 256 MiB cap** (release build, measured). That is fine as a bounded scan, but S6-B must call it off the UI thread (inside the export job, or before the dialog completes). Record this in S6-B.
10. **P3 — Header comment `write.rs:5`** says the loop was "moved here verbatim". Since 7e3d190 it has gained the clip. Add "+ the Stage 5 clip".

## Specific questions from the moderator
- **(6) Stroke padding with miter joins:** not an issue. Every PDF stroke is written with `RoundCap`/`RoundJoin` (`write.rs:177`, `:212`), so no point of the stroke lies farther than w/2 from the curve, and half-stroke padding is exact. The only slack is `outline_bbox`'s 12-step flattening. It can under-read an extremum that falls between samples by the sagitta (≈0.2 pt on a r = 100 quarter arc), which trims a hair at the page edge. That is a nit and needs no action.
- **(5) Dead code:** `PlannedPage`, `ClipMasksNotSupported`: none remain in the code (grep clean; they are only in the WO's fallback text, which is correct).
- **(8) `.gitattributes`:** `tests/fixtures/.gitattributes` = `*.pdf binary`, scoped to that folder. `git check-attr` shows `binary/diff/merge/text` unset for the fixtures. The fixtures are not ignored by any `.gitignore`. Correct.
- **Privacy walk otherwise:** hidden path, hidden layer, hidden board, names, the model, Info: all absent. `hidden_and_locked_state_change_no_export_byte` is the strongest test in the suite. The only leak I could construct is finding 1.
