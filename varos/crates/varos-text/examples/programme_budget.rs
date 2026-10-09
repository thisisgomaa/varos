//! Reproducible headless timings; explicit bundled fonts, no host discovery.
use std::time::Instant;
use varos_text::{composer::*, *};
fn fonts() -> FontSet {
    FontSet::new(
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
    .unwrap()
}
fn summarize(name: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    println!(
        "{name}: n={n} p50={:.3} p95={:.3} max={:.3} ms",
        samples[n / 2],
        samples[(n * 95 / 100).min(n - 1)],
        samples[n - 1]
    );
}
fn main() {
    println!("profile=release; timings include result destruction and sequential source clone/edit; fonts=Inter/Plex bundled; units=ms");
    for count in [10000, 100000] {
        let seed = "السَّلَامُ عليكم، Logo شعار 12.50 موعدنا؟ ";
        let text: String = seed.chars().cycle().take(count).collect();
        let mut e = Engine::new(fonts()).unwrap();
        let start = Instant::now();
        drop(e.layout(&Request::new(&text, 48., Some(600.))).unwrap());
        println!("cold scalars={count}: {:.3} ms", start.elapsed().as_secs_f64() * 1000.);
        let mut samples = Vec::new();
        for _ in 0..41 {
            let t = Instant::now();
            drop(e.layout(&Request::new(&text, 48., Some(600.))).unwrap());
            samples.push(t.elapsed().as_secs_f64() * 1000.);
        }
        summarize(&format!("full scalars={count}"), samples);
        let mut cache = incremental::Incremental::new(fonts(), 64 * 1024 * 1024).unwrap();
        cache.layout(&Request::new(&text, 48., Some(600.)), || false).unwrap();
        let mut current = text.clone();
        let mut samples = Vec::new();
        let mut edit_byte = 0;
        for i in 0..200 {
            let t = Instant::now();
            let mut next = current.clone();
            if i % 2 == 0 {
                let target = match (i / 2) % 3 {
                    0 => 0,
                    1 => next.len() / 2,
                    _ => next.len(),
                };
                edit_byte = target;
                while !next.is_char_boundary(edit_byte) {
                    edit_byte -= 1;
                }
                next.insert(edit_byte, 'ب');
            } else {
                next.remove(edit_byte);
            }
            drop(cache.layout(&Request::new(&next, 48., Some(600.)), || false).unwrap());
            current = next;
            samples.push(t.elapsed().as_secs_f64() * 1000.);
        }
        let oracle = e.layout(&Request::new(&current, 48., Some(600.))).unwrap();
        assert_eq!(cache.layout(&Request::new(&current, 48., Some(600.)), || false).unwrap(), oracle);
        summarize(&format!("incremental scalars={count}"), samples);
        println!("cache bytes={} engine bytes={}", cache.counters.cache_bytes, cache.engine_cache_bytes());
    }
    let text = "السلام عليكم ورحمة الله، موعدنا الثلاثاء الساعة 10:30 صباحًا. ".repeat(15);
    let mut e = Engine::new(fonts()).unwrap();
    for composer in [Composer::Greedy, Composer::EveryLine] {
        let options = ParagraphOptions { composer, ..Default::default() };
        let mut samples = Vec::new();
        for _ in 0..41 {
            let t = Instant::now();
            drop(e.compose(&Request::new(&text, 24., Some(450.)), &options).unwrap());
            samples.push(t.elapsed().as_secs_f64() * 1000.);
        }
        summarize(&format!("compose {composer:?} scalars={}", text.chars().count()), samples);
    }
}
