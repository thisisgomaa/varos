> **Status:** current — Active project document, governed by the authority ladder in `docs/foundation/FOUNDATION_CHARTER.md` §3.
# CLAUDE.md — standing rules for AI sessions on Varos

## What this project is
Varos: a free, open-source, Windows-first vector editor (Illustrator alternative) in Rust — wgpu + winit + egui (manual path, **never eframe** in the real app). Arabic-first typography is the long-term moat. Built by Ahmed (designer, not a coder — talk simple, no jargon, Arabic preferred) with AI sessions.

## Hard architecture laws
- `varos-core` = pure logic. Zero GPU/window/UI deps. The compiler enforces this seam — never weaken it.
- No test may construct a GPU `Renderer` or an `EventLoop` (CI runs headless).
- The product UI is native Winit + Egui on WGPU. No Tauri, DOM, web views, or Electron thinking. GPU rendering is required for V1; a CPU renderer is a future option, not a promise, and GPU startup failure must stay readable ([ADR-0001](docs/adr/ADR-0001-native-gpu-ui-stack.md)).
- The V1 persisted schema is the versioned JSON representation of `varos-core`'s Serde model. There is no introspectable inspector/plugin/AI schema yet; do not promise or infer one ([ADR-0004](docs/adr/ADR-0004-v1-schema-policy.md)).
- Updates, if implemented, must be visible and user-controlled; silent background installation is rejected ([ADR-0007](docs/adr/ADR-0007-visible-update-policy.md)).

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
- `docs/foundation/STATUS.md` — current program state · `docs/adr/` — accepted architecture decisions
- `docs/UI_DIRECTION.md` — current visual direction · `docs/reference/BOX_SYSTEM_PLAN.md` — shell reference
- `docs/LAYERS_VISION.md` — current layers/masks intent · `docs/audits/` — tracked risk register
