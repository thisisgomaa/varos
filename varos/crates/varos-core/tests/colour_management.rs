use varos_core::{
    colour_management::*,
    colour_management_commands::Command,
    format::{self, Limits},
    model::Paint,
    EditCommand, Editor,
};
#[test]
fn explicit_conversions_boundaries_and_spot_tints() {
    let c = Colour::Cmyk { c: 0.2, m: 0.3, y: 0.4, k: 0.1 };
    let rgb = c.to_srgb(0.7, None).unwrap();
    for (a, b) in rgb.into_iter().zip([0.72, 0.63, 0.54, 0.7]) {
        assert!((a - b).abs() < 1e-6);
    }
    for rgb in [[0.; 4], [1.; 4], [0.9, 0.2, 0.7, 1.]] {
        let Cmyk { c, m, y, k } = Cmyk::from_rgb(rgb);
        let back = Colour::Cmyk { c, m, y, k }.naive_srgb(rgb[3]);
        for (a, b) in rgb.into_iter().zip(back) {
            assert!((a - b).abs() < 1e-6);
        }
    }
    let spot = Colour::Spot { name: "Ink".into(), tint: 0., alt: Cmyk { c: 1., m: 0., y: 0., k: 0. } };
    assert_eq!(spot.naive_srgb(1.), [1.; 4]);
    assert!(Colour::Gray { value: f32::NAN }.validate().is_err());
}
#[test]
fn icc_transform_is_explicit_and_bad_profiles_fail() {
    let p = IccProfile::new("sRGB".into(), &moxcms::ColorProfile::new_srgb().encode().unwrap()).unwrap();
    let rgb = Colour::Rgb { r: 0.2, g: 0.5, b: 0.7 }.to_srgb(0.6, Some(&p)).unwrap();
    for (a, b) in rgb.into_iter().zip([0.2, 0.5, 0.7, 0.6]) {
        assert!((a - b).abs() < 0.001);
    }
    assert!(Colour::Cmyk { c: 0., m: 0., y: 0., k: 1. }.to_srgb(1., Some(&p)).is_err());
    assert!(IccProfile { name: "bad".into(), data: "00".repeat(128) }.parse().is_err());
}
#[test]
fn fixtures_swatch_roundtrip_and_refusals() {
    let input = include_bytes!("fixtures/v12/colours.json");
    let loaded = format::decode_model(input, None, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.doc.colour_mode, ColourMode::Cmyk);
    assert!(matches!(loaded.doc.swatches[0].paint, Paint::Managed(ManagedColour { colour: Colour::Spot { .. }, .. })));
    let encoded = format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap();
    assert_eq!(format::decode_model(encoded.as_bytes(), None, &Limits::DEFAULT).unwrap().doc, loaded.doc);
    for input in [
        include_bytes!("fixtures/v12/refuse-v9.json").as_slice(),
        include_bytes!("fixtures/v12/refuse-v11.json").as_slice(),
        include_bytes!("fixtures/v12/refuse-future.json").as_slice(),
    ] {
        assert!(format::decode_model(input, None, &Limits::DEFAULT).is_err());
    }
}
#[test]
fn rgb_body_is_identical_and_mode_is_undoable() {
    let input = include_str!("fixtures/v5/plain.json");
    let loaded = format::decode_model(input.as_bytes(), None, &Limits::DEFAULT).unwrap();
    assert_eq!(
        format::encode_model(&loaded.doc, &Limits::DEFAULT).unwrap().trim_end(),
        input.replacen("\"varos\":5", "\"varos\":12", 1).trim_end()
    );
    let mut ed = Editor::new();
    ed.replace_doc(loaded.doc);
    let before = ed.doc.clone();
    ed.try_execute(EditCommand::ColourManagement(Command::Mode { mode: ColourMode::Cmyk })).unwrap();
    assert!(!ed.doc.content_eq(&before));
    ed.execute(EditCommand::Undo).unwrap();
    assert!(ed.doc.content_eq(&before));
    assert_eq!(
        varos_core::new_document::Settings { colour_mode: ColourMode::Cmyk, ..Default::default() }
            .document()
            .unwrap()
            .colour_mode,
        ColourMode::Cmyk
    );
}
#[test]
fn proof_overprint_change_scene_identity_without_editing_authored_content() {
    let mut ed = Editor::new();
    ed.replace_doc(
        format::decode_model(include_bytes!("fixtures/v12/colours.json"), None, &Limits::DEFAULT).unwrap().doc,
    );
    let before = ed.doc.clone();
    assert!(ed.try_execute(EditCommand::ColourManagement(Command::Proof { enabled: true })).is_err());
    let view = varos_core::geom::View { pan: [0.; 2], zoom: 1. };
    let signature = varos_core::scene::scene_signature(&ed, view, [100, 100]);
    ed.try_execute(EditCommand::ColourManagement(Command::Overprint { enabled: true })).unwrap();
    assert_ne!(signature, varos_core::scene::scene_signature(&ed, view, [100, 100]));
    assert!(before.content_eq(&ed.doc));
    assert_eq!(varos_core::colour_preview::multiply([0., 1., 1.], [1., 0., 1.], 1.), [0., 0., 1.]);
    assert_eq!(varos_core::colour_preview::multiply([0.2, 0.4, 0.6], [0., 0., 0.], 0.), [0.2, 0.4, 0.6]);
}
#[test]
fn every_swatch_model_retains_source_channels_and_global_identity() {
    let mut doc =
        format::decode_model(include_bytes!("fixtures/v12/colours.json"), None, &Limits::DEFAULT).unwrap().doc;
    for colour in [
        Colour::Rgb { r: 0.1, g: 0.2, b: 0.3 },
        Colour::Cmyk { c: 0.2, m: 0.3, y: 0.4, k: 0.5 },
        Colour::Gray { value: 0.6 },
        Colour::Spot { name: "Brand ink".into(), tint: 0.7, alt: Cmyk { c: 0.1, m: 0.9, y: 0., k: 0.1 } },
    ] {
        doc.swatches[0].paint = Paint::Managed(ManagedColour { colour, alpha: 0.8 });
        let encoded = format::encode_model(&doc, &Limits::DEFAULT).unwrap();
        let back = format::decode_model(encoded.as_bytes(), None, &Limits::DEFAULT).unwrap().doc;
        assert_eq!(back.swatches, doc.swatches);
        assert!(back.swatches[0].global);
    }
}

#[test]
fn profile_and_proof_are_explicit_scene_inputs() {
    let mut ed = Editor::new();
    ed.replace_doc(format::decode_model(include_bytes!("fixtures/v5/plain.json"), None, &Limits::DEFAULT).unwrap().doc);
    let view = varos_core::geom::View { pan: [0.; 2], zoom: 1. };
    let before = varos_core::scene::scene_signature(&ed, view, [100, 100]);
    let p = IccProfile::new("sRGB".into(), &moxcms::ColorProfile::new_srgb().encode().unwrap()).unwrap();
    ed.try_execute(EditCommand::ColourManagement(Command::Profile { profile: Some(p.clone()) })).unwrap();
    assert_ne!(before, varos_core::scene::scene_signature(&ed, view, [100, 100]));
    ed.try_execute(EditCommand::ColourManagement(Command::Proof { enabled: true })).unwrap();
    let c = varos_core::colour_preview::proof_rgb([0.2, 0.3, 0.4, 0.6], &p).unwrap();
    for (a, b) in c.into_iter().zip([0.2, 0.3, 0.4, 0.6]) {
        assert!((a - b).abs() < 0.001);
    }
    let style = varos_core::scene::SceneStyle { checkerboard: [[1.; 4]; 2], outline: [0.; 4], canvas: [1.; 4] };
    let scene = varos_core::scene::build_scene_in_view_styled(&ed, view, [100, 100], style);
    assert!(scene.errors.is_empty(), "{:?}", scene.errors);
}

#[test]
fn ordinary_paint_and_live_reject_spot_conflicts_atomically() {
    use varos_core::{colour_commands::ColourCommand as C, editor::PaintTarget, model::Path};
    let ink = |c| {
        Paint::Managed(ManagedColour {
            colour: Colour::Spot { name: "Same ink".into(), tint: 0.8, alt: Cmyk { c, m: 0.2, y: 0.3, k: 0.4 } },
            alpha: 0.6,
        })
    };
    for live in [false, true] {
        let mut ed = Editor::new();
        for id in 1..=2 {
            let mut p = Path::new(id, vec![], true, None, None, 1.);
            p.fill = ink(0.1);
            ed.doc.paths.push(p);
        }
        ed.doc.sync_tree();
        ed.objsel.insert(1);
        let before = ed.doc.clone();
        let revision = ed.rev;
        let command = if live {
            C::Live { target: PaintTarget::Fill, paint: ink(0.9) }
        } else {
            C::Paint { target: PaintTarget::Fill, paint: ink(0.9) }
        };
        assert!(ed.try_execute(EditCommand::Colour(command)).is_err());
        assert_eq!(ed.doc, before);
        assert_eq!(ed.rev, revision);
        assert!(!ed.transaction_open());
        format::encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
        // Replacing every occurrence with the new alternate is allowed.
        ed.objsel.insert(2);
        ed.try_execute(EditCommand::Colour(C::Paint { target: PaintTarget::Fill, paint: ink(0.9) })).unwrap();
        format::encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
    }
}
