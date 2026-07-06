# VAROS Engineering Review — Practices, not Features

**Date:** 2026-07-05
**Author:** Claude (Opus 4.8), commissioned by Ahmed
**Purpose:** A grounded, defensible engineering comparison between **VAROS** and **[basarai](https://github.com/amElnagdy/basarai)** — a professionally-built SaaS by a specialist developer — extracting the *transferable engineering practices* VAROS should adopt. This is a **planning input**, meant to be argued with, not a verdict.
**Status:** v2 — every quantitative claim re-verified against commit `62732a0` by a 4-lens adversarial pass (accuracy, context-fairness, completeness, technical soundness). Corrections from that pass are folded in.

> **Framing rule used throughout.** basarai and VAROS live in different worlds:
> - **basarai** = multi-user web SaaS → untrusted input, network, secrets, auth. CI, validation, and security are *existential*.
> - **VAROS** = single-user native desktop app in Rust, hand-tested for *feel*. Rust's type system delivers at compile time much of what basarai buys at runtime.
>
> Every recommendation is tagged **[GENUINE GAP]** (a real practice VAROS is missing regardless of context), **[CONDITIONAL]** (adopt only when a specific trigger appears), or **[CONTEXT — do not copy]** (something basarai does that does *not* transfer to a solo native app). The goal is to steal the discipline, not the stack.

---

## 1. Executive summary

VAROS's **architecture and core test coverage are already stronger than basarai's** — the compiler-enforced pure-core seam and 80 headless core tests beat most projects, hobby or professional. The **production crash surface is genuinely small and concentrated** (see §3.3, corrected): only ~33 non-test `unwrap`s outside `#[cfg(test)]` and just **2** `expect`s in the whole codebase. What VAROS lacks is the **automation and safety scaffolding** that turns "working code" into "professional code":

1. **No CI.** 89 tests exist and run *by hand only*. Nothing guards regressions on push.
2. **No enforced lint/format.** `clippy` isn't even installed; there is no `rustfmt.toml` / `clippy.toml`.
3. **Two hard-panic clusters at the edges** — GPU init (`expect("no GPU adapter")`) and Win32 handle creation — crash the whole app instead of degrading. A panic hook already exists (`main.rs:244`) but only logs; it should also become a readable fatal dialog.
4. **Two large files** — `ui.rs` (2,510 LOC / 84 fns, in `varos-app`, **0 tests**) and `editor.rs` (1,956 LOC, in `varos-core`, **covered** by the 80 tests). These are in *different* crates with *different* refactor risk — a distinction the split plan must respect.
5. **Sprawling docs, no per-feature "definition of done."** VAROS out-documents basarai by a wide margin, but its docs are one giant narrative; basarai's numbered spec/plan/tasks folders give every feature explicit acceptance criteria.
6. **Three unaudited native-specific surfaces** the first draft missed: **12 `unsafe` FFI regions** in `cursors.rs`, a **496-crate** dependency tree with no `cargo-audit`/`cargo-deny`, and a **`.vrs` schema** whose forward-migration story stops at additive serde.

None of these require copying basarai's SaaS machinery (auth, Vault, Docker, Supabase — all **[CONTEXT — do not copy]**). They are cheap, native-appropriate disciplines with high leverage.

**The one-line takeaway:** *Add the automatic discipline (CI runs your tests + lint + dep-audit), stop letting the app hard-panic at the GPU/Win32 edges, and put a soundness note on the `unsafe` FFI. Your architecture is already ahead.*

---

## 2. Methodology (so this is auditable)

Every quantitative claim was measured directly against the working tree at commit `62732a0` on `main`, then independently re-measured by an adversarial fact-checker. Raw counts:

- **Size:** 4-crate Cargo workspace, ~10,325 lines of Rust.
  - `varos-core` — 27 files / 5,426 LOC = **16 src files (4,012 LOC) + 11 integration-test files (1,414 LOC)**. Pure logic, no GPU/window deps.
  - `varos-app` — 3 files, 3,557 LOC (UI/OS; `cursors.rs` 435, `main.rs` 612, `ui.rs` 2,510)
  - `varos-render-wgpu` — 2 files, 947 LOC (`lib.rs` 595, `tess.rs` 352)
  - `varos-pdf` — 2 files, 395 LOC
- **Tests:** 89 `#[test]` total → **core 80, pdf 5, render 4, app 0**. 11 integration files in `varos-core/tests/`. The render tests are CPU-pure tessellation tests; **no test constructs a GPU `Renderer` or an `EventLoop`** (this matters for CI — §3.1).
- **Error handling (corrected — production vs test split):**
  | | in source (`src/`) | of which outside `#[cfg(test)]` | in test files | naive total |
  |---|---|---|---|---|
  | `unwrap()` | 40 | **33** | 37 | 77 |
  | `expect()` | **2** (both `varos-render-wgpu/src/lib.rs` L205/212) | 2 | 25 | 27 |
  | `panic!/todo!/unreachable!` | 6 | 6 | 0 | 6 |
  | `Result<` | 10 (5 `file.rs`, 5 `pdf/lib.rs`) | 10 | 0 | 10 |

  > **This correction strengthens the thesis, it doesn't weaken it.** The real production panic surface is ~33 unwraps + 2 expects — small and concentrated at the OS/GPU edge — not the 77/27 a naive grep suggests. The data boundaries (`file.rs`, `pdf/lib.rs`) already propagate `Result<_, String>` via `map_err` (6 and 9 sites respectively).
- **Tooling:** no `.github/workflows` anywhere in the repo, no `rustfmt.toml`/`clippy.toml`, `clippy` component **not installed** (`cargo clippy` errors "not installed for toolchain"), no root `README`/`LICENSE`/`CLAUDE.md`/`AGENTS.md` (a `THIRD_PARTY_NOTICES.md` does exist).
- **Native surfaces (added in v2):** 12 `unsafe` regions in `varos-app/src/cursors.rs`; **496 transitive crates** in the tracked `Cargo.lock`; `.vrs` format at `VRS_VERSION = 1` with a header version-gate + atomic write + `sync_tree()` legacy upgrade.
- **basarai baseline:** GitHub Spec Kit (`specs/NNN-feature/{spec,plan,tasks}.md`, 9 features), service-layer split, `providers/{base,openai_image,gemini_image}.py` adapter pattern, structured `error_mapping`, `pytest` + `ruff`, **CodeRabbit** automated PR review, JWKS asymmetric JWT, Supabase Vault, Pydantic 2.x, `CLAUDE.md` + `AGENTS.md`.

---

## 3. Dimension-by-dimension

### 3.1 Continuous Integration — **[GENUINE GAP] · highest leverage · lowest effort**

**Evidence.** basarai runs `pytest` + `ruff` and layers CodeRabbit review on every PR. VAROS has 89 real tests that only ever run when Ahmed types `cargo test`. There is no `.github/workflows/`.

**Why it matters here specifically.** The pure core (geometry, booleans, layers, snap, transform, serde round-trip) is *exactly* the kind of code that breaks silently three commits later and isn't caught by feel-testing the UI. CI is the cheapest possible insurance for the 80 tests that already exist.

**This does not conflict with "Ahmed tests feel by hand."** CI protects the deterministic core; it makes no claim about UI feel. Complementary, not competing.

**One correctness caveat (from the adversarial pass).** `cargo test --workspace` is safe *today* because every test is CPU-pure and windows-latest ships a WARP software adapter — but the safety is incidental. The invariant to protect: **no test may construct a `Renderer` or `EventLoop`.** The day someone adds a GPU integration test, headless CI will hang/fail on the very `expect("no GPU adapter")` from §3.3. Keep GPU/window construction out of the test path, or gate such tests behind `#[ignore]` / a feature flag. See the corrected YAML in §5.1.

---

### 3.2 Lint & format discipline — **[GENUINE GAP] · low effort**

**Evidence.** `clippy` is not installed on the toolchain. No `rustfmt.toml` or `clippy.toml`. Formatting is whatever the editor did that day.

**Why it matters.** basarai enforces `ruff`. Clippy is stronger — it catches real correctness smells (needless clones, ignored `Result`s, inefficient iterators), not just style. Running it once on 10k LOC produces a free punch-list.

**Consistency guard (from the adversarial pass).** Enable **default** clippy only. Do **not** turn on the `clippy::unwrap_used` / `expect_used` restriction lints — they would flag exactly the internal-invariant unwraps §3.3 and §3.4 say to *keep*, putting two sections of this report at war. `-D warnings` on the default set is the right gate; the restriction lints are not.

**Recommendation.** `rustup component add clippy`; commit a minimal `rustfmt.toml` (even empty, to pin defaults); run `cargo fmt --all` once and commit it **before** turning on the `--check` gate (otherwise the first CI run red-fails on formatting, not on the intended clippy findings). Then wire both into CI.

---

### 3.3 Error handling & crash resilience — **[GENUINE GAP, narrow] · medium effort**

**Evidence — the good.** The *data boundaries* are already disciplined. `file.rs` and `pdf/lib.rs` propagate `Result<_, String>` with `map_err` on every I/O and parse step, with purpose-written human messages ("this file was saved by a newer Varos — please update"). Better than most projects; credit due.

**Evidence — the real risk (corrected).** Two genuine hard-panic clusters, both untestable by the 80 core tests and both in app-layer code with 0 coverage:

| File | Line | Call | Failure in the wild | Severity |
|---|---|---|---|---|
| `varos-render-wgpu/src/lib.rs` | 205 | `.expect("no GPU adapter")` | User with no/old GPU → **hard crash, no dialog** | **High** |
| `varos-render-wgpu/src/lib.rs` | 212 | `.expect("no device")` | GPU device init fails → hard crash | **High** |
| `varos-render-wgpu/src/lib.rs` | 202 | `create_surface(...).unwrap()` | Surface creation fails → crash | High |
| `varos-app/src/cursors.rs` | 224, 237 | `CreateDIBSection/CreateIconIndirect(...).unwrap()` | Win32 GDI handle exhaustion → crash | Medium (low-probability) |
| `varos-app/src/main.rs` | 263, 276, 611 | `EventLoop::new` / `create_window` / `event_loop.run(...).unwrap()` | Startup on odd display config → crash | Medium |

**Removed from this table after review:** the first draft listed `cursors.rs:158` (`usvg::Tree::from_str(...).unwrap()`) as a "malformed cursor SVG → crash." That was **unfair and wrong**: line 158 parses a **hardcoded compile-time `const`** SVG, so it can only fail on a developer typo caught on the first render on Ahmed's own machine — not a field failure. The *actual* external-file cursor path (`cursors.rs:186-187`) already degrades gracefully with `.ok()?`. That path is correct as-is.

**The basarai lesson, translated.** basarai's `error_mapping` turns every provider failure into a *human-readable message that preserves state* — the app never dies. The native equivalent: **at the GPU/Win32 edges, degrade to a dialog, not a panic.** "Your graphics driver couldn't start — update it or run in software mode" beats a stack trace.

**Recommendations, in order:**
1. **De-panic the two clusters above** — a friendly fatal-error window for unrecoverable GPU/window init; the Win32 handle failures are low-probability but sit inside `unsafe` (see §3.8) so they deserve handling too.
2. **Promote the existing panic hook to a user-facing safety net.** `main.rs:244` already calls `std::panic::set_hook(...)` — but it only logs. Make it *also* show a readable fatal dialog and write a crash log to `%APPDATA%/Varos/` (same location pattern as the existing `window.txt`). **This single hook protects every path the table above will never enumerate** — it is the true native translation of basarai's error observability, and it's the highest-leverage item in this section.

**Explicitly NOT recommended: a blanket `unwrap` purge.** The ~33 internal-invariant unwraps in the pure core (indexing a path that logically must exist) are *acceptable* — they assert invariants and are covered by the 80 tests. The priority is strictly the GPU/Win32 surface.

---

### 3.4 `String` errors → typed errors — **[CONDITIONAL] · adopt when a call site branches on kind**

**Reframed after review (was over-tagged "GENUINE GAP").** basarai needs structured `error_mapping` because a *server* must branch on failure *kind* (rate-limit → retry, bad-key → re-auth) and return codes over a wire. VAROS's file/PDF errors terminate at **one place: a dialog Ahmed reads**, and are already good human sentences. There are 10 `Result<` sites total; **none currently matches on error kind.**

So `thiserror`/`VarosError` is sound Rust, not SaaS-only — but it's *speculative structure* until there's a real consumer. **Adopt it the moment a call site needs to branch** — the natural first trigger is exactly in the code today: "file saved by a newer version" should offer an *update* action, while "corrupt file" should offer *recovery*. When you build that branch, a typed enum earns its keep. Until then, `String` messages that are already good UI text are a legitimate choice.

---

### 3.5 Module decomposition — **[GENUINE GAP] · medium effort · two DIFFERENT risk profiles**

**Evidence.** `ui.rs` = 2,510 LOC / 84 functions. `editor.rs` = 1,956 LOC. Together ~43% of all Rust. But — corrected — **they live in different crates with opposite test coverage:**

- `editor.rs` is in **`varos-core`** → **covered** by the 80-test suite (`transform.rs`, `groups.rs`, `layers.rs`, `snap.rs`, etc. exercise its public API). Splitting it as a pure move-refactor **behind the green suite is safe.**
- `ui.rs` is in **`varos-app`** → **0 tests.** A move-refactor here has **no net** — the report's own §3.3 admits app paths are only hand-exercised.

**Recommendation (differentiated):**
- **`editor.rs`:** split freely behind green tests — separate command/mutation logic from selection/query logic.
- **`ui.rs`:** either (i) add a thin layer of characterization/smoke tests for `varos-app` first, or (ii) accept it as a **manual-verification** refactor per the standing "Ahmed tests feel by hand" rule — but do it deliberately, knowing there's no automated safety net. Split along seams: `ui/tools.rs`, `ui/panels.rs`, `ui/input.rs`, `ui/render_glue.rs`.

The core's other modules (`boolean`, `geom`, `scene`, `model`, `units`, `tools/`) are already well-factored — proof Ahmed knows how; these two just outgrew their split.

---

### 3.6 Spec & process discipline — **[GENUINE GAP in structure, not volume] · low-medium effort**

**Evidence.** The counter-intuitive one: **VAROS out-documents basarai several times over** (~28 markdown specs, a 250 KB `DETAILED_ROADMAP.md`, `plan.html` as source of truth, plus `VAROS_CONSTITUTION.md`). The gap is *structure*, not effort. basarai's Spec Kit gives every feature a self-contained folder with `spec.md` (what + acceptance criteria), `plan.md` (how), `tasks.md` (steps) — an explicit, reviewable "definition of done" per feature. VAROS's knowledge is richer but lives in a few very large narrative documents, so "is feature X done, and how would we know?" has no crisp per-feature answer.

**The lesson is not "write more docs"** — it's **give each feature a bounded unit with acceptance criteria.** `VAROS_CONSTITUTION.md` shows Ahmed already thinks in this idiom.

**Recommendation.** Adopt a *lightweight* per-feature folder for **new** work (`specs/NNN-feature/spec.md` with explicit acceptance criteria + a task checklist), keeping `plan.html`/`DETAILED_ROADMAP.md` as the living roadmap. Don't retrofit old features; start at the next one.

---

### 3.7 Adapter / trait seams — **[GENUINE GAP, minor] · low effort**

**Evidence.** basarai's `providers/base.py` defines one interface; `openai_image.py` / `gemini_image.py` implement it, so adding a provider is one new file. VAROS already uses this instinct at its best seam — the **compiler-enforced** `core → render → app` dependency rule (`varos-core` has zero GPU/window deps). That boundary is *cleaner than basarai's* because the type system enforces it.

**Where to extend it.** Anything with multiple backends should get the same trait treatment — most obviously **export/import** (`.vrs` / PDF / future SVG / PNG). A `trait Exporter` / `trait Importer` makes "add SVG export" a new file, not scattered edits.

---

### 3.8 `unsafe` / FFI soundness — **[GENUINE GAP] · NEW in v2 · highest-risk code in the project**

**Evidence.** `varos-app/src/cursors.rs` contains **12 `unsafe` regions** — raw Win32 GDI/HCURSOR construction (`build_hcursor`, ~L209-241) and a window-subclass `wndproc` (`mod win`, ~L285-424). It does manual premultiply into a `from_raw_parts_mut` slice sized from caller-supplied `w*h` (L228), casts `lp.0 as *mut NCCALCSIZE_PARAMS` (L294), and installs an `extern "system"` callback. **A memory-safety bug here is silent corruption Rust does NOT catch and the 80 core tests can NEVER reach** (Win32, app-layer, coverage 0).

The irony the first draft missed: §3.3 correctly identified the Win32 edge as dangerous and even listed `cursors.rs:224/237` for their *panics* — but walked right past the fact that those same lines sit inside `unsafe` doing pointer arithmetic. The panic is the visible risk; the soundness is the invisible one.

**Recommendation (all cheap, all native-appropriate).** `#![deny(unsafe_op_in_unsafe_fn)]` at the crate root; a `// SAFETY:` comment on each block stating the invariant it relies on; narrow each `unsafe` scope to the FFI call only (not whole functions); run the FFI paths under `cargo miri` where the Win32 calls allow, or at least ASan on a debug build. This is squarely transferable discipline and protects the one place a bug is catastrophic and invisible.

---

### 3.9 Dependency / supply-chain hygiene — **[GENUINE GAP] · NEW in v2 · two-line CI win**

**Evidence.** The tracked `Cargo.lock` pulls **496 transitive crates** from a small direct set (wgpu, winit, `windows` 0.62, resvg/usvg, image, lopdf, i_overlay, pdf-writer). VAROS parses **untrusted external input** — PDF via lopdf, SVG via usvg — straight through these deps. There is no `cargo-audit` (RUSTSEC advisories) or `cargo-deny` (advisories + licenses + duplicate/yanked). Combined with the missing `LICENSE`, the 496 unvetted transitive licenses are also a compliance blind spot.

**Recommendation.** Add `cargo audit` (or `cargo deny check`) to CI — same leverage-per-effort as clippy, and the direct native-Rust equivalent of basarai's dependency posture. Start it non-blocking to triage the current advisory list, then make it a gate. Included in the §5.1 YAML.

---

### 3.10 `.vrs` schema evolution — **[GENUINE GAP, forward-looking] · NEW in v2 · protects users' work product**

**Evidence — credit first.** The code already has a genuinely thoughtful versioning design the first draft under-credited: `file.rs` has `VRS_VERSION: u32 = 1`, a header-first version check that refuses *newer* files with a clear message (`doc_from_blob`), atomic temp-and-rename writes so a crash can't half-write a file, and `sync_tree()` performing a real legacy upgrade on load. `vrs.rs` tests round-trip identity, newer-version refusal, and garbage rejection.

**The missing half.** This durability rests entirely on `#[serde(default)]` additive tolerance — which handles *added* fields but has **no story for renames, removals, or semantic changes.** The moment the model changes shape (`VRS_VERSION 1 → 2`), there is no migration function and no policy. For a native editor, **users' files are their work product** — forward/backward compatibility of `.vrs` is the single most important durability guarantee, more than any lint.

**Recommendation.** Two things, both cheap now and expensive later: (1) a **golden-file corpus** — real old `.vrs` files committed as fixtures that a test asserts must keep opening; (2) a written **migration policy** for when additive-serde stops being enough (a `migrate(from_version)` step in `sync_tree`). This is the native equivalent of DB migration discipline — genuinely transferable, not SaaS-only.

---

## 4. Where VAROS is already ahead (do not regress these)

- **Compiler-enforced pure-core seam.** basarai's layer separation is a convention; VAROS's is a *law* the build enforces. The stronger design.
- **80 headless core tests** — a genuine, meaningful suite on deterministic logic. Many professional projects have zero.
- **Small, concentrated production panic surface** (~33 unwraps + 2 expects) — much better than the naive count suggests.
- **Disciplined data-boundary error handling** (`file.rs`, `pdf`) with human messages — already at the level the recommendations ask for.
- **A real `.vrs` versioning design** (version gate + atomic writes + legacy upgrade) — ahead of most solo projects.
- **Documentation depth** dwarfs basarai's. **Conventional commits** already in place.

---

## 5. Prioritized action backlog

Ordered by (leverage ÷ effort). Items 1–3 are roughly one afternoon and remove the biggest risks.

| # | Action | Type | Effort | Payoff |
|---|---|---|---|---|
| 1 | **Add CI** (fmt + clippy + `cargo test` + build + `cargo audit`) — see §5.1 | GENUINE GAP | ~1–2 hr | Regressions + advisories caught automatically |
| 2 | `rustup component add clippy`; `cargo fmt --all` commit; one-time clippy cleanup (default lints only) | GENUINE GAP | 1–2 hrs | Free correctness punch-list across 10k LOC |
| 3 | Root `CLAUDE.md` + `LICENSE` + short `README` | MIXED | ~30 min | Every AI session starts correct; legal clarity |
| 4 | **Promote the panic hook** (`main.rs:244`) to fatal-dialog + crash-log, then de-panic the GPU/Win32 clusters (§3.3) | GENUINE GAP | ~1 day | App stops hard-crashing on other machines |
| 5 | **`unsafe`/FFI soundness pass** on `cursors.rs` (SAFETY comments, `deny(unsafe_op_in_unsafe_fn)`, narrow scopes, miri) | GENUINE GAP | ~half day | Guards the one catastrophic-and-invisible bug class |
| 6 | Split `editor.rs` (behind green tests) — and `ui.rs` deliberately as manual-verify or after adding app smoke tests | GENUINE GAP | ~1 day | Maintainability; unblocks future UI work |
| 7 | `.vrs` golden-file corpus + written migration policy | GENUINE GAP | ~half day | Protects users' work product across versions |
| 8 | Lightweight per-feature `specs/NNN/` with acceptance criteria (new features only) | GENUINE GAP (structure) | ongoing | Crisp "definition of done" per feature |
| 9 | `String` → `VarosError` enum | **CONDITIONAL** | ~half day+ | Do it *when* a call site must branch on error kind (e.g. newer-file → update vs corrupt → recover) |
| 10 | `Exporter`/`Importer` traits when export formats exceed two | GENUINE GAP (minor) | ~half day | New format = new file, not scattered edits |

*Lower-priority note:* VAROS's whole reason to exist is **instant feel** (`animation_time=0`, "برنامج شغل"). Correctness tests won't catch a silent frame-time/allocation regression in booleans or rendering. A small `criterion` benchmark on the hot paths is worth a one-liner in the roadmap — but it's genuinely lower-leverage for a solo author who feel-tests every build, so it sits below the backlog above.

### 5.1 Concrete first step — the CI file (corrected)

```yaml
# .github/workflows/ci.yml
name: ci
on:
  push: { branches: [main] }
  pull_request:
jobs:
  check:
    runs-on: windows-latest        # matches the real target (Win32/WebView2/wgpu)
    defaults:
      run:
        working-directory: varos    # Cargo workspace sits one level below git root
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: clippy, rustfmt }
      - uses: Swatinem/rust-cache@v2
        with: { workspaces: varos } # REQUIRED — manifest is not at repo root, or the cache no-ops
      - run: cargo fmt --all --check          # land a `cargo fmt --all` commit BEFORE enabling this
      - run: cargo clippy --workspace --all-targets -- -D warnings
        continue-on-error: true               # non-blocking for the FIRST cleanup cycle; remove after
      - run: cargo test --workspace           # safe while no test builds a Renderer/EventLoop (see §3.1)
      - run: cargo build --workspace
      - run: cargo install cargo-audit && cargo audit
        continue-on-error: true               # triage advisories first, then make it a gate
```

**Why the changes from the naive version:** (a) `working-directory` is hoisted to `defaults` so *every* step runs in `varos/`, not just some; (b) `rust-cache` gets `workspaces: varos` or it silently caches nothing; (c) `fmt --check` is honest about needing a formatting commit first, else it red-fails before clippy runs; (d) clippy and audit start `continue-on-error` for one cleanup cycle, then flip to gates; (e) `cargo test --workspace` is annotated with the headless invariant from §3.1.

---

## 6. What to explicitly NOT copy from basarai — **[CONTEXT]**

Correct *for a web SaaS*, wasted or wrong for a single-user native app:

- **JWKS / asymmetric JWT / auth** — no users, no sessions, no server.
- **Supabase Vault / encrypted secrets** — VAROS holds no third-party keys or user secrets.
- **Docker / tini / single-container deploy** — VAROS ships as a native `.exe`.
- **Supabase / Postgres** — VAROS persists to local `.vrs`/PDF files; no database belongs here.
- **Pydantic-style runtime validation of internal data** — Rust enforces this at compile time; only *external* input (file/PDF/SVG parsing, already handled) needs runtime validation.

**Reverse-check (the one thing §6 nearly missed):** basarai's *error observability* does have a native translation worth stealing — but it's the **panic-hook → dialog + crash-log** already promoted to backlog item #4, not any of the infra above.

---

## 7. Suggested discussion agenda for planning

1. **CI now, or wait?** (Recommendation: now — an afternoon, and it protects everything else. Note the headless-test invariant.)
2. **Error-handling policy:** ratify the rule "*GPU/Win32/external edges never panic; internal invariants may*," then schedule the §3.3 table + the panic-hook dialog (#4).
3. **`unsafe` soundness:** agree a SAFETY-comment + `deny(unsafe_op_in_unsafe_fn)` standard for `cursors.rs` (#5) — cheapest guard on the scariest code.
4. **God-file split:** treat `editor.rs` (safe, tested) and `ui.rs` (no net) as *separate* decisions, not one.
5. **`.vrs` durability:** golden-file corpus + migration policy before `VRS_VERSION` ever goes to 2 (#7).
6. **Spec structure:** lightweight per-feature folders for new features, or keep the single-roadmap model?
7. **Root `CLAUDE.md`:** codify the standing rules (no-animations, Illustrator-parity shortcuts, hand-painted UI, pure-core seam) so AI sessions start aligned — likely the cheapest high-value item.
8. **`VarosError` (conditional):** agree the *trigger* (first call site that branches on error kind) rather than doing it speculatively.

---

*This report is deliberately opinionated to be argued with. Every number in §2 was re-verified by an independent adversarial pass; challenge any recommendation that doesn't fit VAROS's reality.*
