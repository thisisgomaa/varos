# Panel footer and content sizing — 2026-11-04

Status: **provisional (awaits owner)**. Local working-tree patch on `04af06a50`;
no commit, push or publication. The owner reported Color spacing and the common panel footer/
resize behavior. This changes the shared presenter, with small marked hooks in existing bodies.

## Result

- Every full group owns a bottom strip; its body ends at that strip. Stacked groups reuse the
  strip as the split handle, so painting and hit testing agree.
- The active body supplies a width-specific intrinsic height. Fixed controls fit naturally;
  only explicit flexible regions receive surplus height: Gradient's ramp, Color's spectrum,
  Navigator's preview, and list viewports. Minimums preserve controls and a usable viewport.
- Empty lists remain resizable. Bottom actions retain their size and stay below the viewport.
- Active-tab minima replace all-tab minima. Split minima add the children instead of preserving
  an obsolete ratio. Floating and re-expanded columns project a wider active tab's minimum.
- Fixed/flexible stacks, compact flyouts and collapse/expand retain remembered useful sizes.
  Content drags freeze sizing; width drags refresh wrapping without collapsing the preview.
- Measurements are transient. Rendering does not rewrite saved extents, ratios or workspaces.
- Color has an eight-point inset, top-aligned channels and compact warning placeholders. Empty
  Recent Colors consumes no space; populated recents follow the spectrum. The spectrum stays
  stationary while dragging; after release it can give space to newly recorded colors.

## Local evidence

| Command / check | Result |
|---|---|
| `cargo test --locked -j 4 --workspace -- --test-threads=4` | 11,194 passed, 0 failed, 40 ignored; before the final additional re-expanded active-tab-width regression |
| `cargo test --locked -j 4 -p vectorcraft-ui-egui -- --test-threads=4` | Final behavior: 2,781 unit + 2 integration passed; 25 ignored. The only later source cleanup removed an unused superseded width helper; clippy checks the resulting test targets. |
| `cargo clippy --locked -j 4 --workspace --all-targets -- -D warnings` | Passed |
| `CARGO_BUILD_JOBS=4 cargo xtask wasm` | Passed |
| `cargo fmt --all -- --check`; `git diff --check` | Passed |
| Two opt-in software-capture tests | Passed; 13 crops inspected |
| `VAROS_PRODUCT_VERSION=0.20.2-panels.5 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 cargo build --locked -j 4 --release -p vectorcraft -p vectorcraft-cli` | Passed, 7m 22s |

`panel_system/sizing_tests.rs` covers all standard registry panels for stable finite geometry,
plus actual resize gestures, minimums, fixed/flexible stacks in both orders, bottom docking,
held-width ramp geometry, active tab widths, compact flyouts, collapse/expand and unchanged
serialized layouts. `color_flyout_tests.rs` covers slider modes, narrow widths, recents and
pointer stability. Two independent read-only source reviews found the edge cases above; the
final targeted pass reported no additional blocking finding.

Local logs and crops: `plan/panel-sizing-2026-11-04/` (gitignored). Captures show Gradient,
Swatches and Color at two sizes, fixed Align requested tall, and grouped Color in RGB empty/
recent, CMYK narrow, HSB, Grayscale and Hide Options. They are synthetic headless frames,
not screenshots of a native window or evidence of owner acceptance.

## Acceptance still required

In the real window, resize Color/Gradient and list panels down to their minimum and back up;
check that only the preview/list grows, the footer stays at the body end, fixed panels stop at
content, and mixed stacks and tab switches behave naturally. Existing workspaces must retain
their arrangement. The owner decides whether the appearance and feel are accepted.

## Local build handoff

Installed build **20261104.1** into the existing working Varos bundle after verifying it was
closed. The app was not launched or controlled. The previous build **20261102.20** is retained
under `dist/panel-sizing-2026-11-04/rollback/Varos.app`; settings were not touched.

`dist/panel-sizing-2026-11-04/manifest.json`, `verification.json` and `post-install-check.json`
record the source HEAD, exact dirty-source patch SHA-256, bundle identity, installed checksums
and rollback path. All five bundle files match the manifest; deep strict code-signature
verification passed. The packaged CLI's `--version` completed (`vectorcraft-cli 0.17.0`, the
retained engine version). This is a local trial build, not a published release.

## Owner feedback

2026-11-04: the owner confirmed the panel layout/sizing result was good. This scope is accepted.
A separate follow-up concerns the wheel picker's header eyedropper dismissing its palette;
that behavior was not changed by the panel-sizing patch.
