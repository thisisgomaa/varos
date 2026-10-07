//! Width/font/language invalidation timings, separate from character-edit ceilings.
use std::time::Instant;
use varos_text_spike::{incremental::Incremental, *};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let count: usize = args[1].parse().unwrap();
    let kind = &args[2];
    let control = &args[3];
    assert!([10000, 100000].contains(&count));
    assert!(["long", "many"].contains(&kind.as_str()));
    assert!(["width", "font", "language"].contains(&control.as_str()));
    let text: String = "Logo شعار 12 "
        .chars()
        .cycle()
        .take(count)
        .enumerate()
        .map(|(i, c)| if kind == "many" && i % 100 == 99 { '\n' } else { c })
        .collect();
    let mut cache = Incremental::default();
    cache.layout(&Request::new(&text, 48., Some(600.)), || false).unwrap();
    let mut times = Vec::new();
    let mut reshaped = 0;
    let mut hits = 0;
    let (mut max_gap, mut gap_at) = (0f64, String::new());
    let (mut runs_shaped, mut lines_refit, mut analysis_reused) = (0, 0, 0);
    for i in 0..204 {
        let t = Instant::now();
        let mut req = Request::new(&text, 48., Some(600.));
        match control.as_str() {
            "width" => req.width = Some(if i % 2 == 0 { 120. } else { 600. }),
            "font" => req.face = if i % 2 == 0 { Face::Plex } else { Face::Inter },
            "language" => req.language = ["ar", "fa", "ur", "und"][i % 4],
            _ => unreachable!(),
        }
        let layout = cache.layout(&req, || false).unwrap();
        std::hint::black_box(&layout);
        drop(layout);
        times.push(t.elapsed().as_secs_f64() * 1000.);
        reshaped += cache.counters.reshaped;
        hits += cache.counters.hits;
        let w = &cache.counters.work;
        if w.max_poll_gap_ms > max_gap {
            (max_gap, gap_at) = (w.max_poll_gap_ms, w.max_poll_gap_at.clone());
        }
        runs_shaped += w.runs_shaped;
        lines_refit += w.lines_refit;
        analysis_reused += w.analysis_reused;
        if i % 50 == 0 {
            assert_eq!(cache.layout(&req, || false).unwrap(), Engine::default().layout(&req).unwrap());
        }
    }
    println!("control_ms={times:?}");
    println!("runs_shaped={runs_shaped} lines_refit={lines_refit} analysis_reused={analysis_reused} max_poll_gap_ms={max_gap:.3} at {gap_at}");
    times.sort_by(f64::total_cmp);
    println!("count={count} kind={kind} control={control} samples=204 p50_ms={:.3} p95_ms={:.3} max_ms={:.3} reshaped={reshaped} hits={hits} cache_bytes={}",times[102],times[193],times[203],cache.counters.cache_bytes+cache.engine_cache_bytes());
}
