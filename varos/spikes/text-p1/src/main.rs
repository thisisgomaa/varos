//! Small CLI: proof artifacts or one isolated release measurement workload.
use std::{error::Error, path::Path, time::Instant};
use varos_text_spike::*;
mod report;
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("proofs") => report::proofs(Path::new(args.get(2).ok_or("proofs requires output directory")?)),
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
