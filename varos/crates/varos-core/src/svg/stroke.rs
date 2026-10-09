//! Styled SVG paint; ordinary strokes remain editable native attributes, complex coverage is baked.
use super::*;
use crate::stroke::{StrokeAlign, StrokeCap, StrokeJoin};
pub(super) fn native(p: &Path) -> bool {
    (p.stroke_style.is_default() || crate::stroke::evaluate::has_length(p))
        && (p.stroke_style.align == StrokeAlign::Center || !p.closed)
        && !p.stroke_style.align_dashes_to_corners
        && (p.closed || (p.stroke_style.arrows.start.is_none() && p.stroke_style.arrows.end.is_none()))
        && p.stroke_style.dash.iter().all(|v| *v > 0.0)
}
fn attrs(p: &Path, decimals: Option<u8>) -> String {
    let num = |v| number(v, decimals);
    let s = &p.stroke_style;
    let cap = match s.cap {
        StrokeCap::Butt => "butt",
        StrokeCap::Round => "round",
        StrokeCap::Square => "square",
    };
    let join = match s.join {
        StrokeJoin::Miter => "miter",
        StrokeJoin::Round => "round",
        StrokeJoin::Bevel => "bevel",
    };
    let mut out = format!(
        "stroke-width=\"{}\" stroke-linecap=\"{cap}\" stroke-linejoin=\"{join}\" stroke-miterlimit=\"{}\"",
        num(p.stroke_width),
        num(s.miter_limit)
    );
    if !s.dash.is_empty() {
        out.push_str(&format!(
            " stroke-dasharray=\"{}\" stroke-dashoffset=\"{}\"",
            s.dash.iter().map(|v| num(*v)).collect::<Vec<_>>().join(","),
            num(s.dash_phase)
        ));
    }
    out
}
pub(super) fn coverage_data(rings: &[Vec<crate::Pt>], xf: &Xform, decimals: Option<u8>) -> String {
    let num = |v| number(v, decimals);
    let mut out = String::new();
    for r in rings {
        if let Some(first) = r.first() {
            let p = xf.apply(*first);
            out.push_str(&format!("M{} {}", num(p[0]), num(p[1])));
            for q in &r[1..] {
                let p = xf.apply(*q);
                out.push_str(&format!("L{} {}", num(p[0]), num(p[1])));
            }
            out.push('Z');
        }
    }
    out
}
pub(super) fn paint(out: &mut String, d: &Drawn<'_>, decimals: Option<u8>) -> Result<(), ExportError> {
    let num = |v| number(v, decimals);
    let p = d.p;
    let data = path_data(p, &d.xf, decimals);
    let coverage = crate::stroke::evaluate(p, 0.01, &|| false).map_err(stroke_error)?;
    let band = coverage_data(&coverage.rings, &d.xf, decimals);
    out.push_str(&format!(
        "<g id=\"{}\" opacity=\"{}\">\n",
        id("path", p.id as usize, p.name.as_deref().unwrap_or("")),
        num(p.opacity)
    ));
    if let Some(name) = &p.name {
        title(out, name);
    }
    let knockout = d.fill.is_some() && d.stroke.is_some_and(|c| c[3] < 0.999);
    if knockout {
        let b = d.bbox;
        out.push_str(&format!("<defs><mask id=\"knockout-{}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"white\"/><path d=\"{band}\" fill=\"black\" fill-rule=\"evenodd\"/></mask></defs>\n",p.id,num(b.0),num(b.1),num(b.2-b.0),num(b.3-b.1),num(b.0),num(b.1),num(b.2-b.0),num(b.3-b.1)));
    }
    if let Some(c) = d.fill {
        let mask = if knockout { format!(" mask=\"url(#knockout-{})\"", p.id) } else { String::new() };
        out.push_str(&format!(
            "<path d=\"{data}\" fill=\"{}\" fill-opacity=\"{}\" fill-rule=\"evenodd\"{mask}/>\n",
            color(c),
            num(c[3])
        ));
    }
    if let Some(c) = d.stroke {
        if p.stroke_width > 0.0 {
            if native(p) {
                out.push_str(&format!(
                    "<path d=\"{data}\" fill=\"none\" stroke=\"{}\" stroke-opacity=\"{}\" {}/>\n",
                    color(c),
                    num(c[3]),
                    attrs(p, decimals)
                ));
            } else {
                out.push_str(&format!(
                    "<path d=\"{band}\" fill=\"{}\" fill-opacity=\"{}\" fill-rule=\"evenodd\"/>\n",
                    color(c),
                    num(c[3])
                ));
            }
        }
    }
    out.push_str("</g>\n");
    Ok(())
}
