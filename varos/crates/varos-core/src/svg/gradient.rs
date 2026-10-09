//! Lane B: user-space SVG gradient definitions; shared midpoint LUT, no paint averaging.
use super::*;
use crate::{
    gradient::{GradientKind, Spread},
    model::Paint,
};
fn reference(out: &mut String, paint: &Paint, key: &str, xf: Xform) -> (String, f32) {
    match paint {
        Paint::None => ("none".into(), 1.),
        Paint::Solid(c) => (color(*c), c[3]),
        Paint::Gradient(g) => {
            let g = g.transformed(xf);
            let kind = if g.kind == GradientKind::Linear { "linearGradient" } else { "radialGradient" };
            let coords = if g.kind == GradientKind::Linear {
                "x1=\"0\" y1=\"0\" x2=\"1\" y2=\"0\"".into()
            } else {
                format!("cx=\"0\" cy=\"0\" r=\"1\" fx=\"{}\" fy=\"{}\"", num(g.focal[0]), num(g.focal[1]))
            };
            let spread = match g.spread {
                Spread::Pad => "pad",
                Spread::Reflect => "reflect",
                Spread::Repeat => "repeat",
            };
            out.push_str(&format!("<defs><{kind} id=\"{key}\" gradientUnits=\"userSpaceOnUse\" {coords} spreadMethod=\"{spread}\" gradientTransform=\"matrix({})\">",numbers(&g.placement, None)));
            // Explicit stop breakpoints preserve hard transitions; midpoint samples preserve the engine curve.
            let mut samples: Vec<(f32, crate::Rgba)> = g
                .stops
                .iter()
                .map(|s| (s.offset, [s.colour[0], s.colour[1], s.colour[2], s.colour[3] * s.opacity]))
                .collect();
            for w in g.stops.windows(2) {
                for i in 1..32 {
                    let t = w[0].offset + (w[1].offset - w[0].offset) * i as f32 / 32.;
                    samples.push((t, g.sample_pad(t)));
                }
                let t = w[0].offset + (w[1].offset - w[0].offset) * w[0].midpoint;
                samples.push((t, g.sample_pad(t)));
            }
            samples.sort_by(|a, b| a.0.total_cmp(&b.0));
            for (t, c) in samples {
                out.push_str(&format!(
                    "<stop offset=\"{t:.8}\" stop-color=\"{}\" stop-opacity=\"{}\"/>",
                    color(c),
                    num(c[3])
                ));
            }
            out.push_str(&format!("</{kind}></defs>\n"));
            (format!("url(#{key})"), 1.)
        }
        _ => ("none".into(), 1.),
    }
}
pub(super) fn paint(out: &mut String, d: &Drawn<'_>, doc: &Document) -> Result<(), ExportError> {
    let p = d.p;
    let f = p.appearance().fill().resolved(doc);
    let s = p.appearance().stroke().resolved(doc);
    let (fill, fa) = reference(out, &f, &format!("gradient-{}-fill", p.id), d.xf);
    let (stroke, sa) = reference(out, &s, &format!("gradient-{}-stroke", p.id), d.xf);
    let data = path_data(p, &d.xf, None);
    out.push_str(&format!(
        "<g id=\"{}\" opacity=\"{}\">\n",
        id("path", p.id as usize, p.name.as_deref().unwrap_or("")),
        num(p.opacity)
    ));
    if let Some(name) = &p.name {
        title(out, name);
    }
    let cov = crate::stroke::evaluate(p, 0.01, &|| false).map_err(stroke_error)?;
    let band = stroke::coverage_data(&cov.rings, &d.xf, None);
    let translucent = match &s {
        Paint::Gradient(g) => g.stops.iter().any(|s| s.colour[3] * s.opacity < 0.999),
        Paint::Solid(c) => c[3] < 0.999,
        _ => false,
    };
    let knockout = f.is_painted() && s.is_painted() && p.stroke_width > 0. && translucent;
    if knockout {
        let b = d.bbox;
        out.push_str(&format!("<defs><mask id=\"gradient-knockout-{}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\"/><path d=\"{band}\" fill=\"black\" fill-rule=\"evenodd\"/></mask></defs>\n",p.id,num(b.0),num(b.1),num(b.2-b.0),num(b.3-b.1),num(b.0),num(b.1),num(b.2-b.0),num(b.3-b.1)));
    }
    if f.is_painted() && p.anchors.len() >= 3 {
        let mask = if knockout { format!(" mask=\"url(#gradient-knockout-{})\"", p.id) } else { String::new() };
        out.push_str(&format!(
            "<path d=\"{data}\" fill=\"{fill}\" fill-opacity=\"{}\" fill-rule=\"evenodd\"{mask}/>\n",
            num(fa)
        ));
    }
    if s.is_painted() && p.stroke_width > 0. {
        out.push_str(&format!(
            "<path d=\"{band}\" fill=\"{stroke}\" fill-opacity=\"{}\" fill-rule=\"evenodd\"/>\n",
            num(sa)
        ));
    }
    out.push_str("</g>\n");
    Ok(())
}
