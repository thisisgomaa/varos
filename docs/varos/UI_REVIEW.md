# Varos — provisional UI awaiting the owner's design (2026-10-20, main 425f4162)

Every item below was built by agents without a design pass. The owner redesigns in Figma; a lane then
implements the design. Items are grouped by where they live. "Hook" = the upstream file that shows it.

## A. Shell and identity
| # | Piece | What it does today | Where | Design need |
|---|---|---|---|---|
| A1 | Home screen | Only the welcome content (mark, "Welcome to Varos", New file / Open, quick-start presets). No rails, panels, bars. | `crates/ui-egui/src/home.rs` + `varos/home.rs` | Full redesign (Varos Start). |
| A2 | Brightness levels | Dark / Medium Dark / Medium Light / Light are Varos palettes (cool, higher contrast). Fonts Inter, radii 3/8, no shadows. | `varos/theme.rs` | Confirm palettes, pick the default, name the levels. |
| A3 | App mark placements | Title-bar mark, About, Home, DMG background. Uses `assets/app-icon/*`. | `brand.rs`, `varos/names.rs` | New icon + mark set (owner to supply). |
| A4 | Icons | 117 of ours (Lucide + Varos originals); 146 still upstream (77 Lucide imports to copy, 69 to draw). | `varos/icons.rs`, `docs/varos/icon-map.tsv` | Draw the 69 missing in Varos style; decide stroke weight (1.6 vs 2). |
| A5 | Cursors | Varos v1 set as macOS system cursors for Selection/Direct/Pen/Hand/Zoom/shapes/handles; the rest fall back to system cursors (Type I-beam, crosshair tools). | `varos/cursors.rs` | Design the missing states (never hand-drawn: a design source). |

## B. Text panels (Arabic)
| # | Piece | What it does today | Where | Design need |
|---|---|---|---|---|
| B1 | Paragraph › Direction row | Segmented Auto · RTL · LTR; "Auto (RTL)" shows the resolved value. Shown when ME options are on (default on). | `varos/middle_eastern_ui.rs` → `panels/paragraph.rs` | Layout of the "Middle Eastern" group; mixed-selection state. |
| B2 | Paragraph › Kashida row | Segmented Off · Auto · Force + Strength slider 0–100 (greyed when Off); mixed state shows "–". Shown for RTL paragraphs. | `varos/kashida_ui.rs` | Visual of the segmented control + slider; where it sits relative to Justify buttons. |
| B3 | Character › Digits | Dropdown Default / Arabic / Hindi / Farsi. | `varos/middle_eastern_ui.rs` → `panels/character.rs` | Dropdown style; grouping with B4. |
| B4 | Character › Harakat row | Four fixed colour swatches (red, green, gold, black), Hide, As Letters. Acts on selection/Type range; one undo step. | same | Colour chooser (any colour), current-state indication, iconography. |
| B5 | Font menu (Arabic) | "Arabic Fonts" chip → style chips (All · Naskh · Kufi · Ruq'ah · Diwani · Thuluth · Modern Sans); "Arabic Samples" toggle; Arabic family name beside the Latin one (drawn in the family's own face); "AAT" marker. | `varos/arabic_fonts.rs` → `font_menu.rs` | Chip row design, sample line choice and size, marker style, row layout for bilingual names. |
| B6 | Glyphs panel subsets | Arabic Letters / Supplement / Extended-A / Presentation Forms A, B in the Show list. | `panels/glyphs.rs` | None beyond the panel's own redesign. |

## C. Agents
| # | Piece | What it does today | Where | Design need |
|---|---|---|---|---|
| C1 | Canvas presence | Orange (#c76a20) 1.5 px artboard outline + label chip above its top-right corner while an agent works; 1 px orange outlines on new/changed objects appearing one by one (25 ms stagger, ≤1.5 s), fading 0.9 s; artboard outline holds 4 s; human selection (azure) always wins. | `varos/agents.rs` (paint) | Chip typography/placement; outline weights; multi-agent colours? |
| C2 | History rows | Agent steps get an orange icon + agent name right-aligned in orange. | `panels/history.rs` hook | Row design; the future per-agent review list (filter, "back to before this agent"). |

## D. Upstream panels we have NOT touched yet (candidates for Varos designs)
Color panel (→ port the Varos picker), Export / Export for Screens sheets (→ Varos export sheet), Control bar, Toolbar + flyouts, Layers, Properties, Swatches, Stroke, Appearance, Preferences, Document Setup, New Document, About.

## How to review
Open `Varos.app` (v7). For B1–B4: Type tool, drag an area box, type Arabic, open Paragraph / Character panels.
For B5: open the font menu from the Character panel. For C: connect Claude Code (see `docs/varos/lanes/agents-report.txt`).
