//! Diagnostic only: tighter geometry does not change the acceptance renderer.
use varos_text_spike::{outlines::*, proof::*, *};
fn main() {
    let mut e = Engine::default();
    println!("text\tsize\tflatten_pt\tmean_alpha\tmax_alpha\tpixels_above_32");
    for text in ["Logo شعار ۶ السَّلَامُ", "OoQ8@", "بِبَّ", "قُرْآنٌ"] {
        for size in [12., 48., 200.] {
            let l = e.layout(&Request::new(text, size, None)).unwrap();
            let o = layout_outlines(&l).unwrap();
            let b = ink_bounds(&o).unwrap();
            let w = ((b[2] - b[0]) * 2.).ceil() as u32 + 12;
            let h = ((b[3] - b[1]) * 2.).ceil() as u32 + 12;
            let t = tiny_skia::Transform::from_row(2., 0., 0., 2., 6. - b[0] * 2., 6. - b[1] * 2.);
            let direct = nonzero_coverage_single_sample(&o, w, h, t, None, 255).unwrap();
            for tol in [0.005, 0.001, 0.0001] {
                let flat = nonzero_coverage_single_sample(&o, w, h, t, Some(tol), 255).unwrap();
                let d = alpha_diff(&direct, &flat);
                println!("{text}\t{size}\t{tol}\t{:.9}\t{}\t{}", d.mean_delta, d.max_delta, d.severe);
            }
        }
    }
}
