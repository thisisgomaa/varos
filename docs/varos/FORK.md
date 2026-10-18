# Varos fork contract

Status: current, 2026-10-19. Owner: Ahmed. Local and unpublished.

## What this is

Varos is an Arabic-first fork of VectorCraft (storytold / ArtCraft team, MIT OR Apache-2.0,
https://github.com/storytold/vectorcraft). The base is upstream `main` at `cdba0aac` (commit dated
2026-10-18) plus the Varos lanes merged on top. The git remote `upstream` points at
storytold/vectorcraft. The fork has no public repository yet.

## Why we forked

- **The owner's goal:** a strong Arabic-first vector tool that ships, is used for real work, and is
  developed by a community. VectorCraft is already a working editor; Ahmed hand-tested it on
  2026-10-17 and liked it, except that it slows down with many vectors.
- **Performance is a known, contained problem.** Measured in a release build, 4 render threads,
  1920x1080: the CPU renderer is linear in visible paths, 1k = 3.6 ms, 5k = 16 ms, 20k = 64 ms per
  frame. Culling works. The ceiling is the CPU renderer, not the document core. Plan: tile cache
  first (2-4 weeks), GPU decision after (estimated 6-10 weeks for an interactive core).
- **Arabic transfers.** Kashida from `varos-text` was proven through VectorCraft's own HarfRust
  shaper and its real text node (6 tatweel insertions in a 600 px justified line, rendered and
  inspected). VectorCraft already had shaping, bidi and word-space justification; kashida was the
  missing piece.

## What we keep from Varos

- `varos-text` kashida policy (which joins, in what order, how much slack) — consumed through
  `crates/text-arabic/`; measuring stays in VectorCraft's shaper (`crates/text/src/kashida.rs`).
- Bridge without pairing: any same-user agent may read, edit and save; safety is undo and (later)
  per-agent history, never approval prompts or Keychain prompts.
- Design language: warm-black ramp (`#141313`), azure only for active/selection/focus, no shadows,
  3 px controls / 8 px boxes, tokens in one place (`crates/ui-egui/src/varos/theme.rs`).
- Icons (Varos originals and the owner's Lucide picks) and the v1.1 cursor set. Cursors and icons
  are never hand-drawn by an agent; they come from a design source.

## Rules

1. **Additive changes only.** New code goes in new files: `crates/ui-egui/src/varos/`,
   `crates/text-arabic/`, and new asset folders. Upstream files get only minimal hook lines, each
   marked with a `Varos` comment. No reformatting or refactoring of upstream code.
2. **Weekly upstream merge:** `git fetch upstream && git merge upstream/main`. First test: stage 0
   against upstream (3,966 commits, 1,144 files in 8 days) gave 11 conflicted files, 1-2 small hunks
   each, mostly append-type (ASSETS.md, NOTICE, Cargo files, tests) plus 6 small hooks.
3. **General fixes go upstream** as PRs to storytold/vectorcraft (bugs, performance, anything not
   Varos-specific). Opening a PR is a publish action: the owner approves each one.
4. **Naming:** the product name is Varos. Crate names (`vectorcraft-*`), the binary
   (`vectorcraft`), the bundle executable/icon resource (`VectorCraft`), document extensions and
   persisted identifiers stay upstream's for now, to keep merges cheap and files compatible.
5. **Attribution:** keep upstream's copyright, `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE` and the
   contributor screens. Every new asset gets a row in `ASSETS.md` (path, author, source, licence)
   and a summary in `NOTICE`; licence texts sit next to the assets. The About screens show the fork
   notice. The ArtCraft logos in `docs/brand/` are upstream trademarks: never modified, never used
   for Varos.
6. **Upstream's AGENTS.md applies:** no GPL/AGPL code, no Adobe or other non-licensed artwork,
   `cargo xtask brands` (no other vendors' product names in user-visible text).
7. **Never commit proprietary reference material** (e.g. extracted third-party cursors kept locally
   for functional reference).

## Licences

- Fork code: MIT OR Apache-2.0, like upstream.
- `varos-text` (Varos repo, `crates/varos-text`): relicensed from GPL-3.0-only to MIT OR Apache-2.0
  by the owner on 2026-10-18 (Varos commit `6bf9787`). Pinned here at rev `5936eaf`.
- Varos v1.1 cursor set: MIT OR Apache-2.0, 2026-10-18 (same commit).
- Original Varos icon glyphs: MIT OR Apache-2.0, 2026-10-18 (Varos commit `3ec29e2`).
- Varos app icon: original work, (c) 2026 Ahmed Gomaa, MIT OR Apache-2.0.
- Bundled third-party: Lucide (ISC/MIT), Inter, JetBrains Mono, IBM Plex Sans Arabic (OFL 1.1);
  Noto Sans Arabic as a test fixture only (OFL 1.1). Details in `NOTICE` and `ASSETS.md`.

## When we publish

Only when there is something real to show in all three: **Arabic** (kashida and Arabic type that
beat the alternatives), **performance** (large documents stay smooth), **design** (the Varos look
and feel, owner-approved). Publishing is the owner's decision. Checklist: `BACKLOG.md` item 10.
