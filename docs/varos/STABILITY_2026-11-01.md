# VSTAB.1 — panel interaction stabilization

Status: all 12 code gates PASS, 2026-11-01. Native release acceptance remains blocked.

## Scope and baseline

Owner request: update the current Varos build, test and repair panels, dragging, resizing and the
colour picker, preserving the approved design and adding no new features. Review current upstream
fixes before delivery.

- Starting main: `ec3188b973e78dd77c61aac7a7ce7e11aa8ce52d`.
- Existing hand-test bundle: `0.17.0`, build `018ee491`, bundle ID `com.varos.app`, at
  `~/Documents/AI workspace/reference/artcraft/spikes/Varos.app`.
- `/Applications/Varos.app` is a separate application (`com.varos.editor`); do not replace it.
- Working branch: `codex/stabilize-panels-2026-11-01`.
- Preserve the palette, icons, dimensions and approved floating/docking design. No upstream
  redesign or new add-ons are included in this stabilization scope.

## Upstream review

Fetched upstream/main on 2026-11-01:
[`1c4a7831ebdd9746b0f253413fe4f1fefc543bfc`](https://github.com/storytold/vectorcraft/commit/1c4a7831ebdd9746b0f253413fe4f1fefc543bfc).
1,441 commits are not ancestors of the starting Varos HEAD (including merge commits).
This is a targeted backport, not a full upstream merge: upstream includes substantial file moves,
new add-ons, new features and chrome changes outside the owner's stabilization scope.

Implemented targeted backports (code gates passed; native acceptance tracked below):

| Upstream commit | Behavior | Application to Varos |
|---|---|---|
| [44ef4ab4b](https://github.com/storytold/vectorcraft/commit/44ef4ab4bc163542fc956745c52c3f3eb59bff13) | Pixel-aligned canvas pointer mapping under fractional dock sizes | Shared canvas, including the Varos shell |
| [37211078c](https://github.com/storytold/vectorcraft/commit/37211078c2bf08592cbe075f0e1b1e32adb1eb11) | Saved accent and canvas colour survive the first frame | Reapply saved preferences without changing palette defaults |
| [437280315](https://github.com/storytold/vectorcraft/commit/437280315f9ba7e9732faa0d79213e8e736d71b7) | Enter/Escape finishing a floating-window field do not also trigger main-window shortcuts | Legacy native floating panels |
| [18709b0b6](https://github.com/storytold/vectorcraft/commit/18709b0b63a365658a0d1ed09b88648a84e5748c) | Reject multiple leading hashes in a hex colour field | Both classic and Varos picker parsers |

The dropdown rewrite `13d837363` also updates craft-libs and moves font-menu internals. It is not
being claimed as integrated. The freeform gradient swatch fix `14c461620` is deferred: the Varos popover has its own interaction path; this pass does not import upstream panel changes without a Varos reproduction.

## Verification matrix

| Area | Required observation | Result |
|---|---|---|
| Upstream | Current main fetched and relevant fixes reviewed | PASS: snapshot above |
| Divider resize | Live sizing, minima, collapse, restore, Escape, pointer loss, focus loss | Headless regression PASS; native pending |
| Tools columns | Edge drag 1/2/3 columns; cancel keeps original columns | Headless regression PASS; native pending |
| Panel ownership | Dock, float, icon, tab reorder, close/reopen; no duplicated or missing panels | Existing headless shell suite PASS; native pending |
| Picker | Fill/stroke, solid/gradient, OK, Cancel, outside click, Escape, narrow viewport, undo | Existing release UI tests PASS; native pending |
| Persistence | Layout survives serialization/relaunch | Serialization tests PASS; actual relaunch pending |
| Native macOS | Real window pointer interactions and screenshots on the exact delivered bundle | BLOCKED: Computer Use reports "Computer Use was not approved to use Varos" even after permission was reported enabled; no native PASS claimed |
| Build identity | Installed bundle matches tested source and executable | Build/install receipts below record the source, hashes and installation; not a native interaction PASS |

Tests and software-rendered captures do not establish native mouse or installed-bundle acceptance.
Keep those results distinct.

## Reproduced defects and checks

Before production changes, the shell suite reported **106 passed, 3 failed, 8 ignored**. The three
new tests reproduced cancelled divider resizing committing its preview, cancelled tools-column
resizing changing the column count, and cancelled collapsed-column restoration reopening it.
Escape on the release frame, pointer loss and focus loss are covered for all three gestures in the final test source.

After the fix, the release UI suite reported **2,594 passed, 1 failed, 18 ignored**. All three
reproductions passed. The remaining failure was in a newly backported preference test: upstream
expects accent-coloured links, whereas the existing Varos theme deliberately uses neutral text.
The assertion now verifies the existing Varos colour; no production palette change was made.
The final full CI run passed that adaptation and the final resize guard.

The release build used above also produced after-captures for the standard shell, three-column
toolbar and open picker. Compared with the pre-fix executable, each image is pixel-identical
except for the live render-time readout at `(1261, 886)–(1284, 895)`. The picker capture was
visually inspected. These are software captures, not screenshots of the installed macOS app.

Evidence files: `/tmp/varos-stability-2026-11-01/` (`baseline.txt`, `ui-release.log`, `ci.log`,
`before-shell.png`, `before-columns.png`, `before-picker.png`). The pre-change test executable
was preserved there as `ui-before` for independent reproduction.

The first full CI run passed formatting and strict clippy, then a loopback handshake test was
blocked by the execution sandbox (`Operation not permitted`). The authorized unrestricted rerun
passed that handshake, then exposed a stale app-bin test fixture: the legacy docking recovery
test was exercising Varos shell float routing, so no legacy `docking_hidden` location existed.
The fixture now explicitly selects the legacy shell before constructing its legacy saved data;
its corruption/recovery assertions remain unchanged. Production defaults remain unchanged.

The next full run reached the UI suite after **8,351** preceding passing tests (see the log for
exact suite totals). UI results were **2,590 passed, 5 failed, 18 ignored**. All five failures were
debug-only `TexturesDelta` drop assertions in existing headless tests (clipping, picker Escape,
two tab-scroll checks and board capture targeting). Their output now explicitly clears unused
texture uploads, following egui's documented contract; behavioral assertions are unchanged.
The final UI suite now passes: **2,595 passed, 0 failed, 18 intentionally ignored capture/manual
checks**. All workspace unit/integration tests and doctests passed in this rerun (11,009 passing
checks at that point); all remaining CI steps subsequently passed.

Final-source debug captures were also generated (`final-shell.png`, `final-columns.png`,
`final-picker.png`). All three match the pre-change release image pixel-for-pixel above the
status strip; only live render/UI timing digits differ (maximum bounds `(1261,886)–(1354,895)`).
The three-column capture was visually inspected. See `capture-comparison.txt`.

The no-bundled-fonts tests/lints and shared PDF dependency check also passed. The asset gate then
found 29 existing files without attribution rows. Source SVG headers, capture tests, the approved
HTML handoff and introducing commits established provenance for 28 unchanged files; ASSETS.md
and NOTICE now record them. The remaining owner-supplied sketch's redistribution provenance was
not established: it was preserved byte-for-byte by moving it to gitignored
`plan/local-reference/shell-sketch-2026-10-27.png`, and its documentation pointer was updated.
It was not inspected or used to derive behavior in this work. No shipped artwork changed.
`cargo xtask assets` now passes for all 537 distributed asset files. The subsequent full CI run passed all 12 steps.

## Final gate result and release receipts

`CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true RUST_TEST_THREADS=2 cargo xtask ci`
completed successfully outside the execution sandbox (loopback tests require it): **all 12 steps
passed** — fmt, strict clippy, tests, doctests, no-bundled-fonts, PDF dependencies, assets, brands,
cursors, translation report, layers and wasm. Across 94 test-result summaries: **11,013 passed,
0 failed, 33 ignored**; the unchanged ignored cases are manual/capture/known limitations and are
not counted as passed. The UI suite alone is **2,595 passed, 0 failed, 18 ignored**.
Final log: `/tmp/varos-stability-2026-11-01/ci-unrestricted.log`.

The code commit is the build identity for the **0.18.0 stabilization candidate** (engine 0.17.0).
After this commit, build with `VAROS_PRODUCT_VERSION=0.18.0 cargo build --offline --locked
--release -j 2 -p vectorcraft -p vectorcraft-cli`. Packaging verifies both executables' identity,
signs the bundle ad hoc, verifies its signature and records every bundled file's SHA-256 in
`dist/stability-2026-11-01/manifest.json`. Installation verifies those hashes, refuses the wrong
bundle ID or a running fork, retains the previous fork as a timestamped backup, and verifies that
`/Applications/Varos.app` is unchanged. The generated receipt is
`dist/stability-2026-11-01/installed.json` (also `/tmp/varos-stability-2026-11-01/installed.json`).
Until those receipts exist, packaging/installation are not claimed by this source document.
They record completion separately from native acceptance and avoid changing the commit that the
binary identifies after it has been built.

## Native acceptance still required

Use only the fork bundle (`com.varos.app`), a new disposable document and a temporary save path.
Record observed behavior per step, not just whether a click was sent:

1. Float a tab, move the floating box, resize it, dock it again, and reorder its tabs. Check that
   each panel appears exactly once and the canvas remains interactive.
2. Resize horizontal and vertical seams; cancel a live resize with Escape; move focus away
   during a drag. Check original dimensions are restored, then repeat a completed resize.
3. Collapse and restore the right column; cancel restoration while pulling its strip edge.
   Resize the tools strip to one, two and three columns, including an interrupted drag.
4. Open Fill and Stroke pickers; enter valid and malformed hex values; drag colour/opacity;
   verify OK, Cancel, Escape and the existing outside-click behavior. Confirm one Undo for a
   committed edit and no document change after Cancel.
5. Switch solid/gradient, edit a stop, cancel, then reopen. Verify no extra picker, blocked
   canvas input or unrequested selection/tool change.
6. Reduce and restore the window size; minimize and restore the macOS window. Check panel
   and picker visibility and correct pointer targeting near edges.
7. Save the test document and layout, quit normally, relaunch, reopen the test document and
   verify the layout and document contents. Do not overwrite an owner document.

All seven remain unverified on the newly delivered native binary until GUI access succeeds.

## Revision history

- 2026-11-01: pinned upstream backports, reproduced and repaired gesture cancellation, passed all 12 gates, recorded unchanged visual captures and the native-control blocker.
