//! U0-A CPU-only font/layout/editing checks. No Renderer or EventLoop.
use egui::{Color32, Context, Event, FontFamily, FontId, Id, Key, Modifiers, RawInput};
use varos_app::shell::fonts;

fn input(ppp: f32, events: Vec<Event>) -> RawInput {
    let mut input = RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
        events,
        ..Default::default()
    };
    input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(ppp);
    input
}
fn context(ppp: f32) -> Context {
    let ctx = Context::default();
    fonts::install(&ctx);
    let _ = ctx.run_ui(input(ppp, vec![]), |_| {}); // install + atlas warm-up
    ctx
}
fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat: false, modifiers }
}
fn edit(ctx: &Context, text: &mut String, events: Vec<Event>) -> egui::FullOutput {
    ctx.run_ui(input(ctx.pixels_per_point(), events), |ui| {
        let response = egui::TextEdit::singleline(text).id(Id::new("font-spike")).desired_width(320.0).show(ui);
        response.response.request_focus();
    })
}

#[test]
fn bundled_faces_cover_latin_arabic_and_combining_marks() {
    for (name, required) in [
        ("IBM Plex Sans", "Varos New Open Recent 0123456789.,%−×°…"),
        ("IBM Plex Mono", "0123456789.,%−×°"),
        ("IBM Plex Sans Arabic", "لوحة أولى تصميم جديد عربي English ١٢٣٤٥٦٧٨٩٠ بَ"),
    ] {
        let defs = fonts::definitions();
        let data = &defs.font_data[name];
        let face = ttf_parser::Face::parse(&data.font, data.index).unwrap();
        assert_eq!(face.weight().to_number(), 400);
        for ch in required.chars().filter(|c| !c.is_whitespace()) {
            assert!(face.glyph_index(ch).is_some(), "{name}: missing {ch}");
        }
    }
}

#[test]
fn fallback_chain_covers_shortcuts_and_mono_digits_are_tabular_at_both_scales() {
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        ctx.fonts_mut(|f| {
            for family in [FontFamily::Proportional, FontFamily::Monospace] {
                let id = FontId::new(13.0, family);
                let defs = fonts::definitions();
                for ch in "⌘⌥⇧⌃←→↑↓−×°…•".chars() {
                    assert!(
                        defs.families[&id.family].iter().any(|name| {
                            let data = &defs.font_data[name];
                            ttf_parser::Face::parse(&data.font, data.index).unwrap().glyph_index(ch).is_some()
                        }),
                        "missing {ch} in {:?}",
                        id.family
                    );
                }
            }
            let font = FontId::monospace(12.5);
            let widths: Vec<_> = "0123456789".chars().map(|ch| f.glyph_width(&font, ch)).collect();
            assert!(widths.iter().all(|w| (w - widths[0]).abs() < 0.01), "{widths:?}");
        });
    }
}

#[test]
fn paste_select_all_copy_preserve_logical_names_and_long_paths() {
    for ppp in [1.0, 2.0] {
        for sample in [
            "لوحة أولى",
            "Logo عربي 2026.vrs",
            "/Users/designer/Documents/مشروع جديد/Very long project name with spaces/نسخة نهائية 02.vrs",
            "بَ",
        ] {
            let ctx = context(ppp);
            let mut text = String::new();
            let _ = edit(&ctx, &mut text, vec![]);
            let _ = edit(&ctx, &mut text, vec![Event::Paste(sample.into())]);
            assert_eq!(text, sample);
            let _ = edit(&ctx, &mut text, vec![key(Key::A, Modifiers::COMMAND)]);
            let out = edit(&ctx, &mut text, vec![Event::Copy]);
            assert!(
                out.platform_output
                    .commands
                    .iter()
                    .any(|c| matches!(c, egui::OutputCommand::CopyText(s) if s == sample)),
                "clipboard lost logical text: {sample:?}"
            );
        }
    }
}

/// Diagnostic, not an Arabic-support acceptance gate. Keep raw evidence separate from glyph coverage.
#[test]
#[ignore = "manual U0-A shaping/bidi/caret/grapheme diagnostic; inspect output and the native window"]
fn text_layout_and_editing_spike() {
    let defs = fonts::definitions();
    for ch in "⌘⌥⇧⌃".chars() {
        let owners: Vec<_> = defs
            .font_data
            .iter()
            .filter_map(|(name, data)| {
                ttf_parser::Face::parse(&data.font, data.index).unwrap().glyph_index(ch).map(|_| name.as_str())
            })
            .collect();
        println!("symbol {ch}: {owners:?}");
    }
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        // Reproduce the candidate global Arabic fallback without enabling it in production.
        let mut candidate = fonts::definitions();
        candidate.families.get_mut(&FontFamily::Proportional).unwrap().insert(1, "IBM Plex Sans Arabic".into());
        ctx.set_fonts(candidate);
        let _ = ctx.run_ui(input(ppp, vec![]), |_| {});
        ctx.fonts_mut(|f| {
            for text in ["لوحة أولى", "Logo عربي 2026.vrs", "بَ"] {
                let galley = f.layout_no_wrap(text.into(), FontId::proportional(13.0), Color32::WHITE);
                let visual: String = galley.rows.iter().flat_map(|r| r.glyphs.iter()).map(|g| g.chr).collect();
                println!(
                    "ppp={ppp} logical={text:?} visual-glyph-order={visual:?} size={:?} logical-end-caret-x={}",
                    galley.size(),
                    galley.pos_from_cursor(egui::text::CCursor::new(text.chars().count())).min.x
                );
                for row in &galley.rows {
                    println!(
                        "positions={:?}",
                        row.glyphs.iter().map(|g| (g.chr, g.pos.x, g.advance_width)).collect::<Vec<_>>()
                    );
                }
            }
        });
        let mut text = "بَ".to_string();
        let _ = edit(&ctx, &mut text, vec![]);
        let _ = edit(&ctx, &mut text, vec![key(Key::End, Modifiers::NONE)]);
        let _ = edit(&ctx, &mut text, vec![key(Key::Backspace, Modifiers::NONE)]);
        println!("ppp={ppp} combining-grapheme after End+Backspace={text:?}");
    }
}
