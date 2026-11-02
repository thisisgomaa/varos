# Varos history

Status: current, 2026-11-02. Facts only; each line is backed by git history, a decision record or a
measurement. Dates are 2026.

## 1. Before the name: BStudio (05-25 → 06-20)

| Date | What happened |
|---|---|
| 05-25 | Work starts under the working name **BStudio**: an Arabic design tool. 25 commits on the first day; the first Rust/WASM kashida engine works end to end the same morning. |
| 05-25 → 06-15 | Browser-based editors are tried: a custom Skia/WebGL editor, then tldraw with the kashida engine plugged in, then a rebuilt "v1". |
| 06-16 | The v1 rebuild is abandoned and a Penpot-based build is tried. |

Lesson kept from this period: automated tests can pass while the real interaction is broken, so
the owner's hand test in the real window is the judge.

## 2. The name (06-21)

Decision record 004, 2026-06-21: the product is **VAROS** = **V**ector · **A**rabic · **R**TL ·
**O**pen **S**ource, pronounced "VAH-ros" («فاروس»). A coined word, so it is available and ownable;
V carries the real meaning (a vector tool); it reads correctly in Arabic and English. It replaced
BStudio as the public name.

## 3. Varos's own engine (06-25 → 10-19)

A from-scratch Rust editor: pure-logic core, native winit + egui UI, GPU rendering on wgpu.

| Date | What happened |
|---|---|
| 06-25 | First commit of the Varos engine repository ("Rust core + wgpu + winit"). |
| 07-06 | Repository prepared for publication under GPL-3.0. |
| 07-12 → 09-22 | Pause: one commit in ten weeks. |
| 09-23 | Work resumes, macOS first. |
| 10-06 | The AI-native direction is written down: people and agents on the same files, a Bridge (MCP + CLI) first. |
| 10-17 | VectorCraft, an open-source Rust vector editor (MIT OR Apache-2.0), is hand-tested: good, but slower with many vectors. Three overnight spikes measure it (section 4). |
| 10-18 | `varos-text` (the Arabic text crate), the Varos cursor set and the original icon glyphs are relicensed from GPL-3.0-only to MIT OR Apache-2.0, so they can travel into an MIT/Apache codebase. |
| 10-19 | Last engine commit. |

At the end: **806 commits** (06-25 → 10-19), about **183k lines of Rust**, about **2.5k tests**, and
**24 architecture decision records** (ADR files). This engine is preserved on branch
`legacy-engine` / tag `legacy-engine-2026-10-19`.

## 4. The decision to build on VectorCraft (10-18 → 10-19)

The question was "keep building our own, or build on a working editor". It was decided on measured
results:

- **Performance ceiling is contained.** VectorCraft renders on the CPU, linear in visible paths:
  1k paths = 3.6 ms, 5k = 16 ms, 20k = 64 ms per frame (release build, 4 render threads,
  1920x1080). Culling works; the limit is the renderer, not the document core, so it can be fixed
  in place (tile cache first, GPU decision after).
- **Arabic transfers.** Varos's kashida policy from `varos-text` was driven through VectorCraft's
  own HarfRust shaper and real text node: 6 tatweel insertions in a 600 px justified line, rendered
  and inspected. VectorCraft already had shaping, bidi and word-space justification.
- **Upstream merges are affordable.** A trial merge of the stage-0 work across 2,808 upstream
  commits (1,144 files) gave 11 conflicted files with 1-2 small hunks each.

What was left behind with the old engine: its own GPU renderer, its file format, the GPL licence and
full control of the architecture. What travelled: `varos-text`, the Bridge philosophy (no pairing,
no approval prompts; undo is the safety net), the design language, the icons and cursors, and the
colour-picker design.

## 5. What Varos added on VectorCraft (10-18 → today)

Measured at HEAD `292bd2418` (2026-11-02); status of each item in [STATE.md](STATE.md).

- **Identity:** Varos name, app icon, bundle `com.varos.app`, `.varos` as the default save
  extension; community promotion removed.
- **Arabic:** kashida engine (`crates/text/src/kashida.rs`, 714 lines) with Off / Auto / Force and
  a strength, working with OpenType and AAT (morx) Arabic fonts and never stretching inside the name
  of God; Arabic Type Essentials (direction, digits, harakat colour/hide without breaking joins);
  Arabic font menu; Arabic PDF copy-back fixes.
- **Look and shell:** Varos tokens and fonts, 157 icon overrides, native macOS cursors, a clean
  Home screen, the new shell, and the unified panel workspace (provisional, awaiting the owner).
- **Documents:** native format version 5 with independent Pages (upstream: version 4).
- **Agents:** `crates/varos-agents` (agent identity, document diffs, on-canvas presence, activity
  log), an always-on local listener with a private token, an MCP bridge that attaches to the open
  app, actor-tagged history and orange on-canvas presence.
- **Upstream kept current:** merges of 239 (10-20) and 1,503 upstream commits (10-26, 61 conflicts).
- **Size:** 188 commits; +56,276 / -2,334 Rust lines (about 8% of ~705k);
  `crates/ui-egui/src/varos/` ~36.3k lines with 508 tests.

Most of the editor (engine, renderer, file formats, tools, effects, many panel bodies) is still
VectorCraft's. Varos is "based on VectorCraft by the ArtCraft team".

## 6. What comes next

The ordered list is [BACKLOG.md](BACKLOG.md): the owner's hand test of the panel workspace, the
staged upstream merge, performance (tile cache), and the AI-native features. A free community
add-on library is a goal, not yet started.
