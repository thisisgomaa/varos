> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos

**An Arabic-first, free and open-source vector editor — built in public. It opens instantly, and its working files are already PDFs.**

Built in Rust, drawn directly on the GPU (wgpu + egui, no Electron, no web view). Windows-first. Built in public by a designer and AI pair-programming sessions — every decision documented, every stage hand-tested.

> ⚠️ **Early days, on purpose.** Varos is a working drawing tool under heavy construction — not yet a daily design tool (text is coming, see the roadmap below). We publish early because we build in the open.

<!-- TODO(ahmed): hero GIF — 10 seconds of the pen tool drawing a curve. This image IS the first impression. -->

## Why another vector editor?

1. **No loading.** It opens, you draw. Every interaction answers instantly — the only animation in the app is your own work.
2. **Your working file is a PDF.** A `.vrs` file is simultaneously a valid PDF: send it to a client or a print shop and it just opens — no Export step — then reopen it in Varos and keep editing.
3. **Arabic that doesn't break.** Every major design tool treats Arabic letters as an afterthought — disconnected glyphs, reversed direction, no real justification. We are building an Arabic text engine with proper shaping and real *kashida* justification as a core feature, not a plugin. This is the hill we chose.
4. **Free and open, permanently.** GPL-3.0. Your work belongs to you; the tool can never be taken closed against you.

## Status — honest version

| Working today | Coming next | The vision |
|---|---|---|
| Shapes, pen & bezier editing | New floating-panel UI (built, being merged) | Full Arabic engine with kashida |
| Pathfinder (boolean ops) | Masks & clipping | Sandboxed WASM plugins over one schema |
| Layers with per-artboard sections, cross-board drag | Gradients & swatches | AI that manipulates the document via the schema — no hallucinated coordinates |
| Smart snapping & guides | Text system | Community template library |
| `.vrs` save (= valid PDF) | PNG/SVG export; SVG import | |
| 223 headless tests at the audited 2026-07-11 baseline; local fmt + clippy `-D warnings` + full-suite gates are green (GitHub triggers are enabled, but hosted runners remain blocked by account verification; see [STATUS](docs/foundation/STATUS.md)) | | |

## Build

```bash
# stable Rust (rustup.rs), then:
git clone https://github.com/<org>/varos
cd varos/varos
cargo run --release -p varos-app
```

The Cargo workspace lives in `varos/`. Architecture in one line: `varos-core` (pure logic, zero GPU/window deps — where most headless behavior tests live) → `varos-render-wgpu` (GPU tessellation & painting) → `varos-app` (window, input, UI). The compiler enforces the seam.

## Contributing

We're a small project with an unusually deep paper trail — start with [CONTRIBUTING.md](CONTRIBUTING.md) and the `good first issue` label. Design decisions live in `docs/` with dates and reasons; the visual law is `docs/UI_DIRECTION.md`.

Areas that need owners: icons, translations, docs, Windows-on-iGPU testing.

## License

Code: [GPL-3.0](LICENSE). The Varos name and logo are trademarks of the project — see [TRADEMARK.md](TRADEMARK.md). Anything you design with Varos is entirely yours.
