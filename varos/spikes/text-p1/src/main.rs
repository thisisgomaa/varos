//! Small CLI: proof artifacts or one isolated release measurement workload.
use std::{error::Error, path::Path, time::Instant};
use varos_text_spike::*;
mod report;

/// Counting allocator for heap measurements in the benchmark binary only
/// (live and peak bytes; a few relaxed atomics per allocation, included in timings).
mod heap {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
    pub static LIVE: AtomicUsize = AtomicUsize::new(0);
    pub static PEAK: AtomicUsize = AtomicUsize::new(0);
    pub struct Counting;
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(layout) };
            if !p.is_null() {
                let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
                PEAK.fetch_max(live, Relaxed);
            }
            p
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) };
            LIVE.fetch_sub(layout.size(), Relaxed);
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let p = unsafe { System.realloc(ptr, layout, new_size) };
            if !p.is_null() {
                if new_size >= layout.size() {
                    let live = LIVE.fetch_add(new_size - layout.size(), Relaxed) + new_size - layout.size();
                    PEAK.fetch_max(live, Relaxed);
                } else {
                    LIVE.fetch_sub(layout.size() - new_size, Relaxed);
                }
            }
            p
        }
    }
    /// Reset the peak to the current live heap and return it.
    pub fn mark() -> usize {
        let live = LIVE.load(Relaxed);
        PEAK.store(live, Relaxed);
        live
    }
    pub fn peak() -> usize {
        PEAK.load(Relaxed)
    }
    pub fn live() -> usize {
        LIVE.load(Relaxed)
    }
}
#[global_allocator]
static GLOBAL: heap::Counting = heap::Counting;
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
        // Golden identity without rasterization: the exact `corpus-layouts.txt`
        // text `proofs` writes, from the cold engine and from the cached path.
        Some("corpus-dump") => corpus_dump(Path::new(args.get(2).ok_or("corpus-dump requires output directory")?)),
        Some("edits") => sequential(
            args.get(2).ok_or("count")?.parse()?,
            args.get(3).ok_or("kind")?,
            args.get(4).ok_or("position")?,
            args.get(5).map_or("motif", String::as_str),
        ),
        // Slice probe at the 1 MiB text cap: one cold layout and one middle edit.
        Some("cap-probe") => {
            let mut text: String = "Logo شعار 12 ".chars().cycle().take(800_000).collect();
            while text.len() > 1_048_576 {
                text.pop();
            }
            let mut cache = varos_text_spike::incremental::Incremental::default();
            for step in ["cold", "edit"] {
                if step == "edit" {
                    let at = text.char_indices().nth(text.chars().count() / 2).unwrap().0;
                    text.insert(at, 'X');
                }
                let t = Instant::now();
                let layout = cache.layout(&Request::new(&text, 48., Some(600.)), || false)?;
                let w = &cache.counters.work;
                println!(
                    "{step} bytes={} lines={} ms={:.3} max_poll_gap_ms={:.3} at {} cache_bytes={} evicted={}",
                    text.len(),
                    layout.lines.len(),
                    t.elapsed().as_secs_f64() * 1000.,
                    w.max_poll_gap_ms,
                    w.max_poll_gap_at,
                    cache.counters.cache_bytes,
                    cache.counters.evicted
                );
            }
            Ok(())
        }
        Some("bench") => benchmark(args.get(2).ok_or("bench requires scalar count")?.parse()?),
        _ => Err("throwaway P1: use `proofs DIRECTORY` or `bench 1000|10000|100000`".into()),
    }
}
fn corpus_dump(dir: &Path) -> Result<(), Box<dyn Error>> {
    use std::fmt::Write as _;
    std::fs::create_dir_all(dir)?;
    let mut engine = Engine::default();
    let mut cached = varos_text_spike::incremental::Incremental::default();
    let (mut cold_dump, mut cached_dump) = (String::new(), String::new());
    for row in &CORPUS {
        for (input, text) in row.texts.iter().enumerate() {
            for face in [Face::Inter, Face::Plex] {
                for size in SIZES_PT {
                    for width in [None, Some(120.), Some(600.)] {
                        let mut request = Request::new(text, size, width);
                        request.face = face;
                        let layout = engine.layout(&request)?;
                        writeln!(cold_dump, "{} input={input} {face:?} {size}pt width={width:?}\n{layout:#?}", row.id)?;
                        let layout = cached.layout(&request, || false)?;
                        writeln!(
                            cached_dump,
                            "{} input={input} {face:?} {size}pt width={width:?}\n{layout:#?}",
                            row.id
                        )?;
                    }
                }
            }
        }
    }
    std::fs::write(dir.join("corpus-layouts.txt"), &cold_dump)?;
    std::fs::write(dir.join("corpus-layouts-cached.txt"), &cached_dump)?;
    println!("cold_equals_cached={}", cold_dump == cached_dump);
    Ok(())
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

/// Diagnostic corpora beyond the gate motif: `words` (varied pseudo-random
/// Latin/Arabic words, so line slack varies) and `arabic` (marked Arabic prose:
/// one long RTL level run). Same scalar counts; not substitutes for the motif.
fn corpus_text(count: usize, text_kind: &str) -> Result<String, Box<dyn Error>> {
    Ok(match text_kind {
        "motif" => "Logo شعار 12 ".chars().cycle().take(count).collect(),
        "arabic" => {
            "السَّلَامُ عَلَيْكُمْ وَرَحْمَةُ اللهِ، مَوْعِدُنَا يَوْمَ الثُّلَاثَاءِ السَّاعَةَ ١٠:٣٠ صَبَاحًا. ".chars().cycle().take(count).collect()
        }
        "words" => {
            const WORDS: &[&str] =
                &["Logo", "شعار", "12", "السلام", "عليكم", "office", "café", "ورحمة", "v2", "الله", "typography", "و"];
            let mut x = 7u64;
            let mut out = String::new();
            while out.chars().count() < count {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                out.push_str(WORDS[(x % WORDS.len() as u64) as usize]);
                out.push(' ');
            }
            out.chars().take(count).collect()
        }
        _ => return Err("unsupported text kind".into()),
    })
}

fn sequential(count: usize, kind: &str, position: &str, text_kind: &str) -> Result<(), Box<dyn Error>> {
    use varos_text_spike::incremental::Incremental;
    if ![10000, 100000].contains(&count)
        || !["long", "many"].contains(&kind)
        || !["start", "middle", "end"].contains(&position)
    {
        return Err("unsupported sequential workload".into());
    }
    let mut text = corpus_text(count, text_kind)?;
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
    let mut work = varos_text_spike::converge::WorkCounters::default();
    let mut max_refit = 0;
    let mut max_shaped = 0;
    let mut max_transient = 0;
    let mut max_live = 0;
    let mut gap_at = (0f64, String::new());
    let mut profile: Vec<(&'static str, &'static str, f64, f64, usize)> = Vec::new();
    // Cooperative cancellation slices: the longest interval between successive
    // polls, including entry-to-first and last-to-return, over every timed edit.
    let last_poll = std::cell::Cell::new(Instant::now());
    let max_gap = std::cell::Cell::new(0f64);
    let poll = || {
        let now = Instant::now();
        max_gap.set(max_gap.get().max(now.duration_since(last_poll.get()).as_secs_f64() * 1000.));
        last_poll.set(now);
        false
    };
    for i in 0..204 {
        let before = heap::mark();
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
        last_poll.set(Instant::now());
        let layout = cache.layout(&Request::new(&text, 48., Some(600.)), poll)?;
        poll();
        std::hint::black_box(&layout);
        drop(layout);
        samples.push(start.elapsed().as_secs_f64() * 1000.);
        max_transient = max_transient.max(heap::peak().saturating_sub(before));
        max_live = max_live.max(heap::live());
        let w = &cache.counters.work;
        if w.max_poll_gap_ms > gap_at.0 {
            gap_at = (w.max_poll_gap_ms, format!("edit {i}: {}", w.max_poll_gap_at));
        }
        for st in &w.stretches {
            match profile.iter_mut().find(|p| p.0 == st.0 && p.1 == st.1) {
                Some(p) => {
                    p.2 += st.2;
                    p.3 = p.3.max(st.3);
                    p.4 += st.4;
                }
                None => profile.push(*st),
            }
        }
        max_refit = max_refit.max(w.lines_refit);
        max_shaped = max_shaped.max(w.runs_shaped);
        for (total, add) in [
            (&mut work.analysis_full, w.analysis_full),
            (&mut work.analysis_incremental, w.analysis_incremental),
            (&mut work.analysis_reused, w.analysis_reused),
            (&mut work.runs_shaped, w.runs_shaped),
            (&mut work.bytes_shaped, w.bytes_shaped),
            (&mut work.segments_rebuilt, w.segments_rebuilt),
            (&mut work.units_rebuilt, w.units_rebuilt),
            (&mut work.units_total, w.units_total),
            (&mut work.lines_refit, w.lines_refit),
            (&mut work.lines_reused, w.lines_reused),
            (&mut work.edge_reshapes, w.edge_reshapes),
            (&mut work.fallback, w.fallback),
            (&mut work.cancel_checks, w.cancel_checks),
        ] {
            *total += add;
        }
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
    println!("work_totals={work:?}");
    println!("engine_max_poll_gap_ms={:.3} at {}", gap_at.0, gap_at.1);
    profile.sort_by(|a, b| b.2.total_cmp(&a.2));
    for (from, to, total, max, count) in &profile {
        println!("stage {from} -> {to}: mean_per_edit_ms={:.3} max_slice_ms={max:.3} slices={count}", total / 204.);
    }
    println!(
        "per_edit_max lines_refit={max_refit} runs_shaped={max_shaped} cancel_max_gap_ms={:.3} transient_heap_peak_bytes={max_transient} live_heap_peak_bytes={max_live}",
        max_gap.get()
    );
    println!("count={count} kind={kind} position={position} text={text_kind} samples={} cold_ms={cold_ms:.3} p50_ms={:.3} p95_ms={:.3} max_ms={:.3} reshaped={reshaped} hits={hits} paragraph_cache_bytes={cached} engine_cache_bytes={}",samples.len(),samples[102],samples[193],samples[203],cache.engine_cache_bytes());
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
