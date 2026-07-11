> **Status:** historical — Preserved project history; not current authority under `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Rust Port Spike (next task)

Decision (2026-06-23, see `VAROS_CONSTITUTION.md`): Varos is a **desktop** app — **Hybrid: Rust core + web panels + Tauri shell**. Before building the whole thing in Rust, **prove it on a small piece first.** Read `VAROS_CONSTITUTION.md` and `VAROS_PLAN.md` first.

## Goal
Port **just the pen + one shape (rectangle)** from the web prototype (`pen-spike.html`) to a **Rust + Tauri desktop app** with a **wgpu** canvas. Answer TWO questions:
1. Does the modeless **pen + white-arrow FEEL survive** the port (still feels like Illustrator)?
2. Is the **build/run iteration loop tolerable** for us?

## Stack
- **Tauri** — real desktop window / `.exe`.
- **wgpu** for the canvas — render the paths/anchors **yourself on the GPU** (NOT DOM/SVG). Add a CPU fallback if quick; otherwise note it for later.
- **Rust** for the core: the path data model (anchors with `in`/`out` handles + corner/smooth type) and the modeless interaction.
- **Minimal UI:** just a few tool buttons (pen / white-arrow / rectangle). Panels come later — NOT now.

## What to port (keep it SMALL)
- **Pen (P):** click = corner, drag = smooth (symmetric handles), close on first anchor.
- **Direct Selection / white arrow (A):** grab any anchor from outside (no edit mode, no double-click), drag it; drag a handle (collinear coupling on smooth, **Alt** to break — match the web prototype, which Ahmed approved).
- **Rectangle (M):** drag to draw (4 corner anchors, Shift=square, Alt=center), editable by the white arrow.
- **`pen-spike.html` is the exact reference for the feel — match it.**

## OUT OF SCOPE
Fill/stroke, other shapes, layers, panels, text, export, undo polish. Just enough to judge the feel + the loop.

## Gate (Ahmed, in the real window, by hand)
- Does it feel like the web prototype (the Illustrator feel)?
- Is the compile/run loop OK to keep working in?
- **PASS** → we continue building Varos in Rust desktop. **FAIL** → report exactly what broke; we stay on the web prototype and reassess.

## Working rules (unchanged)
One small piece at a time; Ahmed verifies in the real app; simple short Egyptian Arabic; if he says rebuild, do it the first time.
