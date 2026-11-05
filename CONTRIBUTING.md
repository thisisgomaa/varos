# Contributing to Varos

Varos is based on VectorCraft by the ArtCraft team, under MIT OR Apache-2.0. General engine,
format and performance fixes are best sent [upstream](https://github.com/storytold/vectorcraft).
For Varos-specific changes, open an issue describing the problem before proposing a large change.
Design, scope and release decisions belong to the maintainers.

Read [AGENTS.md](AGENTS.md) and [the fork rules](docs/varos/FORK.md). Varos UI code lives in
`crates/ui-egui/src/varos/`; dedicated crates include `crates/text-arabic/`, `crates/varos-text/`
and `crates/varos-agents/`. Keep upstream edits to the smallest necessary hooks, marked `Varos`.
Use conventional commits, such as `fix: preserve Arabic text order`, and the existing pull request
template. Describe the change, tests and remaining limitations.

## Local checks

Run one Cargo build at a time, with four jobs:

```sh
CARGO_BUILD_JOBS=4 cargo xtask ci
git diff --check
```

The current `xtask` runs these 12 steps in order: `fmt` (check), `clippy` (warnings denied),
`test`, `doctests`, `no-bundled-fonts`, `pdf-deps`, `assets`, `brands`, `cursors` (warnings only),
`i18n-report`, `layers`, and `wasm`. It stops at the first failure. See
[xtask/src/main.rs](xtask/src/main.rs) for the commands. Report actual local results;
GitHub CI has never run on this project. Visible changes remain provisional until the maintainers
accept them in the real window.

## Compatibility and assets

- Keep `.varos`, `.vectorcraft` and `.drawcraft` reading paths working. Preserve wasm compilation.
- Keyboard shortcuts must match Adobe Illustrator's. Use public documentation as the reference;
  never run, automate, inspect or copy Adobe software to recreate behavior or assets.
- No proprietary assets or GPL/AGPL code. Use original MIT OR Apache-2.0 work or material with
  redistribution permission. Attribute every asset in [ASSETS.md](ASSETS.md) with its path,
  author, source and licence; include the licence text and the required NOTICE entry.
- Never bundle fonts without a redistribution licence. New font files belong in
  [craft-fonts](https://github.com/storytold/craft-fonts), not this repository. Its optional
  `CRAFT_FONTS_DIR` build input must remain optional; builds must work without it.
- Do not add ArtCraft marks. Preserve upstream copyright and licence notices.
