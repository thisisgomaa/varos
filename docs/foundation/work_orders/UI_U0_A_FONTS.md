> **Status:** reference — U0-A implementation/evidence, 2026-09-27; merged to `main` in `2cb072a` (2026-10-04). The Plex UI fonts described here were since replaced by Inter 400/500/600 + JetBrains Mono (`429c3f2`, merged `33aa767`); the Arabic gate recorded here still applies ([UI_SYSTEM §8](../../specs/UI_SYSTEM.md#8-arabic--rtl-gate-owner-piece)).
# U0-A — deterministic UI fonts and text readiness

## Outcome and scope

The installed loader searched `C:/Windows/Fonts` even on Mac; Mac fell through to egui defaults.
The work branch embeds IBM Plex Sans/Mono Regular, with licensed Noto symbol fallbacks. The
existing five-role text ramp moves unchanged to shell/tokens.rs. No controls, panel layout,
command routing, stored names or font-system rewrite. This is the minimum font foundation for
U0-B/C, not the Start screen or an Arabic editor.

Owner-approved Plex Arabic is packaged and registered as a named diagnostic family, **not
promoted into live proportional/mono fallbacks**: the pinned egui 0.35 probe failed cluster,
bidi/caret and grapheme expectations. K5 permits an explicit limitation rather than claiming
success from glyph coverage. The limited Latin/numeric rollout can proceed; Arabic acceptance
is a separate open text-layout repair before enabling the family in editable names. Neither
reversing stored strings nor rewriting names into presentation forms is an acceptable workaround.

Files: `shell/fonts.rs`, `shell/tokens.rs`, `shell/mod.rs`, minimal loader/style wiring in `ui.rs`,
font assets/manifest/OFL notices, CPU-only integration tests and Mac bundle license packaging.
`ttf-parser 0.25.1` is a test-only direct dependency already present transitively in the lock;
no new package/version or production dependency. No system-font search, subsetting, GPU test,
new UI framework or vendor patch. Default egui fonts remain enabled.

## Inventory before rollout

| Area | Existing value / disposition |
|---|---|
| Roles | Heading 13.5, Body 13, Button 12.5, Small 11, Monospace 12.5 pt; preserved and centralized. Sans for names, mono for numbers. Regular 400 only; no unused bold/medium files or synthesized weight promise. |
| Local overrides in ui.rs | Sizes 9.5–26 pt remain (12/12.5/11.5 predominate); one 9.5 site is still an old QW6 follow-up. No blanket panel migration in A. |
| Warm palette | BG #141313, PANEL #1b1919, SURFACE #242121, HOVER #2b2828; TEXT #e9e6e3, MUTED #8f8a86, FAINT #6e6a66; unchanged. |
| Semantic colors | GUIDE #ff54a8 reserved for guides, AMBER #f0b429 content/sample palette (not an established warning token), NONE_RED #e05c5c for none swatches, CLOSE_RED #c42b1c close control. Do not repurpose them as new generic error/warning roles without a consumer. |
| Radii/rhythm | Control 3, box 8, capsule 11, seam gap 12 pt; unchanged. Existing control heights vary; new kit targets 24 pt minimum in B. |
| Contrast, opaque backgrounds | FAINT on PANEL/SURFACE/HOVER: 3.26/2.98/2.73; MUTED: 5.12/4.68/4.28; TEXT: 14.08/12.85/11.76. New B controls must use TEXT for hover content; FAINT→MUTED remains the approved migration direction. These calculations do not validate alpha-composited selected/disabled states. |

`tokens.rs` remains the runtime authority. No parser/test enforces agreement with an old HTML
mockup or Markdown. No palette or spacing redesign has been introduced.

## Assets and licenses

Sources are [IBM Plex](https://github.com/IBM/plex/tree/763c36ef9117782905ae010056dfbe8fd2653a25)
and [Noto fonts](https://github.com/notofonts/noto-fonts/tree/ffebf8c1ee449e544955a7e813c54f9b73848eac).
Exact files, hashes and original notices are in app `assets/fonts/manifest.json` and its README.
Complete originals retain their names and licenses. Mac packaging includes the notices/manifest
in Resources/Licenses/Fonts. Noto Symbols 2 supplies ⌘⌥⇧; Symbols supplies ⌃, which Plex and
stock egui fallbacks do not cover together. These supplement the existing fallback fonts.

## Probe and acceptance boundaries

- Real cmap coverage: Latin labels, ASCII and Arabic digits, Arabic base/combining characters,
  UI punctuation and all four Mac modifier symbols. Numeric advance widths are tabular at
  1 and 2 pixels-per-point. Font weights are checked as 400.
- Scripted TextEdit at both scales: paste → select all → copy returns byte-identical logical
  Arabic/mixed names and a long Arabic/Latin path. This does **not** validate visual selection
  or pointer-to-logical-text mapping.
- Candidate Arabic fallback diagnostic: `لوحة أولى` (9 scalar values) produces 14 glyph records,
  including spurious zero-width continuations. A mixed Latin/Arabic name similarly gains records.
  The pinned epaint implementation assumes ascending clusters in its continuation bookkeeping,
  but the Arabic shaper emits RTL clusters. Its run splitting is font-based, not bidi/script-aware.
  Keeping the strings intact and calling the shaping library alone is therefore insufficient.
- End+Backspace on `بَ` removes the combining mark and leaves `ب`; grapheme deletion is not
  accepted as complete. Caret diagnostics are logged separately from clipboard preservation.
- Native Mac candidate preview: Apple M5 / Metal, real window; Latin labels/numbers visible,
  Arabic words showed wrong ordering. No claim of successful Arabic editing or full UI audit.
  The final active chains exclude Arabic; no text-data transformation or RTL vendor patch.

Reproduce from `varos/`:

```sh
cargo test --locked -p varos-app --test fonts
cargo test --locked -p varos-app --test fonts text_layout_and_editing_spike -- --ignored --nocapture
```

Fresh full-gate counts, release binary size delta and final native check are recorded in GATE_LOG.
Next: minimum action/icon buttons, headings, rows, Home chip and notices in U0-B/C; E2 follows.
Arabic repair stays visible as an open item rather than silently blocking all Start work.
