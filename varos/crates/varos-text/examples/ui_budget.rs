//! One short release run; excludes font initialization from per-label layout cost.
use std::{hint::black_box, time::Instant};
use varos_text::*;
fn main() {
    let init = Instant::now();
    let fonts = FontSet::new(
        vec![
            FontFace::new("Inter", 400, include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice().into()).unwrap(),
            FontFace::new(
                "IBM Plex Sans Arabic",
                400,
                include_bytes!("../assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice().into(),
            )
            .unwrap(),
        ],
        FallbackPolicy { common: vec![FaceId(1), FaceId(0)], scripts: vec![] },
    )
    .unwrap();
    let mut engine = Engine::new(fonts).unwrap();
    println!("font initialization ms: {:.3}", init.elapsed().as_secs_f64() * 1000.);
    // Fresh engines isolate first face use; warm-engine misses use different words,
    // rather than warming the same shape runs with only a numeric suffix changed.
    let names: Vec<String> = (0..41)
        .map(|n| {
            let mut seed = n as u32 + 1;
            let words: String = (0..24)
                .map(|_| {
                    seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                    char::from(b'a' + ((seed >> 16) % 26) as u8)
                })
                .collect();
            format!("لوحة {words} Café {words} شعار ").chars().cycle().take(120).collect()
        })
        .collect();
    assert_eq!(names.iter().collect::<std::collections::HashSet<_>>().len(), 41);
    for cold in [true, false] {
        if !cold {
            engine.layout(&Request::new("لوحة Café warmup", 12., None)).unwrap();
        }
        let mut times = Vec::new();
        for name in &names {
            let mut fresh;
            let sample_engine = if cold {
                fresh = Engine::new(engine.font_set().clone()).unwrap();
                &mut fresh
            } else {
                &mut engine
            };
            let start = Instant::now();
            black_box(sample_engine.layout(&Request::new(name, 12., None)).unwrap());
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
        let first = times[0];
        times.sort_by(f64::total_cmp);
        println!(
            "{} 120 scalars (41 samples) ms: first={first:.6} p50={:.6} p95={:.6} max={:.6}",
            if cold { "cold-first (fresh Engine per sample)" } else { "warm-engine label-miss (distinct words)" },
            times[20],
            times[38],
            times[40]
        );
    }
    let mut cache = LabelCache::new(&engine);
    let key = LabelKey {
        text: "لوحة Café logo".into(),
        face: FaceId(0),
        size_bits: 12f32.to_bits(),
        width_bits: None,
        role: 0,
        ppp_bits: 2f32.to_bits(),
    };
    cache.layout(&mut engine, &key, 0).unwrap();
    let mut vertices = Vec::<[f32; 2]>::with_capacity(512);
    let start = Instant::now();
    for _ in 0..1000 {
        let layout = cache.get(black_box(&key), 1).unwrap();
        vertices.clear();
        for g in layout.lines.iter().flat_map(|l| &l.glyphs) {
            vertices.extend_from_slice(&[
                [g.x, g.y - g.size],
                [g.x + g.advance, g.y - g.size],
                [g.x + g.advance, g.y],
                [g.x, g.y],
            ]);
        }
        black_box(&vertices);
    }
    println!("warm lookup + CPU quad positions us/label (1000): {:.6}", start.elapsed().as_secs_f64() * 1e6 / 1000.);
    println!("label cache entries/payload: {:?}; caps: {LABEL_ENTRIES}/{LABEL_BYTES}", cache.stats());
    let mut full = LabelCache::new(&engine);
    let full_key = LabelKey { text: "O".into(), ..key.clone() };
    for role in 0..LABEL_ENTRIES as u32 {
        full.layout(&mut engine, &LabelKey { role, ..full_key.clone() }, 1).unwrap();
    }
    assert_eq!(full.stats().0, LABEL_ENTRIES);
    // Expiration is a once-per-frame host cost, outside repeated warm lookups.
    full.expire(2);
    let start = Instant::now();
    for _ in 0..1000 {
        black_box(full.layout(&mut engine, black_box(&full_key), 2).unwrap());
    }
    println!(
        "full-cache warm layout us/label (1000): {:.6}; entries/payload: {:?}",
        start.elapsed().as_secs_f64() * 1e6 / 1000.,
        full.stats()
    );
    println!("engine cache payload bytes (separate): {}", engine.cache_bytes());
    println!("Atlas lookup, UV/colour/index emission and GPU upload are T2; this measures CPU positions only.");
}
