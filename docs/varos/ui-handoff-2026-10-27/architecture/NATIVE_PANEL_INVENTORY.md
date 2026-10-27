# Native panel inventory

Source read on 2026-10-27: `~/Documents/AI workspace/varos-next/crates/ui-egui/src/state.rs`. This is the locally inspected source, not proof of the currently running app or a production branch. Verify the checkout before implementation.

Registered dock tabs: Properties, Layers, Libraries (3). Registered icon panels: 39. Total: **42 registered panels**; the main design viewport is an additional editor concept in the agreed architecture.

| Registered ID or source expression | Label | Icon key |
| --- | --- | --- |
| `properties` | Properties | `dc-options` |
| `layers` | Layers | `layers` |
| `libraries` | Libraries | `library` |
| `"color"` | Color | `palette` |
| `"colorGuide"` | Color Guide | `dc-color-guide` |
| `"swatches"` | Swatches | `swatch-book` |
| `"brushes"` | Brushes | `paintbrush` |
| `"symbols"` | Symbols | `spray-can` |
| `"patternOptions"` | Pattern Options | `grid-3x3` |
| `"stroke"` | Stroke | `dc-stroke` |
| `"gradient"` | Gradient | `dc-gradient` |
| `"transparency"` | Transparency | `dc-transparency` |
| `"appearance"` | Appearance | `dc-appearance` |
| `"graphicStyles"` | Graphic Styles | `dc-graphic-styles` |
| `"artboards"` | Artboards | `dc-artboards` |
| `"transform"` | Transform | `dc-transform-panel` |
| `"align"` | Align | `dc-align` |
| `"pathfinder"` | Pathfinder | `dc-pathfinder` |
| `crate::panels::blend::ID` | Blend | `dc-blend` |
| `"character"` | Character | `type` |
| `"paragraph"` | Paragraph | `pilcrow` |
| `"glyphs"` | Glyphs | `text-cursor-input` |
| `"charStyles"` | Character Styles | `type` |
| `"openType"` | OpenType | `dc-touch-type` |
| `"paraStyles"` | Paragraph Styles | `pilcrow` |
| `"tabs"` | Tabs | `align-horizontal-justify-start` |
| `"history"` | History | `history` |
| `"actions"` | Actions | `dc-actions` |
| `"info"` | Info | `info` |
| `"docInfo"` | Document Info | `dc-list-view` |
| `"navigator"` | Navigator | `map` |
| `"separations"` | Separations Preview | `printer` |
| `"imageTrace"` | Image Trace | `image` |
| `"magicWand"` | Magic Wand | `wand-sparkles` |
| `crate::panels::flattener_preview::ID` | Flattener Preview | `eye` |
| `"attributes"` | Attributes | `settings` |
| `crate::panels::svg_interactivity::ID` | SVG Interactivity | `spline-pointer` |
| `"colorThemes"` | Color Themes | `sun` |
| `crate::panels::links::ID` | Links | `link` |
| `crate::panels::asset_export::ID` | Asset Export | `share-2` |
| `crate::panels::css_properties::ID` | CSS Properties | `globe` |
| `crate::panels::variables::ID` | Variables | `chart-column` |
