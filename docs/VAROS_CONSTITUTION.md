> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# Varos — Constitution (الدستور)

Final decisions, locked **2026-06-23**. These are the foundation — change only deliberately, never by accident.

## Identity
1. **Name:** Varos. **File extension: `.vrs`**.
2. **Mission:** a **free, open-source, desktop** alternative to **Adobe Illustrator** that the community can **easily build tools on** — the "Blender of 2D vector".
3. **License:** **GPL** (copyleft, like Blender) — stays free and open forever; anyone who modifies and distributes must keep their code open too.
4. **Strength / wedge:** free + open-source + Illustrator-power + **easy to add tools**. *(Arabic deferred to post-v1 — not the current strength.)*

## Platform
5. **Desktop first, Windows first** (Mac/Linux later).
6. **Offline + local files (`.vrs`)** from day one. The user owns their files. *(Cloud/sync optional, later.)*
7. **Auto-update:** silent, **background, incremental** (only what changed); works offline between updates.

## Architecture
8. **Hybrid stack:** a **Rust core** + **web panels** (UI) + a native **desktop shell** = a real installed app. *(Same recipe as VS Code / Slack / Figma desktop.)*
9. **Shell: Tauri.**
10. **Canvas:** self-drawn on the **GPU (wgpu)**, **never the DOM**, with a **CPU fallback** so weak machines degrade (slow) instead of crash.
11. **Hard seam:** the Rust core never depends on the web panels — so the UI can be swapped later with no rewrite (the escape hatch Illustrator never gave itself).
12. **Single-schema principle: DEFERRED.** Revisit after building a few real tools (extract it from real work, don't guess it up front). Intent: one definition → file + panel + AI + plugins.
13. **AI-native + plugins:** long-term platform principles (revisit alongside the schema).

## v1 — the finish line (definition of "a real program")
14. v1 ships when it can: **draw** (pen + shapes) · **color** (fill/stroke) · **arrange** (move/scale/rotate, layers, align) · **combine** (Pathfinder/Shape Builder) · **simple text** · **export** (SVG/PNG). Build to this line, then **ship** — don't keep adding.

## Way of working
15. **Build piece by piece** — one small tool at a time.
16. **Verification = Ahmed using it in the real app, by hand.** Never "done" from an automated test (automated tests lie about interactive feel).
17. **Port-spike before commitment** — prove the feel survives in Rust on a small piece before building the whole thing in Rust.
18. **Keep the web prototype** (`pen-spike.html`) as the proven feel-reference and fallback.
19. **Don't drown** — build only the ~15 core tools (see `ILLUSTRATOR_TOOLS_CATALOG.md`), each in its 80/20 form; defer/skip the rest.
20. **Structure pass** — after a few tools, reorganize the working code into a clean skeleton (prototype → real program). A reorganize, NOT a rewrite.

*See also: `VAROS_PLAN.md` (phased roadmap) · `ILLUSTRATOR_TOOLS_CATALOG.md` (tools + build order) · Figma plan board + tools-catalog board.*
