> **Status:** current — P11.1 measurement and implementation evidence, governed by `FOUNDATION_CHARTER.md`.
# P11.1 Performance Evidence

## Reproduction harness

Run the CPU-only harness from `varos/`:

```powershell
cargo run -p varos-render-wgpu --release --example perf_harness -j 4
```

It reports the median of 15 measured iterations after one warm-up. `build_scene` measures the
render-agnostic core scene, `build_content` measures CPU tessellation, and `cpu_frame` measures both
together. Vertex counts prevent a faster result from silently doing less work. The three fixed scenes
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

Pending implementation.
