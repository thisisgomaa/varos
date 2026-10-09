// Adapted from VectorCraft crates/pathops/tests/prop_pathops.rs @a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
// proptest is unavailable offline; reproducible seeded polygons exercise the same set identities.
use varos_core::boolean::{run_boolean_curves, BoolOp, ResultShape, Seg};
use varos_core::geom::{cubic, point_in_poly, Pt};
fn ring(s: &[Seg]) -> Vec<Pt> {
    s.iter().flat_map(|s| (0..16).map(move |i| cubic(s.0, s.1, s.2, s.3, i as f32 / 16.0))).collect()
}
fn area_ring(p: &[Pt]) -> f64 {
    (0..p.len())
        .map(|i| {
            let a = p[i];
            let b = p[(i + 1) % p.len()];
            a[0] as f64 * b[1] as f64 - b[0] as f64 * a[1] as f64
        })
        .sum::<f64>()
        .abs()
        * 0.5
}
fn area(r: &[ResultShape]) -> f64 {
    r.iter().map(|r| area_ring(&ring(&r.outer)) - r.holes.iter().map(|h| area_ring(&ring(h))).sum::<f64>()).sum()
}
fn inside(r: &[ResultShape], p: Pt) -> bool {
    r.iter().any(|r| point_in_poly(&ring(&r.outer), p) && !r.holes.iter().any(|h| point_in_poly(&ring(h), p)))
}
fn polygon(seed: &mut u64) -> Vec<Seg> {
    let mut next = || {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((*seed >> 32) as u32) as f32 / u32::MAX as f32
    };
    let cx = next() * 50.0;
    let cy = next() * 50.0;
    let p: Vec<Pt> = (0..6)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / 6.0;
            let r = 8.0 + next() * 20.0;
            [cx + r * a.cos(), cy + r * a.sin()]
        })
        .collect();
    (0..p.len())
        .map(|i| {
            let a = p[i];
            let b = p[(i + 1) % p.len()];
            (a, a, b, b)
        })
        .collect()
}
fn seeded_oracle(k: usize) {
    let mut seed = 0xa469568;
    let mut failures = Vec::new();
    for case in 0..200 {
        let a = polygon(&mut seed);
        let b = polygon(&mut seed);
        let aa = area_ring(&ring(&a));
        let ab = area_ring(&ring(&b));
        let ops = [BoolOp::Unite, BoolOp::Intersect, BoolOp::MinusFront, BoolOp::Exclude];
        let r: Vec<_> = ops.iter().map(|op| run_boolean_curves(*op, &[vec![a.clone()], vec![b.clone()]])).collect();
        let ar: Vec<_> = r.iter().map(|r| area(r)).collect();
        let tol = 2e-3 * (aa + ab).max(1.0);
        let area_ok = match k {
            0 => (ar[0] - (aa + ab - ar[1])).abs() < tol,
            1 => ar[1] <= aa.min(ab) + tol,
            2 => (ar[2] + ar[1] - aa).abs() < tol && ar[2] <= aa + tol,
            3 => (ar[3] - (ar[0] - ar[1])).abs() < tol,
            _ => unreachable!(),
        };
        let mut bad_points = 0;
        for x in 0..13 {
            for y in 0..13 {
                let p = [x as f32 * 7.0 - 15.37, y as f32 * 7.0 - 15.19];
                let ia = point_in_poly(&ring(&a), p);
                let ib = point_in_poly(&ring(&b), p);
                let want = [ia || ib, ia && ib, ia && !ib, ia != ib][k];
                if inside(&r[k], p) != want {
                    bad_points += 1;
                }
            }
        }
        if !area_ok || bad_points > 0 {
            failures.push(format!("case {case}: areas {ar:?}, area_ok={area_ok}, bad_points={bad_points}"));
        }
    }
    println!("op {k}: {}/200 failed cases", failures.len());
    assert!(failures.is_empty(), "{} oracle failures: {failures:#?}", failures.len());
}
#[test]
fn seeded_unite_oracle() {
    seeded_oracle(0);
}
#[test]
fn seeded_intersect_oracle() {
    seeded_oracle(1);
}
#[test]
fn seeded_minus_front_oracle() {
    seeded_oracle(2);
}
#[test]
fn seeded_exclude_oracle() {
    seeded_oracle(3);
}

fn straight(points: &[Pt]) -> Vec<Seg> {
    (0..points.len())
        .map(|i| {
            let a = points[i];
            let b = points[(i + 1) % points.len()];
            (a, a, b, b)
        })
        .collect()
}
#[test]
fn exclude_overlapping_squares_and_three_operand_fold() {
    let points = [
        vec![[5., -5.], [25., -5.], [25., 15.], [5., 15.]],
        vec![[0., 0.], [20., 0.], [20., 20.], [0., 20.]],
        vec![[10., 5.], [30., 5.], [30., 25.], [10., 25.]],
    ];
    let shapes: Vec<_> = points.iter().map(|p| vec![straight(p)]).collect();
    assert!((area(&run_boolean_curves(BoolOp::Exclude, &shapes[..2])) - 350.).abs() < 1e-3);
    // XOR area = sum(single) - 2*sum(pair intersections) + 4*triple intersection.
    let result = run_boolean_curves(BoolOp::Exclude, &shapes);
    assert!((area(&result) - 550.).abs() < 1e-3);
    for x in -5..35 {
        for y in -10..30 {
            let p = [x as f32 + 0.37, y as f32 + 0.19];
            let want = points.iter().filter(|ring| point_in_poly(ring, p)).count() % 2 == 1;
            assert_eq!(inside(&result, p), want, "{p:?}");
        }
    }
}
#[test]
fn hexagram_touching_tip_unite_and_minus_front() {
    for offset in [[0., 0.], [37., -23.]] {
        let translate =
            |p: &[Pt]| straight(&p.iter().map(|p| [p[0] + offset[0], p[1] + offset[1]]).collect::<Vec<_>>());
        let a = translate(&[[0., 0.], [20., 0.], [10., 20.]]);
        let b = translate(&[[0., 10.], [20., 10.], [10., -10.]]);
        assert!((area(&run_boolean_curves(BoolOp::Unite, &[vec![a.clone()], vec![b.clone()]])) - 275.).abs() < 1e-3);
        assert!((area(&run_boolean_curves(BoolOp::MinusFront, &[vec![b], vec![a]])) - 75.).abs() < 1e-3);
    }
}

#[test]
fn seeded_self_and_commutative_identities() {
    let mut seed = 0xa469568;
    let mut failures = Vec::new();
    for case in 0..32 {
        let a = polygon(&mut seed);
        let b = polygon(&mut seed);
        let aa = area_ring(&ring(&a));
        for (k, op) in [BoolOp::Unite, BoolOp::Intersect, BoolOp::Exclude].into_iter().enumerate() {
            let ab = area(&run_boolean_curves(op, &[vec![a.clone()], vec![b.clone()]]));
            let ba = area(&run_boolean_curves(op, &[vec![b.clone()], vec![a.clone()]]));
            if (ab - ba).abs() >= 2e-3 * aa.max(1.) {
                failures.push(format!("commutativity case {case}, op {k}: {ab} vs {ba}"));
            }
        }
        for (k, (op, want)) in
            [(BoolOp::Unite, aa), (BoolOp::Intersect, aa), (BoolOp::MinusFront, 0.), (BoolOp::Exclude, 0.)]
                .into_iter()
                .enumerate()
        {
            let got = area(&run_boolean_curves(op, &[vec![a.clone()], vec![a.clone()]]));
            if (got - want).abs() >= 2e-3 * aa.max(1.) {
                failures.push(format!("self operation case {case}, op {k}: {got} vs {want}"));
            }
        }
    }
    assert!(failures.is_empty(), "{} oracle failures: {failures:#?}", failures.len());
}

#[test]
fn overlapping_curves_keep_handles_when_area_guard_passes() {
    let circle = |cx: f32| {
        let k = 5.5228477;
        vec![
            ([cx + 10., 0.], [cx + 10., k], [cx + k, 10.], [cx, 10.]),
            ([cx, 10.], [cx - k, 10.], [cx - 10., k], [cx - 10., 0.]),
            ([cx - 10., 0.], [cx - 10., -k], [cx - k, -10.], [cx, -10.]),
            ([cx, -10.], [cx + k, -10.], [cx + 10., -k], [cx + 10., 0.]),
        ]
    };
    let result = run_boolean_curves(BoolOp::Unite, &[vec![circle(0.)], vec![circle(10.)]]);
    let expected = 200. * std::f64::consts::PI - (200. * 0.5_f64.acos() - 5. * 300_f64.sqrt());
    assert!((area(&result) - expected).abs() < expected * 2e-3);
    assert!(
        result.iter().flat_map(|r| &r.outer).any(|s| {
            let edge = [s.3[0] - s.0[0], s.3[1] - s.0[1]];
            let handle = [s.1[0] - s.0[0], s.1[1] - s.0[1]];
            (edge[0] * handle[1] - edge[1] * handle[0]).abs() > 1e-3
        }),
        "the guarded primary result must retain curved segments"
    );
}
