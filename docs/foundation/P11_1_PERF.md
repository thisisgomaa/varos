> **Status:** current — P11.1 measurement and implementation evidence, governed by `FOUNDATION_CHARTER.md`.
# P11.1 Performance Evidence

## Reproduction harness

Run the CPU-only harness from `varos/`:

```powershell
cargo run -p varos-render-wgpu --release --example perf_harness -j 4
```

It reports the median of 15 measured iterations after one warm-up. `build_scene` measures the
render-agnostic core scene, `build_content` measures CPU tessellation, `cold` measures both together,
and `hit` measures the shared conservative signature path that skips both. Vertex counts prevent a
faster cold result from silently doing less work. The three fixed scenes
are: one selected 150-anchor curved path at ppu 3.0, 500 rectangles at ppu 0.3, and 100 twelve-anchor
curved paths at ppu 1.0. The harness requires no window, surface, adapter, or GPU.

For live frames, launch the application with `VAROS_PERF=1`. The app writes `build_scene`,
`build_content`, `render_ui`, and full redraw timing to stderr; without the flag the only runtime cost
is one environment lookup after a rendered frame.

## Baseline

Measured on 2026-07-12, Windows release build, before any P11.1 optimization:

| Scene | build_scene | build_content | cpu_frame | fill / foreground / opacity vertices |
|---|---:|---:|---:|---:|
| A: selected curved-150, ppu 3.0 | 0.106 ms | 2.425 ms | 2.696 ms | 3,609 / 93,672 / 0 |
| B: rectangles-500, ppu 0.3 | 3.208 ms | 0.717 ms | 3.864 ms | 10,500 / 12,000 / 0 |
| C: curves-100, ppu 1.0 | 0.678 ms | 22.464 ms | 23.931 ms | 29,700 / 756,000 / 0 |

Command exited 0. These values are the acceptance denominators for P11.1; the target is at least 10x
on A and at least 3x on C using the `cpu_frame` column.

## After P11.1

Measured on 2026-07-12 with the same command, machine, release profile, scenes, and iteration count:

| Scene | build_scene | build_content | cold miss | cache hit | fill / foreground / opacity vertices | Cold speedup |
|---|---:|---:|---:|---:|---:|---:|
| A: selected curved-150, ppu 3.0 | 0.083 ms | 0.154 ms | 0.260 ms | 0.005 ms | 3,609 / 7,344 / 0 | **10.37x** |
| B: rectangles-500, ppu 0.3 | 2.547 ms | 0.488 ms | 3.115 ms | 0.001 ms | 10,500 / 12,000 / 0 | **1.24x** |
| C: curves-100, ppu 1.0 | 0.560 ms | 1.815 ms | 2.486 ms | 0.002 ms | 29,700 / 72,000 / 0 | **9.63x** |

Targets pass on cold misses: A exceeds 10x and C exceeds 3x. On an unchanged input frame, the CPU
canvas path is only the signature check; the renderer reuses its resolved offscreen scene and still
processes egui plus present. The headless `hit` number intentionally excludes GPU/egui work, exactly as
the baseline excluded it. `VAROS_PERF=1` reports the actual live full-frame and render timings.

## Implementation evidence

- Frame-local geometry is subdivided once and passed by reference to fill, stroke, masks, and skeleton:
  `varos-core/src/scene.rs:307-411` (`9da0df3`). This is not the cross-frame subdivision cache reserved
  for P11.2.
- Stroke discs now remain at caps and direction changes of at least five degrees, with CPU tests for a
  real 90-degree corner and near-collinear adaptive points: `varos-render-wgpu/src/tess.rs:80-116`
  (`e644107`). Unit-circle geometry is initialized once and stroke buffers reserve their known output
  capacity at `tess.rs:57-88` (`4281dc9`).
- The conservative signature lives beside `build_scene` at `varos-core/src/scene.rs:71`; it includes
  document revision, live selected geometry/paint, view, frame size, selection, hover, drag/artboard
  drag, modifiers, active artboard, and overlay state. Cache hits use `render_ui_cached` at
  `varos-render-wgpu/src/lib.rs:993`, while surface failure invalidates the app-side signature
  (`60a038e`, `227bf87`).
- Layers rows are gated by document/tree/selection/search/collapse state, and thumbnails have an
  independent raw-geometry/paint key: `varos-app/src/ui.rs:320-392,1126-1131` (`9acc5df`). Pointer-only
  frames therefore perform no layer row rebuild or curve subdivision.
- Runtime instrumentation is opt-in only: `VAROS_PERF=1` logs scene cache hit/miss, build-content,
  render-ui, and full-frame timing at `varos-app/src/main.rs:1046` and
  `varos-render-wgpu/src/lib.rs:1092`. The headless harness was the first commit (`1b55f22`).

## Scope and verification

P11.1 does not add viewport culling, a cross-frame subdivision cache, undo-storage changes, PresentMode
changes, or snap-engine changes. The 232 pre-existing tests were not edited. Seven focused tests were
added for join retention/elision, scene-signature invalidation, and layer/thumbnail cache keys.

Final branch-tip verification:

| Gate | Result |
|---|---|
| `tools/check_links.ps1` | PASS: 74 first-party docs, 81 relative links, 58 heading anchors |
| `tools/check_dep_directions.ps1` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `cargo test --workspace -j 4` | PASS: 239 tests, 0 failed |
| `cargo clippy --workspace --all-targets -j 4 -- -D warnings` | PASS |
| `cargo build --release -p varos-app -j 4` | PASS |
| `git diff --check main..HEAD` | PASS |

Manual-test binary: `varos/target/release/varos.exe`, 18,638,336 bytes, SHA-256
`AD877F216B8429BECB25821D8C69C905865B591636F3425D321B5405E7F93D44`.
