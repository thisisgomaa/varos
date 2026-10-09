use super::*;
fn draw(colour: Pixel) -> Prim {
    Prim::Draw(varos_core::Prim::Disc { c: [0.0, 0.0], r: 1.0, color: colour })
}
fn paint(p: &varos_core::Prim, dst: &mut [Pixel]) {
    if let varos_core::Prim::Disc { color, .. } = p {
        let s = [color[0] * color[3], color[1] * color[3], color[2] * color[3], color[3]];
        for pixel in dst {
            *pixel = composite(Blend::Normal, *pixel, s, 1.0);
        }
    }
}
fn begin(opacity: f32, blend: Blend) -> Prim {
    Prim::LayerBegin { opacity, blend, mask: None }
}
fn close(a: Pixel, b: Pixel) {
    for i in 0..4 {
        assert!((a[i] - b[i]).abs() < 1e-5, "{a:?} != {b:?}");
    }
}
#[test]
fn all_sixteen_modes_match_frozen_opaque_formula_pixels() {
    // Independent published channel equations at Cb=.25, Cs=.75. Neutral RGB makes
    // the four nonseparable modes reduce to luminosity-preserving results.
    let expected =
        [0.75, 0.1875, 0.8125, 0.375, 0.25, 0.75, 1.0, 0.0, 0.625, 0.375, 0.5, 0.625, 0.25, 0.25, 0.25, 0.75];
    for (mode, e) in Blend::ALL.into_iter().zip(expected) {
        close(composite(mode, [0.25, 0.25, 0.25, 1.0], [0.75, 0.75, 0.75, 1.0], 1.0), [e, e, e, 1.0]);
        let mut dst = vec![[0.25, 0.25, 0.25, 1.0]];
        CpuLayers::default()
            .render(
                &[begin(1.0, mode), draw([0.75, 0.75, 0.75, 1.0]), Prim::LayerEnd],
                [1, 1],
                1.0,
                Limits::default(),
                &mut dst,
                paint,
            )
            .unwrap();
        close(dst[0], [e, e, e, 1.0]);
    }
}
#[test]
fn translucent_general_formula_and_zero_alpha() {
    // Multiply, Ab=.6, As=.4: result premultiplied = .6*.6*.2 + .4*.4*.8 + .4*.6*.16.
    close(
        composite(Blend::Multiply, [0.12, 0.12, 0.12, 0.6], [0.32, 0.32, 0.32, 0.4], 1.0),
        [0.2384, 0.2384, 0.2384, 0.76],
    );
    for mode in Blend::ALL {
        close(composite(mode, [0.0; 4], [0.2, 0.1, 0.3, 0.5], 0.6), [0.12, 0.06, 0.18, 0.3]);
        close(composite(mode, [0.1, 0.2, 0.3, 0.5], [0.0; 4], 1.0), [0.1, 0.2, 0.3, 0.5]);
    }
}
#[test]
fn nonseparable_hue_saturation_luminosity_invariants() {
    for b in [[0.1, 0.8, 0.4], [0.99, 0.01, 0.25], [0.0; 3], [1.0; 3]] {
        for s in [[0.9, 0.1, 0.6], [0.2, 0.2, 0.2], [1.0, 0.0, 0.0]] {
            for mode in [Blend::Hue, Blend::Saturation, Blend::Color, Blend::Luminosity] {
                let c = blend_rgb(mode, b, s);
                assert!(c.iter().all(|v| *v >= -1e-5 && *v <= 1.00001), "{mode:?} {c:?}");
                assert!((lum(c) - if mode == Blend::Luminosity { lum(s) } else { lum(b) }).abs() < 1e-5);
            }
        }
    }
}
#[test]
fn nested_opacity_and_mask_apply_once_at_end() {
    let list = [
        begin(0.5, Blend::Normal),
        draw([1.0, 0.0, 0.0, 1.0]),
        Prim::LayerBegin { opacity: 0.5, blend: Blend::Normal, mask: Some(vec![0.5]) },
        draw([0.0, 0.0, 1.0, 1.0]),
        Prim::LayerEnd,
        Prim::LayerEnd,
    ];
    let mut dst = vec![[0.0; 4]];
    let report = CpuLayers::default().render(&list, [1, 1], 1.0, Limits::default(), &mut dst, paint).unwrap();
    close(dst[0], [0.375, 0.0, 0.125, 0.5]);
    assert_eq!(report.passes, 2);
    assert!(!report.flattened);
}
#[test]
fn depth_and_budget_flatten_without_losing_draws_or_leaking_stack() {
    for limits in [Limits { depth: 1, bytes: 1024 }, Limits { depth: 16, bytes: 0 }] {
        let mut dst = vec![[0.0; 4]];
        let report = CpuLayers::default()
            .render(
                &[
                    begin(1.0, Blend::Normal),
                    begin(0.0, Blend::Multiply),
                    draw([1.0, 0.0, 0.0, 1.0]),
                    Prim::LayerEnd,
                    draw([0.0, 1.0, 0.0, 1.0]),
                    Prim::LayerEnd,
                ],
                [1, 1],
                1.0,
                limits,
                &mut dst,
                paint,
            )
            .unwrap();
        assert!(report.flattened);
        assert!(report.peak_bytes <= limits.bytes);
        close(dst[0], [0.0, 1.0, 0.0, 1.0]);
    }
}
#[test]
fn blur_impulse_matches_separable_formula_zero_extension_and_zoom() {
    let mut src = vec![[0.0; 4]; 81];
    src[40] = [1.0; 4];
    let w = kernel(1.0, 1.0);
    let blurred = gaussian(&src, [9, 9], &w);
    let r = w.len() / 2;
    close(blurred[40], [w[r] * w[r]; 4]);
    close(blurred[41], [w[r] * w[r + 1]; 4]);
    assert!((blurred.iter().map(|p| p[3]).sum::<f32>() - 1.0).abs() < 1e-5);
    let w2 = kernel(1.0, 2.0);
    assert_eq!(w2.len(), 13);
    assert!(w2[w2.len() / 2] < w[r]);
    assert_eq!(gaussian(&src, [9, 9], &kernel(0.0, 2.0)), src);
    let tiny = gaussian(&[[1.0; 4]], [1, 1], &w);
    close(tiny[0], [w[r] * w[r]; 4]);
}
#[test]
fn cache_uses_object_revision_zoom_and_size_and_idle_is_zero_work() {
    let list = |revision| {
        [
            begin(1.0, Blend::Normal),
            draw([0.4, 0.1, 0.3, 0.5]),
            Prim::Blur { object: 7, revision, radius: 1.0 },
            Prim::LayerEnd,
        ]
    };
    let mut renderer = CpuLayers::default();
    let mut dst = vec![[0.0; 4]; 9];
    let first = renderer.render(&list(1), [3, 3], 1.0, Limits::default(), &mut dst, paint).unwrap();
    assert_eq!(first.passes, 3);
    dst.fill([0.0; 4]);
    let expected = dst.clone();
    let idle = renderer.render(&[], [3, 3], 1.0, Limits::default(), &mut dst, |_, _| panic!("idle draw")).unwrap();
    assert_eq!(idle, Report::default());
    assert_eq!(dst, expected);
    assert_eq!(renderer.render(&list(1), [3, 3], 1.001, Limits::default(), &mut dst, paint).unwrap().cache_hits, 1);
    assert_eq!(renderer.render(&list(2), [3, 3], 1.0, Limits::default(), &mut dst, paint).unwrap().cache_hits, 0);
    assert_eq!(renderer.render(&list(2), [3, 3], 2.0, Limits::default(), &mut dst, paint).unwrap().cache_hits, 0);
}
#[test]
fn shadow_and_outer_glow_use_alpha_blur_offset_colour_and_preserve_content() {
    for (offset, outer) in [([1.0, 0.0], false), ([0.0, 0.0], true)] {
        let list = [
            begin(1.0, Blend::Normal),
            draw([1.0; 4]),
            Prim::Shadow { object: 1, revision: 1, offset, blur: 0.0, colour: [0.0, 0.0, 1.0, 0.5], outer },
            Prim::LayerEnd,
        ];
        let mut dst = vec![[0.0; 4]; 3];
        CpuLayers::default().render(&list, [3, 1], 1.0, Limits::default(), &mut dst, |_, d| d[1] = [1.0; 4]).unwrap();
        close(dst[1], [1.0; 4]);
        close(dst[2], if outer { [0.0; 4] } else { [0.0, 0.0, 0.5, 0.5] });
        close(dst[0], [0.0; 4]);
    }
}
#[test]
fn malformed_lists_and_nonfinite_inputs_fail_before_drawing() {
    for list in [
        vec![Prim::LayerEnd],
        vec![begin(1.0, Blend::Normal)],
        vec![Prim::LayerBegin { opacity: 1.0, blend: Blend::Normal, mask: Some(vec![f32::NAN]) }],
        vec![Prim::Blur { object: 0, revision: 0, radius: f32::INFINITY }],
    ] {
        let mut dst = vec![[0.1; 4]];
        assert!(CpuLayers::default()
            .render(&list, [1, 1], 1.0, Limits::default(), &mut dst, |_, _| panic!("invalid draw"))
            .is_err());
        close(dst[0], [0.1; 4]);
    }
}
#[test]
#[ignore = "headless timing probe, run with --ignored --nocapture"]
fn measured_layer_timings() {
    let size = [128, 128];
    let mut dst = vec![[0.0; 4]; 16384];
    let mut renderer = CpuLayers::default();
    let list = [
        begin(0.8, Blend::Multiply),
        draw([0.8, 0.2, 0.4, 0.8]),
        Prim::Blur { object: 1, revision: 1, radius: 3.0 },
        Prim::Shadow {
            object: 2,
            revision: 1,
            offset: [3.0, 2.0],
            blur: 3.0,
            colour: [0.0, 0.0, 0.0, 0.5],
            outer: false,
        },
        Prim::LayerEnd,
    ];
    let start = std::time::Instant::now();
    let report = renderer.render(&list, size, 1.0, Limits::default(), &mut dst, paint).unwrap();
    eprintln!(
        "Lane D 128x128 blur+shadow cold={:.3}ms passes={} peak={}B",
        start.elapsed().as_secs_f64() * 1000.0,
        report.passes,
        report.peak_bytes
    );
    let start = std::time::Instant::now();
    let report = renderer.render(&list, size, 1.0, Limits::default(), &mut dst, paint).unwrap();
    eprintln!("Lane D warm={:.3}ms hits={}", start.elapsed().as_secs_f64() * 1000.0, report.cache_hits);
    let start = std::time::Instant::now();
    for _ in 0..10000 {
        renderer.render(&[], size, 1.0, Limits::default(), &mut dst, paint).unwrap();
    }
    eprintln!("Lane D idle10000={:.3}ms", start.elapsed().as_secs_f64() * 1000.0);
}
#[test]
fn shared_draw_list_plan_balances_flattened_frames_and_skips_their_effects() {
    let list = [
        begin(1.0, Blend::Normal),
        begin(0.4, Blend::Multiply),
        draw([1.0; 4]),
        Prim::Blur { object: 0, revision: 0, radius: 1.0 },
        Prim::LayerEnd,
        Prim::Shadow { object: 0, revision: 0, offset: [0.0; 2], blur: 1.0, colour: [0.0; 4], outer: true },
        Prim::LayerEnd,
    ];
    assert_eq!(
        plan(&list, [1, 1], 1.0, Limits { depth: 1, bytes: 1024 }).unwrap(),
        [
            Step::Begin { isolated: true },
            Step::Begin { isolated: false },
            Step::Draw,
            Step::Blur { enabled: false },
            Step::End { isolated: false },
            Step::Shadow { enabled: true },
            Step::End { isolated: true }
        ]
    );
    assert_eq!(
        plan(&[begin(1.0, Blend::Normal), Prim::LayerEnd], [1, 1], 1.0, Limits { depth: 16, bytes: 85 }).unwrap(),
        [Step::Begin { isolated: true }, Step::End { isolated: true }]
    );
    assert_eq!(
        plan(&[begin(1.0, Blend::Normal), Prim::LayerEnd], [1, 1], 1.0, Limits { depth: 16, bytes: 84 }).unwrap(),
        [Step::Begin { isolated: false }, Step::End { isolated: false }]
    );
}
#[test]
fn coloured_cpu_pixels_match_straight_alpha_formula_for_every_mode() {
    for mode in Blend::ALL {
        for ab in [0.0, 0.3, 1.0] {
            for as_ in [0.0, 0.6, 1.0] {
                let cb = [0.1, 0.6, 0.9];
                let cs = [0.8, 0.2, 0.4];
                let opacity = 0.7;
                let b = [cb[0] * ab, cb[1] * ab, cb[2] * ab, ab];
                let a = as_ * opacity;
                let rgb = coloured_golden(mode);
                let formula = [
                    (1.0 - a) * ab * cb[0] + (1.0 - ab) * a * cs[0] + a * ab * rgb[0],
                    (1.0 - a) * ab * cb[1] + (1.0 - ab) * a * cs[1] + a * ab * rgb[1],
                    (1.0 - a) * ab * cb[2] + (1.0 - ab) * a * cs[2] + a * ab * rgb[2],
                    a + ab * (1.0 - a),
                ];
                let mut dst = vec![b];
                CpuLayers::default()
                    .render(
                        &[begin(opacity, mode), draw([cs[0], cs[1], cs[2], as_]), Prim::LayerEnd],
                        [1, 1],
                        1.0,
                        Limits::default(),
                        &mut dst,
                        paint,
                    )
                    .unwrap();
                close(dst[0], formula);
            }
        }
    }
}
#[test]
fn invalid_large_effects_are_refused_instead_of_silently_clamped() {
    for (effect, message) in [
        (Prim::Blur { object: 0, revision: 0, radius: 129.0 }, "invalid blur"),
        (Prim::Blur { object: 0, revision: 0, radius: f32::INFINITY }, "invalid blur"),
        (
            Prim::Shadow { object: 0, revision: 0, offset: [f32::MAX, 0.0], blur: 1.0, colour: [0.0; 4], outer: false },
            "invalid shadow",
        ),
        (
            Prim::Shadow { object: 0, revision: 0, offset: [0.0; 2], blur: 129.0, colour: [0.0; 4], outer: false },
            "invalid shadow",
        ),
    ] {
        let list = [begin(1.0, Blend::Normal), effect, Prim::LayerEnd];
        assert!(validate(&list, [1, 1], 1.0).unwrap_err().starts_with(message));
        let mut dst = [[0.1; 4]];
        assert!(CpuLayers::default()
            .render(&list, [1, 1], 1.0, Limits::default(), &mut dst, |_, _| panic!("invalid draw"))
            .is_err());
        close(dst[0], [0.1; 4]);
    }
}
#[test]
fn pooled_buffers_and_shadow_cache_stay_inside_a_reduced_budget() {
    let list = [
        begin(1.0, Blend::Normal),
        draw([0.8, 0.2, 0.4, 0.8]),
        Prim::Shadow {
            object: 3,
            revision: 1,
            offset: [-1.0, 0.0],
            blur: 1.0,
            colour: [0.0, 0.0, 1.0, 0.5],
            outer: true,
        },
        Prim::LayerEnd,
    ];
    let mut renderer = CpuLayers::default();
    let mut dst = vec![[0.0; 4]; 3];
    let limits = Limits { depth: 1, bytes: 256 };
    let cold = renderer.render(&list, [3, 1], 1.0, limits, &mut dst, paint).unwrap();
    assert_eq!(cold.passes, 4);
    let warm = renderer.render(&list, [3, 1], 1.0, limits, &mut dst, paint).unwrap();
    assert_eq!(warm.passes, 2);
    assert_eq!(warm.cache_hits, 1);
    assert!(cold.peak_bytes <= limits.bytes);
    assert!(warm.peak_bytes <= limits.bytes);
    renderer.invalidate(3);
    assert_eq!(renderer.render(&list, [3, 1], 1.0, limits, &mut dst, paint).unwrap().cache_hits, 0);
    let flat = renderer.render(&list, [3, 1], 1.0, Limits { depth: 1, bytes: 0 }, &mut dst, paint).unwrap();
    assert!(flat.flattened);
    assert_eq!(flat.peak_bytes, 0);
}

#[test]
fn deep_pool_then_tiny_shadow_budget_reserves_scratch() {
    let mut renderer = CpuLayers::default();
    let mut dst = [[0.0; 4]];
    let mut nested = vec![begin(1.0, Blend::Normal); 16];
    nested.extend(vec![Prim::LayerEnd; 16]);
    renderer.render(&nested, [1, 1], 1.0, Limits::default(), &mut dst, paint).unwrap();
    assert_eq!(renderer.pool.len(), 16);
    let list = [
        begin(1.0, Blend::Normal),
        draw([1.0; 4]),
        Prim::Shadow { object: 1, revision: 1, offset: [0.0; 2], blur: 0.0, colour: [0.0; 4], outer: true },
        Prim::LayerEnd,
    ];
    let report = renderer.render(&list, [1, 1], 1.0, Limits { depth: 16, bytes: 85 }, &mut dst, paint).unwrap();
    assert!(!report.flattened);
    assert!(report.peak_bytes <= 85, "{report:?}");
}
#[test]
fn shared_effect_cache_replaces_edits_and_evicts_least_recently_used() {
    let key = |object, revision| CacheKey::new(object, revision, 1.0, 1.0, [1, 1]);
    let mut cache = EffectCache::default();
    for revision in [1, 2, 3] {
        assert!(cache.prepare(key(1, revision), 2));
        cache.insert(key(1, revision), revision);
    }
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.get(&key(1, 3)), Some(&3));
    assert!(cache.prepare(key(2, 1), 2));
    cache.insert(key(2, 1), 2);
    assert_eq!(cache.get(&key(1, 3)), Some(&3));
    assert!(cache.prepare(key(3, 1), 2));
    cache.insert(key(3, 1), 3);
    assert!(cache.get(&key(2, 1)).is_none());
    assert_eq!(cache.get(&key(1, 3)), Some(&3));
    assert!(!cache.prepare(key(4, 1), 0));
    assert_eq!(pool_capacity(85, 64, 16, 16), 0);
    assert_eq!(pool_capacity(43, 32, 8, 8), 0);
}
#[test]
fn edited_effects_hit_on_repeat_at_two_entry_capacity() {
    for shadow in [false, true] {
        let mut renderer = CpuLayers::default();
        let mut dst = [[0.0; 4]];
        for (revision, hits) in [(1, 0), (2, 0), (3, 0), (3, 1)] {
            let effect = if shadow {
                Prim::Shadow { object: 1, revision, offset: [0.0; 2], blur: 1.0, colour: [0.0; 4], outer: true }
            } else {
                Prim::Blur { object: 1, revision, radius: 1.0 }
            };
            let list = [begin(1.0, Blend::Normal), draw([1.0; 4]), effect, Prim::LayerEnd];
            let report =
                renderer.render(&list, [1, 1], 1.0, Limits { depth: 16, bytes: 128 }, &mut dst, paint).unwrap();
            assert_eq!(report.cache_hits, hits);
            assert!(report.peak_bytes <= 128);
        }
    }
}
#[test]
fn dodge_burn_near_endpoints_have_independent_goldens() {
    for (mode, b, s, expected) in [
        (Blend::ColorDodge, 0.00005, 1.0, 1.0),
        (Blend::ColorDodge, 0.00005, 0.5, 0.0001),
        (Blend::ColorDodge, 0.0, 1.0, 0.0),
        (Blend::ColorBurn, 0.99995, 0.0, 0.0),
        (Blend::ColorBurn, 0.99995, 0.5, 0.9999),
        (Blend::ColorBurn, 1.0, 0.0, 1.0),
    ] {
        close(composite(mode, [b, b, b, 1.0], [s, s, s, 1.0], 1.0), [expected, expected, expected, 1.0]);
    }
}
fn coloured_golden(mode: Blend) -> [f32; 3] {
    // Cb=(.1,.6,.9), Cs=(.8,.2,.4): independent frozen W3C equation values.
    match mode {
        Blend::Normal => [0.8, 0.2, 0.4],
        Blend::Multiply => [0.08, 0.12, 0.36],
        Blend::Screen => [0.82, 0.68, 0.94],
        Blend::Overlay => [0.16, 0.36, 0.88],
        Blend::Darken => [0.1, 0.2, 0.4],
        Blend::Lighten => [0.8, 0.6, 0.9],
        Blend::ColorDodge => [0.5, 0.75, 1.0],
        Blend::ColorBurn => [0.0, 0.0, 0.75],
        Blend::HardLight => [0.64, 0.24, 0.72],
        Blend::SoftLight => [0.2176, 0.456, 0.882],
        Blend::Difference => [0.7, 0.4, 0.5],
        Blend::Exclusion => [0.74, 0.56, 0.58],
        Blend::Hue => [1.0, 0.22060302, 0.48040201],
        Blend::Saturation => [0.19575, 0.57075, 0.79575],
        Blend::Color => [0.881, 0.281, 0.481],
        Blend::Luminosity => [0.019, 0.519, 0.819],
    }
}
