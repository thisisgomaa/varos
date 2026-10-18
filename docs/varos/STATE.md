# Varos fork: state

The one truth table for what exists today. Updated: 2026-10-19, at `main` = `753ca856`
(upstream `cdba0aac` + Varos lanes). If anything else disagrees with this file, this file wins
until it is corrected.

Status values:
- **done**: built, gates green, nothing pending from the owner for this scope.
- **provisional (awaits owner)**: built and gates green, but the owner has not accepted it in the
  real window yet, or said it is not right yet.
- **known-broken**: exists, but the owner or a test showed it wrong.
- **not started**.

| Item | Status | What is true | Evidence |
|---|---|---|---|
| Name / branding | done | Window, menus, About, Home, pickers, messages and packaging show "Varos"; bundle id `com.varos.app`; data folder `Application Support/Varos` (no automatic migration from `VectorCraft`). Owner: name OK. | `lanes/brand-cursors-perf-report.txt` (lane brand: UI suite 1665 passed); `crates/ui-egui/src/varos/names.rs` |
| No Discord / community promotion | done | Community module, header button, Home/About/Help promotion rows and the Discord command/URL removed; contributor attribution kept. | `lanes/brand-cursors-perf-report.txt` (lane brand) |
| Real app icon | provisional (awaits owner) | All 15 icon files are the original Varos icon, byte-identical to `/Applications/Varos.app/Contents/Resources/Varos.icns`. Dock/Finder appearance in an installed bundle not checked. | `lanes/brand2-report.txt` (SHA-256 check, 1682 passed / 2 ignored) |
| Home without menus | provisional (awaits owner) | No in-window menu bar or app-bar menus on Home. On macOS the native "Varos" app menu remains, with New/Open moved into it. Owner (2026-10-19): still not right, wants no menus at all. | `lanes/brand2-report.txt`; `BACKLOG.md` item 1 |
| Theme / fonts / radii | provisional (awaits owner) | Varos warm-dark tokens (`#141313` base), Inter / JetBrains Mono / IBM Plex Sans Arabic, 3 px controls / 8 px boxes, no shadows. | `crates/ui-egui/src/varos/theme.rs` and its tests; stage-0 lane A report is not in `lanes/` (see lanes/gates-2026-10-19.log beyond the code) |
| Icons | done (117) / not started (rest) | 117 Varos overrides (92 exact, 25 close); unmatched names keep upstream art. Remaining upstream: 77 rows that need a Lucide import, 69 that need drawing. | `lanes/icons-report.txt`, `lanes/icons-overrides.txt`, `icon-map.tsv` |
| Cursors | provisional (awaits owner) | Varos v1.1 set (38 SVGs) as real NSCursor objects on macOS, 1x/2x. States without a Varos cursor (Type I-beam, plain crosshair, ...) use the macOS system cursor; no painted glyph on macOS. | `lanes/brand-cursors-perf-report.txt` (lane cursors: 1668 passed); commit `ce3b1dae` |
| Pointer lag: painted glyph | done (owner 2026-10-19: «أسرع كتير فعلاً») | The painted cursor glyph that trailed the pointer by a frame or more is gone on macOS (system or native cursor instead). Owner confirmed the speed-up in the bundle built from `ce3b1dae`. | commit `ce3b1dae`; tests `macos_states_without_a_varos_cursor_take_the_system_cursor`, `system_fallback_builds_never_paint_a_glyph` |
| Pointer lag: A2 (UI-frame trailing) | not started | Hover overlays and readouts drawn by the UI frame may still trail the pointer. Never measured in a real window. | `lanes/brand-cursors-perf-report.txt` (lane perf: "physical desktop latency ... requires the owner's window") |
| Perf profiler | done | Opt-in `VAROS_PROFILE=1` prints the top 10 UI sections once a second; off = one flag check. Shortcut table is skipped on frames with no key press (headless p50 2.146 -> 0.390 ms). Render thread cap raised from `min(4)` to `min(8)` (stage 0). | `lanes/brand-cursors-perf-report.txt` (lane perf); `crates/ui-egui/src/varos/profile.rs`; `crates/render/src/lib.rs:2244` |
| Kashida: OpenType Arabic fonts | done (owner: too subtle) | Justified right-to-left Arabic area text (Type tool paragraphs included) gets tatweels, set in the paragraph's own face only. Works with OpenType Arabic fonts (SF Arabic, Noto Naskh/Sans Arabic, Arial, Tahoma, Times New Roman); fixed Balanced strength. Owner (2026-10-19): too subtle. | `crates/text/src/kashida.rs` header; `crates/text-arabic/README.md`; 14 tests in `crates/text/src/tests_kashida.rs`; commits `fae2b0b2`, `b24f70ce`, `ce3b1dae`. `lanes/arabic-report.txt` is the stage-0 spike (it used a Noto fallback; the current code never uses another font) |
| Kashida: any font + controls | not started | AAT fonts (Geeza Pro, Al Nile, Baghdad, Damascus, KufiStandardGK) get word spacing only. No on/off buttons or strength slider. | `crates/text-arabic/README.md`; `BACKLOG.md` item 2 |
| `varos-text` dependency | done | Pinned git rev `5936eaf` over SSH (`github.com/thisisgomaa/varos`), `default-features = false` (policy only, no second shaper). The comment in `crates/text-arabic/Cargo.toml` still says "path to the Varos worktree" and is stale. | `crates/text-arabic/Cargo.toml` |
| Gates | done (last run) | 1684 UI tests and 300 text tests green at the moderator's last run. | moderator's run, no log in the repo (unverified here); latest lane log: `lanes/brand2-report.txt` (1682 passed, 2 ignored) |

## Build, run, hand-test (macOS)

1. Build (one build at a time; the owner's Mac overheats): `cargo build -j 4 --release -p vectorcraft`.
2. Quick run without a bundle: `target/release/vectorcraft` (prefix `VAROS_PROFILE=1` for timings in the terminal).
3. Bundle: create `Varos.app/Contents/{MacOS,Resources}`, copy `target/release/vectorcraft` to `Contents/MacOS/VectorCraft`.
4. Copy `assets/app-icon/vectorcraft.icns` to `Contents/Resources/VectorCraft.icns` (must match `CFBundleIconFile` = `VectorCraft`, not `Varos.icns`).
5. `Contents/Info.plist` from `packaging/macos/Info.plist.in` (fill `@VERSION@`, `@SHORT_VERSION@`, `@BUILD_SHA@`), then ad-hoc sign: `codesign --force -s - Varos.app`. The full signed release path is `packaging/macos/package.sh`.
6. Hand test: the owner opens the bundle and follows the task card's hand-test steps; anything that looks or feels wrong goes into `BACKLOG.md` in his words.
