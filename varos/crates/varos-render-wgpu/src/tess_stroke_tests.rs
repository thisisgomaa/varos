//! Filled stroke triangulation oracles; no Renderer, device or EventLoop is constructed.
use super::*;
#[test]
fn every_frozen_styled_outline_triangulates_without_covering_holes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v5");
    for name in std::fs::read_to_string(root.join("INDEX")).unwrap().lines() {
        let doc = varos_core::format::decode_model(
            &std::fs::read(root.join(format!("{name}.json"))).unwrap(),
            None,
            &varos_core::format::Limits::DEFAULT,
        )
        .unwrap()
        .doc;
        let p = &doc.paths[0];
        if p.stroke_style.is_default() {
            continue;
        }
        let coverage = varos_core::stroke::evaluate(p, 0.01, &|| false).unwrap();
        let prim = Prim::StrokeCoverage {
            rings: coverage.rings.clone(),
            color: [1.0, 0.0, 0.0, 1.0],
            clip: None,
            native: None,
        };
        let vertices = build_fg(&[prim], View::identity(), 1.0, 200.0, 200.0);
        let triangles: Vec<[[f32; 2]; 3]> = vertices
            .as_chunks::<3>()
            .0
            .iter()
            .map(|tri| std::array::from_fn(|i| [(tri[i].pos[0] + 1.0) * 100.0, (1.0 - tri[i].pos[1]) * 100.0]))
            .collect();
        let contains = |q: Pt, t: &[Pt; 3]| {
            let cross = |a: Pt, b: Pt| (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0]);
            let c = [cross(t[0], t[1]), cross(t[1], t[2]), cross(t[2], t[0])];
            let area = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[1][1] - t[0][1]) * (t[2][0] - t[0][0]);
            area.abs() > 1e-6 && (c.iter().all(|v| *v >= 0.0) || c.iter().all(|v| *v <= 0.0))
        };
        for x in (-20..160).step_by(3) {
            for y in (-20..140).step_by(3) {
                let q = [x as f32 + 0.137, y as f32 + 0.219];
                assert_eq!(
                    triangles.iter().any(|t| contains(q, t)),
                    varos_core::stroke::evaluate::contains(&coverage.rings, q, 0.0),
                    "{name}: {q:?}"
                );
            }
        }
    }
}
#[test]
fn default_style_preserves_scene_and_draw_buffers() {
    let mut ed = varos_core::Editor::new();
    ed.replace_doc(
        varos_core::format::decode_model(
            include_bytes!("../../varos-core/tests/fixtures/v5/plain.json"),
            None,
            &varos_core::format::Limits::DEFAULT,
        )
        .unwrap()
        .doc,
    );
    let before = varos_core::build_scene(&ed, 1.0);
    ed.doc.paths[0].stroke_style = varos_core::stroke::StrokeStyle::default();
    let after = varos_core::build_scene(&ed, 1.0);
    assert_eq!(before.content, after.content);
    let (a, b, c, draws) = build_content(&before.content, View::identity(), 1.0, 200.0, 200.0);
    let (x, y, z, other) = build_content(&after.content, View::identity(), 1.0, 200.0, 200.0);
    assert_eq!(bytemuck::cast_slice::<Vertex, u8>(&a), bytemuck::cast_slice::<Vertex, u8>(&x));
    assert_eq!(bytemuck::cast_slice::<Vertex, u8>(&b), bytemuck::cast_slice::<Vertex, u8>(&y));
    assert_eq!(bytemuck::cast_slice::<Vertex, u8>(&c), bytemuck::cast_slice::<Vertex, u8>(&z));
    assert_eq!(draws, other);
}
