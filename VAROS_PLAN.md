# Varos — Plan (الخطوط العريضة)

Locked direction from the Constitution (`VAROS_CONSTITUTION.md`). Detailed tool list + build order in `ILLUSTRATOR_TOOLS_CATALOG.md`.

## Where we are
- ✅ **Pen + Direct-Select (white arrow)** feel — DONE, ~98% (web prototype `pen-spike.html`).
- ✅ **Shapes** (rectangle / ellipse / triangle / polygon) — DONE on the web prototype.

## Phase 0 — Rust Port Spike  ◀ NOW
Port **pen + one shape (rectangle)** to a **Rust + Tauri** desktop window with a **wgpu** canvas. Goal: confirm (a) the feel survives the port, (b) the build/run loop is tolerable. Ahmed verifies in the real window.
- **PASS** → green light, continue in Rust desktop.
- **FAIL** → learned in a day; stay on web and reassess.
- *(Brief: `RUST_PORT_SPIKE_NOTES.md`.)*

## Phase 1 — Rust desktop foundation + core drawing
Set up the clean Rust project: core data model (paths/anchors/shapes), wgpu canvas + CPU fallback, Tauri shell, a thin web-panel area. Re-establish pen + shapes cleanly. **Establish the "add-a-tool" pattern** so every later tool is a quick copy.

## Phase 2 — Paint (color the work)
Per-object **Fill & Stroke** model · toolbar swatches (X / Shift+X / D / none) · **Color panel** (hex/RGB) · **Stroke panel** (weight/caps/joins/dashes) · **Eyedropper (I)**.

## Phase 3 — Arrange
**Transform panel** (exact X/Y, W/H, rotate) + Selection bounding-box · **Layers panel** (reorder/hide/lock/group) · **Align & Distribute** · **Zoom + Hand**.
*(Do the **structure pass** around here — clean modular skeleton.)*

## Phase 4 — Combine (the differentiator)
Build the **boolean engine once (Clipper2)** → **Pathfinder panel** + **Shape Builder** on top. Finish the pen's anchor trio (add/delete/convert). **Line tool**, **cut trio** (scissors/knife/eraser), **Artboard**.

## Phase 5 — Text + Export → v1
**Smart Type tool** (point/area/path, Latin first) + a simple text engine · **Export** (SVG/PNG). → **ship v1.**

## Deferred (post-v1)
Arabic + kashida · gradients / mesh / brushes / Live Paint · advanced transforms & warps · the single-schema formalization · AI-native automation · plugin SDK · Mac/Linux · cloud sync.

## The two rules that protect everything
- **Selecting ≠ doing** — keep the modeless black-arrow / white-arrow feel.
- **Always ship a tool WITH its driving panel** — a tool without its panel is half-built.
