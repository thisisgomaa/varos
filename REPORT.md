# Lane E — Phase 8 (feat/w2-view)
- Outline, Pixel Preview/snap, Navigator, presentation/Trim and canvas preferences: implemented (provisional UI, owner design review pending).
- API 1.2 view command/discovery + headless CLI retained; normal/export paint behavior preserved.
- No .vrs format/version/migration changes; settings key remains canvas_color [u8;3]; no borrowed code/dependencies.
## Fix round
- Accepted all four P2 findings; no disagreements with the reviewer.
- Pixel Preview samples resolved artwork before screen-resolution overlays; final blit/layer composite use separate normal sampling state.
- Outline now consumes existing cached, viewport-culled/clipped geometry; selection changes do not reflatten paths.
- Navigator invalidates on live document/camera changes and publishes each texture with its matching camera; idle remains Wait.
- Already-active canvas colour returns before scheduling settings persistence; changed colours still save.
- Four added headless regressions cover recorder ordering, cache/culling, live artwork/artboard proxies, and unchanged/changed saves.
- Renderer evidence is WGSL validation and recorder-order regression; native visual acceptance remains unverified.
- Image integration remains REQUIRED: this base has only Layer/Group/Path nodes; moderator must add image-box outlines with the image lane.
- Shared scene/recovery hooks delimited; UI shell/tokens/kit and ratchet thresholds untouched; ui.rs remains 830/843 lines.
- fmt, dependency directions, diff whitespace, native + Windows all-targets Clippy -D warnings: PASS.
- cargo test --offline --workspace -j 3 --no-fail-fast: PASS — 1898 passed, 0 failed, 15 ignored.
- Shell ratchets: 3 passed; Bridge ratchets/frozen 1.0/1.1 fixtures: 6 passed; no fixture edits.
- Evidence: /tmp/w2-fix-*.log; initial test-fixture mistakes corrected before final workspace rerun.
- No git writes/commit/push/merge, GUI launch, install, or new independent review; moderator/owner acceptance pending.
