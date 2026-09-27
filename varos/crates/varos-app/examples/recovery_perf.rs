//! CPU/disk-only F1 timing probe: cargo run -p varos-app --example recovery_perf --release
//! Scene B from perf_harness, scaled from 500 rectangles to 5,000; no renderer/event loop.
use std::{hint::black_box, time::Instant};
use varos_app::storage::{
    checksum::new_nonce,
    durable::{write_replace, RealFs, WriteOutcome},
};
use varos_core::model::{Anchor, Document, Path};

fn rectangles(count: u32) -> Document {
    let mut doc = Document::default();
    for i in 0..count {
        let (x, y) = ((i % 25) as f32 * 34.0, (i / 25) as f32 * 34.0);
        let anchors = [[x, y], [x + 26.0, y], [x + 26.0, y + 26.0], [x, y + 26.0]]
            .into_iter()
            .enumerate()
            .map(|(j, p)| Anchor { id: 100_000 + i * 4 + j as u32, p, hin: None, hout: None, smooth: false })
            .collect();
        doc.paths.push(Path::new(
            10_000 + i,
            anchors,
            true,
            Some([0.72, 0.34, 0.20, 1.0]),
            Some([0.08, 0.08, 0.09, 1.0]),
            1.5,
        ));
    }
    doc.ids = 100_000 + count * 4;
    doc.sync_tree();
    doc
}
fn main() {
    let doc = rectangles(5_000);
    let mut samples = Vec::new();
    for _ in 0..100 {
        let at = Instant::now();
        let copy = black_box(black_box(&doc).clone());
        samples.push(at.elapsed().as_secs_f64() * 1000.0);
        drop(copy);
    }
    samples.sort_by(f64::total_cmp);
    println!("Scene B ×10 / 5,000 rectangles: clone median {:.3} ms, p95 {:.3} ms", samples[50], samples[95]);
    let mut count = 5_000;
    let mut doc = doc;
    while varos_pdf::write_pdf(&doc).unwrap().len() < 10_000_000 {
        count *= 2;
        doc = rectangles(count);
    }
    let path = std::env::temp_dir().join(format!("varos-f1-perf-{}.vrs", new_nonce()));
    let at = Instant::now();
    let bytes = varos_pdf::write_pdf(&doc).unwrap();
    let outcome = write_replace(&RealFs, &path, &bytes, &new_nonce()).unwrap();
    let elapsed = at.elapsed();
    assert!(matches!(outcome, WriteOutcome::Durable));
    println!(
        "Synchronous PDF encode + durable save: {} rectangles, {} bytes, {:.3} ms",
        count,
        bytes.len(),
        elapsed.as_secs_f64() * 1000.0
    );
    std::fs::remove_file(path).unwrap();
}
