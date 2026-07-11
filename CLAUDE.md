> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# CLAUDE.md — standing rules for AI sessions on Varos

## What this project is
Varos: a free, open-source, Windows-first vector editor (Illustrator alternative) in Rust — wgpu + winit + egui (manual path, **never eframe** in the real app). Arabic-first typography is the long-term moat. Built by Ahmed (designer, not a coder — talk simple, no jargon, Arabic preferred) with AI sessions.

## Hard architecture laws
- `varos-core` = pure logic. Zero GPU/window/UI deps. The compiler enforces this seam — never weaken it.
- No test may construct a GPU `Renderer` or an `EventLoop` (CI runs headless).
- UI is drawn by us on the GPU. No DOM, no web views, no Electron thinking.
- One schema is the single source of truth (file + inspector + future plugins/AI all read it).

## Visual constitution (docs/UI_DIRECTION.md is law)
- Warm-black ramp (`#141313` signature); azure `#0c8ce9` is a scalpel — active/selection/focus ONLY.
- **No shadows. No animations** (`animation_time = 0`) — a work tool answers instantly.
- Corners: 3px controls / 8px boxes. Typography-only decoration. Tokens live in ONE place (`shell/tokens.rs`).
- ONE-HOME rule: every domain has one Section-home; control bar & menus are mirrors only.

## Process laws
- Work in gated pieces; Ahmed hand-tests every stage in the real window before the next.
- 100% honesty: claims need evidence (tests run, numbers measured). Never report "works" without proof.
- Conventional commits, staged **by name** — never `git add -A`. No push unless told.
- `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` must be green before any commit claim.
- Design decisions get written to `docs/` with date + reason before big code.
- **Never commit proprietary reference material** (e.g. any extracted third-party assets kept locally for functional reference). `.gitignore` entries for these are load-bearing.

## Key docs
- `docs/UI_DIRECTION.md` — visual law · `docs/BOX_SYSTEM_PLAN.md` — shell build plan
- `docs/LAYERS_VISION.md` — layers/masks design of record · `docs/DETAILED_ROADMAP.md` + `docs/plan.html` — roadmap
- `docs/ENGINEERING_REVIEW.md` — engineering-practices audit · `docs/MASTER_PLAN_V1_LAUNCH.md` — launch master plan
