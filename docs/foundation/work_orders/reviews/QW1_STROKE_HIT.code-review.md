> **Status:** reference — independent code review, 2026-09-24.

# Code review: QW1, thick strokes hit where they are painted (Astra F08) (`fix/qw1-stroke-hit` @ `00cfc41`, base `5ef0687`)

**Verdict: REQUEST CHANGES. There is one P1, and the fix is one line plus one test. The rest is sound and can merge after that.**
1. The painted-band reach, the occlusion walk, the "filled-only" marquee rule and the rotate-ring guard are correct. They are tested, and the tests catch mutations. They follow the work order and its review (P1-1, P2-1, P2-2).
2. `cubic_nearest` stalls on **straight segments**. Every rectangle and polygon edge is `(p0, p0, p3, p3)`. Near a corner, a click exactly on the centreline still measures far off: up to 21.9 units on a 20000 edge. So "distance to the TRUE cubic" is not true for the most common shape (P1-1).
3. Hover is 1.3× slower (measured). That is under the 2× bar, so it is not a P2. The absolute cost was already high before this piece (P3-1).
4. The marquee hole-rim branch has no test. A mutation that removes it passes the whole suite (P2-1).
5. The occlusion is safe: a thick band lower in the stack does not take a click from a thin path on top (verified with a throwaway test).

## Gates (measured by me in the worktree, `varos/`)
- `cargo test --workspace -j 4`: **360 passed, 0 failed**. This matches the report. `hit_thick_stroke.rs` 13/13, `occlusion.rs` +1.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `cargo fmt --all -- --check`: clean.
- `cargo clippy -p varos-app --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin` with `-D warnings`: both clean.
- `git merge-tree` against the current work branch `claude/sweet-cerf-1sg30t` @ `580436f`: clean, no conflicts.
- Hover benchmark (release; 500 unfilled rings × 50 cubic segments; 1000 `path_under` probes, mostly misses; 2 runs each). **Old `5ef0687`: 10.2 / 10.9 ms per call. New: 13.6 / 13.5 ms per call (≈1.3×).**
- Marquee benchmark (`path_in_rect` × 500 paths per move). ppu 1: old 4.2–4.5 ms, new 2.6–3.1 ms. ppu 8: old 4.4–4.5 ms, new 6.3–7.1 ms (≈1.5×, from `ring_px` chords of ~4 screen px).
- Mutation checks:
  - Setting Newton to 0 iterations fails `centreline_between_flatten_samples_is_hit`.
  - Removing the hole-rim branch in `path_in_rect` fails **nothing** (P2-1).
- The throwaway tests and the benchmark were run, then deleted. A copy is at `scratchpad/zz_review_throwaway.rs`. The worktree was left clean.

## Verified correct (evidence)
- **Numerical robustness (brief item 1).**
  - Zero-length segment (two coincident anchors): d = 5.000 exactly, t = 0.083, finite.
  - Handles that coincide with their anchors: finite, t within [0, 1].
  - A teardrop whose two anchors coincide, and a far point at (1e6, 1e6): finite.
  - NaN cursor: `nearest_seg` returns `(0, 0, NaN)` and `path_under` returns `None`, with no panic. The old code did the same.
  - `t` is clamped to `[lo, hi] ⊂ [0, 1]`. The `d < guess_d` guard means Newton can never make the answer worse.
- **Occlusion (brief item 4).** Setup: an 80-wide band at the bottom, with a 1-wide line on top of it at x = 20.
  - On the line, and 7 px off it: the top line wins.
  - At 10 px off: the band below wins, as it should.
  - A filled thin rectangle on top of the band: its fill wins.
  - The top-down walk (`editor.rs:519-537`) returns the first path that hits. The top path's reach (8/ppu + its own half width) always covers its own painted area, so a lower band cannot take a click from it.
- **Marquee semantics (brief item 3).** Nothing contradicts them:
  - `SNAP_TRANSFORM_SPEC.md` has no marquee enclose/touch rule.
  - `VECTOR_BUILD_SPEC.md:34` says "Marquee = **touch-intersect**", and the hollow of an unfilled ring touches nothing that is painted.
  - `input_feel.rs` has no marquee tests. `occlusion.rs:73` and `live_transform.rs:154` pass without edits.
- **Snap (brief item 5).**
  - Every `snap.rs` test uses straight edges probed at t = 0.5, where the old sample was already exact, so none of them should have changed.
  - Object-geometry snap (`editor.rs:2920`) now gets the refined distance. Anchor-drag snap does not (P3-3).
- **Rotate-ring guard (brief item 6).**
  - `is_none_or(|id| self.objsel.contains(&id))` (`editor.rs:771`) is exactly what review P1-1 asked for.
  - The side effect: a click within 8 px outside a corner, on the selection's own outline, now rotates instead of re-selecting what is already selected. That is desired. Re-selecting the current selection does nothing useful, and the Illustrator bounding box shows the rotate cursor outside a corner even over the selected art's own stroke.
  - The other side is pinned: another object's band still wins (`rotate_ring_yields_to_another_objects_band`). Flipping the guard fails `rotate_ring_still_works_over_the_selections_own_thick_band`.
  - Keep §6 hand test "V on a thick rectangle's corner still rotates".
- **Laws.**
  - Core stays pure, with no new dependencies.
  - The tests construct no Renderer and no EventLoop.
  - No document writes outside an EditCommand: hit-testing is read-only, and Pen/Convert paths are unchanged.
  - `scene.rs`, `varos-app` and the renderer are not touched.
  - Three commits, staged by name.
- **Scope.** `pen_hint` (`editor.rs:3419-3431`) is outside Owns. The edit is justified: the hint must mirror the Pen's tolerance, and the edit also fixes a pre-existing world-vs-local bug for rotated units. Convert (`convert.rs:50`) was added per review P2-2.
- **Attribution (brief item 7).** All three commits end with `Co-Authored-By: Claude Opus 5.5 (1M context)` + `Claude-Session: …session_01RVFA8CCAw5H85J9D4LqJci`. That matches this session's attribution reminder. Noted; not a blocker.

## Findings

### P1 — must fix before merge

**P1-1 · Newton gives up on straight segments, so a click on the centreline near a corner still misses.**
- **Where:** `model.rs:1978-1980`. `fp = |B′|² + r·B″`. On a straight segment `p1 = p0, p2 = p3` (`hout`/`hin` = `None`, `unwrap_or(anchor)`), the parametrisation is `3t² − 2t³`. Near both ends `r·B″` outweighs `|B′|²`, so `fp ≤ 1e-12` and the loop `break`s. The result is `guess_d`: the curve evaluated at the chord-**interpolated** `t`, not the chord point. That is off along the line by up to about 0.11 % of the segment length.
- **Measured:**
  - A 1000 line, probed exactly on it: worst d = **1.10** (at x = 1.64).
  - A 20000 × 20000 rectangle, top edge, exactly on the centreline: worst d = **21.87** (at x = 32.5). At 327 %, **59 of 20001** probes on the first half of the edge **miss** (`path_under` returns `None`). That is ~30 units, or ~100 screen px, beside each corner.
  - A curve with one retracted handle (a common Pen corner-curve): worst **0.65** on the curve.
  - The pinned tests do not see any of this. They use a circle, or a straight edge probed at mid-span.
- **Scale:** this is not a regression. On the same edge the old 25-sample code was off by up to ~50. But it contradicts the work-order contract ("distance to the true curve") and the doc comments at `model.rs:742-746` and `:1937-1944`. Straight edges are the most common geometry there is.
- **Fix (one line):** use Gauss–Newton. Drop the curvature term: `let fp = d1[0] * d1[0] + d1[1] * d1[1];`. It is always ≥ 0, and it converges quadratically when the residual is small, which is the hit-test case. Measured on a copy of `cubic_nearest`:
  - Straight 20000 edge, on the curve: worst **22.00 → 0.072**. At 30 off: error **7.15 → 0.00006**.
  - Retracted handle: **0.649 → 0.0021**.
  - Circle r 10000: unchanged at 0.0034.
  - Also update the comment on the `break`, which currently says "flat/degenerate spot (e.g. a straight segment's zero-length handle end)".
- **Test:** `big_rect_edge_near_corner_centreline_is_hit`. A 20000 rect with stroke 1 at ppu 3.27. For x ∈ [0, 200] in steps of 0.5 on y = 0, `path_under` is `Some(1)` and `edge_dist < 0.1`. Also add a Pen add-anchor near a corner that lands on the line (`|added.y| < 1e-3`, `|added.x − x| < 0.1`).

### P2 — should fix

**P2-1 · The marquee hole-rim branch is untested.**
- `editor.rs:740` (`p.holes.iter().any(… touches(&Document::ring_px(h, true, ppu)))`) is claimed in the commit message ("outer + hole rims"). But `if false && …` passes all 360 tests.
- **Fix:** add `marquee_touching_only_a_donut_hole_rim_selects_it`:
  - A filled donut, outer r 400, hole r 200, stroke 40.
  - A marquee at x 185..195 (on the hole's inner band) → caught.
  - A marquee at x −50..50 (inside the hole, where the centre is not in the fill) → not caught.

### P3 — nits / follow-ups
1. **Hover cost is pre-existing.** It was 10–11 ms, and is now 13.5 ms per `path_under` at 25k segments (release). `main.rs:91` also calls `transform_hit`, which calls `path_under` a second time per hover whenever something is selected. Log it under P11.3. The mitigation is:
   - a per-path bbox reject (outline bbox grown by `reach`) before `edge_dist`;
   - in `transform_hit`, test the cheap `dist(pos, corner) ≤ 22/ppu` **before** calling `path_under`.

   The brief's "sagitta-gated refinement" is not needed at 1.3×.
2. **Duplicate helper.** `seg_touches_rect` (`editor.rs:336`) duplicates `scene.rs:979 clip_seg_rect(..).is_some()`, which has the same rect tuple. The duplicate is forced by "Must not touch `scene.rs`". Follow-up: move one version (the Liang–Barsky one, which has no 8-iteration give-up) to `geom.rs` and use it from both places.
3. **Anchor-drag snap still uses samples.** `nearest_edge` (`editor.rs:2951`) samples 25 points, so object snap and anchor-drag snap now disagree on curves. Follow-up: make `cubic_nearest` `pub(crate)` and reuse it.
4. **Occlusion test.** Adopt the reviewer's test `lower_thick_band_does_not_steal_from_thin_top_path` (see Verified) into `occlusion.rs`, so brief item 4 stays pinned.
5. **Untested edge cases.**
   - The `pen_hint` local-frame fix has no rotated-unit test.
   - The `ring.len() == 1` branch in `path_in_rect` (`editor.rs:731`, a single-anchor path) is untested. It is cheap to pin.
