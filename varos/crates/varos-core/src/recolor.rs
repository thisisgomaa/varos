//! Adapted from VectorCraft color/src/recolor.rs@a469568 (MIT OR Apache-2.0).
use crate::{
    colour_lab::{delta_e2000, srgb_to_lab},
    geom::Rgba,
    model::Paint,
};
pub fn cluster(colors: &[Rgba], weights: &[f32], n: usize) -> Vec<Vec<usize>> {
    let pts: Vec<[f32; 3]> = colors
        .iter()
        .map(|c| {
            let l = srgb_to_lab([c[0], c[1], c[2]]);
            [l.l * 0.5, l.a, l.b]
        })
        .collect();
    let w = |i: usize| weights.get(i).copied().unwrap_or(1.0).max(1e-6);
    let d2 = |a: &[f32; 3], b: &[f32; 3]| (a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2);
    let n = n.max(1);
    if pts.is_empty() {
        return vec![];
    }
    // Seeds: the heaviest colour, then each time the one farthest from every seed so far.
    let first = (0..pts.len()).fold(0, |best, i| if w(i) > w(best) { i } else { best });
    let mut centres = vec![pts[first]];
    let mut near: Vec<f32> = pts.iter().map(|p| d2(p, &pts[first])).collect();
    while centres.len() < n {
        let (i, far) = near.iter().enumerate().fold((0, 0.0f32), |b, (i, d)| if *d > b.1 { (i, *d) } else { b });
        if far <= 1e-6 {
            break;
        }
        centres.push(pts[i]);
        for (j, p) in pts.iter().enumerate() {
            near[j] = near[j].min(d2(p, &pts[i]));
        }
    }
    let nearest = |p: &[f32; 3], cs: &[[f32; 3]]| {
        (0..cs.len()).min_by(|a, b| d2(p, &cs[*a]).total_cmp(&d2(p, &cs[*b]))).unwrap_or(0)
    };
    let mut of: Vec<usize> = pts.iter().map(|p| nearest(p, &centres)).collect();
    for _ in 0..50 {
        let mut sum = vec![([0.0f32; 3], 0.0f32); centres.len()];
        for (i, p) in pts.iter().enumerate() {
            let s = &mut sum[of[i]];
            for (acc, v) in s.0.iter_mut().zip(p) {
                *acc += v * w(i);
            }
            s.1 += w(i);
        }
        for (c, (s, wt)) in centres.iter_mut().zip(&sum) {
            if *wt > 0.0 {
                *c = s.map(|v| v / wt);
            }
        }
        let next: Vec<usize> = pts.iter().map(|p| nearest(p, &centres)).collect();
        if next == of {
            break;
        }
        of = next;
    }
    let mut rows: Vec<Vec<usize>> = vec![vec![]; centres.len()];
    for (i, c) in of.into_iter().enumerate() {
        rows[c].push(i);
    }
    rows.retain(|r| !r.is_empty());
    rows.sort_by_key(|r| r[0]);
    rows
}

pub fn nearest(c: Rgba, palette: &[Rgba]) -> Rgba {
    let lab = srgb_to_lab([c[0], c[1], c[2]]);
    palette
        .iter()
        .min_by(|a, b| {
            delta_e2000(srgb_to_lab([a[0], a[1], a[2]]), lab)
                .total_cmp(&delta_e2000(srgb_to_lab([b[0], b[1], b[2]]), lab))
        })
        .map_or(c, |p| [p[0], p[1], p[2], c[3]])
}
pub fn map_paint(p: &Paint, palette: &[Rgba]) -> Paint {
    match p {
        Paint::Solid(c) => Paint::Solid(nearest(*c, palette)),
        Paint::Gradient(g) => {
            let mut g = g.clone();
            for s in &mut g.stops {
                s.colour = nearest(s.colour, palette);
            }
            Paint::Gradient(g)
        }
        _ => p.clone(),
    }
}

pub fn selected_colours(ed: &crate::Editor) -> Vec<Rgba> {
    let ids = ed.selected_pids();
    let mut out = vec![];
    for p in &ed.doc.paths {
        if !ids.contains(&p.id) {
            continue;
        }
        for paint in [p.appearance().fill(), p.appearance().stroke()] {
            match paint.resolved(&ed.doc) {
                Paint::Solid(c) => {
                    if !out.contains(&c) {
                        out.push(c);
                    }
                }
                Paint::Gradient(g) => {
                    for s in g.stops {
                        if !out.contains(&s.colour) {
                            out.push(s.colour);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    out
}
pub fn reduced(colours: &[Rgba], count: usize) -> Vec<Rgba> {
    cluster(colours, &[], count)
        .into_iter()
        .map(|row| {
            let mut mean = crate::colour_lab::Lab::default();
            for i in &row {
                let c = colours[*i];
                let lab = srgb_to_lab([c[0], c[1], c[2]]);
                mean.l += lab.l;
                mean.a += lab.a;
                mean.b += lab.b;
            }
            let n = row.len() as f32;
            let rgb = crate::colour_lab::lab_to_srgb(crate::colour_lab::Lab::new(mean.l / n, mean.a / n, mean.b / n));
            [rgb[0], rgb[1], rgb[2], 1.]
        })
        .collect()
}
