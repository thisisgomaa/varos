//! U0-A CPU-only font/layout/editing checks. No Renderer or EventLoop.
use egui::{Color32, Context, Event, FontFamily, FontId, Id, Key, Modifiers, RawInput};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use varos_app::shell::{fonts, tokens};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts");

#[derive(Deserialize)]
struct Manifest {
    files: Vec<ManifestFile>,
}
#[derive(Deserialize)]
struct ManifestFile {
    file: String,
    bytes: u64,
    sha256: String,
}

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
fn every_bundled_file_matches_the_manifest_sha256_and_size() {
    let manifest: Manifest = serde_json::from_str(include_str!("../assets/fonts/manifest.json")).unwrap();
    let mut total = 0;
    for entry in manifest.files {
        let bytes = std::fs::read(format!("{ASSETS}/{}", entry.file)).unwrap();
        assert_eq!(bytes.len() as u64, entry.bytes, "{} byte length", entry.file);
        let digest: String = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(digest, entry.sha256, "{} SHA-256", entry.file);
        total += bytes.len();
    }
    println!("bundled font assets: {total} bytes");
}

#[test]
fn static_faces_have_the_requested_weights_and_coverage() {
    for (name, required) in [
        ("Inter Regular", "Varos New Open Recent 0123456789.,%−×°…·"),
        ("Inter Medium", "Varos New Open Recent 0123456789.,%−×°…·"),
        ("Inter SemiBold", "Varos New Open Recent 0123456789.,%−×°…·"),
        ("JetBrains Mono Regular", "0123456789.,%−×°…·"),
        ("IBM Plex Sans Arabic", "لوحة أولى تصميم جديد عربي English ١٢٣٤٥٦٧٨٩٠ بَ"),
    ] {
        let defs = fonts::definitions();
        let data = &defs.font_data[name];
        let face = ttf_parser::Face::parse(&data.font, data.index).unwrap();
        let expected = match name {
            "Inter Medium" => 500,
            "Inter SemiBold" => 600,
            _ => 400,
        };
        assert_eq!(face.weight().to_number(), expected);
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
            for family in [
                FontFamily::Proportional,
                FontFamily::Monospace,
                FontFamily::Name(fonts::UI_400.into()),
                FontFamily::Name(fonts::UI_500.into()),
                FontFamily::Name(fonts::UI_600.into()),
                FontFamily::Name(fonts::MONO_400.into()),
            ] {
                let id = FontId::new(13.0, family);
                let defs = fonts::definitions();
                for ch in (' '..='~').chain("⌘⌥⇧⌃←→↑↓−…×·•".chars()) {
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
            let font = FontId::new(11.0, FontFamily::Name(fonts::MONO_400.into()));
            let widths: Vec<_> = "0123456789".chars().map(|ch| f.glyph_width(&font, ch)).collect();
            assert!(widths.iter().all(|w| (w - widths[0]).abs() < 0.01), "{widths:?}");
        });
    }
}

#[test]
fn weighted_family_keeps_the_builtin_fallback_tail() {
    let defs = fonts::definitions();
    let family = FontFamily::Name(fonts::UI_600.into());
    assert!(defs.families[&family].iter().any(|name| {
        let data = &defs.font_data[name];
        ttf_parser::Face::parse(&data.font, data.index).unwrap().glyph_index('😀').is_some()
    }));
}

#[test]
fn numeric_value_digits_and_field_width_are_tabular() {
    let ctx = context(1.0);
    ctx.fonts_mut(|fonts| {
        let font = tokens::numeric_value(tokens::FIELD_TEXT);
        let digits: Vec<_> = "0123456789".chars().map(|ch| fonts.glyph_width(&font, ch)).collect();
        assert!(digits.iter().all(|width| (width - digits[0]).abs() < 0.01), "{digits:?}");
        let mut width = |text: &str| fonts.layout_no_wrap(text.into(), font.clone(), Color32::WHITE).size().x;
        assert!((width("111.1") - width("888.8")).abs() < 0.01);
    });
}

#[test]
fn inter_static_weights_have_distinct_glyph_outlines() {
    let defs = fonts::definitions();
    let signature = |name: &str| {
        let data = &defs.font_data[name];
        let face = ttf_parser::Face::parse(&data.font, data.index).unwrap();
        "Varos 2026"
            .chars()
            .filter_map(|ch| face.glyph_index(ch))
            .map(|glyph| (face.glyph_hor_advance(glyph), face.glyph_bounding_box(glyph)))
            .collect::<Vec<_>>()
    };
    let regular = signature("Inter Regular");
    let medium = signature("Inter Medium");
    let semibold = signature("Inter SemiBold");
    assert_ne!(regular, medium);
    assert_ne!(medium, semibold);
    assert_ne!(regular, semibold);
}

#[test]
fn arabic_remains_behind_its_named_family_gate() {
    let defs = fonts::definitions();
    for family in [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name(fonts::UI_400.into()),
        FontFamily::Name(fonts::UI_500.into()),
        FontFamily::Name(fonts::UI_600.into()),
        FontFamily::Name(fonts::MONO_400.into()),
    ] {
        assert!(!defs.families[&family].iter().any(|name| name == "IBM Plex Sans Arabic"));
    }
    assert_eq!(defs.families[&FontFamily::Name("IBM Plex Sans Arabic".into())][0], "IBM Plex Sans Arabic");
}

#[test]
fn start_v2_type_tokens_map_to_static_weight_families() {
    for (font, size, family) in [
        (tokens::h1(), 30.0, fonts::UI_600),
        (tokens::h2(), 18.0, fonts::UI_600),
        (tokens::button(), 15.0, fonts::UI_500),
        (tokens::name(), 14.0, fonts::UI_600),
        (tokens::body(), 13.0, fonts::UI_400),
        (tokens::small(), 12.0, fonts::UI_400),
        (tokens::tag(), 11.0, fonts::UI_500),
        (tokens::mono(), 11.0, fonts::MONO_400),
    ] {
        assert_eq!(font.size, size);
        assert_eq!(font.family, FontFamily::Name(family.into()));
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
