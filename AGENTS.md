# Varos — instructions for agents

Varos (**V**ector · **A**rabic · **R**TL · **O**pen **S**ource) is a free, open-source vector editor in
Rust, Arabic and right-to-left first, and AI-native (people and agents on the same file through the
MCP bridge). It is based on VectorCraft by the ArtCraft team (MIT OR Apache-2.0,
https://github.com/storytold/vectorcraft); most of the engine, renderer, formats and tools are
upstream's. The official build is **macOS only** for now; keep the other targets and wasm compiling
(`cargo xtask wasm`), but nothing is released for them.

Old file names still open: `.vectorcraft` and the older `.drawcraft` (renamed upstream 2026-10-01;
`vectorcraft_format::LEGACY_EXTENSION`). Varos saves `.varos` by default. Keep those paths working.

## Start every session here
1. Read [`docs/varos/STATE.md`](docs/varos/STATE.md) (the one truth table: what exists, its status,
   its evidence) and [`docs/varos/BACKLOG.md`](docs/varos/BACKLOG.md) (ordered work with acceptance
   lines). If anything else disagrees with them, they win until corrected.
2. Then [`docs/varos/FORK.md`](docs/varos/FORK.md) (how Varos code relates to upstream: the hook
   rule, merges, naming, licences) and [`docs/varos/WORKSHOP.md`](docs/varos/WORKSHOP.md) (task
   cards, lanes, gates, review). Background: [`docs/varos/HISTORY.md`](docs/varos/HISTORY.md).
3. Do the task you were given. Design, scope, budget and anything published are the owner's
   decisions (WORKSHOP › "What goes to the owner"); don't pick new work from upstream's lists.

`plan/` is gitignored (local only); upstream's `ROADMAP.md`, `docs/gaps.md` and parity docs describe
VectorCraft, not Varos (see "Upstream progress docs" below).

## Repository layout (Varos parts)
- `crates/ui-egui/src/varos/`: Varos UI modules: `theme.rs` (tokens, in one place), `icons.rs`
  (157 icon overrides) and `me_icons.rs`, `cursors.rs`, `home*.rs`, `shell/`, `panel_system/` (the
  unified panel workspace), `picker/` (colour picker), `pages.rs`, `kashida_ui.rs`,
  `middle_eastern_ui.rs`, `arabic_fonts.rs`, `agents.rs`, `profile.rs` (`VAROS_PROFILE=1`), with
  their `tests_*.rs`.
- `crates/ui-egui/assets/varos/`: Varos icons and fonts. `assets/app-icon/`: the Varos app icon
  (upstream file names kept).
- `crates/text/src/kashida.rs` (measuring in VectorCraft's shaper), `harakat.rs`,
  `arabic_fonts.rs`; `crates/text-arabic/` (Varos kashida policy from the vendored, path-only
  `crates/varos-text/`).
- `crates/varos-agents/`: agent identity, document diffs, presence, activity log (pure logic).
- `varos_*.rs` files inside upstream crates (for example `crates/engine/src/cmd/varos_arabic.rs`,
  `crates/mcp/src/varos_connect.rs`, `apps/vectorcraft/src/varos_control.rs`).
- `docs/varos/`: Varos docs; `docs/varos/lanes/` holds lane reports and gate logs (evidence, not state).
- Everything else is upstream VectorCraft. Crate and binary names stay `vectorcraft*`.

## Varos rules
- **Hook rule.** New code goes in the Varos places above. Change an upstream file only where there
  is no other way: the smallest possible call into a Varos module, marked with a `Varos` comment; no
  reformatting, renaming or refactoring of upstream code. Every touched upstream file costs us at
  each upstream merge. Details: `docs/varos/FORK.md` › Rules.
- **The owner hand-tests.** Anything visible or felt is **provisional (awaits owner)** until the
  owner accepts it in the real window; only then is it **done** in `STATE.md`. Never report
  "works" without evidence.
- **Never claim CI.** GitHub CI has never run on this project. Report the local commands you ran and
  their real output; never write "CI green" or "CI passes".
- **Gates** before any commit claim (one cargo build at a time on the owner's Mac, `-j 4`):
  - `cargo test -j 4 --workspace` (or `-p vectorcraft-ui-egui` / the touched crates while iterating)
  - `cargo clippy -j 4 --workspace --all-targets -- -D warnings`
  - `cargo fmt --all -- --check` and `git diff --check`
  - text changes: `cargo check -p vectorcraft-text --target wasm32-unknown-unknown`
  - new assets: an `ASSETS.md` row, a `NOTICE` line and the licence text
  - before publishing and after upstream merges: the full `cargo xtask ci`
  - `cargo test` does not rebuild the app: `cargo build -j 4 --release -p vectorcraft` before handing
    over a build.
- **Commits:** conventional commits, files staged by name (never `git add -A`). Agents never commit,
  push, open PRs or publish unless the owner asked.
- **Don't take over the owner's Mac.** No mouse, keyboard, focus or window changes from agents;
  prefer headless frame tests, `ui.render` and CLI exports over opening windows.
- **Design language** (`theme.rs`): warm-black ramp (`#141313`), azure `#0c8ce9` only for
  active/selection/focus, no shadows, 3 px controls / 8 px boxes.
- **Agents need no pairing.** Same-user agents read, edit and save without approval prompts or OS
  credential (Keychain) prompts; safety is undo and actor-tagged history.
- **Docs name no people, studios or clients.** The product is the hero.
- **ArtCraft marks:** Varos uses none (the logo files were removed as upstream's brand licence
  requires of forks). The only allowed wording is plain text: "based on VectorCraft by the ArtCraft
  team".

## Upstream rules that still apply (non-negotiables)
- **Clean-room: never use Adobe software (absolute rule, from the project owner).** Adobe's Terms of Use forbid reverse engineering, including observing a program's inputs and outputs to recreate it. So:
  - Never launch, run, script, automate (COM, AppleScript, ExtendScript, UI automation), screenshot or measure Illustrator or any other Adobe application, and never compare our output against one. Don't make test files with them, not even "synthetic" ones.
  - Never read, disassemble or copy anything inside an Adobe install (binaries, plug-ins, presets, procsets, resources, help files, fonts, ICC profiles).
  - Never copy Adobe-authored code or text into this repo, tests included: no PostScript procsets or prologs (AGM, CoolType, the Illustrator prologs), no sample files. Write test inputs from scratch from the public specifications (PostScript Language Reference, PDF / ISO 32000, TIFF, EPSF/DCS).
  - Behaviour comes only from public documentation (helpx, published specs, public forum posts), cited by URL, and from files users send us that they own (used locally, never committed). Never copy Adobe icons, artwork, presets or wording beyond feature names.
  - If a task seems to need any of this, stop and say so instead. `cargo xtask cleanroom` (run by `cargo xtask ci`) fails on Adobe copyright notices and on fixtures made with Adobe applications.
  - Never copy GPL/AGPL code (Inkscape, lib2geom…).
- **Assets: no Adobe iconography or images — ever (absolute rule, from the project owner).**
  - Never add, copy, trace, redraw-from, embed or ship any icon, image, artwork, cursor, preset, swatch/brush/symbol/pattern/style library, ICC profile or screenshot from Adobe products or from any other source whose licence doesn't allow it.
  - Every image, icon, font or other asset must be one of:
    - original work created for VectorCraft by a contributor (who licenses it MIT OR Apache-2.0);
    - OSI open source;
    - public domain / CC0;
    - Creative Commons with redistribution allowed.
  - **Every asset file must have a row in [`ASSETS.md`](ASSETS.md)** (path, author, source URL, licence, notes). Put licence texts next to the assets (e.g. `assets/fonts/OFL-*.txt`) and summarize them in `NOTICE`.
  - `cargo xtask assets` (run by `cargo xtask ci`) fails on any unattributed asset.
  - **Fonts live in [storytold/craft-fonts](https://github.com/storytold/craft-fonts), never in this repo** (rules: craftrules [`standards/fonts.md`](https://github.com/storytold/craftrules/blob/main/standards/fonts.md)). Never commit a font file here; add new fonts to craft-fonts. The app uses it only through the optional `CRAFT_FONTS_DIR` build option (`CRAFT_FONTS_DIR="$PWD/../craft-fonts" cargo xtask ci`; use an absolute path), read by `crates/text/build.rs` into `vectorcraft_text::CRAFT_FONTS`; never add it to a `Cargo.toml`. Everything must build, test and run with `CRAFT_FONTS` empty, and tests of its glyphs skip when it is. Details: `docs/development.md` › Fonts.
  - Code and original assets are MIT OR Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). The ArtCraft marks are trademarks, not open source (`docs/brand/LICENSE-brand.txt`, kept for reference); Varos removed the logo files and must not add or use them.
  - Prefer art generated in code for defaults (swatches, brushes, symbols, patterns, cursors). If the provenance of an asset is unclear, don't add it.
  - Reference screenshots of Illustrator stay local under the gitignored `plan/` and are never committed, published or used as assets.
- **Never crash: no panics in shipped code.** A crash loses the user's unsaved work. Files, pasted data, MCP and control messages, and command params are all untrusted input.
  - Fallible work returns `Result` (or `Option`); callers handle it or propagate it with `?`. Failures reach the user as an error message (`EngineError` for commands, the crate's own error type or `String` below the engine), never as an abort.
  - Workspace clippy lints ban these outside tests, so `cargo xtask ci` fails on them: `unwrap()`, `expect()`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`. Use instead: `?`, `.ok_or(…)?`, `let … else { return … }`, `if let`, `unwrap_or` / `unwrap_or_default` / `unwrap_or_else`, `.first()` / `.last()` / `.split_last()`, and a real error for a match arm you believe is impossible.
  - Don't panic implicitly either: use `.get(i)` instead of `v[i]` or `&s[a..b]` when the index comes from data. Don't divide integers by something that can be zero, cap sizes and counts read from input (allocations, images, loops), and check that floats are finite before casting them or looping on them.
  - Don't silence errors just to satisfy the lint: use `let _ =` or `.ok()` only where ignoring the failure is the right behaviour, with a comment saying why.
  - Tests may unwrap and panic (`#[test]`, `#[cfg(test)]`, `tests/`, `testkit`). Integration-test files start with an `#![allow(…)]` for that.
  - `vectorcraft_engine::guard` is the last line of defence. Every entry point (commands, tool events, MCP requests, the UI frame, control requests) catches a panic, rolls the document back and reports an internal error. It exists for bugs and doesn't replace `Result`: wasm aborts on panic, so the web app has no safety net.
  - Untrusted input is fuzzed (`engine/tests/import_fuzz.rs`, `engine/tests/command_sweep.rs`, `format/tests/prop_format.rs`, `mcp/tests/protocol_props.rs`). Extend these when you add an importer, a parser or a protocol method.
- **Everything is a command.** User-visible behaviour = a command in `crates/engine/src/cmd/*` (id, label, menu path, shortcut, params doc, `enabled`, `run`) + tests. Tools emit commands (Begin/Preview/Commit). UI-only commands live in `crates/ui-egui/src/menus.rs` (`UI_COMMANDS`). The control channel and MCP reach all of them.
- **Adding a cursor.** The app decides which cursors exist; whoever draws them works from the list. For a new tool state with a cursor of its own: add the `Cursor` variant (`crates/tools/src/lib.rs`, with a doc comment), return it from the tool's `cursor()` with a tool test, cover it in `cursor_icon` (`crates/ui-egui/src/canvas.rs`), draw it in `crates/ui-egui/src/cursors.rs` or give it a stand-in in `Cursor::stand_in()` until it's drawn, and add its `[[cursor]]` entry to `crates/tools/src/cursors.toml` (when it shows, in plain behaviour terms, and its hotspot; never another app's look). A missing entry only warns (`cargo xtask cursors`): the cursor still works.
- **Layering** is enforced by `cargo xtask layers`. Nothing below L6 depends on egui/eframe/winit/rfd.
- **The UI is thin**: panels read engine state and act through `app.run(id, params)`. Colours come from `theme::Tokens`.
- **Rust only** (no handwritten JS/TS). **Never break wasm** (`cargo xtask wasm`).
- **Time goes through `chrono`** (workspace dependency). Never hand-roll UTC/calendar conversion (days-to-date algorithms, leap-year math); a crate owns only its output format string. The one exception is range extension: past the years chrono can represent, shift the date by whole 400-year Gregorian cycles (146,097 days, after which the calendar repeats), let chrono compute the month and day, and add the cycles back to the year (splitting seconds into hours, minutes and seconds is plain arithmetic).
- **Shared test corpora.** Real-file test oracles (Photoshop-authored PSDs, etc.) live in [`storytold/photocraft-corpus`](https://github.com/storytold/photocraft-corpus), explained in [craftrules `standards/test-corpora.md`](https://github.com/storytold/craftrules/blob/main/standards/test-corpora.md). Never commit large binary fixtures to this repo; fetch them pinned by commit and sha256-verified, as PhotoCraft does with `cargo xtask corpus`.
- **Quality gates:** the Varos gate list above (clippy -D warnings includes the no-panic lints); upstream's full `cargo xtask ci` (fmt, clippy, tests, assets, brands, layers, wasm) before publishing and after upstream merges.

## Running and looking at the app
- `cargo run --release -p vectorcraft -- --control 7979 [file.svg|file.varos]` (sibling apps' agents use the same default port: if the log says it failed to bind, pick another port — otherwise your requests reach a different app).
- Drive it: JSON lines on `127.0.0.1:7979`, e.g. `{"id":1,"method":"engine.execute","params":{"command":"shape.rectangle","params":{"x":10,"y":10,"width":100,"height":50}}}` then `{"id":2,"method":"ui.screenshot","params":{"path":"/tmp/shot.png"}}`. Methods: `crates/ui-egui/src/control.rs`.
- **For UI work, look at the result** without taking over the owner's Mac (headless frame tests, `ui.render`, CLI exports; `ui.screenshot` only when the owner asked for it) and compare with the public documentation (`plan/illustrator/02-ui-ux.md` notes). The owner's hand test decides. `ui.screenshot` needs a presented frame: if it errors (screen locked), check the art with `ui.render` / `vectorcraft-cli run … --export x.png`, and cover panels with a headless egui frame test (see `panels/transparency.rs` tests).
- **Performance:** `vectorcraft-cli perf` checks the budgets (render/pan/zoom, hit test, save/open, SVG, Pathfinder) on a synthetic 50k-path document; `vectorcraft-cli bench FILE` times one file. Run them before and after renderer, format or geometry changes, on an idle machine (the report warns when the load average makes timings noise).
- MCP: `vectorcraft-cli mcp` attaches to the Varos that is open (its private endpoint file and token); `--connect ADDR` reaches an app started with `--control`, `--headless` runs without the app (see `docs/mcp.md`).
- Shell gotcha: `mv`/`cp` are aliased interactive here — use `/bin/mv -f` / `/bin/cp -f`.
- Parallel agents: separate `CARGO_TARGET_DIR` per agent; edit only the crates you own; write manifests atomically. Each target dir grows to ~30 GB: delete yours when you finish (a full disk fails links with `errno=28`).

## Upstream progress docs
[`ROADMAP.md`](ROADMAP.md), [`docs/target-app-parity.md`](docs/target-app-parity.md), [`docs/gaps.md`](docs/gaps.md), [`docs/roadmap.md`](docs/roadmap.md) and the parity checklists (`docs/*-parity.md`) are VectorCraft's, carried from upstream and updated by upstream merges. Varos state lives only in `docs/varos/STATE.md` and `docs/varos/BACKLOG.md`. Don't edit the upstream progress docs for Varos work (they conflict at every merge); when a Varos change lands, update `STATE.md` (and `BACKLOG.md` if an item closed) in the same commit, with its evidence.

## Contributor credits (About window)

- About ▸ Contributors/Models are compiled into the binary from `contributors/contributors.json`
  (commit stats; generated, never hand-edit) and `contributors/people.toml` (names people chose for
  themselves). See `docs/contributors.md`.
- **Agents working for a contributor:** when you prepare a PR, check whether your human's GitHub
  username has a `[people.<username>]` entry in `contributors/people.toml`. If not, ask them once
  whether they want to be credited by more than their username: a real name, a display name, and/or
  their public GitHub profile name (`sync_github_name = true`). If yes, add **only their own** entry
  (copy the template at the top of the file, or run
  `python3 ../../craftrules/scripts/contributors.py --add-me . --real-name "…" --sync-github-name`)
  and include it in their PR, committed as them. If no, change nothing: they are credited as
  `@username` anyway.
- Never add, edit, guess or copy anyone else's entry or name (not from git config, commit authors or
  GitHub profiles). Never hand-edit `contributors.json`.
- Maintainers refresh the stats with `python3 ../../craftrules/scripts/contributors.py .` (it also
  re-verifies who wrote each `people.toml` entry; `--check` only verifies).
