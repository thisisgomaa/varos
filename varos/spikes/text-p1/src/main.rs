//! Small CLI: proof artifacts or one isolated release measurement workload.
use std::{error::Error, path::Path, time::Instant};
use varos_text_spike::*;
mod report;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("profile") => {
            let text: String = "Logo شعار 12 ".chars().cycle().take(100000).collect();
            let mut engine = Engine::default();
            for i in 0..6 {
                let mut last = Instant::now();
                let l = engine.layout_instrumented(&Request::new(&text, 48., Some(600.)), |stage| {
                    let now = Instant::now();
                    println!("{i} {stage} {:.3}ms", now.duration_since(last).as_secs_f64() * 1000.);
                    last = now;
                })?;
                std::hint::black_box(l);
            }
            Ok(())
        }
        Some("draw-bench") => draw_benchmark(),
        Some("downsample-4x") => {
            let input = tiny_skia::Pixmap::load_png(args.get(2).ok_or("input PNG")?)?;
            proof::downsample_4x(&input)?.save_png(args.get(3).ok_or("output PNG")?)?;
            Ok(())
        }
        Some("compare-png") => {
            let a = tiny_skia::Pixmap::load_png(args.get(2).ok_or("first PNG")?)?;
            let b = tiny_skia::Pixmap::load_png(args.get(3).ok_or("second PNG")?)?;
            let d = proof::alpha_diff(&a, &b);
            println!("{d:#?}");
            if d.mean_delta > 1. || d.severe > 0 {
                Err("pixel parity failed".into())
            } else {
                Ok(())
            }
        }
        Some("parity") => {
            std::fs::write(args.get(2).ok_or("parity output")?, parity::fixtures())?;
            Ok(())
        }
        Some("proofs") => report::proofs(Path::new(args.get(2).ok_or("proofs requires output directory")?)),
        Some("edits") => {
            sequential(args.get(2).ok_or("count")?.parse()?, args.get(3).ok_or("kind")?, args.get(4).ok_or("position")?)
        }
        Some("bench") => benchmark(args.get(2).ok_or("bench requires scalar count")?.parse()?),
        _ => Err("throwaway P1: use `proofs DIRECTORY` or `bench 1000|10000|100000`".into()),
    }
}
fn benchmark(count: usize) -> Result<(), Box<dyn Error>> {
    if ![1000, 10000, 100000].contains(&count) {
        return Err("unsupported workload".into());
    }
    let text: String = "Logo شعار 12 ".chars().cycle().take(count).collect();
    let start = Instant::now();
    let mut engine = Engine::default();
    let font_ms = start.elapsed().as_secs_f64() * 1000.;
    let request = Request::new(&text, 48., Some(600.));
    let start = Instant::now();
    let result = engine.layout(&request)?;
    let cold_ms = start.elapsed().as_secs_f64() * 1000.;
    let mut edits = Vec::new();
    let middle = text.char_indices().nth(count / 2).unwrap().0;
    // Insert/remove one ASCII character in a warm engine; full paragraph relayout,
    // no synthetic cached result and no outlines/proof raster in this timing.
    for i in 0..41 {
        let mut edited = text.clone();
        edited.insert(middle, if i % 2 == 0 { 'X' } else { 'Y' });
        let start = Instant::now();
        let layout = engine.layout(&Request::new(&edited, 48., Some(600.)))?;
        std::hint::black_box(&layout);
        edits.push(start.elapsed().as_secs_f64() * 1000.);
    }
    edits.sort_by(f64::total_cmp);
    println!(
        "| {count} | {} | {font_ms:.3} | {cold_ms:.3} | {:.3} | {:.3} | {} | {} |",
        text.len(),
        edits[20],
        edits[38],
        result.lines.len(),
        result.lines.iter().map(|l| l.glyphs.len()).sum::<usize>()
    );
    println!("warm_ms_sorted={edits:?}");
    Ok(())
}

fn sequential(count: usize, kind: &str, position: &str) -> Result<(), Box<dyn Error>> {
    use varos_text_spike::incremental::Incremental;
    if ![10000, 100000].contains(&count)
        || !["long", "many"].contains(&kind)
        || !["start", "middle", "end"].contains(&position)
    {
        return Err("unsupported sequential workload".into());
    }
    let mut text: String = "Logo شعار 12 ".chars().cycle().take(count).collect();
    if kind == "many" {
        text = text.chars().enumerate().map(|(i, c)| if i % 100 == 99 { '\n' } else { c }).collect();
    }
    let mut cache = Incremental::default();
    let start = Instant::now();
    let cold = cache.layout(&Request::new(&text, 48., Some(600.)), || false)?;
    let cold_ms = start.elapsed().as_secs_f64() * 1000.;
    drop(cold);
    let mut samples = Vec::new();
    let mut reshaped = 0;
    let mut hits = 0;
    let mut cached = 0;
    let mut insertion = 0;
    for i in 0..204 {
        let start = Instant::now();
        if i % 2 == 0 {
            insertion = match position {
                "start" => 0,
                "end" => text.len(),
                _ => text.char_indices().nth(text.chars().count() / 2).unwrap().0,
            };
            text.insert(insertion, 'X');
        } else {
            text.remove(insertion);
        }
        let layout = cache.layout(&Request::new(&text, 48., Some(600.)), || false)?;
        std::hint::black_box(&layout);
        drop(layout);
        samples.push(start.elapsed().as_secs_f64() * 1000.);
        reshaped += cache.counters.reshaped;
        hits += cache.counters.hits;
        cached = cached.max(cache.counters.cache_bytes);
        if i % 50 == 0 {
            // Correctness checks are outside the timed edit/update interval.
            let r = Request::new(&text, 48., Some(600.));
            let warm = cache.layout(&r, || false)?;
            let cold = Engine::default().layout(&r)?;
            assert_eq!(warm, cold, "cached output must equal full recomputation");
        }
    }
    println!("sequential_ms={samples:?}");
    samples.sort_by(f64::total_cmp);
    let (span_hits, span_misses) = cache.shape_span_cache_stats();
    println!("shape_span_cache_hits={span_hits} shape_span_cache_misses={span_misses}");
    println!("count={count} kind={kind} position={position} samples={} cold_ms={cold_ms:.3} p50_ms={:.3} p95_ms={:.3} max_ms={:.3} reshaped={reshaped} hits={hits} paragraph_cache_bytes={cached} engine_cache_bytes={}",samples.len(),samples[102],samples[193],samples[203],cache.engine_cache_bytes());
    Ok(())
}

fn draw_benchmark() -> Result<(), Box<dyn Error>> {
    let mut engine = Engine::default();
    let layout = engine.layout(&Request::new("Logo شعار ۶ السَّلَامُ", 48., Some(600.)))?;
    let mut outlines_ms = Vec::new();
    let mut draw_ms = Vec::new();
    let mut pdf_ms = Vec::new();
    let mut pdf_size = 0;
    for _ in 0..41 {
        let t = Instant::now();
        let outlines = outlines::layout_outlines(&layout)?;
        outlines_ms.push(t.elapsed().as_secs_f64() * 1000.);
        let t = Instant::now();
        let pixels =
            proof::nonzero_coverage(&outlines, 1400, 300, tiny_skia::Transform::from_scale(2., 2.), None, 255)?;
        std::hint::black_box(&pixels);
        draw_ms.push(t.elapsed().as_secs_f64() * 1000.);
        let t = Instant::now();
        let pdf = export::pdf(&outlines, 700., 150.);
        pdf_size = pdf.len();
        std::hint::black_box(&pdf);
        pdf_ms.push(t.elapsed().as_secs_f64() * 1000.);
    }
    for (name, mut values) in [("outlines", outlines_ms), ("native_nonzero_draw", draw_ms), ("pdf_emit", pdf_ms)] {
        values.sort_by(f64::total_cmp);
        println!("{name} samples=41 p50_ms={:.3} p95_ms={:.3} max_ms={:.3}", values[20], values[38], values[40]);
    }
    println!("fixture=Logo/Arabic/Plex/marks 48pt 600pt_width 1400x300px pdf_bytes={pdf_size}");
    Ok(())
}
