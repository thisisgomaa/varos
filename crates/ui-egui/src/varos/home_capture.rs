//! Test-only software capture of egui's real tessellated output when WindowServer is unavailable.
//! Not native window acceptance; the optional PNGs are inspection evidence only.
use super::*;
use std::collections::HashMap;

#[test]
#[ignore = "optional software visual capture; set VAROS_HOME_CAPTURE_DIR"]
fn home_capture_at_desktop_and_compact_widths() {
    let Some(root) = std::env::var_os("VAROS_HOME_CAPTURE_DIR").map(std::path::PathBuf::from) else { return };
    std::fs::create_dir_all(&root).unwrap();
    for (width, height) in [(1280, 800), (800, 800)] {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
        egui_extras::install_image_loaders(&ctx);
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.fonts_ready = true;
        app.canvas.worker_started = true;
        app.custom_titlebar = true;
        let clients = root.join("Documents").join("Clients");
        std::fs::create_dir_all(&clients).unwrap();
        let recent = clients.join("Ramadan poster.vectorcraft");
        std::fs::write(&recent, vectorcraft_format::save(&vectorcraft_doc::Document::new(100.0, 100.0), false)).unwrap();
        app.ui.recent_files = vec![recent.to_string_lossy().into_owned(), "/private/tmp/Exploration/Icon set v3.svg".into()];
        let mut textures = HashMap::<egui::TextureId, egui::ColorImage>::new();
        let mut hover = None;
        for frame in 0..3 {
            let raw = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(width as f32, height as f32))),
                max_texture_side: Some(8192),
                events: hover.map(|p| vec![egui::Event::PointerMoved(p)]).unwrap_or_default(),
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| app.ui(ui));
            for (id, deltas) in &out.textures_delta.set {
                for delta in deltas {
                    let egui::ImageData::Color(image) = &delta.image;
                    if let Some([x, y]) = delta.pos {
                        let target = textures.get_mut(id).unwrap();
                        for row in 0..image.size[1] {
                            for col in 0..image.size[0] {
                                target.pixels[(y + row) * target.size[0] + x + col] = image.pixels[row * image.size[0] + col];
                            }
                        }
                    } else {
                        textures.insert(*id, (**image).clone());
                    }
                }
            }
            out.textures_delta.clear(); // Texture updates above are applied to the software atlas.
            hover = out.shapes.iter().find_map(|s| match &s.shape {
                egui::Shape::Text(t) if t.galley.text() == "Post" => Some(t.visual_bounding_rect().center() - vec2(0.0, 45.0)),
                _ => None,
            });
            if frame == 2 {
                let meshes = ctx.tessellate(out.shapes, out.pixels_per_point);
                let pixels = raster(&meshes, &textures, width, height);
                image::save_buffer(root.join(format!("home-{width}.png")), &pixels, width, height, image::ColorType::Rgba8).unwrap();
            }
        }
    }
}

fn raster(meshes: &[egui::ClippedPrimitive], textures: &HashMap<egui::TextureId, egui::ColorImage>, w: u32, h: u32) -> Vec<u8> {
    let mut pixels = vec![[0.0_f32; 4]; (w * h) as usize];
    for primitive in meshes {
        let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive else { continue };
        let Some(texture) = textures.get(&mesh.texture_id) else { continue };
        for triangle in mesh.indices.chunks_exact(3) {
            let [a, b, c] = [mesh.vertices[triangle[0] as usize], mesh.vertices[triangle[1] as usize], mesh.vertices[triangle[2] as usize]];
            let edge = |u: egui::Pos2, v: egui::Pos2, p: egui::Pos2| (v.x - u.x) * (p.y - u.y) - (v.y - u.y) * (p.x - u.x);
            let area = edge(a.pos, b.pos, c.pos);
            if area.abs() < 1e-6 {
                continue;
            }
            let min = a.pos.min(b.pos).min(c.pos).max(primitive.clip_rect.min).max(egui::Pos2::ZERO);
            let max = a.pos.max(b.pos).max(c.pos).min(primitive.clip_rect.max).min(pos2(w as f32, h as f32));
            for y in min.y.floor() as u32..max.y.ceil() as u32 {
                for x in min.x.floor() as u32..max.x.ceil() as u32 {
                    let at = pos2(x as f32 + 0.5, y as f32 + 0.5);
                    let weights = [edge(b.pos, c.pos, at) / area, edge(c.pos, a.pos, at) / area, edge(a.pos, b.pos, at) / area];
                    if weights.iter().any(|k| *k < 0.0) {
                        continue;
                    }
                    let uv = a.uv.to_vec2() * weights[0] + b.uv.to_vec2() * weights[1] + c.uv.to_vec2() * weights[2];
                    let tx = (uv.x * texture.size[0] as f32).floor().clamp(0.0, texture.size[0].saturating_sub(1) as f32) as usize;
                    let ty = (uv.y * texture.size[1] as f32).floor().clamp(0.0, texture.size[1].saturating_sub(1) as f32) as usize;
                    let tex = texture.pixels[ty * texture.size[0] + tx].to_array();
                    let src: [f32; 4] = std::array::from_fn(|i| {
                        (a.color.to_array()[i] as f32 * weights[0]
                            + b.color.to_array()[i] as f32 * weights[1]
                            + c.color.to_array()[i] as f32 * weights[2])
                            * tex[i] as f32
                            / (255.0 * 255.0)
                    });
                    let dst = &mut pixels[(y * w + x) as usize];
                    for i in 0..4 {
                        dst[i] = src[i] + dst[i] * (1.0 - src[3]);
                    }
                }
            }
        }
    }
    pixels.into_iter().flat_map(|p| p.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8)).collect()
}
