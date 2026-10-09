//! Headless Lane F geometry contracts. No editor, renderer or event loop is constructed.
use kurbo::{CubicBez, ParamCurve, ParamCurveNearest, PathEl, Point};
use varos_core::{
    geom::{
        self,
        kurbo::{from_bez_path, from_compound_bez_path, to_bez_path, KurboPath},
        shapes::*,
    },
    model::{Anchor, AnchorRing, Path},
};
fn a(p: [f32; 2]) -> Anchor {
    Anchor { id: 0, p, hin: None, hout: None, smooth: false }
}
fn poly(pts: &[[f32; 2]], closed: bool) -> Path {
    Path::new(9, pts.iter().copied().map(a).collect(), closed, None, None, 1.)
}
fn p(v: [f32; 2]) -> Point {
    Point::new(v[0] as f64, v[1] as f64)
}
fn cubics(path: &Path) -> Vec<CubicBez> {
    if path.anchors.len() < 2 {
        return vec![];
    }
    (0..if path.closed { path.anchors.len() } else { path.anchors.len() - 1 })
        .map(|i| {
            let a = &path.anchors[i];
            let b = &path.anchors[(i + 1) % path.anchors.len()];
            CubicBez::new(p(a.p), p(a.hout.unwrap_or(a.p)), p(b.hin.unwrap_or(b.p)), p(b.p))
        })
        .collect()
}
fn distance(q: Point, cs: &[CubicBez]) -> f64 {
    cs.iter().map(|c| q.distance(c.eval(c.nearest(q, 1e-8).t))).fold(f64::INFINITY, f64::min)
}
fn assert_geometry_eq(a: &Path, b: &Path) {
    assert_eq!(a.closed, b.closed);
    assert_eq!(a.anchors.len(), b.anchors.len());
    for (a, b) in a.anchors.iter().zip(&b.anchors) {
        assert!(geom::dist(a.p, b.p) <= 1e-6);
    }
    assert_eq!(a.holes.len(), b.holes.len());
    for (a, b) in a.holes.iter().zip(&b.holes) {
        assert_eq!(a.len(), b.len());
        for (a, b) in a.iter().zip(b) {
            assert!(geom::dist(a.p, b.p) <= 1e-6);
        }
    }
}
#[test]
fn every_fixture_path_round_trips() {
    fn visit(dir: &std::path::Path, count: &mut usize) {
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().path()).collect();
        files.sort();
        for file in files {
            if file.is_dir() {
                visit(&file, count);
                continue;
            }
            if file.extension().is_none_or(|e| e != "vrs") {
                continue;
            }
            let body = std::fs::read(&file).unwrap();
            // PDF fixture twins embed exactly the JSON fixture's paths and have separate PDF tests.
            if body.starts_with(b"%PDF") {
                continue;
            }
            let paths = match varos_core::file::doc_from_blob(std::str::from_utf8(&body).unwrap()) {
                Ok(doc) => doc.paths,
                Err(e) => {
                    assert!(file.components().any(|c| c.as_os_str() == "refused"), "{}: {e}", file.display());
                    // Refused documents can still contain representable geometry. Exercise those
                    // paths directly; malformed model numbers/fields are the format lane's tests.
                    let json: serde_json::Value = serde_json::from_slice(&body).expect("fixture JSON");
                    json["doc"]["paths"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|v| serde_json::from_value::<Path>(v.clone()).ok())
                        .collect()
                }
            };
            for path in paths {
                let adapter = KurboPath::new(&path);
                let raw = to_bez_path(&path);
                assert_eq!(adapter.geometry(), &raw);
                let restored = adapter.into_path();
                assert_geometry_eq(&path, &restored);
                assert_eq!(restored, path, "IDs, handles and smooth flags");
                let decoded = from_compound_bez_path(&raw).unwrap();
                assert_geometry_eq(&path, &decoded);
                *count += 1;
            }
        }
    }
    let mut count = 0;
    visit(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"), &mut count);
    println!("fixture paths round-tripped: {count}");
    assert!(count >= 20, "covered {count} paths");
}
#[test]
fn adapter_preserves_flags_and_empty_duplicate_rings() {
    let mut path = poly(&[[0., 0.], [10., 0.], [0., 0.]], true);
    path.anchors[0].smooth = true;
    path.anchors[1].hin = Some([3., 4.]);
    path.anchors[1].hout = Some([12., 5.]);
    path.holes = vec![vec![], vec![a([1., 1.])]];
    assert_eq!(KurboPath::new(&path).into_path(), path);
    let mut bez = kurbo::BezPath::new();
    bez.move_to((0., 0.));
    bez.quad_to((3., 6.), (9., 0.));
    bez.move_to((20., 0.));
    bez.line_to((30., 0.));
    let parts = from_bez_path(&bez);
    assert_eq!(parts.len(), 2);
    assert!(from_compound_bez_path(&bez).is_err());
    assert_eq!(parts[0].anchors[0].hout, Some([2., 4.]));
    assert_eq!(parts[0].anchors[1].hin, Some([5., 4.]));
}
#[test]
fn rounded_rect_corner_radii_and_kappa() {
    let path = rounded_rectangle([0., 0., 100., 60.], [5., 10., 15., 20.]);
    assert!(path.closed);
    assert_eq!(path.anchors.len(), 8);
    assert_eq!(path.anchors[0].p, [0., 5.]);
    assert!((path.anchors[0].hout.unwrap()[1] - (5. - 5. * KAPPA as f32)).abs() < 1e-6);
    assert_eq!(rounded_rectangle([100., 60., 0., 0.], [0.; 4]).anchors.len(), 4);
    let clamped = rounded_rectangle([0., 0., 20., 10.], [100.; 4]);
    assert_eq!(clamped.anchors[0].p, [0., 5.]);
    assert_eq!(rounded_rectangle([0., 0., 20., 10.], [0., 2., 0., 0.]).anchors.len(), 5);
}
#[test]
fn polygons_stars_counts_radii_and_symmetry() {
    for n in [0, 3, 4, 7, 1000, 2000] {
        let path = polygon([3., 4.], 10., n, 0.);
        assert_eq!(path.anchors.len(), n.clamp(3, 1000));
        assert!(path.closed);
        for a in path.anchors {
            assert!((geom::dist(a.p, [3., 4.]) - 10.).abs() < 2e-6);
        }
    }
    let square = polygon([0., 0.], 10., 4, 0.);
    for i in 0..2 {
        assert!(geom::length(geom::add(square.anchors[i].p, square.anchors[i + 2].p)) < 1e-5);
    }
    let rotated = polygon([0., 0.], 10., 4, std::f32::consts::FRAC_PI_2);
    assert!(geom::dist(rotated.anchors[0].p, [10., 0.]) < 1e-5);
    let star = star([0., 0.], 20., 8., 5, 0.);
    assert_eq!(star.anchors.len(), 10);
    assert!(star.closed);
    for (i, a) in star.anchors.iter().enumerate() {
        assert!((geom::length(a.p) - if i % 2 == 0 { 20. } else { 8. }).abs() < 2e-6);
    }
}
#[test]
fn arcs_spirals_grids_deterministic_and_closedness() {
    let open = arc([0., 0.], [10., 20.], 0., std::f32::consts::PI, ArcClosure::Open);
    assert_eq!(open.anchors.len(), 3);
    assert!(!open.closed);
    let chord = arc([0., 0.], [10., 20.], 0., std::f32::consts::PI, ArcClosure::Chord);
    assert!(chord.closed);
    assert!(!chord.anchors[0].smooth && !chord.anchors.last().unwrap().smooth);
    assert_eq!(chord.anchors.len(), 3);
    let pie = arc([0., 0.], [10., 20.], 0., std::f32::consts::PI, ArcClosure::Pie);
    assert_eq!(pie.anchors.len(), 4);
    assert_eq!(pie.anchors[3].p, [0., 0.]);
    assert!(pie.closed);
    let circle = arc([0., 0.], [10., 10.], 0., std::f32::consts::TAU, ArcClosure::Open);
    assert!(circle.closed);
    assert_eq!(circle.anchors.len(), 4);
    let neg = arc([0., 0.], [10., 20.], 0., -std::f32::consts::FRAC_PI_2, ArcClosure::Open);
    assert!(geom::dist(neg.anchors[1].p, [0., -20.]) < 1e-5);
    let spiral = spiral([0., 0.], 10., 2., 0.5, 32);
    assert_eq!(spiral.anchors.len(), 33);
    assert!(!spiral.closed);
    assert!((geom::length(spiral.anchors[32].p) - 2.5).abs() < 1e-5);
    let rect = rectangular_grid(2, 3, [0., 0., 60., 40.]);
    assert_eq!(rect.len(), 7);
    assert!(rect.iter().all(|p| !p.closed && p.anchors.len() == 2));
    assert!(rectangular_grid(0, 3, [0.; 4]).is_empty());
    let polar = polar_grid(3, 8, [0., 0.], [10., 20.]);
    assert_eq!(polar.len(), 11);
    assert!(polar[..3].iter().all(|p| p.closed && p.anchors.len() == 4));
    assert!(polar[3..].iter().all(|p| !p.closed));
    assert!(polar_grid(0, 0, [0.; 2], [1.; 2]).is_empty());
    for path in rect.into_iter().chain(polar).chain([open, chord, pie, circle, neg, spiral]) {
        assert_eq!(KurboPath::new(&path).into_path(), path);
        assert_geometry_eq(&path, &from_compound_bez_path(&to_bez_path(&path)).unwrap());
    }
    assert_eq!(to_bez_path(&polygon([1., 2.], 4., 7, 0.3)), to_bez_path(&polygon([1., 2.], 4., 7, 0.3)));
    assert!(matches!(to_bez_path(&line([0., 0.], [1., 1.])).elements()[1], PathEl::LineTo(_)));
}
#[test]
fn fit_circle_samples_within_tolerance() {
    let points: Vec<_> = (0..=128)
        .map(|i| {
            let t = std::f32::consts::TAU * i as f32 / 128.;
            [50. * t.cos(), 50. * t.sin()]
        })
        .collect();
    let fitted = geom::fit_points(&points, 0.1);
    assert!(fitted.anchors.len() < points.len());
    let cs = cubics(&fitted);
    for q in points {
        assert!(distance(p(q), &cs) <= 0.10001);
    }
    assert_eq!(KurboPath::new(&fitted).into_path(), fitted);
    assert!(geom::fit_points(&[], 0.1).anchors.is_empty());
    assert_eq!(geom::fit_points(&[[1., 2.]; 4], 0.1).anchors.len(), 1);
}
#[test]
fn simplify_reduces_anchors_and_certifies_geometric_error() {
    let points: Vec<_> = (0..=40).map(|i| [i as f32, (i as f32 * 0.04).sin()]).collect();
    let path = poly(&points, false);
    let simplified = geom::simplify(&path, 0.05, 180.);
    assert!(simplified.anchors.len() < path.anchors.len());
    let original = cubics(&path);
    let output = cubics(&simplified);
    for (source, target) in [(&original, &output), (&output, &original)] {
        for c in source {
            for i in 0..=100 {
                assert!(distance(c.eval(i as f64 / 100.), target) <= 0.050001);
            }
        }
    }
    assert_eq!(geom::simplify(&path, 0., 30.), path);
    let corner = poly(&[[0., 0.], [5., 0.], [10., 0.], [10., 5.], [10., 10.]], false);
    let simplified = geom::simplify(&corner, 0.05, 30.);
    assert!(simplified.anchors.iter().any(|a| a.p == [10., 0.]));
    assert_eq!(simplified.anchors[0].p, corner.anchors[0].p);
}
#[test]
fn simplify_curves_holes_and_metadata() {
    let original = arc([0., 0.], [20., 20.], 0., std::f32::consts::TAU, ArcClosure::Chord);
    let mut dense = geom::add_anchor_points(&geom::add_anchor_points(&original));
    dense.holes.push(dense.anchors.clone());
    dense.name = Some("Keep".into());
    let reduced = geom::simplify(&dense, 0.2, 180.);
    assert!(reduced.anchors.len() < dense.anchors.len());
    assert!(reduced.holes[0].len() < dense.holes[0].len());
    assert_eq!(reduced.name, dense.name);
    assert!(reduced.closed);
    let cs = cubics(&reduced);
    for c in cubics(&original) {
        for i in 0..100 {
            assert!(distance(c.eval(i as f64 / 100.), &cs) <= 0.20001);
        }
    }
}
#[test]
fn smooth_keeps_open_endpoints_and_respects_range() {
    let mut path = poly(&[[0., 0.], [10., 10.], [20., 0.], [30., 10.]], false);
    path.anchors[0].hout = Some([3., 1.]);
    path.anchors[3].hin = Some([29., 9.]);
    path.holes.push(path.anchors.clone());
    let smoothed = geom::smooth(&path, 1., 1..2);
    assert_eq!(smoothed.anchors[0], path.anchors[0]);
    assert_eq!(smoothed.anchors[3], path.anchors[3]);
    assert_eq!(smoothed.anchors[2], path.anchors[2]);
    assert!(smoothed.anchors[1].smooth);
    assert_ne!(smoothed.anchors[1].hin, path.anchors[1].hin);
    assert_ne!(smoothed.holes[0][1].hin, path.holes[0][1].hin);
    assert_eq!(geom::smooth(&path, 0., 0..10), path);
    assert_eq!(KurboPath::new(&smoothed).into_path(), smoothed);
}
#[test]
fn midpoint_subdivision_preserves_cubic_geometry_and_ids() {
    let mut path = line([0., 0.], [20., 0.]);
    path.anchors[0].id = 7;
    path.anchors[1].id = 8;
    path.anchors[0].hout = Some([3., 20.]);
    path.anchors[1].hin = Some([17., 20.]);
    let cs = cubics(&path);
    let added = geom::add_anchor_points(&path);
    let new = cubics(&added);
    assert_eq!(added.anchors.len(), 3);
    assert_eq!(added.anchors[0].id, 7);
    assert_eq!(added.anchors[2].id, 8);
    assert_eq!(added.anchors[1].id, 0);
    for i in 0..=100 {
        let t = i as f64 / 100.;
        let q = if t <= 0.5 { new[0].eval(t * 2.) } else { new[1].eval(t * 2. - 1.) };
        assert!(cs[0].eval(t).distance(q) < 1e-6);
    }
    let mut circle = arc([0., 0.], [10., 10.], 0., std::f32::consts::TAU, ArcClosure::Chord);
    circle.holes.push(circle.anchors.clone());
    let added = geom::add_anchor_points(&circle);
    assert_eq!(added.anchors.len(), 8);
    assert_eq!(added.holes[0].len(), 8);
    assert!(added.closed);
    let line = geom::add_anchor_points(&line([0., 0.], [10., 0.]));
    assert_eq!(line.anchors[1].p, [5., 0.]);
    assert!(line.anchors.iter().all(|a| a.hin.is_none() && a.hout.is_none()));
}
#[test]
fn average_moves_handles_and_ignores_invalid_duplicate_addresses() {
    let mut path = line([0., 0.], [10., 20.]);
    path.anchors[0].hout = Some([2., 3.]);
    path.holes.push(vec![a([20., 40.])]);
    let x = (AnchorRing::Outer, 0);
    let y = (AnchorRing::Hole(0), 0);
    let selection = [x, y, x, (AnchorRing::Outer, 99)];
    let out = geom::average_anchors(&path, &selection, geom::AverageAxis::Both);
    assert_eq!(out.anchors[0].p, [10., 20.]);
    assert_eq!(out.anchors[0].hout, Some([12., 23.]));
    assert_eq!(out.holes[0][0].p, [10., 20.]);
    assert_eq!(geom::average_anchors(&path, &selection, geom::AverageAxis::Horizontal).anchors[0].p, [0., 20.]);
    assert_eq!(geom::average_anchors(&path, &selection, geom::AverageAxis::Vertical).anchors[0].p, [10., 0.]);
    assert_eq!(geom::average_anchors(&path, &[], geom::AverageAxis::Both), path);
}
#[test]
fn joins_reverse_handles_merge_nearby_and_keep_closed_paths() {
    let left = line([0., 0.], [10., 0.]);
    let mut right = line([20., 0.], [10., 0.]);
    right.anchors[1].hin = Some([14., 2.]);
    let joined = geom::join_open_paths(&[left.clone(), right], 0.01);
    assert_eq!(joined.len(), 1);
    assert_eq!(joined[0].anchors.len(), 3);
    assert_eq!(joined[0].anchors[1].hout, Some([14., 2.]));
    assert!(!joined[0].closed);
    let connected = geom::join_open_paths(&[left.clone(), line([30., 0.], [40., 0.])], 0.01);
    assert_eq!(connected[0].anchors.len(), 4);
    assert_eq!(connected[0].anchors[1].hout, None);
    let closed = geom::join_open_paths(&[left], 0.01);
    assert!(closed[0].closed);
    let polygon = polygon([0., 0.], 2., 3, 0.);
    assert_eq!(geom::join_open_paths(std::slice::from_ref(&polygon), 0.1), vec![polygon]);
    assert!(geom::join_open_paths(&[], 0.1).is_empty());
}

#[test]
fn fit_certifies_samples_after_large_coordinate_handle_quantisation() {
    for points in [
        vec![[100_000_000., 0.], [100_000_008., 8.], [100_000_024., 0.]],
        vec![[0., 100_000_000.], [8., 100_000_008.], [0., 100_000_024.]],
    ] {
        let fitted = geom::fit_points(&points, 0.01);
        for sample in points {
            assert!(distance(p(sample), &cubics(&fitted)) <= 0.01);
        }
    }
}

#[test]
fn adapter_keeps_all_segment_types_after_close() {
    for next in [
        PathEl::LineTo(kurbo::Point::new(20., 0.)),
        PathEl::QuadTo(kurbo::Point::new(15., 10.), kurbo::Point::new(20., 0.)),
        PathEl::CurveTo(kurbo::Point::new(5., 10.), kurbo::Point::new(15., 10.), kurbo::Point::new(20., 0.)),
    ] {
        let mut bez = kurbo::BezPath::new();
        bez.move_to((0., 0.));
        bez.line_to((10., 0.));
        bez.close_path();
        bez.close_path();
        bez.push(next);
        bez.line_to((30., 0.));
        let parts = from_bez_path(&bez);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].closed);
        assert!(!parts[1].closed);
        assert_eq!(parts[1].anchors.iter().map(|a| a.p).collect::<Vec<_>>(), vec![[0., 0.], [20., 0.], [30., 0.]]);
        let mut expected = kurbo::BezPath::new();
        expected.move_to((0., 0.));
        expected.push(next);
        expected.line_to((30., 0.));
        assert_geometry_eq(&parts[1], &from_bez_path(&expected)[0]);
        assert!(from_compound_bez_path(&bez).is_err());
    }
    let mut bez = kurbo::BezPath::new();
    bez.move_to((0., 0.));
    bez.line_to((10., 0.));
    bez.close_path();
    bez.move_to((40., 0.));
    bez.line_to((50., 0.));
    assert_eq!(from_bez_path(&bez)[1].anchors[0].p, [40., 0.]);
}
