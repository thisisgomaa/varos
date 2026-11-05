# Known issues — Varos 0.1 Preview

This is a pre-alpha. Use copies of important files and save often. Confirmed limitations and
areas still awaiting native validation are distinguished below. Report new problems through
[GitHub Issues](https://github.com/thisisgomaa/varos/issues/new/choose); English and Arabic are welcome.

## Text & Arabic

- **Kashida with fully vowelled Geeza Pro:** elongation may be absent even with kashida enabled.
  Try another Arabic font; check the result visually. Other kashida/font-menu behavior remains
  provisional. There is no confirmed fix for this font/harakat combination.
- **Mixed text selections:** Direction shows the first paragraph rather than a mixed state;
  harakat colour swatches do not show the current state. Inspect and edit one paragraph at a time.
  Custom harakat colours and vertical adjustment are not available in the UI; use the four
  supplied swatches for colour.
- **Language and UI glyph coverage:** newer Arabic/type labels in the translation catalogs have
  not had native-speaker review; the medium UI font fallback still awaits a native glyph check.
  If labels are unclear or glyphs are missing, try the English interface and report the label.

## PDF

- **Mixed Arabic/Latin re-import:** a PDF without preserved editing data can reorder text when
  reopened in Varos. Keep a `.varos` master and retain the default PDF editing data when exporting
  for further editing in Varos. Plain PDF import has no reliable workaround for text order.
- **Vowelled Arabic re-import:** without editing data, Arabic with harakat can return as duplicated
  letters/marks split into fragments. This affects both ordinary and coloured harakat. Keep the
  native master and preserve PDF editing data; inspect text after importing external PDFs.
- **External copy/paste is not fully verified:** ligature and stroked Force-kashida regressions
  pass locally, but external PDF-reader acceptance is pending. Check copied text in your intended
  reader; use the native text as the source if extraction differs.

## Interface

- **“Soon” controls do nothing:** AI chat/images/history, sharing, Dieline, Extension Library,
  3D Box Preview, Comments, Preflight, Print rules, press conditions and marked templates/link
  actions are placeholders. There is no workaround or promised date. Existing Crop Image and
  Getting started actions remain available when applicable; the MCP bridge is separate.
- **Native menu presentation:** macOS menus show “— Soon” as text instead of a badge. The disabled
  Dieline submenu cannot expand there; its explanatory submenu is available in the in-window menu.
  Neither presentation enables the feature.
- **Document tabs cannot detach into separate windows:** dragging outside the strip cancels the
  reorder. Keep documents in tabs and drag horizontally within the strip.
- **Workspace interactions still need native validation:** Home, panel docking/floating, modeless
  Preferences and the picker eyedropper have provisional coverage. Sizing and the shared visual
  foundation are accepted, but this does not certify every gesture. If a panel is hidden, reopen
  it from Window; report repeatable layout, focus or picker problems with steps.
- **Large artwork and pointer feedback:** many visible paths can slow the CPU renderer; hover
  overlays/readouts may trail the pointer (native latency is not measured). Try hiding unneeded
  layers or testing a smaller copy; there is no confirmed general performance fix.

## Files

- **Recovery is periodic, not continuous saving:** automatic recovery copies and crash recovery
  exist. Defaults are enabled, every two minutes, under
  `~/Library/Application Support/Varos/Data Recovery/`. The timer and Restore flow have headless
  tests; this documentation review does not certify recovery after a real macOS crash. Changes
  since the last completed copy can be lost; disabled recovery, an unwritable folder or the
  optional “off for complex documents” setting can prevent copies. Check **Preferences → File
  Handling → Data Recovery**, save manually, and keep backups. After an abnormal exit, choose
  **Restore** if offered, then save the recovered documents. **Discard** deletes the copies;
  **Cancel** keeps them for a later launch. Recovery does not overwrite your working file.
- **Read-only save locations:** saving to a location that refuses writes can fail. Save is intended
  to offer Save As, but the real macOS sheet flow remains unverified. Use **Save As** in a writable
  folder and confirm the saved file before closing the document.
- **Pages are provisional:** native format v5 stores independent pages, but a persistent page
  readout in the editor chrome is still missing. Use the Pages panel to check the active page,
  verify all pages after reopening, and keep a `.varos` master. Older `.vectorcraft` and
  `.drawcraft` files still open; do not assume an older editor can read a v5 project.

## Platform

- **Apple Silicon only:** the preview ZIP will not run on Intel Macs, Windows or Linux. Use an
  M-series Mac with macOS 11 or newer; there is no supported preview download for other platforms.
- **Unverified-developer warning:** the app is ad-hoc signed and not notarized. Follow the
  [first-open instructions](README.md#install-on-macos) after checking the release checksum.
- **Updates are manual:** there is no auto-update. Check
  [Releases](https://github.com/thisisgomaa/varos/releases) for a newer preview and its known issues.

Maintainer evidence: [STATE](docs/varos/STATE.md), [BACKLOG](docs/varos/BACKLOG.md),
[Soon surfaces](docs/varos/lanes/soon-ui-2026-11-04.md),
[PDF regression notes](crates/pdf/tests/fixtures/arabic-copy-back.md), and
[recovery tests](crates/ui-egui/src/tests_recovery.rs). Provisional does not mean a defect is
confirmed; it means native validation remains incomplete.
