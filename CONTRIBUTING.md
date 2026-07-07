# Contributing to Varos

Thank you for even opening this file. Varos is built by a tiny team (one designer-founder + AI pair-programming sessions), so every real contribution moves the needle.

## Build & test

```bash
cd varos            # the Cargo workspace lives here, one level below repo root
cargo build         # or: cargo run -p varos-app
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings   # CI gates on this
cargo fmt --all --check
```

All four must pass — CI enforces them on every PR.

## Ground rules

- **The core stays pure.** `varos-core` must never grow a GPU, window, or filesystem-UI dependency. The compiler enforces it; PRs that break the seam are declined.
- **No test may construct a GPU `Renderer` or an `EventLoop`** — the suite is CPU-pure by design and must stay runnable on headless CI.
- **Feel is a feature.** `animation_time = 0` is law: no fades, no eased panels. If your change adds a delay a hand can feel, it needs a very good reason.
- **The visual constitution** (`docs/UI_DIRECTION.md`) governs all UI: warm-black palette, azure used surgically, no shadows, hand-painted widgets.
- Commit style: conventional commits (`feat(layers): …`, `fix(snap): …`), staged by name — never `git add -A`.

## Licensing of contributions

Varos is GPL-3.0. By submitting a contribution you:

1. Certify the [Developer Certificate of Origin](https://developercertificate.org/) — sign your commits with `git commit -s` (`Signed-off-by: Your Name <email>`).
2. License your contribution under GPL-3.0, **and additionally grant the project maintainer (Ahmed Gomaa) a perpetual, irrevocable right to relicense your contribution** as part of future Varos releases.

**Why clause 2 exists — the honest version.** Varos itself stays GPL, permanently; nobody (including us) can take *this* codebase away from you. The extra grant exists for one reason: sustainability. It keeps doors open like a Microsoft Store build or a commercially-licensed edition that *funds* the free one — the same pattern that pays Blender's and Krita's developers — without having to track down every past contributor for permission years later. It is not a mechanism to close the project: the GPL code that exists is GPL forever.

If you're not comfortable with clause 2, open an issue and we'll talk before you write code — DCO-only contributions to clearly-bounded areas are negotiable.

## Where to start

- Issues labeled **`good first issue`** are prepared to be finishable in one sitting.
- Icons, translations, documentation, and testing on varied Windows GPUs (especially Intel iGPU) are always welcome — no Rust required.
- Big features start with a short design note in `docs/` (see existing ones for the pattern: problem → decision → gates), not with code.

## AI-assisted contributions

Welcome — this project is largely built that way. Two rules: you must understand and stand behind every line you submit, and the test/clippy gates apply equally. See `CLAUDE.md` for the standing session rules we use ourselves.
