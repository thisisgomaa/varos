//! Complete production meshes, atlas updates, clip rectangles and alpha composition;
//! no engine/raster calls or per-glyph alignment in the reference comparison.
use super::*;
fn pixels(ctx: &egui::Context, out: egui::FullOutput) -> Vec<u8> {
    let ppp = out.pixels_per_point;
    let mut textures = HashMap::<egui::TextureId, egui::ColorImage>::new();
    for (id, delta) in out.textures_delta.set {
        let egui::ImageData::Color(image) = delta.image;
        if let Some([x, y]) = delta.pos {
            let atlas = textures.get_mut(&id).unwrap();
            for row in 0..image.size[1] {
                let start = (y + row) * atlas.size[0] + x;
                atlas.pixels[start..start + image.size[0]]
                    .copy_from_slice(&image.pixels[row * image.size[0]..(row + 1) * image.size[0]]);
            }
        } else {
            textures.insert(id, (*image).clone());
        }
    }
    let width = (160.0 * ppp) as usize;
    let height = (60.0 * ppp) as usize;
    let mut result = vec![0u8; width * height];
    for primitive in ctx.tessellate(out.shapes, ppp) {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else { panic!("unexpected callback") };
        let texture = &textures[&mesh.texture_id];
        // Text meshes contain axis-aligned glyph quads; visit each once (no shared-edge double blend).
        assert_eq!(mesh.vertices.len() % 4, 0);
        for (index, quad) in mesh.vertices.as_chunks::<4>().0.iter().enumerate() {
            let base = index as u32 * 4;
            assert_eq!(
                &mesh.indices[index * 6..index * 6 + 6],
                &[base, base + 1, base + 2, base + 2, base + 1, base + 3]
            );
            let bounds = Rect::from_min_max(quad[0].pos, quad[3].pos);
            let visible = bounds.intersect(primitive.clip_rect);
            for y in 0..height {
                for x in 0..width {
                    let pos = egui::pos2((x as f32 + 0.5) / ppp, (y as f32 + 0.5) / ppp);
                    if pos.x < visible.left()
                        || pos.x >= visible.right()
                        || pos.y < visible.top()
                        || pos.y >= visible.bottom()
                    {
                        continue;
                    }
                    let uv = quad[0].uv + (quad[3].uv - quad[0].uv) * ((pos - bounds.min) / bounds.size());
                    let tx = (uv.x * texture.size[0] as f32).floor() as usize;
                    let ty = (uv.y * texture.size[1] as f32).floor() as usize;
                    let alpha =
                        u32::from(texture.pixels[ty * texture.size[0] + tx].a()) * u32::from(quad[0].color.a()) / 255;
                    let dest = &mut result[y * width + x];
                    *dest = (alpha + u32::from(*dest) * (255 - alpha) / 255) as u8;
                }
            }
        }
    }
    result
}
#[test]
fn latin_label_matches_epaint_pixels() {
    for ppp in [1.0, 2.0] {
        for clipped in [false, true] {
            let render = |mode| {
                let ctx = egui::Context::default();
                crate::shell::fonts::install(&ctx);
                ctx.set_pixels_per_point(ppp);
                let out = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(160.0, 60.0))),
                        ..Default::default()
                    },
                    |ui| {
                        if clipped {
                            ui.set_clip_rect(Rect::from_min_max(egui::pos2(13.0, 9.0), egui::pos2(55.0, 21.0)));
                        }
                        if mode == 0 {
                            label(ui, "Untitled-1", t::small(), Color32::WHITE);
                        } else {
                            let galley = ui.painter().layout_no_wrap("Untitled-1".into(), t::small(), Color32::WHITE);
                            let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                            if mode == 1 {
                                ui.painter().shaped_galley(rect.min, galley, Color32::WHITE);
                            } else {
                                ui.painter().galley(rect.min, galley, Color32::WHITE);
                            }
                        }
                    },
                );
                pixels(&ctx, out)
            };
            let expected = render(2);
            for mode in [0, 1] {
                let actual = render(mode);
                assert!(actual.iter().any(|a| *a > 0));
                let diffs: Vec<_> = actual.iter().zip(&expected).map(|(a, b)| a.abs_diff(*b)).collect();
                let sum: usize = diffs.iter().map(|d| *d as usize).sum();
                let max = *diffs.iter().max().unwrap();
                assert!(
                    sum <= diffs.len() && max <= 32,
                    "{ppp}x mode={mode} clipped={clipped}: mean={} max={max}",
                    sum as f32 / diffs.len() as f32
                );
            }
        }
    }
}
