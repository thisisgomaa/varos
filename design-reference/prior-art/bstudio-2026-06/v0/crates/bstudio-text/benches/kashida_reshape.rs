//! Block B5 — kashida re-shape benchmark.
//!
//! Measures `insert_kashida` (the moat) across realistic inputs to validate
//! the 60fps budget (< 16 ms per call). Also bencs `shape_paragraph` for
//! mixed-script BiDi input to confirm Block D adds acceptable overhead.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use bstudio_text::{insert_kashida, shape_paragraph};

const AMIRI: &[u8] = include_bytes!("../../../assets/fonts/Amiri-Regular.ttf");

// Cases — (name, text, n_kashida)
const BASMALA: &str = "بسم الله الرحمن الرحيم";
const LONG_LINE: &str = "الحمد لله رب العالمين الرحمن الرحيم مالك يوم الدين";
const MIXED: &str = "مرحبا Pivot Studio العربية";

fn bench_insert_kashida(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_kashida");

    for (name, text, n) in [
        ("basmala_0k", BASMALA, 0),
        ("basmala_3k", BASMALA, 3),
        ("basmala_8k", BASMALA, 8),
        ("long_line_3k", LONG_LINE, 3),
        ("mixed_3k", MIXED, 3),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &(text, n),
            |b, &(t, k)| {
                b.iter(|| {
                    let res = insert_kashida(black_box(AMIRI), black_box(t), black_box(k))
                        .expect("kashida insert");
                    black_box(res);
                });
            },
        );
    }
    group.finish();
}

fn bench_shape_paragraph(c: &mut Criterion) {
    let mut group = c.benchmark_group("shape_paragraph");
    group.bench_function("mixed_bidi", |b| {
        b.iter(|| {
            let runs = shape_paragraph(black_box(AMIRI), black_box("موقع Pivot Studio للتصميم"))
                .expect("shape_paragraph");
            black_box(runs);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_insert_kashida, bench_shape_paragraph);
criterion_main!(benches);
