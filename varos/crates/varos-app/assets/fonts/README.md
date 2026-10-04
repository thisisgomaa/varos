# Bundled UI fonts

Varos embeds static, full (not subset) OFL faces: Inter Regular 400, Medium 500 and
SemiBold 600 for UI; JetBrains Mono Regular 400 for numbers, sizes, paths and dates.
Inter is pinned at optical size 14. The variable sources came from the Google Fonts
`ofl/inter` and `ofl/jetbrainsmono` paths; exact commands, sizes and hashes are in
`manifest.json`.

Noto Sans Symbols 2 and Noto Sans Symbols remain the shortcut-symbol fallbacks for
⌘⌥⇧⌃ and arrows. Existing egui default fonts remain final fallbacks.

IBM Plex Sans Arabic remains registered only as the named diagnostic family
`IBM Plex Sans Arabic`. It is deliberately absent from proportional, monospace and all
four weight-aware production chains because egui 0.35's RTL cluster/cursor mapping did
not pass the U0-A gate. Do not change that gate based on glyph coverage alone.

The original notices are `Inter-OFL.txt`, `JetBrainsMono-OFL.txt`,
`plex-sans-arabic-LICENSE.txt` and `noto-symbols-LICENSE.txt`. The Mac bundle script
copies this directory's notices and manifest into Resources/Licenses/Fonts.

For updates, reproduce the commands in the manifest, verify every recorded length and
SHA-256, and run `cargo test -p varos-app --test fonts`. No runtime download or system
font lookup is permitted.
