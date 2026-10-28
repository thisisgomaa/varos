# Colour round — Figma-dark neutrals + Varos azure accent (owner decision 2026-10-28)

Neutrals follow Figma's dark UI3 palette exactly; the accent stays Varos azure #0C8CE9, used ONLY for
selection / focus / active / drop preview; agent presence stays orange #F29A3E; no shadows; light mode uses
the Figma-light neutrals. Scope: `crates/ui-egui/src/theme.rs` and `crates/ui-egui/src/varos/theme.rs`
colour values (the Varos file overrides the base and owns the agent orange) + value-asserting tests + docs.
Map every `Tokens` field to the closest role; derive missing roles from the nearest neutral with a comment.

Dark: bg/panel #2C2C2C · canvas #1E1E1E · border #444444 · border_strong #555555 · text #FFFFFF ·
text_secondary #B3B3B3 · text_tertiary #888888 · icon #F5F5F5 · icon_secondary #B3B3B3 · hover #383838 ·
pressed #444444 · field #383838 · field_hover_border #4D4D4D · tab_bar #1E1E1E · tab_active #2C2C2C ·
menu #1E1E1E · menu_text #FFFFFF · menu_dim #9A9A9A · danger #F24822 · warning #FFCD29 · guide #F24822 ·
accent #0C8CE9 · accent_hover #2E9EEE · accent_on #FFFFFF · selected_row #1E3A52 · selected_row_child #243445 ·
focus #0C8CE9 · agent #F29A3E.
Light: bg/panel #FFFFFF · canvas #F5F5F5 · border #E6E6E6 · border_strong #D9D9D9 · text #1E1E1E ·
text_secondary #6E6E6E · text_tertiary #8A8A8A · icon #1E1E1E · icon_secondary #757575 · hover #F5F5F5 ·
pressed #E6E6E6 · field #F5F5F5 · field_hover_border #E6E6E6 · tab_bar #E8E8E8 · tab_active #FFFFFF ·
menu #1E1E1E · menu_text #FFFFFF · menu_dim #9A9A9A · accent #0C8CE9 · accent_hover #0A7BCE ·
selected_row #E3F1FC · selected_row_child #F0F7FD · focus #0C8CE9.
The four brightness presets interpolate consistently between these endpoints. Contrast: body text ≥ 4.5:1,
secondary text and focus ring ≥ 3:1 (unit test); the known exception (azure on pressed surfaces ≈ 2.8:1)
is recorded, not "fixed" — the owner wants the exact palette.
