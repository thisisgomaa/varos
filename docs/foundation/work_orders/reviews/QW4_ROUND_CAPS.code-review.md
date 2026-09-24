> **Status:** reference — independent code review, 2026-09-24.

# Code review: QW4, smooth round caps (PAINS_LOG P13) (`fix/qw4-round-caps` @ `a0d8bef`, base `5ef0687`)

**Verdict: APPROVE WITH NITS. The geometry is correct. Two cheap P2 speed fixes should land on this branch before merge.**
1. The half-disc cap (a 180° `stroke_join` against the reversed frame) sits on the correct side at both ends, for either turn direction, and with duplicate end points. It shares the quad corners bit for bit. The seam join closes every closed ring, including double repeats. All of this was verified with throwaway tests.
2. The vertex counts in the report reproduce exactly (A, C, D, E, overlays). The 5 pin edits are count updates only.
3. The scene C "Linux allocator artefact" is real and reproducible: harness content time is **3× base** in every head run. One `Vec::with_capacity` removes it (P2-1).
4. Caps now pay one `sin_cos` per chord. The old caps used a cached ring. A rotation recurrence halves cap and join time with no test change (P2-2).
5. At 4000%, 80-wide open ends cost 3.6× the vertices of the old discs. This is the price of the 0.25 px rule, which the joins already pay. It is visible only in pathological pile-ups (P3-1).

## Gates (measured by me in the worktree, `varos/`)
- `cargo test --workspace -j 4`: **357 passed, 0 failed**. This matches the report. The commit message's "312" was measured before the merge.
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: clean. `cargo fmt --all -- --check`: clean.
- `cargo clippy -p varos-app --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin`, with `-D warnings`: both clean.
- Laws: the diff touches only `varos-render-wgpu/src/tess.rs` and `tess_round3_tests.rs` (count pins only). The tests are CPU-only, with no `Renderer` or `EventLoop`. There are no new dependencies.
- Harness (release, Linux, base vs head, 2 runs each): vertex counts match the report exactly.
  - Scene C content: base 0.80–0.87 ms in 6 of 7 runs (one 2.23 ms outlier). Head **2.28–3.19 ms in 7 of 7 runs**.
- Throwaway work was done in scratchpad copies (`scratchpad/qw4head`, `qw4base`, probe `scratchpad/qw4_probe.rs`), never in the worktree. The worktree is clean.
  - An early probe run shared the worktree's `target/release` and overwrote its `varos-*` release artefacts with base code. I deleted those `.fingerprint/varos-*` entries, so the next release build there rebuilds from source. Debug artefacts were never touched.

## Verified correct (brief items)
- **(1) Cap side.** Throwaway test `probe_cap_centroids_on_correct_side`, 280 polylines: r ∈ {0.8, 2, 40, 1600}, 5 headings, turns 0/±60/±90/±170, with and without `[a,a,…,c,c]` duplicates.
  - The start-cap centroid is at −4r/3π along the first tangent: behind the first point.
  - The end-cap centroid is at +4r/3π along the last tangent: ahead of the last point.
  - Lateral offset is ≈ 0, and each cap's area is a half-disc within the 0.25 px sagitta.
  - Why it holds: `cross` of a frame and its reversal is exactly 0.0 in IEEE, so `s = 1`, `outer = 0`, θ = π. The arc midpoint is `n` rotated −90°, which is `r·dir`. Turn direction therefore cannot affect a cap.
- **(2) Closed rings.** `flatten.rs:66` → `world_ring_px` → `ring_px(closed = true)` already ends on anchor 0, and `scene.rs:517` pushes it **again**. Every hole ring therefore arrives as `[…, A, A]`. So does a closed path whose last anchor sits on the first.
  - The zero-length tail segment is skipped (`len == 0.0`), and the seam join uses the last non-zero frame.
  - Throwaway tests confirm this: tail dup, triple, head dup and both give the same mesh size as a single repeat, and the seam disc is covered. Through the real scene, the duplicate-anchor square and the hole seam are covered at 100% and 4000%, with the double repeat confirmed at the tessellator.
  - Tiny rings (side 2–4 px, r 10–40 px) cover every corner, including the seam.
- **(5) Mutation checks.**
  - Removing the seam join fails 4 of the implementer's tests (`closed_ring_seam_is_joined_not_capped`, `closed_path_seam_is_covered_in_and_out_of_view`, `rectangle_seam_corner_is_round_at_4000_percent`, `harness_curved_scenes_…`) plus my 2 probes. The report says 6; I count 4 of theirs.
  - Leaving the start cap unreversed fails 2 of theirs plus my centroid probe.
  - Keeping the zero-area tail triangle fails 10.
- **(6)** `tess_round3_tests.rs` changes 2 `assert_eq!` counts and their comments, nothing else. The 5 changed pins are `tess.rs` ×3 and round3 ×2, as the plan review listed.
- **(7)** Commit trailers are the implementer/orchestrator session's own (`Claude-Session …session_01RVFA8…`). This is a note, not a blocker.

## Findings

**P2-1 · Scene C is 3× slower in the numbers-of-record harness. The cause is an allocator threshold, and a reservation fixes it.** `tess.rs:647` `let mut fgv = Vec::new();`
- Head drops C from 100,500 to 86,400 vertices, but its content time goes from 0.83 ms to 2.2–3.2 ms (7 of 7 runs).
- Cause: glibc's mmap threshold. With `GLIBC_TUNABLES=glibc.malloc.mmap_threshold=64MB` the time is 0.88–1.0 ms.
- `Vec::with_capacity(1 << 17)` on `fgv` alone gives **0.75–0.81 ms** in 5 of 5 runs, faster than base. A and E are unchanged or faster.
- Emitting straight into `fgv` (no temporary `build_fg` Vec) does **not** fix it (2.2–2.4 ms).
- Mac (the official build) uses a different allocator, so the effect there is unmeasured. The harness is still the gate of record, and "artefact" should not be written down with no fix.
- **Fix:** reserve `fgv` in `build_content`. A fixed 128 Ki-vertex (3 MB) floor works. Better: keep last frame's `fgv.len()` as the hint, or reuse the buffer across frames (a P11 follow-up). Re-run the harness and record the numbers in the gate log.

**P2-2 · One `sin_cos` per chord makes caps slower than the old discs, even with fewer vertices.** `tess.rs:253`
- Probe: 2000 open two-point paths, release, allocator noise removed.
  - w80 at 100% (r 40): base 300,000 vertices in 1.14 ms. Head **180,000 in 1.81 ms**.
  - The 4000% pile-up (see P3-1): 9.3 ms.
- **Fix** (5 lines, verified: all 41 `varos-render-wgpu` lib tests pass, including the bit-exact corner and sagitta tests): compute `let (sd, cd) = (-s * theta / steps as f64).sin_cos();` once, then rotate `cur = [cur0*cd - cur1*sd, cur0*sd + cur1*cd]` each step. The last vertex stays `cout`, so there is no drift at the shared edge.
  - Result: w80 at 100%: **0.95–1.02 ms**, below base. The pile-up: 4.5–5.1 ms. Joins get the same gain.

**P3-1 · Vertex growth at high zoom (brief item 3). Accept it and record it.**
- Caps grow past the old 72-vertex disc only above a screen radius of about 117 px. At r = 1600 px a cap is 89 chords = 264 vertices, and the r = 1e9 pin reaches the 128-chord cap (762 vertices).
- 2000 open two-point paths, 80 wide, at 4000%:
  - Harness-like grid, view-culled: 5,250 → 18,690 vertices, 0.017 → 0.14 ms.
  - All 2000 in view (a contrived pile-up): **300,000 → 1,068,000 vertices** (7.2 → 25.6 MB), 1.1 → 9.3 ms, or about 4.8 ms with P2-2.
  - At 100%, w2, w10 and w80 all **drop** (48k / 60k / 180k vs 300k).
- No change needed. Record the break-even radius in P11_2_PERF.md.

**P3-2 · Stale rationale is still in the code and the commit.**
- `tess.rs:1263` says "They also hid a missing seam join on closed rings", and the commit body says the same. Plan review P2-3 showed that half-caps alone cover the seam wedge for any turn ≤ 180°. The seam join is a vertex-count choice. Only the 24-gon facet error was hidden.
- **Fix:** reword the comment.

**P3-3 · The comment at `tess.rs:189` says the seam join is "exact even for an open path whose ends happen to coincide".** It is as exact as any interior round join (outer wedge + quads), not as exact as two full caps. Say "same coverage as an interior join".

**P3-4 · Duplicated test helpers.**
- `blob_path` (`tess.rs:1413`) repeats the radii and handle construction of `thick_curved_stroke_band_is_intact_in_and_out_of_view` (`tess.rs:1204`) verbatim.
- `one_path_editor` repeats the setup at `tess_round3_tests.rs:79` (`measure`).
- **Fix:** have the older test call `blob_path(80.4)`.

**P3-5 · The reserve hint `pts.len() * 3 + 24` (`tess.rs:151`) now under-counts every cap above r ≈ 2 px (a cap is 18–762 vertices).** It is only a hint, and amortised growth covers it. Either drop the cap term or compute it from the step rule. Otherwise leave it.
