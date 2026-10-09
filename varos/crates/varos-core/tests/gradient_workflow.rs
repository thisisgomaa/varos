use varos_core::{
    colour_commands::ColourCommand as C,
    editor::{PaintTarget, ToolKind},
    format::{self, Limits},
    geom::View,
    gradient::{Gradient, GradientKind, Stop},
    model::{Paint, ShapeKind},
    palette_io::{self, PaletteFormat},
    swatches::Swatch,
    EditCommand, Editor,
};
fn editor() -> (Editor, u32) {
    let mut e = Editor::new();
    let id = e
        .try_execute_created(EditCommand::AddShape {
            kind: ShapeKind::Rect,
            bounds: [0., 0., 100., 100.],
            parent: None,
            fill: Some([1., 0., 0., 1.]),
            stroke: None,
            stroke_width: 4.,
            opacity: 1.,
            name: None,
        })
        .unwrap();
    e.try_execute(EditCommand::SelectPaths(vec![id])).unwrap();
    (e, id)
}
fn paint(e: &mut Editor, g: Gradient) {
    e.try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: Paint::Gradient(g) })).unwrap();
}
#[test]
fn paint_roundtrips_scene_hit_and_live_signature() {
    let (mut e, id) = editor();
    for kind in [GradientKind::Linear, GradientKind::Radial] {
        let mut g = Gradient { kind, ..Default::default() };
        g.stops[0].midpoint = 0.2;
        g.stops[1].opacity = 0.3;
        paint(&mut e, g.clone());
        assert_eq!(e.path_under([20., 20.]), Some(id));
        let blob = format::encode_model(&e.doc, &Limits::DEFAULT).unwrap();
        assert_eq!(format::decode_model(blob.as_bytes(), None, &Limits::DEFAULT).unwrap().doc, e.doc);
        let a = varos_core::scene::scene_signature(&e, View { pan: [0., 0.], zoom: 1. }, [100, 100]);
        e.try_execute(EditCommand::Colour(C::Begin)).unwrap();
        g.stops[0].colour = [0., 1., 0., 1.];
        e.try_execute(EditCommand::Colour(C::Live { target: PaintTarget::Fill, paint: Paint::Gradient(g) })).unwrap();
        let b = varos_core::scene::scene_signature(&e, View { pan: [0., 0.], zoom: 1. }, [100, 100]);
        assert_ne!(a, b);
        e.try_execute(EditCommand::Colour(C::Cancel)).unwrap();
        assert_eq!(a, varos_core::scene::scene_signature(&e, View { pan: [0., 0.], zoom: 1. }, [100, 100]));
    }
}
#[test]
fn gesture_has_one_undo_and_noop_has_none() {
    let (mut e, _) = editor();
    let original = e.doc.clone();
    e.execute_ui(EditCommand::Colour(C::Tool));
    assert!(e.tool == ToolKind::Gradient);
    let rev = e.rev;
    e.pointer_down([10., 10.]);
    for x in 20..60 {
        e.pointer_move([x as f32, 20.]);
    }
    e.pointer_up();
    assert_eq!(e.rev, rev + 1);
    assert!(matches!(e.doc.paths[0].fill, Paint::Gradient(_)));
    e.undo();
    assert_eq!(e.doc, original);
    let rev = e.rev;
    e.pointer_down([10., 10.]);
    e.pointer_up();
    assert_eq!(e.rev, rev);
}
#[test]
fn global_swatches_update_and_deletion_materializes_with_undo() {
    let (mut e, _) = editor();
    let s = Swatch {
        id: 1,
        name: "Ink".into(),
        paint: Paint::Gradient(Gradient::default()),
        global: true,
        group: "Brand".into(),
    };
    e.try_execute(EditCommand::Colour(C::UpsertSwatch { swatch: s.clone() })).unwrap();
    e.try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: Paint::SwatchRef { id: 1 } }))
        .unwrap();
    let mut updated = s;
    updated.paint = Paint::Solid([0., 1., 0., 1.]);
    e.try_execute(EditCommand::Colour(C::UpsertSwatch { swatch: updated.clone() })).unwrap();
    assert_eq!(e.doc.paths[0].fill.resolved(&e.doc), updated.paint);
    e.try_execute(EditCommand::Colour(C::DeleteSwatch { id: 1 })).unwrap();
    assert_eq!(e.doc.paths[0].fill, updated.paint);
    e.undo();
    assert_eq!(e.doc.paths[0].fill, Paint::SwatchRef { id: 1 });
    assert!(e
        .try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: Paint::SwatchRef { id: 99 } }))
        .is_err());
}
#[test]
fn clipboard_duplicate_pathfinder_and_eyedropper_carry_owned_paint() {
    let (mut e, id) = editor();
    let g = Gradient::default();
    paint(&mut e, g.clone());
    let copied = varos_core::clipboard::Clipboard::capture(&e.doc, &[id]);
    let ids = copied.paste_into(&mut e.doc, [0., 0.]);
    let i = e.doc.pidx(ids[0]).unwrap();
    assert_eq!(e.doc.paths[i].fill, Paint::Gradient(g.clone()));
    e.try_execute(EditCommand::SelectPaths(ids.clone())).unwrap();
    e.try_execute(EditCommand::Eyedropper { source: id, options: Default::default(), colour_only: false }).unwrap();
    assert_eq!(e.doc.paths[i].fill, Paint::Gradient(g));
    e.try_execute(EditCommand::SelectPaths(vec![id, ids[0]])).unwrap();
    e.try_execute(EditCommand::Boolean(varos_core::BoolOp::Unite)).unwrap();
    assert!(e.doc.paths.iter().all(|p| matches!(p.fill, Paint::Gradient(_))));
}
#[test]
fn palettes_groups_and_global_ase_gpl_roundtrip_and_refusals() {
    let palette = palette_io::library();
    for f in [PaletteFormat::Native, PaletteFormat::Gpl, PaletteFormat::Ase] {
        let encoded = palette_io::encode(&palette, f).unwrap();
        let read = palette_io::decode(&encoded, f).unwrap();
        assert_eq!(read, palette);
    }
    let mut global = palette[0].clone();
    global.global = true;
    assert_eq!(
        palette_io::decode(&palette_io::encode(&[global.clone()], PaletteFormat::Ase).unwrap(), PaletteFormat::Ase)
            .unwrap(),
        vec![global]
    );
    assert!(palette_io::decode(b"ASEF", PaletteFormat::Ase).is_err());
    let gradient = Swatch {
        id: 1,
        name: "Gradient".into(),
        group: String::new(),
        global: false,
        paint: Paint::Gradient(Gradient::default()),
    };
    assert!(palette_io::encode(&[gradient], PaletteFormat::Gpl).is_err());
}
#[test]
fn recolor_maps_gradient_stops_preserving_geometry_opacity_and_one_undo() {
    let (mut e, _) = editor();
    let g = Gradient {
        stops: vec![Stop::new(0., [1., 0., 0., 0.3]), Stop::new(1., [0., 0., 1., 0.8])],
        ..Default::default()
    };
    paint(&mut e, g);
    let before = e.doc.clone();
    let rev = e.rev;
    e.try_execute(EditCommand::Colour(C::Recolor { palette: vec![[0., 1., 0., 1.]] })).unwrap();
    assert_eq!(e.rev, rev + 1);
    let Paint::Gradient(g) = &e.doc.paths[0].fill else { panic!() };
    assert_eq!(g.stops[0].colour, [0., 1., 0., 0.3]);
    assert_eq!(e.doc.paths[0].anchors, before.paths[0].anchors);
    e.undo();
    assert_eq!(e.doc, before);
    assert_eq!(
        varos_core::recolor::cluster(&[[1., 0., 0., 1.], [0.99, 0., 0., 1.], [0., 0., 1., 1.]], &[], 2).len(),
        2
    );
}
#[test]
fn locked_targets_and_malformed_stops_refused_atomically() {
    let (mut e, id) = editor();
    let mut g = Gradient::default();
    g.stops[0].offset = f32::NAN;
    let before = e.doc.clone();
    assert!(e
        .try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: Paint::Gradient(g) }))
        .is_err());
    assert_eq!(e.doc, before);
    let leaf = e.doc.node_of_path(id).unwrap();
    e.doc.nodes.iter_mut().find(|n| n.id == leaf).unwrap().locked = true;
    assert!(e
        .try_execute(EditCommand::Colour(C::Paint {
            target: PaintTarget::Fill,
            paint: Paint::Gradient(Gradient::default())
        }))
        .is_err());
}

#[test]
fn annotator_stop_midpoint_radial_handles_and_escape() {
    let (mut e, _) = editor();
    paint(&mut e, Gradient::default());
    e.execute_ui(EditCommand::Colour(C::Tool));
    let before = e.doc.clone();
    let rev = e.rev;
    // Offset stop handles have their own row, distinct from the placement endpoints.
    e.pointer_down([0., 12.]);
    e.pointer_move([20., 12.]);
    e.pointer_up();
    let Paint::Gradient(g) = &e.doc.paths[0].fill else { panic!() };
    assert!((g.stops[0].offset - 0.2).abs() < 1e-5);
    assert_eq!(e.rev, rev + 1);
    e.undo();
    assert_eq!(e.doc, before);
    e.pointer_down([50., -12.]);
    e.pointer_move([30., -12.]);
    e.pointer_up();
    let Paint::Gradient(g) = &e.doc.paths[0].fill else { panic!() };
    assert!((g.stops[0].midpoint - 0.3).abs() < 1e-5);
    paint(&mut e, Gradient { kind: GradientKind::Radial, ..Default::default() });
    e.pointer_down([0., 100.]);
    e.pointer_move([0., 60.]);
    e.pointer_up();
    let Paint::Gradient(g) = &e.doc.paths[0].fill else { panic!() };
    assert!((g.placement[3] - 60.).abs() < 1e-5);
    e.mods.alt = true;
    e.pointer_down([0., 0.]);
    e.pointer_move([20., 12.]);
    e.pointer_up();
    e.mods.alt = false;
    let Paint::Gradient(g) = &e.doc.paths[0].fill else { panic!() };
    assert!((g.focal[0] - 0.2).abs() < 1e-5 && (g.focal[1] - 0.2).abs() < 1e-5);
    let before = e.doc.clone();
    let rev = e.rev;
    e.pointer_down([100., 0.]);
    e.pointer_move([150., 0.]);
    e.escape();
    e.pointer_up();
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
    assert!(!e.transaction_open());
}
#[test]
fn duplicate_global_paint_and_geometry_transform_keep_owned_placement() {
    let (mut e, id) = editor();
    let g = Gradient::default();
    e.try_execute(EditCommand::Colour(C::UpsertSwatch {
        swatch: Swatch {
            id: 1,
            name: "Global gradient".into(),
            paint: Paint::Gradient(g.clone()),
            global: true,
            group: String::new(),
        },
    }))
    .unwrap();
    e.try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: Paint::SwatchRef { id: 1 } }))
        .unwrap();
    let copied = e.doc.dup_paths(&[id]);
    let i = e.doc.pidx(copied[0]).unwrap();
    assert_eq!(e.doc.paths[i].fill, Paint::Gradient(g.clone()));
    e.try_execute(EditCommand::SelectPaths(copied)).unwrap();
    e.flip(true);
    let Paint::Gradient(flipped) = &e.doc.paths[i].fill else { panic!() };
    assert!((flipped.sample_point([100., 20.])[0] - g.sample_point([0., 20.])[0]).abs() < 1e-5);
    let before = e.doc.clone();
    e.set_obj_bbox(Some(200.), None, Some(200.), None, 0., 0.);
    let Paint::Gradient(mapped) = &e.doc.paths[i].fill else { panic!() };
    assert_eq!(mapped.placement[0], -200.);
    e.undo();
    assert_eq!(e.doc, before);
}

#[test]
fn global_live_paint_invalidates_scene_even_without_a_revision() {
    let (mut e, _) = editor();
    e.try_execute(EditCommand::Colour(C::UpsertSwatch {
        swatch: Swatch {
            id: 1,
            name: "Ink".into(),
            paint: Paint::Gradient(Gradient::default()),
            global: true,
            group: String::new(),
        },
    }))
    .unwrap();
    e.try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: Paint::SwatchRef { id: 1 } }))
        .unwrap();
    let a = varos_core::scene::scene_signature(&e, View::identity(), [100, 100]);
    let Paint::Gradient(g) = &mut e.doc.swatches[0].paint else { panic!() };
    g.stops[0].midpoint = 0.2;
    assert_ne!(a, varos_core::scene::scene_signature(&e, View::identity(), [100, 100]));
}

#[test]
fn reference_to_none_retains_empty_fill_hit_semantics() {
    let (mut e, _) = editor();
    e.try_execute(EditCommand::Colour(C::UpsertSwatch {
        swatch: Swatch { id: 1, name: "None".into(), paint: Paint::None, global: true, group: String::new() },
    }))
    .unwrap();
    e.try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: Paint::SwatchRef { id: 1 } }))
        .unwrap();
    assert_eq!(e.path_under([50., 50.]), None);
}
