//! Headless oracles for Lane B's maths foundation (not a gradient-paint integration test).
use varos_core::{
    gradient::{Gradient, GradientKind, Spread, Stop},
    model::Xform,
};
fn close(a: [f32; 4], b: [f32; 4]) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < 2e-5, "{a} != {b}");
    }
}
#[test]
fn interpolation_midpoint_and_opacity_are_independent() {
    let mut g = Gradient::default();
    g.stops[0].midpoint = 0.25;
    g.stops[0].opacity = 0.2;
    g.stops[1].colour[3] = 0.5;
    close(g.sample(0.25), [0.5, 0.5, 0.5, 0.35]);
    close(g.sample(-1.), [1., 1., 1., 0.2]);
    close(g.sample(2.), [0., 0., 0., 0.5]);
}
#[test]
fn spread_handles_negative_coordinates_and_boundaries() {
    let mut g = Gradient { spread: Spread::Repeat, ..Default::default() };
    close(g.sample(-0.25), g.sample(0.75));
    close(g.sample(1.), g.sample(0.));
    g.spread = Spread::Reflect;
    close(g.sample(-0.25), g.sample(0.25));
    close(g.sample(1.25), g.sample(0.75));
    close(g.sample(1.), [0., 0., 0., 1.]);
    close(g.sample(2.), [1.; 4]);
}
#[test]
fn hard_stop_uses_last_stop_at_duplicate_offset() {
    let mut g = Gradient::default();
    g.stops.insert(1, Stop::new(0., [1., 0., 0., 1.]));
    close(g.sample(0.), [1., 0., 0., 1.]);
    assert!(g.validate().is_ok());
}
#[test]
fn reversing_moves_each_midpoint_to_its_reversed_segment() {
    let mut g = Gradient::default();
    g.stops[0].midpoint = 0.2;
    g.stops.insert(1, Stop::new(0.4, [1., 0., 0., 1.]));
    g.stops[1].midpoint = 0.7;
    let before = g.clone();
    g.reverse();
    assert!(g.validate().is_ok());
    for i in 0..101 {
        let t = i as f32 / 100.;
        close(g.sample(t), before.sample(1. - t));
    }
    g.reverse();
    for i in 0..101 {
        let t = i as f32 / 100.;
        close(g.sample(t), before.sample(t));
    }
    assert_eq!(g.kind, before.kind);
    assert_eq!(g.placement, before.placement);
}
#[test]
fn radial_ellipse_focal_and_rotation_oracles() {
    let mut g = Gradient {
        kind: GradientKind::Radial,
        placement: [40., 0., 0., 20., 10., 15.],
        focal: [0.5, 0.],
        ..Default::default()
    };
    assert!(g.validate().is_ok());
    close(g.sample_point([30., 15.]), [1.; 4]);
    close(g.sample_point([50., 15.]), [0., 0., 0., 1.]);
    g.focal = [0., 0.];
    close(g.sample_point([10., 25.]), [0.5, 0.5, 0.5, 1.]);
    let xf = Xform { rot: 0.7, piv: [10., 15.] };
    let transformed = g.transformed(xf);
    for p in [[10., 15.], [30., 15.], [10., 25.], [50., 15.]] {
        close(g.sample_point(p), transformed.sample_point(xf.apply(p)));
    }
}
#[test]
fn lut_has_exact_endpoints_and_monotonic_ramp() {
    let g = Gradient::default();
    let lut = g.lut();
    assert_eq!(lut.len(), 1024);
    assert_eq!(lut[0], [1.; 4]);
    assert_eq!(lut[1023], [0., 0., 0., 1.]);
    assert!(lut.windows(2).all(|w| w[0][0] >= w[1][0]));
}
#[test]
fn definitions_round_trip_and_unknown_keys_are_refused() {
    let g = Gradient::default();
    let bytes = serde_json::to_vec(&g).unwrap();
    assert_eq!(g, serde_json::from_slice(&bytes).unwrap());
    let mut value = serde_json::to_value(g).unwrap();
    value["unknown"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Gradient>(value).is_err());
}
#[test]
fn invalid_definitions_are_refused_without_repair() {
    let mutate: [fn(&mut Gradient); 8] = [
        |g| g.stops.clear(),
        |g| g.stops[0].colour[0] = f32::NAN,
        |g| g.stops[0].opacity = 2.,
        |g| g.stops[0].midpoint = 0.,
        |g| g.stops[0].offset = 2.,
        |g| g.placement = [0.; 6],
        |g| g.focal = [1., 0.],
        |g| g.placement[0] = f32::INFINITY,
    ];
    for f in mutate {
        let mut g = Gradient::default();
        f(&mut g);
        assert!(g.validate().is_err());
    }
    let mut g = Gradient::default();
    g.stops.swap(0, 1);
    assert!(g.validate().is_err());
}
