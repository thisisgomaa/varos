# VPANEL.S2 — Native panel workspace review

Status: implementation prepared 2026-11-01 after owner acceptance of the two-panel slice and authorization to extend the same system. Automated UI verification is complete, including the final Control fitting and compact color-chip corrections. Candidate build/signature/startup evidence is recorded separately in `dist/panel-workspace-2026-11-01/verification.json` and `manifest.json`. Owner acceptance of this candidate remains pending; automated checks do not certify native acceptance.

## Scope

The shared panel system now registers all 45 existing fixed panel identities, including Tools and Control, and recognizes supported library panels and additional Properties/Control instances. It reuses the existing working panel contents and engine commands. Registry coverage means that those contents can enter the shared hosts, not that every operation in every panel has been newly implemented or individually accepted.

The approved full, icon-with-name, and icon-only presentations remain the basis for ordinary panel groups. Tools and Control have explicit orientation, grouping, and docking policies rather than unrestricted ordinary-panel placement. The latest owner correction makes the tab band a continuous dark surface with an integrated active tab, removes grip marks, and uses the ordinary arrow cursor while dragging. Tabs are now 28 logical points; title 20, control 18, and panel-icon 24 are unchanged. Native UI scaling applies to the panel and toolbar metrics together. Ordinary panel tabs can move between groups, groups can stack or sit beside each other without merging their tabs, and a combined host can move as one block. Invalid bar placements are rejected transactionally from commands and never offered as drop indicators; releasing at an incompatible edge leaves the dragged bar floating. Hosts dock at the workspace edges or float within the workspace. Shared dimensions, tab overflow, split dividers, width/height resizing, and compact-column scrolling are handled by the same ownership and geometry model.

Docked hosts meet the workspace edge with square corners and no X or floating footer. Their dark header remains draggable. Docked Tools/Control omit the redundant name-tab row. A floating bar retains its movable host chrome. Tools remain a separate group when attached beside ordinary groups; Control remains a separate host. Top/bottom bar allocation removes that unused tab/footer height while preserving the remembered floating dimensions. Docked resize handles face the canvas. Docked Control fits the measured intrinsic height of its current wrapped rows, so it has no vertical resize handle or unused remembered-height strip. The measurement is runtime-only and does not dirty named workspaces; the remembered floating size is preserved.

On floating hosts, X hides the whole host without discarding its groups. Window commands restore it. Collapsing does not move a floating host to the old right-hand strip. Transient drag state is separate from saved layout state; Escape and focus loss cancel a layout gesture.

## Existing contents hosted by the system

- **Tools:** the existing tool buttons, flyout choices, color wells, color/gradient/none controls, drawing modes, and Edit Toolbar entry. Tools always use one or two vertical columns, including floating and wide hosts; they dock only at the left or right. They may sit beside ordinary panel groups as a separate column, but cannot merge into their tabs or stack above/below them. The latest owner correction replaces the hosted circles with a compact overlapping square pair: solid Fill at the upper left, a Stroke frame with a panel-colored center at the lower right. The pair is capped at 60 logical points with side insets in narrow hosts; it no longer expands across the full host. The focused well stays in front, and the existing picker, swap, and default-color commands retain their behavior. The Color/Gradient/None chips remain in one compact row beneath it (16-point painted squares inside up to 20-point hit boxes). Short hosts scroll to the remaining controls. The host supplies the shared title bar and dragging, while existing tool-group flyouts retain their own tool behavior. Popovers use the current host geometry.
- **Control:** the existing context-sensitive horizontal controls wrap within the available width and scroll in a short host. Control is a standalone bar, docks only above or below the canvas, and cannot merge or split into ordinary panel groups. Existing fields, commands, and numeric scrubbing remain in use.
- **Properties:** the visible sections follow a fixed order for the current selection/tool context. Section collapsing remains available; section reorder grips are absent in the new workspace. Legacy custom ordering data is preserved for the old shell.
- **Color:** the existing wheel, sliders, harmony, gradient, and classic-body path are retained. The Color host has a 256-point minimum width and a larger default height; an outer vertical scroll viewport makes taller modes reachable. The existing nonmodal color palette remains a separate palette.
- **Other panels:** their existing native bodies and panel menus are hosted by the same tabs, grouping, compact modes, and Window routing. Library/editor instances keep distinct identities.

## Architecture and persistence

`panel_system/registry.rs` defines supported identities, original/licensed icons, and minimum/default sizes. `metrics.rs` centralizes the approved panel chrome and existing toolbar dimensions. `model.rs` validates transactional ownership, including hidden hosts, edge docking, groups, and split ratios. `geometry.rs` supplies the rectangles used for painting and hit targets. `content.rs` adapts Tools and Control without creating a second host-drag system. `mod.rs` coordinates gestures, menus, flyouts, and rendering.

The new state and named workspace store are serialized through the existing UI preference storage. Saved workspaces support switching, reset, creation, rename, and deletion through the existing Window commands. Layout validation and lenient loading reject malformed panel layouts without discarding unrelated preferences. Runtime drag state, popup state, hit rectangles, and compact scroll offsets are not persisted. The built-in workspace is Design; it starts with the accepted Layers/Swatches pair, Tools docked left, and Control docked above. Other fixed panels have hidden owners until opened.

## Isolation and known limits

This candidate uses a new, separate configuration directory. It does **not** copy or migrate the user's working Varos configuration or the first preview's configuration. Its saved arrangements belong to this preview. Existing production apps and the first preview are not replaced.

Panels float inside the application workspace; they are not separate operating-system windows. The registry does not provide new implementations of the underlying editing features. Existing modal document/tool dialogs remain modal. Named workspaces are limited to 64 and panel instances to 128 by the model's validation.

The working tree includes earlier picker clipping and shortcut/undo changes as well as the accepted two-panel foundation. Those edits remain preserved and are included in the packaging source patch. They are not presented as new fixes or newly accepted behavior in this extension. The scoped upstream Swatches range-anchor reset from [a0da3faff](https://github.com/storytold/vectorcraft/commit/a0da3faffebafc0772b5a1902d66fd55b60501e6) remains included; this extension does not perform a broad craft-ui/egui dependency upgrade. Live HEAD/main refs were rechecked: VectorCraft `05d5f99fd7e7dc00db3a9754d594b1214945d855` and craft-libs `f8f8829cc7be65d466d687dae381d1ca60e31242` are unchanged from the preceding review.

## Candidate identity

- Bundle: `dist/panel-workspace-2026-11-01/Varos Panels Preview 2.app`
- Bundle ID: `com.varos.panels-preview2`
- Product and embedded binary version: `0.20.0-panel-workspace.2`
- `CFBundleShortVersionString`: `0.20.0`
- `CFBundleVersion`: `20261101.2`
- Configuration: `~/Library/Application Support/Varos Panels Preview 2`
- Agent endpoint directory: `~/Library/Application Support/Varos Panels Preview 2/agents`
- Startup opt-in: `VAROS_PANEL_REVIEW=1`

The helper in `dist/panel-workspace-2026-11-01/package.py` uses `target/release/vectorcraft` and `vectorcraft-cli`, requires the same embedded product/build identity in both, checks relevant source freshness per binary, copies the original application icon, signs ad hoc, and verifies the bundle signature. It registers no document types, UTIs, or URL schemes. It refuses to replace existing candidate artifacts and does not launch the binaries.

`source.patch` and `manifest.json` record the Git base, dirty-state source patch/hash, captured source file hashes, and binary hashes before and after signing. The capture includes this review document and the panel-system source files, excludes unrelated untracked logs, and does not mistake documentation edits for compiled inputs. These records describe the packaging-time working tree; they are not a reproducible-build attestation.

## Verification status

- Full UI library suite on the final source: **2673 passed, 0 failed, 21 intentionally ignored**. This includes 68 panel-system tests covering model/registry/placement policies, saved workspaces, raw-pointer transitions, docking, tabs, independent Tools detachment, resize geometry, compact scrolling, content adapters, intrinsic Control fitting across viewport widths, and the compact paint-chip row.
- Software capture test: passed; both initial workspace and merged Layers/Swatches tab views were rendered from the app code and inspected. These are headless screenshots, not owner acceptance.
- `cargo fmt --all -- --check`, `git diff --check`, scoped UI/app all-target clippy with `-D warnings`, `cargo xtask layers`, `cargo xtask assets`, and `cargo xtask wasm`: passed.
- An earlier parallel run briefly exceeded a legacy card-drag timing threshold while a release compile ran. The isolated timing recheck passed (0.484/0.525 ms medians), and the final full suite passed with no simultaneous build. No threshold was weakened.
- Matching native/CLI release build, ad-hoc signature, exact startup identity, and native screenshot: see the candidate verification record for their final status.
- Owner review of native appearance, movement, grouping, and editing behavior: **pending**. No full-workspace CI or commit was made for this isolated preview candidate.

The earlier two-panel slice was accepted by the owner. That acceptance does not automatically cover the expanded workspace.

## Owner review sequence

Review one behavior at a time and stop for feedback before moving on:

1. Move Tools from its dark header, float it, resize it, and return it to the left or right edge; also try joining it beside an ordinary group and detaching it again. Check that tool selection and the color wells remain usable.
2. Move Control, return it above or below the canvas, and try its existing controls with and without a selection.
3. Open Properties and Color from Window; check their contents, scrolling, and canvas interaction.
4. Try another panel with tab merge/detach, stacked or adjacent groups, compact edge resizing, and X/Window restore.
5. Finally review workspace save/switch/reset and reopening this isolated preview.

Do not treat the candidate as a replacement for the user's working app or migrate the working configuration without explicit owner acceptance.
