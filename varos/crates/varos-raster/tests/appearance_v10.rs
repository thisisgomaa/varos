//! Lane A: opaque-colour alpha masks, ordered paints and nested row looks have SVG/CPU parity.
use std::sync::{atomic::AtomicBool, Arc};
use varos_core::{
    appearance::Look,
    appearance_edits::{AppearanceEdit as A, MaskEdit as M},
    model::{Anchor, Artboard, Document, Paint, Path},
    EditCommand, Editor,
};
fn path(id: u32, x: f32, size: f32, colour: [f32; 4]) -> Path {
    Path::new(
        id,
        [[x, 0.], [x + size, 0.], [x + size, size], [x, size]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: id * 10 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some(colour),
        None,
        1.,
    )
}
fn editor() -> Editor {
    let mut d = Document::default();
    d.artboards.push(Artboard { w: 64., h: 64., page_color: None, ..Default::default() });
    d.paths = vec![path(10, 0., 40., [1., 0., 0., 1.]), path(20, 10., 20., [0., 0., 0., 0.5])];
    d.ids = 1000;
    d.sync_tree();
    let mut ed = Editor::new();
    ed.replace_doc(d);
    ed
}
fn cpu(ed: &Editor) -> varos_raster::Raster {
    varos_raster::rasterize_artboard_checked(Arc::new(ed.doc.clone()), 0, [64, 64]).unwrap().unwrap()
}
fn svg(ed: &Editor) -> resvg::tiny_skia::Pixmap {
    let plan = varos_core::svg::plan_svg_export(&ed.doc, varos_core::svg::ExportScope::AllVisibleArtboards).unwrap();
    let files = varos_core::svg::export_svg_files(&ed.doc, &plan, &AtomicBool::new(false)).unwrap();
    let tree = resvg::usvg::Tree::from_data(&files[0].bytes, &Default::default()).unwrap();
    let mut px = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut px.as_mut());
    px
}
fn svg_pixel(r: &resvg::tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
    let p = r.pixel(x, y).unwrap();
    [p.red(), p.green(), p.blue(), p.alpha()]
}
fn pixel(r: &varos_raster::Raster, x: usize, y: usize) -> [u8; 4] {
    r.pixels[(y * r.width as usize + x) * 4..(y * r.width as usize + x) * 4 + 4].try_into().unwrap()
}
#[test]
fn alpha_is_colour_independent_and_applied_once_after_paints() {
    let mut ed = editor();
    ed.try_execute(EditCommand::Appearance(A::AddFill { path: 10, paint: Paint::Solid([0., 0., 1., 1.]) })).unwrap();
    let node = ed.doc.node_of_path(10).unwrap();
    let mask = ed.doc.node_of_path(20).unwrap();
    ed.try_execute(EditCommand::Mask(M::Add { node, mask, alpha: true })).unwrap();
    let r = cpu(&ed);
    assert_eq!(pixel(&r, 15, 10), [0, 0, 128, 128]);
    assert_eq!(pixel(&r, 5, 10), [0; 4]);
    let s = svg(&ed);
    assert_eq!(pixel(&r, 15, 10), svg_pixel(&s, 15, 10));
    ed.try_execute(EditCommand::Appearance(A::SetLook { node: 1, look: Some(Look { opacity: 0.5, isolate: true }) }))
        .unwrap();
    let r = cpu(&ed);
    let s = svg(&ed);
    let expected = svg_pixel(&s, 15, 10);
    assert!(pixel(&r, 15, 10).iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 1));
    assert_eq!(pixel(&r, 15, 10)[3], 64);
}
#[test]
fn clip_ignores_mask_alpha_and_nested_masks_keep_intersection() {
    let mut ed = editor();
    let node = ed.doc.node_of_path(10).unwrap();
    let mask = ed.doc.node_of_path(20).unwrap();
    ed.try_execute(EditCommand::Mask(M::Add { node, mask, alpha: false })).unwrap();
    let r = cpu(&ed);
    assert_eq!(pixel(&r, 15, 10), [255, 0, 0, 255]);
    assert_eq!(pixel(&r, 5, 10), [0; 4]);
    let group = ed.doc.node(node).unwrap().parent.unwrap();
    ed.try_execute(EditCommand::Mask(M::Begin { node: group, alpha: true })).unwrap();
    assert!(cpu(&ed).pixels.iter().all(|v| *v == 0));
    ed.doc.paths.push(path(30, 12., 8., [1., 1., 1., 0.5]));
    ed.doc.sync_tree();
    let r = cpu(&ed);
    assert_eq!(pixel(&r, 15, 5), [128, 0, 0, 128]);
    assert_eq!(pixel(&r, 25, 5), [0; 4]);
    let s = svg(&ed);
    assert_eq!(pixel(&r, 15, 5), svg_pixel(&s, 15, 5));
}
