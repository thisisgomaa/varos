use varos_core::{
    effects::{Effect, WarpStyle},
    model::{Anchor, Document, Path},
    stroke::StrokeJoin,
    width_profile::WidthProfile,
};
#[test]
fn cpu_raster_live_effects_equal_baked_output() {
    for effect in [
        Effect::Offset { delta: 5., join: StrokeJoin::Miter, miter: 10. },
        Effect::ZigZag { size: 3., ridges: 3, smooth: true },
        Effect::Transform { copies: 1, movement: [120., 0.], scale: [1., 1.], rotate: 0., reflect: [false, false] },
        Effect::Warp { style: WarpStyle::Arch, bend: 35., h: 10., v: 0. },
    ] {
        let mut d = Document::default();
        let mut p = Path::new(
            10,
            [[20., 20.], [100., 20.], [100., 100.], [20., 100.]]
                .into_iter()
                .enumerate()
                .map(|(i, p)| Anchor { id: 11 + i as u32, p, hin: None, hout: None, smooth: false })
                .collect(),
            true,
            Some([0., 0.5, 1., 1.]),
            Some([1., 0., 0., 1.]),
            5.,
        );
        p.effects.push(effect);
        p.stroke_style.width_profile = Some(WidthProfile::lens());
        d.paths.push(p);
        d.ids = 30;
        d.sync_tree();
        let q = varos_core::effects_document::document(&d).unwrap().into_owned();
        let a = varos_raster::rasterize_canvas(&d, [300, 200], [0., 0.], 1.);
        let b = varos_raster::rasterize_canvas(&q, [300, 200], [0., 0.], 1.);
        assert!(a.errors.is_empty(), "{:?}", a.errors);
        assert_eq!(a.pixels, b.pixels);
    }
}

#[test]
fn overlapping_transform_copies_do_not_cut_out_their_fill() {
    let mut d = Document::default();
    let mut p = Path::new(
        10,
        [[20., 20.], [100., 20.], [100., 100.], [20., 100.]]
            .into_iter()
            .enumerate()
            .map(|(i, p)| Anchor { id: 11 + i as u32, p, hin: None, hout: None, smooth: false })
            .collect(),
        true,
        Some([0., 0.5, 1., 1.]),
        None,
        0.,
    );
    p.effects = vec![Effect::Transform {
        copies: 1,
        movement: [40., 0.],
        scale: [1., 1.],
        rotate: 0.,
        reflect: [false, false],
    }];
    d.paths.push(p);
    d.ids = 30;
    d.sync_tree();
    let raster = varos_raster::rasterize_canvas(&d, [180, 140], [0., 0.], 1.);
    assert_eq!(raster.pixels[(60 * 180 + 80) * 4 + 3], 255);
}
