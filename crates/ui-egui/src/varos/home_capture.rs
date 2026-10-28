//! Deterministic software capture of the real Home surface (no native window required).
use super::*;
use std::collections::HashMap;

#[test]
#[ignore = "visual evidence; set VAROS_HOME_CAPTURE_SIMPLE to the output PNG"]
fn home_capture_simple() {
    let Some(output) = std::env::var_os("VAROS_HOME_CAPTURE_SIMPLE").map(std::path::PathBuf::from) else { return };
    let root = std::env::temp_dir().join(format!("varos-home-capture-{}", std::process::id()));
    let projects = [root.join("Ramadan campaign"), root.join("Client — Nile Studio")];
    for path in &projects {
        std::fs::create_dir_all(path).unwrap();
    }
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    crate::theme::apply(&ctx, crate::theme::Brightness::Dark);
    egui_extras::install_image_loaders(&ctx);
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
    app.ui.home_preferences.projects = projects.to_vec();
    let tokens = Tokens::get(&ctx);
    let color = |c: Color32| format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
    for (i, name) in ["Poster-A.varos", "Logo.varos", "غلاف الكتاب.varos", "Brand-sheet.varos"].iter().enumerate() {
        app.run("file.new", json!({"width":600,"height":450})).unwrap();
        app.run("shape.rectangle", json!({"x":40,"y":40,"width":250,"height":330})).unwrap();
        app.run("paint.setFill", json!({"color":color(tokens.button)})).unwrap();
        app.run("stroke.set", json!({"weight":0})).unwrap();
        app.run("shape.ellipse", json!({"x":340,"y":130,"width":120,"height":120})).unwrap();
        app.run("paint.setFill", json!({"color":color(tokens.accent)})).unwrap();
        app.run("stroke.set", json!({"weight":0})).unwrap();
        let path = projects[i % 2].join(name);
        let doc = &app.session.active().unwrap().doc;
        std::fs::write(&path, vectorcraft_format::save(doc, false)).unwrap();
        let age = [120, 86400, 4 * 86400, 7 * 86400][i];
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(std::time::SystemTime::now() - std::time::Duration::from_secs(age)))
            .unwrap();
        cache_document(path.to_str().unwrap());
        app.ui.recent_files.push(path.to_string_lossy().into_owned());
    }
    app.run("app.home", json!({})).unwrap();
    app.ui.status.clear();
    let minute = vectorcraft_doc::metadata::now_unix().unwrap() / 60;
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("home-identity"), (minute, "Ahmed".to_string(), 19_u32)));
    let mut textures = HashMap::<egui::TextureId, egui::ColorImage>::new();
    for frame in 0..4 {
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(1440.0, 900.0))),
            max_texture_side: Some(8192),
            time: Some(frame as f64 / 60.0),
            ..Default::default()
        };
        let mut out = ctx.run_ui(raw, |ui| show(&mut app, ui, Rect::from_min_size(egui::Pos2::ZERO, vec2(1440.0, 900.0))));
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
        out.textures_delta.clear();
        if frame == 3 {
            let meshes = ctx.tessellate(out.shapes, out.pixels_per_point);
            let pixels = raster(&meshes, &textures, 1440, 900);
            if let Some(parent) = output.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            image::save_buffer(&output, &pixels, 1440, 900, image::ColorType::Rgba8).unwrap();
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}

pub(crate) fn raster(meshes: &[egui::ClippedPrimitive], textures: &HashMap<egui::TextureId, egui::ColorImage>, w: u32, h: u32) -> Vec<u8> {
    let mut pixels = vec![[0.0_f32; 4]; (w * h) as usize];
    for primitive in meshes {
        let egui::epaint::Primitive::Mesh(mesh) = &primitive.primitive else { continue };
        let Some(texture) = textures.get(&mesh.texture_id) else { continue };
        for triangle in mesh.indices.as_chunks::<3>().0 {
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
