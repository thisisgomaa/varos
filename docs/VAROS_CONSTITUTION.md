> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Constitution (الدستور)

Original decisions were locked **2026-06-23**. Architecture, schema, and update clauses were deliberately amended **2026-07-11** by accepted [ADR-0001](adr/ADR-0001-native-gpu-ui-stack.md), [ADR-0004](adr/ADR-0004-v1-schema-policy.md), and [ADR-0007](adr/ADR-0007-visible-update-policy.md).

## Identity
1. **Name:** Varos. **File extension: `.vrs`**.
2. **Mission:** a **free, open-source, desktop** alternative to **Adobe Illustrator** that the community can **easily build tools on** — the "Blender of 2D vector".
3. **License:** **GPL** (copyleft, like Blender) — stays free and open forever; anyone who modifies and distributes must keep their code open too.
4. **Strength / wedge:** free + open-source + Illustrator-power + **easy to add tools**. *(Arabic deferred to post-v1 — not the current strength.)*

## Platform
5. **Desktop first, Windows first** (Mac/Linux later).
6. **Offline + local files (`.vrs`)** from day one. The user owns their files. *(Cloud/sync optional, later.)*
7. **Updates:** visible and user-controlled. Silent background installation is rejected; implementation details remain future work.

## Architecture
8. **Native stack:** a **Rust core** + **Egui UI** + **Winit desktop shell** + **WGPU renderer**. Tauri and web panels are not part of the product architecture.
9. **Shell:** native Winit window/event loop with Egui painted on the application's WGPU surface.
10. **Canvas:** self-drawn on the **GPU (WGPU)**, never the DOM. GPU rendering is required for V1; a CPU renderer is a future option, not a promise. Incompatible GPU startup fails with a readable user-facing error.
11. **Hard seam:** `varos-core` never depends on the application, UI, renderer, windowing, or platform layers.
12. **V1 schema:** the persisted editable schema is the versioned JSON representation of `varos-core`'s Serde model. An introspectable inspector/plugin/AI schema is not implemented and remains future work.
13. **AI-native + plugins:** long-term platform principles (revisit alongside the schema).

## v1 — the finish line (definition of "a real program")
14. v1 ships when it can: **draw** (pen + shapes) · **color** (fill/stroke) · **arrange** (move/scale/rotate, layers, align) · **combine** (Pathfinder/Shape Builder) · **simple text** · **export** (SVG/PNG). Build to this line, then **ship** — don't keep adding.

## Way of working
15. **Build piece by piece** — one small tool at a time.
16. **Verification = Ahmed using it in the real app, by hand.** Never "done" from an automated test (automated tests lie about interactive feel).
17. **Port-spike before commitment** — prove the feel survives in Rust on a small piece before building the whole thing in Rust.
18. **Keep the web prototype** (`pen-spike.html`) as a historical feel-reference, never as a production fallback.
19. **Don't drown** — build only the ~15 core tools (see `ILLUSTRATOR_TOOLS_CATALOG.md`), each in its 80/20 form; defer/skip the rest.
20. **Structure pass** — after a few tools, reorganize the working code into a clean skeleton (prototype → real program). A reorganize, NOT a rewrite.

*See also: `VAROS_PLAN.md` (phased roadmap) · `ILLUSTRATOR_TOOLS_CATALOG.md` (tools + build order) · Figma plan board + tools-catalog board.*
