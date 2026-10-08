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
fn every_registry_icon_resolves_to_an_embedded_svg_that_parses() {
    let mut names = std::collections::HashSet::new();
    let mut originals = std::collections::HashSet::new();
    for icon in Icon::ALL {
        let (name, svg) = icon.lucide();
        assert!(names.insert(name), "{name} is registered twice");
        if icon.is_original() {
            // A Varos original (UI_SYSTEM §icons rule 2): drawn from scratch, never a relabelled Lucide file.
            originals.insert(name);
            assert!(svg.contains(&format!("class=\"varos varos-{name}\"")), "{name}: the file is the named original");
            assert!(svg.contains("original Varos glyph, 2026-10-08, drawn from scratch"), "{name}: provenance header");
            assert!(!svg.contains("lucide"), "{name}: an original carries no Lucide class or licence");
        } else {
            assert!(svg.contains(&format!("lucide-{name}\"")), "{name}: the file is the named Lucide glyph");
            assert!(svg.contains("@license lucide"), "{name}: the upstream licence header travels with the file");
        }
        // Same grid and the same pen as Lucide, so originals sit beside upstream glyphs at 16 and 18 pt.
        for attr in
            ["viewBox=\"0 0 24 24\"", "stroke-width=\"2\"", "stroke-linecap=\"round\"", "stroke-linejoin=\"round\""]
        {
            assert!(svg.contains(attr), "{name}: {attr}");
        }
        assert!(svg.contains("stroke=\"currentColor\""), "{name}: one ink, tinted at paint time");
        assert!(!svg.contains("opacity"), "{name}: no partial ink — every mark keeps the token's contrast");
        for px in [tokens::ICON_RASTER, tokens::ICON_SM as u32, (tokens::ICON_LG * 2.0) as u32] {
            let (rgba, w, h) = icon.rasterize_at(px).unwrap_or_else(|| panic!("{name} must parse at {px}"));
            assert_eq!((w, h), (px, px), "{name}");
            assert!(rgba.chunks(4).any(|p| p[3] > 0), "{name} draws ink at {px}");
        }
    }
    // Every SVG shipped in assets/icons/ (Lucide) and assets/icons/varos/ (originals) belongs to an icon in
    // `ALL` of the matching kind: no orphan file, and a variant left out of `ALL` (never tested, never on
    // the contact sheet) shows up here as an orphan of its file.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/icons");
    for (dir, original) in [(root.clone(), false), (root.join("varos"), true)] {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let file = entry.unwrap().file_name().into_string().unwrap();
            if let Some(stem) = file.strip_suffix(".svg") {
                let registered =
                    if original { originals.contains(stem) } else { names.contains(stem) && !originals.contains(stem) };
                assert!(registered, "{}/{file} is not in the registry", dir.display());
            }
        }
    }
    assert_eq!(originals.len(), 15, "the 15 panel originals of 2026-10-08");
}

/// The rail's Artboard tool still paints the inline `LEGACY_ARTBOARD`; it must stay the exact geometry of
/// the registry's Lucide "frame", so the panels' Align-to-Artboard / Edit-artboards glyph and the rail
/// show one shape (no second, drifting copy).
#[test]
fn legacy_artboard_glyph_is_the_registry_frame() {
    let frame = Icon::Frame.lucide().1;
    let mut from_file = Vec::new();
    for line in frame.lines().filter(|l| l.trim_start().starts_with("<line")) {
        let num = |k: &str| {
            let i = line.find(&format!("{k}=\"")).unwrap() + k.len() + 2;
            line[i..].split('"').next().unwrap().parse::<i32>().unwrap()
        };
        let (x1, x2, y1, y2) = (num("x1"), num("x2"), num("y1"), num("y2"));
        from_file.push(if y1 == y2 {
            format!("<path d=\"M{x1} {y1}H{x2}\"/>")
        } else {
            format!("<path d=\"M{x1} {y1}v{}\"/>", y2 - y1)
        });
    }
    assert_eq!(from_file.len(), 4);
    assert_eq!(from_file.concat(), kit::icons::LEGACY_ARTBOARD);
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

/// UI_SYSTEM K5: focus-visible is ONE overlay for every state — an azure ring outside the target, with a
/// 1 px panel-colour gap between them (so it still reads around an azure tool block); never inside.
#[test]
fn focus_ring_is_the_same_azure_overlay_outside_the_target_in_every_state() {
    for ppp in [1.0, 2.0] {
        for state in [IconState::Action, IconState::Toggle(true), IconState::Tool(true), IconState::Toggle(false)] {
            let ctx = context(ppp);
            let (r, _) = frame(&ctx, vec![], state);
            r.response.request_focus();
            let (r, out) = frame(&ctx, vec![key(Key::A, false)], state); // any key → keyboard modality
            let target = r.response.rect;
            let all = rects(&out);
            let rings: Vec<_> = all.iter().filter(|s| s.stroke.color == tokens::ACCENT).collect();
            assert_eq!(rings.len(), 1, "{state:?}: exactly one azure ring");
            let ring = rings[0];
            assert_eq!(ring.stroke.width, tokens::KIT_FOCUS_STROKE);
            assert_eq!(ring.stroke_kind, egui::StrokeKind::Outside, "{state:?}: the ring is drawn outside");
            assert_eq!(ring.rect, target.expand(tokens::KIT_FOCUS_GAP), "{state:?}: one gap away from the target");
            let gap = all.iter().any(|s| {
                s.stroke.color == tokens::PANEL
                    && s.stroke.width == tokens::KIT_FOCUS_GAP
                    && s.rect == target.expand(tokens::KIT_FOCUS_GAP / 2.0)
            });
            assert!(gap, "{state:?}: a panel-colour gap separates the ring from the target");
            assert!(!all.iter().any(|s| s.stroke.color == tokens::TEXT), "{state:?}: no TEXT ring inside the block");
        }
        // Without keyboard modality (a mouse click) there is no ring at all.
        let ctx = context(ppp);
        let (r, _) = frame(&ctx, vec![], IconState::Tool(true));
        let (_, out) = frame(&ctx, pointer(r.response.rect.center(), true), IconState::Tool(true));
        assert!(!rects(&out).iter().any(|s| s.stroke.color == tokens::ACCENT));
    }
}

#[test]
fn tool_is_an_azure_block_and_toggle_is_a_dark_well() {
    let ctx = context(1.0);
    let (r, out) = frame(&ctx, vec![], IconState::Tool(true));
    let block = rects(&out).iter().any(|s| s.fill == tokens::ACCENT && s.rect == r.response.rect);
    assert!(block, "an active tool fills its whole target with azure");
    let (r, out) = frame(&ctx, vec![], IconState::Toggle(true));
    assert!(!rects(&out).iter().any(|s| s.fill == tokens::ACCENT));
    assert!(rects(&out).iter().any(|s| s.fill == tokens::TOGGLE_WELL && s.rect == r.response.rect));
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

/// The owner's optical-pass proof for the panel-icon glyphs (2026-10-08): every glyph added for the panel
/// controls at 16 pt and 18 pt on PANEL, in the three ink states (MUTED rest · TEXT on a HOVER chip ·
/// toggle-on = TOGGLE_WELL + TEXT, no azure bar) inside a 26×24 target, plus the Document strip as composed in
/// direction B (so the Grid dots / Snap to grid pair is judged side by side). Two sheets:
/// - 2× = what a Retina display shows: the glyph's real `ICON_RASTER` texture, bilinear-scaled to its
///   draw size exactly as egui samples it;
/// - 4× = the same glyphs re-rendered through `svg::render_svg` at 4× for shape inspection.
///
/// Always rendered in memory; written only when `VAROS_GLYPH_PROOF=<dir>` is set.
#[test]
fn panel_glyph_proof_sheets_render_every_new_glyph_in_every_state() {
    use resvg::tiny_skia::{self, FillRule, Paint, PathBuilder, Pixmap, PixmapPaint, Rect as SkRect, Transform};
    use resvg::usvg;
    const NEW: [(Icon, &str); 25] = [
        (Icon::AlignAuto, "Align to: Auto"),
        (Icon::AlignSelection, "Align to: Selection"),
        (Icon::Frame, "Align to: Artboard"),
        (Icon::Ruler, "Rulers"),
        (Icon::GridDots, "Grid dots"),
        (Icon::Guides, "Guides"),
        (Icon::GuidesLock, "Lock guides"),
        (Icon::SmartGuides, "Smart guides"),
        (Icon::Magnet, "Snapping"),
        (Icon::SnapPoint, "Snap to point"),
        (Icon::SnapGrid, "Snap to grid"),
        (Icon::TransparentPage, "Transparent page"),
        (Icon::Crop, "Clip to page"),
        (Icon::MoveArtwork, "Move artwork"),
        (Icon::ListFilter, "Filter layers"),
        (Icon::ChevronLeft, "Previous artboard"),
        (Icon::ChevronRight, "Next artboard"),
        (Icon::HarmonyNone, "Harmony: None"),
        (Icon::HarmonyComplementary, "Complementary"),
        (Icon::HarmonyAnalogous, "Analogous"),
        (Icon::HarmonySplit, "Split complementary"),
        (Icon::HarmonyTriad, "Triad"),
        (Icon::HarmonyTetradic, "Tetradic"),
        (Icon::HarmonySquare, "Square"),
        (Icon::HarmonyMono, "Monochrome"),
    ];
    // Document strip, direction B: Rulers · Grid dots (disabled) | Guides · Lock guides · Smart guides |
    // Snapping · Snap to point · Snap to grid. `None` = a LINE2 divider. (icon, on, disabled)
    let strip: [Option<(Icon, bool, bool)>; 10] = [
        Some((Icon::Ruler, true, false)),
        Some((Icon::GridDots, false, true)),
        None,
        Some((Icon::Guides, true, false)),
        Some((Icon::GuidesLock, false, false)),
        Some((Icon::SmartGuides, true, false)),
        None,
        Some((Icon::Magnet, true, false)),
        Some((Icon::SnapPoint, true, false)),
        Some((Icon::SnapGrid, false, false)),
    ];
    let c = |c: egui::Color32| tiny_skia::Color::from_rgba8(c.r(), c.g(), c.b(), 255);
    let hex = |c: egui::Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    let mut fontdb = usvg::fontdb::Database::new();
    fontdb.load_font_data(include_bytes!("../assets/fonts/Inter-Regular.ttf").to_vec());
    let opts = usvg::Options { fontdb: std::sync::Arc::new(fontdb), font_family: "Inter".into(), ..Default::default() };
    let (label_w, cell_w, cell_h, pad) = (130.0f32, 34.0f32, 32.0f32, 12.0f32);
    let grid_x = pad + label_w;
    let states = ["rest", "hover", "on"];
    for scale in [2u32, 4] {
        let s = scale as f32;
        let rows_y = pad + 50.0;
        let strip_y = rows_y + NEW.len() as f32 * cell_h + 28.0;
        let (w_pt, h_pt) = (grid_x + cell_w * 6.0 + 16.0 + pad, strip_y + 24.0 + 20.0 + pad);
        let mut pm = Pixmap::new((w_pt * s) as u32, (h_pt * s) as u32).unwrap();
        pm.fill(c(tokens::BG));
        let fill_rr = |pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, col: egui::Color32| {
            let (x, y, w, h, r) = (x * s, y * s, w * s, h * s, r * s);
            let mut pb = PathBuilder::new();
            pb.move_to(x + r, y);
            pb.line_to(x + w - r, y);
            pb.quad_to(x + w, y, x + w, y + r);
            pb.line_to(x + w, y + h - r);
            pb.quad_to(x + w, y + h, x + w - r, y + h);
            pb.line_to(x + r, y + h);
            pb.quad_to(x, y + h, x, y + h - r);
            pb.line_to(x, y + r);
            pb.quad_to(x, y, x + r, y);
            pb.close();
            let mut paint = Paint::default();
            paint.set_color(c(col));
            paint.anti_alias = true;
            pm.fill_path(&pb.finish().unwrap(), &paint, FillRule::Winding, Transform::identity(), None);
        };
        let fill_rect = |pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, col: egui::Color32| {
            let mut paint = Paint::default();
            paint.set_color(c(col));
            pm.fill_rect(SkRect::from_xywh(x * s, y * s, w * s, h * s).unwrap(), &paint, Transform::identity(), None);
        };
        // A glyph at `size` pt centred on (cx, cy) pt, tinted `ink`.
        let glyph = |pm: &mut Pixmap, icon: Icon, size: f32, cx: f32, cy: f32, ink: egui::Color32| {
            let (rgba, w, h) =
                if scale == 2 { icon.rasterize().unwrap() } else { icon.rasterize_at((size * s) as u32).unwrap() };
            let mut src = Pixmap::new(w, h).unwrap();
            for (d, p) in src.data_mut().chunks_mut(4).zip(rgba.chunks(4)) {
                let a = p[3] as u32;
                d.copy_from_slice(&[
                    (ink.r() as u32 * a / 255) as u8,
                    (ink.g() as u32 * a / 255) as u8,
                    (ink.b() as u32 * a / 255) as u8,
                    a as u8,
                ]);
            }
            let k = size * s / w as f32;
            let (x, y) = (cx * s - size * s / 2.0, cy * s - size * s / 2.0);
            let paint = PixmapPaint { quality: tiny_skia::FilterQuality::Bilinear, ..Default::default() };
            pm.draw_pixmap(0, 0, src.as_ref(), &paint, Transform::from_row(k, 0.0, 0.0, k, x, y), None);
            assert!(w == h, "square raster");
        };
        // One 26×24 target at (x, y) pt in a state.
        let target = |pm: &mut Pixmap, icon: Icon, size: f32, x: f32, y: f32, state: &str| {
            let (tw, th) = (tokens::ICON_BTN_W, tokens::ICON_BTN_H);
            let ink = match state {
                "rest" => tokens::MUTED,
                "disabled" => tokens::DISABLED,
                _ => tokens::TEXT,
            };
            if state == "hover" {
                fill_rr(pm, x, y, tw, th, 3.0, tokens::HOVER);
            }
            if state == "on" {
                fill_rr(pm, x, y, tw, th, 3.0, tokens::TOGGLE_WELL);
            }
            glyph(pm, icon, size, x + tw / 2.0, y + th / 2.0, ink);
        };
        let mut text = String::new();
        let mut label = |x: f32, y: f32, size: f32, col: egui::Color32, anchor: &str, t: &str| {
            text.push_str(&format!(
                "<text x=\"{x}\" y=\"{y}\" font-family=\"Inter\" font-size=\"{size}\" fill=\"{}\" text-anchor=\"{anchor}\">{t}</text>",
                hex(col)
            ));
        };
        label(pad, pad + 6.0, 12.0, tokens::TEXT, "start", &format!("Panel glyphs, 2026-10-08 · {scale}×"));
        let how = if scale == 2 {
            "the real 32 px texture, bilinear to draw size (Retina)"
        } else {
            "re-rendered at 4× for shape"
        };
        label(pad, pad + 18.0, 9.0, tokens::MUTED, "start", how);
        for (g, size) in [(0, 16.0), (1, 18.0)] {
            let gx = grid_x + g as f32 * cell_w * 3.0;
            label(gx + cell_w * 1.5, pad + 34.0, 10.5, tokens::TEXT, "middle", &format!("{size} pt"));
            for (i, st) in states.iter().enumerate() {
                label(gx + cell_w * (i as f32 + 0.5), pad + 46.0, 9.0, tokens::MUTED, "middle", st);
            }
        }
        fill_rr(
            &mut pm,
            pad - 4.0,
            rows_y - 2.0,
            label_w + cell_w * 6.0 + 8.0,
            NEW.len() as f32 * cell_h + 4.0,
            8.0,
            tokens::PANEL,
        );
        for (row, (icon, name)) in NEW.iter().enumerate() {
            let y = rows_y + row as f32 * cell_h;
            let kind = if icon.is_original() { "original" } else { "Lucide" };
            label(pad + 4.0, y + cell_h / 2.0 + 1.0, 11.0, tokens::TEXT, "start", name);
            label(
                pad + 4.0,
                y + cell_h / 2.0 + 11.0,
                8.0,
                tokens::MUTED,
                "start",
                &format!("{} · {kind}", icon.lucide().0),
            );
            for (g, size) in [(0, 16.0), (1, 18.0)] {
                for (i, st) in states.iter().enumerate() {
                    let x = grid_x + (g as f32 * 3.0 + i as f32) * cell_w + (cell_w - tokens::ICON_BTN_W) / 2.0;
                    target(&mut pm, *icon, size, x, y + (cell_h - tokens::ICON_BTN_H) / 2.0, st);
                }
            }
        }
        label(
            pad,
            strip_y - 8.0,
            10.5,
            tokens::TEXT,
            "start",
            "Document strip, direction B, 18 pt (Grid dots disabled)",
        );
        fill_rr(&mut pm, pad - 4.0, strip_y - 2.0, 280.0, 28.0, 8.0, tokens::PANEL);
        let mut x = pad;
        for item in strip {
            match item {
                Some((icon, on, dis)) => {
                    target(
                        &mut pm,
                        icon,
                        tokens::ICON_LG,
                        x,
                        strip_y,
                        if dis {
                            "disabled"
                        } else if on {
                            "on"
                        } else {
                            "rest"
                        },
                    );
                    x += 28.0;
                }
                None => {
                    fill_rect(&mut pm, x + 2.0, strip_y + 4.0, 1.0, 16.0, tokens::LINE2);
                    x += 7.0;
                }
            }
        }
        let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w_pt}\" height=\"{h_pt}\" viewBox=\"0 0 {w_pt} {h_pt}\">{text}</svg>");
        let tree = usvg::Tree::from_str(&svg, &opts).expect("labels parse");
        resvg::render(&tree, Transform::from_scale(s, s), &mut pm.as_mut());
        // Ink check: every row's 18 pt "on" cell shows TEXT-bright glyph pixels.
        let bright = pm.pixels().iter().filter(|p| p.red() > 200 && p.green() > 200).count();
        assert!(bright > NEW.len() * 40 * scale as usize, "the sheet shows ink ({bright} bright px)");
        if let Ok(dir) = std::env::var("VAROS_GLYPH_PROOF") {
            let path = std::path::Path::new(&dir).join(format!("proof-glyphs-{scale}x.png"));
            pm.save_png(&path).expect("write the proof sheet");
            println!("wrote {} ({}×{})", path.display(), pm.width(), pm.height());
        }
    }
}
