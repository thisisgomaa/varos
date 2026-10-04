//! Icon stage 1 — the one registry and the one icon button. CPU-only (resvg + bare egui contexts at
//! logical 1× and Retina 2×); no window, no GPU.
use egui::{Context, Event, Id, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use varos_app::shell::{
    fonts,
    kit::{self, ControlResponse, Icon, IconState},
    tokens,
};

fn input(ppp: f32, events: Vec<Event>) -> RawInput {
    let mut input = RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(240.0, 120.0))),
        events,
        ..Default::default()
    };
    input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap().native_pixels_per_point = Some(ppp);
    input
}
fn context(ppp: f32) -> Context {
    let ctx = Context::default();
    fonts::install(&ctx);
    tokens::apply(&ctx);
    let _ = ctx.run_ui(input(ppp, vec![]), |_| {});
    ctx
}
fn key(key: Key, repeat: bool) -> Event {
    Event::Key { key, physical_key: Some(key), pressed: true, repeat, modifiers: Modifiers::NONE }
}
fn pointer(pos: Pos2, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE },
    ]
}
const ID: &str = "icon-button-under-test";
fn frame(ctx: &Context, events: Vec<Event>, state: IconState<'_>) -> (ControlResponse, egui::FullOutput) {
    let mut result = None;
    let out = ctx.run_ui(input(ctx.pixels_per_point(), events), |ui| {
        result = Some(kit::icon_button(ui, Id::new(ID), Icon::Trash, "Delete artboard", state));
    });
    (result.unwrap(), out)
}
fn rects(out: &egui::FullOutput) -> Vec<egui::epaint::RectShape> {
    out.shapes.iter().filter_map(|s| if let egui::Shape::Rect(r) = &s.shape { Some(r.clone()) } else { None }).collect()
}

#[test]
fn every_registry_icon_resolves_to_an_embedded_lucide_svg_that_parses() {
    let mut names = std::collections::HashSet::new();
    for icon in Icon::ALL {
        let (name, svg) = icon.lucide();
        assert!(names.insert(name), "{name} is registered twice");
        assert!(svg.contains(&format!("lucide-{name}\"")), "{name}: the file is the named Lucide glyph");
        assert!(svg.contains("@license lucide"), "{name}: the upstream licence header travels with the file");
        assert!(svg.contains("stroke=\"currentColor\""), "{name}: one ink, tinted at paint time");
        for px in [tokens::ICON_RASTER, tokens::ICON_SM as u32, (tokens::ICON_LG * 2.0) as u32] {
            let (rgba, w, h) = icon.rasterize_at(px).unwrap_or_else(|| panic!("{name} must parse at {px}"));
            assert_eq!((w, h), (px, px), "{name}");
            assert!(rgba.chunks(4).any(|p| p[3] > 0), "{name} draws ink at {px}");
        }
    }
    // Every SVG shipped in assets/icons/ belongs to an icon in `ALL`: no orphan file, and a variant left
    // out of `ALL` (never tested, never on the contact sheet) shows up here as an orphan of its file.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
    for entry in std::fs::read_dir(dir).unwrap() {
        let file = entry.unwrap().file_name().into_string().unwrap();
        if let Some(stem) = file.strip_suffix(".svg") {
            assert!(names.contains(stem), "assets/icons/{file} is not in the registry");
        }
    }
}

#[test]
fn icon_button_activates_once_by_pointer_and_by_keyboard_at_1x_and_2x() {
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        let (r, _) = frame(&ctx, vec![], IconState::Action);
        assert!(!r.activated);
        let size = r.response.rect.size();
        assert!(size.x >= tokens::KIT_MIN_TARGET && size.y >= tokens::KIT_MIN_TARGET, "hit target {size:?}");
        assert_eq!(size, egui::vec2(tokens::ICON_BTN_W, tokens::ICON_BTN_H));
        let pos = r.response.rect.center();
        assert!(!frame(&ctx, pointer(pos, true), IconState::Action).0.activated);
        assert!(frame(&ctx, pointer(pos, false), IconState::Action).0.activated);
        r.response.request_focus();
        for button in [Key::Enter, Key::Space] {
            let (r, out) = frame(&ctx, vec![key(button, false)], IconState::Action);
            assert!(r.activated, "{button:?} activates a focused icon button");
            let ring = rects(&out).iter().any(|s| s.stroke.color == tokens::ACCENT);
            assert!(ring, "keyboard focus shows the azure ring");
            assert!(!frame(&ctx, vec![key(button, true)], IconState::Action).0.activated, "repeat is ignored");
        }
    }
}

#[test]
fn disabled_icon_button_never_activates_and_explains_why() {
    for ppp in [1.0, 2.0] {
        let ctx = context(ppp);
        let state = IconState::Disabled("the last artboard can't be deleted");
        let (r, _) = frame(&ctx, vec![], state);
        let pos = r.response.rect.center();
        r.response.request_focus();
        for events in [pointer(pos, true), pointer(pos, false), vec![key(Key::Enter, false)]] {
            let (r, out) = frame(&ctx, events, state);
            assert!(!r.activated && !r.response.enabled());
            assert!(!rects(&out).iter().any(|s| s.fill == tokens::ACCENT || s.stroke.color == tokens::ACCENT));
        }
    }
    let tip = kit::icon_tooltip("Delete artboard", IconState::Disabled("the last artboard can't be deleted"));
    assert_eq!(tip, "Delete artboard — the last artboard can't be deleted");
    assert_eq!(kit::icon_tooltip("Flip horizontal", IconState::Action), "Flip horizontal");
}

#[test]
fn tool_is_an_azure_block_and_toggle_is_a_small_azure_bar() {
    let ctx = context(1.0);
    let (r, out) = frame(&ctx, vec![], IconState::Tool(true));
    let block = rects(&out).iter().any(|s| s.fill == tokens::ACCENT && s.rect == r.response.rect);
    assert!(block, "an active tool fills its whole target with azure");
    let (r, out) = frame(&ctx, vec![], IconState::Toggle(true));
    let azure: Vec<_> = rects(&out).into_iter().filter(|s| s.fill == tokens::ACCENT).collect();
    assert_eq!(azure.len(), 1, "a toggle shows exactly one azure mark");
    assert_eq!(azure[0].rect.size(), egui::vec2(tokens::ICON_BAR_W, tokens::ICON_BAR_H));
    assert!(r.response.rect.contains_rect(azure[0].rect));
    for off in [IconState::Toggle(false), IconState::Tool(false), IconState::Action] {
        let (_, out) = frame(&ctx, vec![], off);
        assert!(!rects(&out).iter().any(|s| s.fill == tokens::ACCENT), "{off:?} shows no azure");
    }
}

/// The contact sheet for the owner: every registry icon at 1× (16 px) and 2× (32 px) on the signature
/// #141313, ink = TEXT, in `Icon::ALL` order (the test prints the order). Always rendered in memory;
/// written to disk only when `VAROS_ICON_SHEET=<path.png>` is set.
#[test]
fn contact_sheet_renders_every_icon_at_1x_and_2x() {
    let (cell, pad) = (48u32, 8u32);
    let cols = Icon::ALL.len() as u32;
    let (w, h) = (pad * 2 + cell * cols, pad * 2 + cell * 2);
    let bg = tokens::BG;
    let ink = tokens::TEXT;
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([bg.r(), bg.g(), bg.b(), 255]));
    for (col, icon) in Icon::ALL.iter().enumerate() {
        for (row, scale) in [1u32, 2].into_iter().enumerate() {
            let px = tokens::ICON_MD as u32 * scale;
            let (rgba, iw, ih) = icon.rasterize_at(px).expect("rasterizes");
            let ox = pad + col as u32 * cell + (cell - iw) / 2;
            let oy = pad + row as u32 * cell + (cell - ih) / 2;
            for y in 0..ih {
                for x in 0..iw {
                    let a = rgba[((y * iw + x) * 4 + 3) as usize] as u32;
                    let dst = img.get_pixel_mut(ox + x, oy + y);
                    for (c, v) in [ink.r(), ink.g(), ink.b()].into_iter().enumerate() {
                        dst.0[c] = ((v as u32 * a + dst.0[c] as u32 * (255 - a)) / 255) as u8;
                    }
                }
            }
        }
    }
    let lit = img.pixels().filter(|p| p.0[0] > bg.r() + 40).count();
    assert!(lit > Icon::ALL.len() * 20, "the sheet shows ink for every icon ({lit} lit px)");
    let order: Vec<_> = Icon::ALL.iter().map(|i| i.lucide().0).collect();
    println!("contact sheet order (left → right): {}", order.join(", "));
    if let Ok(path) = std::env::var("VAROS_ICON_SHEET") {
        img.save(&path).expect("write the contact sheet");
        println!("wrote {path} ({w}×{h})");
    }
}
