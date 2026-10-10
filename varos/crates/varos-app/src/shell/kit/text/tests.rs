use super::*;
use varos_text::{CaretMove, Issue};
#[test]
fn arabic_name_shapes_joined_clusters() {
    let ctx = egui::Context::default();
    let a = layout(&ctx, "لوحة أولى", &t::small(), None, false).expect("bundled font metadata");
    assert_eq!(a.layout.source, "لوحة أولى");
    assert!(!a.layout.issues.iter().any(|i| matches!(i, Issue::UnsupportedCluster(_))));
    assert!(a.layout.lines[0].rtl);
    assert!(a.layout.lines[0]
        .glyphs
        .iter()
        .all(|g| g.face == FaceId(3) || g.cluster.clone().any(|i| a.layout.source.as_bytes()[i] == b' ')));
    let shared = system(&ctx).unwrap();
    let mut s = shared.lock().unwrap();
    let isolated: Vec<_> = "لوحة أولى"
        .chars()
        .filter(|c| !c.is_whitespace())
        .flat_map(|c| {
            s.engine
                .layout(&Request::new(&c.to_string(), t::small().size, None))
                .unwrap()
                .lines
                .into_iter()
                .flat_map(|l| l.glyphs.into_iter().map(|g| g.id))
        })
        .collect();
    let joined: Vec<_> = a.layout.lines[0].glyphs.iter().map(|g| g.id).collect();
    assert_ne!(joined, isolated);
}
#[test]
fn rtl_caret_moves_by_grapheme() {
    let ctx = egui::Context::default();
    let a = layout(&ctx, "السَّلَامُ Logo 12", &t::small(), None, false).unwrap();
    let boundaries: Vec<_> =
        a.layout.source.grapheme_indices(true).map(|(i, _)| i).chain([a.layout.source.len()]).collect();
    for c in &a.layout.carets {
        for motion in [CaretMove::Left, CaretMove::Right, CaretMove::WordLeft, CaretMove::WordRight] {
            if let Some(next) = a.layout.caret_move(c, motion) {
                assert!(boundaries.contains(&next.byte));
            }
        }
    }
}
#[test]
fn rtl_name_elides_at_logical_end_on_grapheme() {
    let ctx = egui::Context::default();
    let source = "السَّلَامُ عليكم لوحة أولى";
    let a = layout(&ctx, source, &t::small(), Some(50.0), true).unwrap();
    assert!(a.layout.source.ends_with('…'));
    assert!(a.size().x <= 50.0);
    let end = a.layout.source.len() - '…'.len_utf8();
    assert!(source.grapheme_indices(true).any(|(i, _)| i == end));
}
#[test]
fn baseline_is_content_independent_and_layout_is_retained() {
    let ctx = egui::Context::default();
    let a = layout(&ctx, "Untitled-1", &t::small(), None, false).unwrap();
    let b = layout(&ctx, "لوحة أولى", &t::small(), None, false).unwrap();
    assert_eq!(a.layout.lines[0].baseline, b.layout.lines[0].baseline);
    let c = layout(&ctx, "لوحة أولى", &t::small(), None, false).unwrap();
    assert!(Arc::ptr_eq(&b.layout, &c.layout));
}
#[test]
fn mesh_uses_shared_atlas_and_idle_frame_uploads_nothing() {
    let ctx = egui::Context::default();
    crate::shell::fonts::install(&ctx);
    let input = || egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(300.0, 100.0))),
        ..Default::default()
    };
    let draw = |ui: &mut egui::Ui| {
        label(ui, "لوحة — Café logo", t::small(), t::TEXT);
    };
    let first = ctx.run_ui(input(), draw);
    assert!(!first.textures_delta.set.is_empty());
    let second = ctx.run_ui(input(), draw);
    assert!(second.shapes.iter().any(|s| matches!(s.shape, egui::Shape::Mesh(_))));
    assert!(second.textures_delta.set.is_empty());
}

#[test]
fn cpu_proof_has_connected_arabic_and_mixed_runs() {
    let ctx = egui::Context::default();
    let shared = system(&ctx).expect("font snapshot");
    let mut proof = image::RgbaImage::from_pixel(1000, 240, image::Rgba([20, 19, 19, 255]));
    let mut ink = 0;
    for (row, text) in ["لوحة أولى", "لوحة — Café logo", "السَّلَامُ Logo 123"].iter().enumerate()
    {
        let label = layout(&ctx, text, &egui::FontId::proportional(26.0), None, false).unwrap();
        let s = shared.lock().unwrap();
        for g in label.layout.lines.iter().flat_map(|l| &l.glyphs) {
            let px = ((g.x + g.offset[0]) * 4.0).round() as i64;
            if let Some(bitmap) = raster::raster(
                &s.engine.font_set().face(g.face).unwrap().bytes,
                g.id,
                g.size,
                px.rem_euclid(4) as u8,
                Default::default(),
            ) {
                let ox = 30 + px.div_euclid(4) as i32 + bitmap.offset.x as i32;
                let oy = 10 + row as i32 * 70 + (g.y + g.offset[1]).round() as i32 + bitmap.offset.y as i32;
                for (i, c) in bitmap.image.pixels.iter().enumerate() {
                    let x = ox + (i % bitmap.image.size[0]) as i32;
                    let y = oy + (i / bitmap.image.size[0]) as i32;
                    if (0..1000).contains(&x) && (0..240).contains(&y) {
                        let a = u32::from(c.a());
                        if a > 0 {
                            ink += 1;
                        }
                        let pixel = proof.get_pixel_mut(x as u32, y as u32);
                        for channel in 0..3 {
                            pixel[channel] =
                                ((u32::from(t::TEXT[channel]) * a + u32::from(pixel[channel]) * (255 - a)) / 255) as u8;
                        }
                    }
                }
            }
        }
    }
    assert!(ink > 1000);
    if let Ok(path) = std::env::var("VAROS_UI_TEXT_PROOF") {
        proof.save(path).unwrap();
    }
}

#[test]
fn latin_glyph_raster_matches_epaint_pixels_at_one_and_two_x() {
    for ppp in [1.0, 2.0] {
        let ctx = egui::Context::default();
        crate::shell::fonts::install(&ctx);
        ctx.set_pixels_per_point(ppp);
        let mut reference = None;
        let output = ctx.run_ui(Default::default(), |ui| {
            reference = Some(ui.painter().layout_no_wrap("Untitled-1".into(), t::small(), t::TEXT));
        });
        let galley = reference.unwrap();
        let atlas = output
            .textures_delta
            .set
            .iter()
            .find(|(id, _)| *id == egui::TextureId::Managed(0))
            .expect("egui font atlas");
        let egui::ImageData::Color(image) = &atlas.1.image;
        let shared = system(&ctx).unwrap();
        let mut s = shared.lock().unwrap();
        let shaped = s.engine.layout(&Request::new("Untitled-1", t::small().size, None)).unwrap();
        for (glyph, g) in galley.rows[0].glyphs.iter().zip(&shaped.lines[0].glyphs) {
            let uv = glyph.uv_rect;
            let width = (uv.max[0] - uv.min[0]) as usize;
            let height = (uv.max[1] - uv.min[1]) as usize;
            if width == 0 || height == 0 {
                continue;
            }
            let bin = ((((g.x + g.offset[0]) * ppp * 4.0).round() as i64).rem_euclid(4)) as u8;
            let bitmap = raster::raster(
                &s.engine.font_set().face(g.face).unwrap().bytes,
                g.id,
                g.size * ppp,
                bin,
                ctx.style_of(ctx.theme()).visuals.text_options,
            )
            .unwrap();
            assert_eq!(bitmap.image.size, [width, height], "{} at {ppp}x", glyph.chr);
            let mut total = 0_u64;
            let mut max = 0_u8;
            for y in 0..height {
                for x in 0..width {
                    let expected = image.pixels[(uv.min[1] as usize + y) * image.size[0] + uv.min[0] as usize + x].a();
                    let difference = expected.abs_diff(bitmap.image.pixels[y * width + x].a());
                    total += u64::from(difference);
                    max = max.max(difference);
                }
            }
            assert!(total <= (width * height) as u64 && max <= 32, "{} at {ppp}x: sum={total}, max={max}", glyph.chr);
        }
    }
}

#[test]
fn coverage_no_regression_vs_egui_chain() {
    let snapshot = fonts().unwrap();
    for (name, data) in crate::shell::fonts::definitions().font_data {
        assert!(
            snapshot.faces().iter().any(|f| f.bytes.as_ref() == data.font.as_ref()),
            "missing fallback font {name}"
        );
    }
}
#[test]
fn mixed_bidi_visual_order_matches_uba_and_arabic_tracking_is_zero() {
    let ctx = egui::Context::default();
    for text in ["Logo شعار v2", "لوحة — Café logo", "السَّلَامُ"] {
        let label = layout(&ctx, text, &t::small(), None, false).unwrap();
        let shared = system(&ctx).unwrap();
        let mut s = shared.lock().unwrap();
        let direct = s.engine.layout(&Request::new(text, t::small().size, None)).unwrap();
        assert_eq!(label.layout.levels, direct.levels);
        let positions = |layout: &Layout| {
            layout
                .lines
                .iter()
                .flat_map(|l| l.glyphs.iter().map(|g| (g.cluster.clone(), g.level, g.x, g.advance)))
                .collect::<Vec<_>>()
        };
        assert_eq!(positions(&label.layout), positions(&direct));
    }
}

#[test]
fn full_atlas_rolls_to_a_live_page_without_losing_the_glyph() {
    let ctx = egui::Context::default();
    let label = layout(&ctx, "م", &t::small(), None, false).unwrap();
    let shared = system(&ctx).unwrap();
    let mut s = shared.lock().unwrap();
    s.reset(&ctx, 1.0, Default::default());
    let old = s.texture.as_ref().unwrap().id();
    s.y = t::UI_ATLAS_SIDE;
    let entry = s.glyph(&ctx, &label.layout.lines[0].glyphs[0], 0).unwrap();
    assert_ne!(entry.texture, old);
    assert_eq!(s.retired.len(), 1);
    assert_eq!(s.retired[0].id(), old);
    assert!(s.glyph(&ctx, &label.layout.lines[0].glyphs[0], 0).is_some());
}

#[test]
fn authored_catalog_keys_remain_literal_in_arabic_mode() {
    let ctx = egui::Context::default();
    crate::shell::fonts::install(&ctx);
    crate::i18n::set(&ctx, crate::i18n::Locale::Ar);
    enable_trace(&ctx);
    let _ = ctx.run_ui(Default::default(), |ui| {
        ui.shaped_authored_label("Delete");
        ui.shaped_label("Delete");
    });
    let records = paint_records(&ctx);
    assert_eq!(records[0].text, "Delete");
    assert_eq!(records[1].text, "حذف");
}
#[test]
fn galley_row_budget_elides_graphemes_and_clips_cell() {
    let ctx = egui::Context::default();
    crate::shell::fonts::install(&ctx);
    enable_trace(&ctx);
    let source = "السَّلَامُ عليكم لوحة أولى ".repeat(20);
    for rows in [1, 2, 3] {
        let _ = ctx.run_ui(Default::default(), |ui| {
            let mut job = egui::text::LayoutJob::simple(source.clone(), t::small(), t::TEXT, 100.0);
            job.wrap.max_rows = rows;
            let galley = ui.fonts_mut(|f| f.layout_job(job));
            ui.painter().shaped_galley(egui::pos2(10.0, 10.0), galley.clone(), t::TEXT);
        });
        let r = paint_records(&ctx).remove(0);
        assert!(r.lines <= rows);
        assert!(r.elided && r.displayed_text.ends_with('…'));
        let end = r.displayed_text.len() - '…'.len_utf8();
        assert!(source.grapheme_indices(true).any(|(i, _)| i == end));
        assert!(r.rect.height() <= r.clip_rect.height() + t::UI_TEXT_WIDTH_EPSILON);
        assert!(r.clip_rect.width() <= 100.0);
        assert!(r.clip_rect.height() <= rows as f32 * 20.0);
    }
}
