# Wheel palette eyedropper — 2026-11-04

Status: provisional, awaits owner. Local uncommitted change on `04af06a50`, following the
owner-accepted panel sizing build 20261104.1.

The wheel header's Eyedropper action unconditionally closed the popover. It now finishes the
current gesture and any stored gesture, then seals only the picker-owned undo group while
preserving the shell palette's anchor. Selecting the tool and sampling the canvas leave the
palette visible in the same place; explicit Close/Escape still dismiss it. The classic modal
picker retains its dismissal because its backdrop otherwise blocks canvas sampling.

All implementation and regression additions are in `varos/picker/mod.rs` and `picker/tests.rs`;
no new upstream hook or asset. Independent read-only review found no blocking issue.

The focused picker suite passed: 48 tests, 0 failed, 1 ignored. The new real-frame test clicks
the header pipette and samples a second rectangle through the actual canvas in the classic
presenter, box shell and panel workspace. It checks palette visibility/position, applied color,
selection preservation, explicit Escape and independent undo for the sample and previous edits.
Additional tests cover an externally owned undo group and a pending local gesture at activation.

Local gates passed:
- `cargo test --locked -j 4 -p vectorcraft-ui-egui -- --test-threads=4`: 2,784 unit tests and
  2 integration tests passed, 0 failed, 25 ignored.
- `cargo clippy --locked -j 4 --workspace --all-targets -- -D warnings`.
- `CARGO_BUILD_JOBS=4 cargo xtask wasm`.
- `cargo fmt --all -- --check` and `git diff --check`.

`VAROS_PRODUCT_VERSION=0.20.2-panels.5 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=4 cargo build
--locked -j 4 --release -p vectorcraft` passed in 4m 06s. Build **20261104.2** was installed after
the owner saved and closed the app. All five installed files match the staged manifest and
`codesign --verify --deep --strict` passed. Build 20261104.1 is retained in
`dist/eyedropper-palette-2026-11-04/rollback/Varos.app`; settings were retained and no app was
launched by the agent. The existing CLI is unchanged from the preceding verified build.

Source identity: `04af06a5080f68ced1d80e29efe515adc1d7363e` plus the uncommitted Rust patch
SHA-256 `6561b762826a14d76ad0f865a93427a83877dd01d13b5a9e2c9ca1ad35c53d13`.
Bundle manifest and staged patch: `dist/eyedropper-palette-2026-11-04/` (gitignored).
Logs are in `plan/eyedropper-palette-2026-11-04/gates/` (gitignored). These are local checks,
not GitHub CI.

Owner hand test: open the wheel palette, activate its header eyedropper, sample a color on the
artwork, and confirm the palette stays visible in place and updates its color; close it manually.
