<p align="center">
  <img src="assets/app-icon/hicolor/256x256/apps/ai.storyteller.vectorcraft.png" width="128" height="128" alt="Varos app icon">
</p>

<h1 align="center">Varos</h1>

<p align="center">
  <b>A free, open-source vector editor, Arabic and right-to-left first, written in Rust.</b>
</p>

## The name

**Varos** = **V**ector · **A**rabic · **R**TL · **O**pen **S**ource: a vector editor, Arabic first,
built for right-to-left scripts, and open source. The name was decided on 2026-06-21 and is
pronounced "VAH-ros" («فاروس»).

## Why Varos

- **Arabic and RTL first.** Arabic type is the starting point, not an afterthought: kashida
  (tatweel) justification with Off / Auto / Force and a strength setting, Arabic font discovery,
  harakat colouring that keeps letters joined, paragraph direction and Arabic digit options.
- **Free and open.** MIT OR Apache-2.0, like the code it is built on. No account, no subscription.
- **AI-native.** People and agents work on the same open file. Any agent running as the same user
  can read, edit and save through the MCP bridge (`vectorcraft-cli mcp` attaches to the Varos that
  is open), its edits are recorded in the history with who made them, and they appear on the canvas
  in orange while a person's selection stays azure. Undo is the safety net; there are no approval
  or Keychain prompts.
- **Later: a community add-on library.** The WebAssembly plug-in API inherited from VectorCraft is
  the base; a shared, free library of add-ons is a goal, not something that exists yet.

## Status

Honest summary, 2026-11-02. The full truth table, with evidence for every row, is
[docs/varos/STATE.md](docs/varos/STATE.md); the ordered work list is
[docs/varos/BACKLOG.md](docs/varos/BACKLOG.md).

- **Platform:** the official build is **macOS only** for now. Other targets still compile from the
  inherited code but are not built, tested or supported by Varos. There are no release downloads
  yet: build from source (below).
- **What works today:** the whole VectorCraft editor underneath (drawing and path tools,
  Pathfinder, type with shaping and bidirectional layout, effects, SVG / PDF / `.ai` and many other
  formats; tour in [docs/UPSTREAM_README.md](docs/UPSTREAM_README.md)), shown as Varos: name, app
  icon, `.varos` as the default save extension (`.vectorcraft` and `.drawcraft` still open), and
  native format version 5 with independent **Pages** (upstream is at version 4).
- **Built, waiting for the owner's hand test** (status *provisional* in STATE): kashida Off / Auto /
  Force, Arabic Type Essentials and the Arabic font menu; Pages; the Varos look (warm-dark tokens,
  fonts, 3 px / 8 px corners, no shadows); 157 Varos icon overrides and the Varos cursor set (native
  cursors on macOS); the clean Home screen; the new shell and the unified **panel workspace**
  (groups, docking at four edges, floating, three presentations; Tools and Control as special bars);
  the colour picker as a popover from the colour well; modeless Preferences; document-tab
  reordering; the agent layer (`crates/varos-agents`).
- **Performance:** rendering is still VectorCraft's CPU renderer. Measured on 2026-10-17 (release
  build, 4 render threads, 1920x1080), frame time grows with visible paths: 1k = 3.6 ms,
  5k = 16 ms, 20k = 64 ms. A tile cache is the next step; a GPU decision follows.
- **Tests:** local gates on 2026-11-02: `cargo test -p vectorcraft-ui-egui` 2,747 passed / 0 failed
  / 23 ignored; `cargo test --workspace` 11,161 passed / 0 failed; `cargo clippy --workspace
  --all-targets -- -D warnings` clean. GitHub CI has never run on this project; the numbers above
  are from the development Mac.

## Build and run

Rust 1.95 or newer (`rust-version` in `Cargo.toml`) on macOS.

```sh
git clone https://github.com/thisisgomaa/varos
cd varos

cargo run --release -p vectorcraft                          # the Varos app
cargo run --release -p vectorcraft -- path/to/file.varos    # open a file
cargo run --release -p vectorcraft -- --control 7979        # + JSON control channel
cargo run --release -p vectorcraft-cli -- mcp               # MCP bridge: attaches to the open Varos
cargo run --release -p vectorcraft-cli -- mcp --headless    # MCP without the app
cargo run --release -p vectorcraft-cli -- run --in examples/ribbons.vectorcraft --export out.pdf   # headless batch
cargo xtask bundle                                          # dist/Varos.app (macOS)
cargo xtask ci                                              # fmt, clippy, tests, assets, vendor names, layering, wasm
```

Crate and binary names stay upstream's (`vectorcraft`, `vectorcraft-cli`) so upstream merges stay
cheap; the app, its window, menus, bundle (`com.varos.app`) and data folder are Varos.

Register the bridge with an agent, for example Claude Code:

```sh
claude mcp add varos -- /path/to/vectorcraft-cli mcp
```

Details: [docs/mcp.md](docs/mcp.md) and [docs/control-protocol.md](docs/control-protocol.md).
Optional CJK fonts, graphics-processor choice and other build options are explained in
[docs/development.md](docs/development.md). The signed macOS release path is
`packaging/macos/package.sh`.

The `crates/text-arabic` crate takes Varos's kashida policy from the `varos-text` crate of this
repository's earlier engine, pinned by git revision over HTTPS (`crates/text-arabic/Cargo.toml`).

## Built on VectorCraft

Varos is based on VectorCraft by the ArtCraft team
([storytold/vectorcraft](https://github.com/storytold/vectorcraft)), and is licensed the same way:
[MIT](LICENSE-MIT) OR [Apache-2.0](LICENSE-APACHE), at your option. Upstream's copyright and
required notices are kept in [NOTICE](NOTICE); every bundled asset is listed with its author,
source and licence in [ASSETS.md](ASSETS.md). Varos is not an ArtCraft product and uses none of
the ArtCraft marks ([docs/brand/LICENSE-brand.txt](docs/brand/LICENSE-brand.txt)).

What Varos adds lives mostly in its own modules (`crates/ui-egui/src/varos/`,
`crates/text-arabic/`, `crates/varos-agents/`) with small, marked hook lines in upstream files;
the rules are in [docs/varos/FORK.md](docs/varos/FORK.md). Upstream VectorCraft's own README is
kept for reference in [docs/UPSTREAM_README.md](docs/UPSTREAM_README.md).

**The legacy engine.** Varos's first engine, a from-scratch Rust/wgpu editor with 806 commits,
lives on branch `legacy-engine` / tag `legacy-engine-2026-10-19`. Why Varos moved to VectorCraft
is told in [docs/varos/HISTORY.md](docs/varos/HISTORY.md).

## Documentation

| Document | What it holds |
|---|---|
| [docs/varos/STATE.md](docs/varos/STATE.md) | The one truth table: what exists, its status and evidence |
| [docs/varos/BACKLOG.md](docs/varos/BACKLOG.md) | Ordered work, each item with its acceptance line |
| [docs/varos/FORK.md](docs/varos/FORK.md) | How Varos relates to upstream: rules, merges, licences |
| [docs/varos/HISTORY.md](docs/varos/HISTORY.md) | The project story, in facts |
| [docs/varos/WORKSHOP.md](docs/varos/WORKSHOP.md) | How work is planned, built, gated and reviewed |
| [docs/varos/PAGES.md](docs/varos/PAGES.md) | Project pages (native format v5) |
| [docs/varos/SHELL_SUMMARY_AR.md](docs/varos/SHELL_SUMMARY_AR.md) | Shell design of record (Arabic) |
| [docs/varos/PANEL_IMPLEMENTATION_PLAN.md](docs/varos/PANEL_IMPLEMENTATION_PLAN.md) | Panel workspace plan and approval record |
| [docs/varos/STABILITY_2026-11-01.md](docs/varos/STABILITY_2026-11-01.md) | Stability work and verification record |
| [docs/varos/AI_PANEL_PLAN.md](docs/varos/AI_PANEL_PLAN.md) | Planned in-app AI panel |
| [docs/varos/lanes/](docs/varos/lanes/) | Lane reports and gate logs (evidence, not state) |
| [AGENTS.md](AGENTS.md) | Rules for agents and contributors |
| [docs/UPSTREAM_README.md](docs/UPSTREAM_README.md) | Upstream VectorCraft README (reference) |

## Licence

MIT OR Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)).
Required notices: [NOTICE](NOTICE). Assets and their licences: [ASSETS.md](ASSETS.md).
