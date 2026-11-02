# Varos and upstream VectorCraft

Status: current, 2026-11-02. Governs how Varos code relates to upstream. Numbers are measured from
git at HEAD `292bd2418` unless a date says otherwise.

## What this is

Varos is an Arabic-first vector editor built on VectorCraft (storytold / ArtCraft team, MIT OR
Apache-2.0, https://github.com/storytold/vectorcraft). The git remote `upstream` points at
storytold/vectorcraft.

Owner decision 2026-11-02: this repository's code replaces the contents of the public repository
`thisisgomaa/varos`; that repository's earlier engine is kept on branch `legacy-engine` and tag
`legacy-engine-2026-10-19` (see [HISTORY.md](HISTORY.md)).

## Timeline with upstream

| Date | Event | Evidence |
|---|---|---|
| 2026-10-09 | Stage-0 spikes start on upstream `a469568c` (2026-10-09) | parent of `0aa3f03b4` |
| 2026-10-18 | Trial merge of the stage-0 work onto upstream `cdba0aac` (2026-10-18): 2,808 upstream commits and 1,144 files since the spike base gave 11 conflicted files, 1-2 small hunks each | merge `dd0e13b39` |
| 2026-10-20 | Upstream merge: 239 commits (through `19e3144d`) | merges `5e778cb43`, `7b64653c5` |
| 2026-10-26 | Upstream merge: 1,503 commits (through `1cdc9873`), 61 conflicted files | merge `9e96e8634`; `lanes/upstream-merge.log` |
| 2026-11-01 | Four upstream stability fixes cherry-picked (VSTAB.1) | [STABILITY_2026-11-01.md](STABILITY_2026-11-01.md) |
| 2026-11-02 | Pending: 975 upstream non-merge commits (1,468 with merges) since `1cdc9873`; 86 predicted conflicted paths; staged plan in [BACKLOG.md](BACKLOG.md) | `git rev-list --count --no-merges upstream/main ^HEAD` |

Current merge base: **`1cdc9873`** (upstream `main`, 2026-10-26). Varos commits on top: **188**
(134 non-merge, 54 merge; first 2026-10-18).

## Why Varos moved onto VectorCraft

- **A working editor that ships.** VectorCraft was hand-tested on 2026-10-17 and judged good,
  except that it slows down with many vectors.
- **Performance is a known, contained problem.** Measured in a release build, 4 render threads,
  1920x1080: the CPU renderer is linear in visible paths, 1k = 3.6 ms, 5k = 16 ms, 20k = 64 ms per
  frame. Culling works. The ceiling is the CPU renderer, not the document core. Plan: tile cache
  first (estimated 2-4 weeks), GPU decision after (estimated 6-10 weeks for an interactive core).
- **Arabic transfers.** Kashida from `varos-text` was proven through VectorCraft's own HarfRust
  shaper and its real text node (6 tatweel insertions in a 600 px justified line, rendered and
  inspected). VectorCraft already had shaping, bidi and word-space justification.

## What Varos brought and added

- `varos-text` kashida policy (which joins, in what order, how much slack), consumed through
  `crates/text-arabic/`; measuring stays in VectorCraft's shaper (`crates/text/src/kashida.rs`,
  714 lines, Off / Auto / Force with a strength). Upstream added its own "Insert Kashidas" on
  2026-10-21; at the 2026-10-26 merge Varos kept its engine.
- Bridge without pairing: any same-user agent may read, edit and save; safety is undo and actor-tagged
  history, never approval prompts or Keychain prompts (`crates/varos-agents/`, `crates/mcp/src/varos_*.rs`).
- Design language: warm-black ramp (`#141313`), azure only for active/selection/focus, no shadows,
  3 px controls / 8 px boxes, tokens in one place (`crates/ui-egui/src/varos/theme.rs`).
- Icons (157 overrides in `crates/ui-egui/src/varos/icons.rs`) and the v1.1 cursor set. Cursors and
  icons are never hand-drawn by an agent; they come from a design source.
- Native format version 5 with Pages (upstream: 4); `.varos` as the default save extension.
- Size (Rust, `git diff 1cdc9873 HEAD`): +56,276 / -2,334 lines in 304 `.rs` files, about 8% of the
  ~705k Rust lines in the tree (this includes the four cherry-picked upstream fixes).
  `crates/ui-egui/src/varos/` holds ~36.3k lines (of which `panel_system/` ~8.2k) and 508 `#[test]`
  functions.

## Rules

1. **Varos code lives in Varos modules; upstream files are edited only where unavoidable.**
   - New code goes in Varos-owned places: `crates/ui-egui/src/varos/`, `crates/text-arabic/`,
     `crates/varos-agents/`, `crates/ui-egui/assets/varos/`, and `varos_*.rs` / clearly Varos files
     inside upstream crates (for example `crates/engine/src/cmd/varos_arabic.rs`,
     `crates/mcp/src/varos_connect.rs`, `apps/vectorcraft/src/varos_control.rs`,
     `crates/text/src/harakat.rs`).
   - An upstream file is changed only when there is no hook point: the change is the smallest
     possible call into a Varos module, marked with a `Varos` comment. No reformatting, renaming or
     refactoring of upstream code.
   - This is not "additive only": at HEAD, 261 upstream files are modified (185 `.rs`, 24 i18n
     catalogs) and 362 files are added outside `docs/varos/`. Every modified upstream file is a merge
     cost, so prefer a new Varos module over a larger hook.
2. **Upstream merges are staged and gated.** `git fetch upstream` every session and record what is
   useful; merge in stages in a dedicated worktree, each stage with the local gates (see
   `AGENTS.md`) before the next. Record date, upstream commit and conflict count in `STATE.md`.
   Upstream UI work for the classic chrome is reference, not merge, where it collides with the Varos
   shell.
3. **General fixes go upstream** as PRs to storytold/vectorcraft (bugs, performance, anything not
   Varos-specific). Opening a PR is a publish action: the owner approves each one.
4. **Naming:** the product name is Varos. Crate names (`vectorcraft-*`), the binary
   (`vectorcraft`) and the bundle executable/icon resource (`VectorCraft`) stay upstream's to keep
   merges cheap. Native saves default to `.varos`; `.vectorcraft` and `.drawcraft` remain readable.
   The serialized `format` identifier stays `vectorcraft` for compatibility; the generator
   identifies Varos. Save a Copy also offers the upstream suffix.
5. **Attribution:** keep upstream's copyright, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE` and the
   contributor screens. Every new asset gets a row in `ASSETS.md` (path, author, source, licence)
   and a summary in `NOTICE`; licence texts sit next to the assets. The only attribution wording the
   brand licence allows is plain text: "based on VectorCraft by the ArtCraft team". The ArtCraft
   logo files were removed on 2026-11-02 as `docs/brand/LICENSE-brand.txt` requires of forks; the
   licence text stays for reference. Never present Varos as an ArtCraft product.
6. **Upstream's `AGENTS.md` rules still apply:** clean-room, no GPL/AGPL code, no Adobe or other
   non-licensed artwork, `cargo xtask brands` (no other vendors' product names in user-visible
   text), no panics in shipped code.
7. **Never commit proprietary reference material** (for example extracted third-party cursors kept
   locally for functional reference).
8. **Docs name no people, studios or clients.** The product is the hero.

## Licences

- Varos code: MIT OR Apache-2.0, like upstream.
- `varos-text` (earlier Varos engine repository, `crates/varos-text`): relicensed from GPL-3.0-only
  to MIT OR Apache-2.0 on 2026-10-18 (commit `6bf9787` there). Pinned here at rev `5936eaf` over
  HTTPS (`https://github.com/thisisgomaa/varos.git`, `crates/text-arabic/Cargo.toml`); that revision
  must stay reachable after the repository replacement (it is on the `legacy-engine` history).
- Varos v1.1 cursor set: MIT OR Apache-2.0, 2026-10-18 (same commit).
- Original Varos icon glyphs: MIT OR Apache-2.0, 2026-10-18 (commit `3ec29e2` there).
- Varos app icon: original work, MIT OR Apache-2.0 (copyright line in `NOTICE` and
  `assets/app-icon/README.md`).
- Bundled third-party: Lucide (ISC/MIT), Inter, JetBrains Mono, IBM Plex Sans Arabic (OFL 1.1);
  Noto Sans Arabic as a test fixture only (OFL 1.1). Details in `NOTICE` and `ASSETS.md`.

## Publishing

Publishing (pushes to the public repository, upstream PRs, releases, posts) is the owner's
decision. The repository replacement was decided on 2026-11-02. A release build should still meet
the three bars: **Arabic** (kashida and Arabic type that beat the alternatives), **performance**
(large documents stay smooth), **design** (the Varos look and feel, owner-approved). Checklist:
`BACKLOG.md` "Publish checklist".

Revision: 2026-11-02 — merge history and base corrected from git (spike base `a469568c`, trial
merge onto `cdba0aac`, current base `1cdc9873`, 2,808 commits in the trial instead of 3,966);
"additive only" replaced by the real hook rule; logos removed; names removed.
