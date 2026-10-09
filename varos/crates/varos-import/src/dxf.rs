//! ADAPT: VectorCraft crates/cad/src/{lib.rs,import.rs}: ASCII groups, entity/layer/unit mapping.
//! Copyright (c) 2026 ArtCraft Team and VectorCraft contributors. MIT OR Apache-2.0.
use crate::{ImportOptions, ImportReport, MAX_BYTES};
use std::collections::BTreeMap;
use varos_core::model::{Anchor, Document, NodeKind, Path};
type Pair<'a> = (i32, &'a str);
type Vertex = ([f32; 2], f32);
type Poly = (u32, [f32; 4], bool, Vec<Vertex>);
pub fn import_dxf(bytes: &[u8]) -> Result<(Document, ImportReport), String> {
    crate::import_file(bytes, crate::Format::Dxf, ImportOptions::default())
}
fn value<'a>(e: &[Pair<'a>], code: i32) -> Option<&'a str> {
    e.iter().find(|(c, _)| *c == code).map(|(_, v)| *v)
}
fn num(e: &[Pair<'_>], code: i32, default: Option<f32>) -> Result<f32, String> {
    let n = match value(e, code) {
        Some(v) => v.parse::<f32>().map_err(|_| format!("Invalid DXF group {code}"))?,
        None => default.ok_or_else(|| format!("Missing DXF group {code}"))?,
    };
    if !n.is_finite() || n.abs() > 1e8 {
        return Err("DXF coordinate exceeds limits".into());
    }
    Ok(n)
}
fn point(e: &[Pair<'_>], x: i32, y: i32, scale: f32) -> Result<[f32; 2], String> {
    Ok([num(e, x, None)? * scale, -num(e, y, None)? * scale])
}
fn emit(doc: &mut Document, parent: u32, a: Vec<Anchor>, closed: bool, colour: [f32; 4]) {
    let id = doc.nid();
    doc.paths.push(Path::new(id, a, closed, None, Some(colour), 1.));
    crate::node(doc, NodeKind::Path(id), parent, "");
}
fn colour(e: &[Pair<'_>], fallback: [f32; 4]) -> Result<[f32; 4], String> {
    if let Some(rgb) = value(e, 420) {
        let rgb = rgb.parse::<u32>().map_err(|_| "Invalid DXF true colour")?;
        if rgb > 0xffffff {
            return Err("Invalid DXF true colour".into());
        }
        return Ok([(rgb >> 16) as f32 / 255., ((rgb >> 8) & 255) as f32 / 255., (rgb & 255) as f32 / 255., 1.]);
    }
    Ok(match value(e, 62).unwrap_or("256").trim_start_matches('-') {
        "1" => [1., 0., 0., 1.],
        "2" => [1., 1., 0., 1.],
        "3" => [0., 1., 0., 1.],
        "4" => [0., 1., 1., 1.],
        "5" => [0., 0., 1., 1.],
        "6" => [1., 0., 1., 1.],
        "7" => [0., 0., 0., 1.],
        "256" => fallback,
        _ => return Err("DXF indexed colour unsupported".into()),
    })
}
/// Circular arcs as cubics; conservative subdivision for radial error < 0.01 points.
fn arc(doc: &mut Document, c: [f32; 2], r: f32, start: f32, sweep: f32) -> Result<Vec<Anchor>, String> {
    if r <= 0. || !r.is_finite() || !sweep.is_finite() || sweep == 0. {
        return Err("Invalid DXF arc".into());
    }
    if c.iter().any(|v| v.abs() + r > 8192.) {
        return Err("DXF arc exceeds coordinate precision range for 0.01 point tolerance".into());
    }
    let step = (0.01 / r).powf(1. / 6.).min(std::f32::consts::FRAC_PI_4);
    let count = (sweep.abs() / step).ceil().max(1.) as usize;
    if count > 4096 {
        return Err("DXF arc subdivision exceeds budget".into());
    }
    let at = |t: f32| [c[0] + r * t.cos(), c[1] - r * t.sin()];
    let mut a = vec![crate::anchor(doc, at(start))];
    for i in 0..count {
        let t = start + sweep * i as f32 / count as f32;
        let dt = sweep / count as f32;
        let end = t + dt;
        let k = 4. / 3. * (dt / 4.).tan() * r;
        a[i].hout = Some([a[i].p[0] - k * t.sin(), a[i].p[1] - k * t.cos()]);
        let mut next = crate::anchor(doc, at(end));
        next.hin = Some([next.p[0] + k * end.sin(), next.p[1] + k * end.cos()]);
        a.push(next);
    }
    Ok(a)
}
fn polyline(doc: &mut Document, v: &[Vertex], closed: bool) -> Result<Vec<Anchor>, String> {
    if v.len() < 2 {
        return Err("DXF polyline needs two vertices".into());
    }
    let mut out = vec![crate::anchor(doc, v[0].0)];
    for i in 0..v.len() - usize::from(!closed) {
        let (p, bulge) = v[i];
        let q = v[(i + 1) % v.len()].0;
        if bulge == 0. {
            out.push(crate::anchor(doc, q));
            continue;
        }
        let dx = q[0] - p[0];
        let dy = -(q[1] - p[1]);
        let length = dx.hypot(dy);
        if length == 0. {
            return Err("Zero-length DXF bulge".into());
        }
        let offset = length * (1. - bulge * bulge) / (4. * bulge);
        let c = [(p[0] + q[0]) / 2. - dy / length * offset, (p[1] + q[1]) / 2. - dx / length * offset];
        let mut s = arc(
            doc,
            c,
            length * (1. + bulge * bulge) / (4. * bulge.abs()),
            (-(p[1] - c[1])).atan2(p[0] - c[0]),
            4. * bulge.atan(),
        )?;
        if let (Some(last), Some(first)) = (out.last_mut(), s.first()) {
            last.hout = first.hout;
        }
        s.remove(0);
        out.extend(s);
        if out.len() > 50_000 {
            return Err("DXF polyline subdivision budget exceeded".into());
        }
    }
    if closed {
        if let Some(last) = out.pop() {
            out[0].hin = last.hin;
        }
    }
    Ok(out)
}
pub(crate) fn read(bytes: &[u8], options: ImportOptions) -> Result<(Document, ImportReport), String> {
    if bytes.len() > MAX_BYTES {
        return Err("DXF exceeds input limit".into());
    }
    if bytes.starts_with(b"AutoCAD Binary DXF") {
        return Err("Binary DXF unsupported; export ASCII DXF".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "DXF must be UTF-8 ASCII group pairs")?;
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    if lines.len() % 2 != 0 || lines.len() > 400_000 {
        return Err("Invalid/oversized DXF group pairs".into());
    }
    let pairs = lines
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| Ok((p[0].parse::<i32>().map_err(|_| "Invalid DXF group code")?, p[1])))
        .collect::<Result<Vec<_>, String>>()?;
    if pairs.last() != Some(&(0, "EOF")) || !pairs.windows(2).any(|p| p == [(0, "SECTION"), (2, "ENTITIES")]) {
        return Err("Expected ASCII DXF ENTITIES section and EOF".into());
    }
    let declared = pairs.windows(2).find(|p| p[0] == (9, "$INSUNITS")).map(|p| p[1].1);
    let scale = options
        .points_per_unit
        .or(match declared {
            Some("1") => Some(72.),
            Some("2") => Some(864.),
            Some("4") => Some(72. / 25.4),
            Some("5") => Some(72. / 2.54),
            Some("6") => Some(72. / 0.0254),
            _ => None,
        })
        .ok_or("DXF unitless/unknown units require explicit points_per_unit")?;
    if !scale.is_finite() || !(0.000001..=1e6).contains(&scale) {
        return Err("Invalid DXF unit scale".into());
    }
    let mut layer_styles = BTreeMap::new();
    for start in 0..pairs.len() {
        if pairs[start] == (0, "LAYER") {
            let end = (start + 1..pairs.len()).find(|i| pairs[*i].0 == 0).unwrap_or(pairs.len());
            let e = &pairs[start..end];
            if value(e, 6).is_some_and(|v| v != "CONTINUOUS") || value(e, 370).is_some() {
                return Err("DXF layer linetype/lineweight unsupported".into());
            }
            let flags = num(e, 70, Some(0.))? as u32;
            let hidden = flags & 1 != 0 || value(e, 62).is_some_and(|v| v.starts_with('-'));
            let locked = flags & 4 != 0;
            layer_styles
                .insert(value(e, 2).ok_or("DXF layer missing name")?, (colour(e, [0., 0., 0., 1.])?, hidden, locked));
        }
    }
    let mut doc = Document::default();
    let mut report = ImportReport::default();
    let mut layers = BTreeMap::new();
    let mut section = "";
    let mut i = 0;
    let mut poly: Option<Poly> = None;
    while i < pairs.len() {
        crate::control::checkpoint()?;
        if pairs[i] == (0, "SECTION") {
            section = pairs.get(i + 1).filter(|p| p.0 == 2).ok_or("Invalid DXF section")?.1;
            i += 2;
            continue;
        }
        if pairs[i] == (0, "ENDSEC") {
            if poly.is_some() {
                return Err("Unterminated DXF POLYLINE".into());
            }
            section = "";
            i += 1;
            continue;
        }
        if section != "ENTITIES" {
            i += 1;
            continue;
        }
        if pairs[i].0 != 0 {
            return Err("DXF entity must start with group 0".into());
        }
        let kind = pairs[i].1;
        let start = i;
        i += 1;
        while i < pairs.len() && pairs[i].0 != 0 {
            i += 1;
        }
        let e = &pairs[start..i];
        if poly.is_some() && !matches!(kind, "VERTEX" | "SEQEND") {
            return Err("Invalid DXF POLYLINE sequence".into());
        }
        for code in [30, 31, 32, 38, 39, 210, 220] {
            if num(e, code, Some(0.))? != 0. {
                return Err("DXF 3D/extruded/thick geometry unsupported".into());
            }
        }
        if num(e, 230, Some(1.))? != 1. || num(e, 67, Some(0.))? != 0. {
            return Err("DXF extrusion/paper layout unsupported".into());
        }
        if value(e, 6).is_some_and(|v| v != "CONTINUOUS" && v != "BYLAYER") || value(e, 370).is_some() {
            return Err("DXF authored linetype/lineweight unsupported".into());
        }
        let name = value(e, 8).unwrap_or("0");
        let (base, hidden, locked) = layer_styles.get(name).copied().unwrap_or(([0., 0., 0., 1.], false, false));
        let parent = if let Some(id) = layers.get(name) {
            *id
        } else {
            let id = if layers.is_empty() {
                doc.active_layer
            } else {
                let id = crate::node(&mut doc, NodeKind::Layer, 0, name);
                if let Some(n) = doc.nodes.iter_mut().find(|n| n.id == id) {
                    n.parent = None;
                }
                doc.roots.push(id);
                id
            };
            if let Some(n) = doc.nodes.iter_mut().find(|n| n.id == id) {
                n.name = name.into();
                n.hidden = hidden;
                n.locked = locked;
            }
            layers.insert(name, id);
            id
        };
        let colour = colour(e, base)?;
        let invisible = num(e, 60, Some(0.))?;
        if invisible != 0. && invisible != 1. {
            return Err("Invalid DXF entity visibility".into());
        }
        let parent = if invisible == 1. || value(e, 62).is_some_and(|v| v.starts_with('-')) {
            let group = crate::node(&mut doc, NodeKind::Group, parent, "Hidden DXF entity");
            if let Some(n) = doc.nodes.iter_mut().find(|n| n.id == group) {
                n.hidden = true;
            }
            group
        } else {
            parent
        };
        match kind {
            "LINE" => {
                let a = point(e, 10, 20, scale)?;
                let b = point(e, 11, 21, scale)?;
                let a = vec![crate::anchor(&mut doc, a), crate::anchor(&mut doc, b)];
                emit(&mut doc, parent, a, false, colour);
            }
            "ARC" | "CIRCLE" => {
                let c = point(e, 10, 20, scale)?;
                let r = num(e, 40, None)? * scale;
                let start = if kind == "CIRCLE" { 0. } else { num(e, 50, None)?.to_radians() };
                let sweep = if kind == "CIRCLE" {
                    std::f32::consts::TAU
                } else {
                    (num(e, 51, None)?.to_radians() - start).rem_euclid(std::f32::consts::TAU)
                };
                let mut a = arc(&mut doc, c, r, start, sweep)?;
                if kind == "CIRCLE" {
                    if let Some(last) = a.pop() {
                        a[0].hin = last.hin;
                    }
                }
                emit(&mut doc, parent, a, kind == "CIRCLE", colour);
                report.loss("DXF circular arcs approximated by cubics; radial tolerance 0.01 point");
            }
            "LWPOLYLINE" => {
                let mut v = Vec::new();
                let mut j = 1;
                while j < e.len() {
                    if e[j].0 == 10 {
                        let start = j;
                        j += 1;
                        while j < e.len() && e[j].0 != 10 {
                            j += 1;
                        }
                        let p = &e[start..j];
                        if num(p, 40, Some(0.))? != 0. || num(p, 41, Some(0.))? != 0. {
                            return Err("DXF polyline widths unsupported".into());
                        }
                        v.push((point(p, 10, 20, scale)?, num(p, 42, Some(0.))?));
                    } else {
                        j += 1;
                    }
                }
                let flags = num(e, 70, Some(0.))? as i32;
                if flags & !129 != 0 {
                    return Err("Unsupported DXF polyline flags".into());
                }
                let closed = flags & 1 != 0;
                if num(e, 43, Some(0.))? != 0. {
                    return Err("DXF polyline width unsupported".into());
                }
                if num(e, 90, None)? != v.len() as f32 {
                    return Err("DXF polyline vertex count does not match group 90".into());
                }
                let a = polyline(&mut doc, &v, closed)?;
                emit(&mut doc, parent, a, closed, colour);
                if v.iter().any(|(_, b)| *b != 0.) {
                    report.loss("DXF bulges approximated by cubics; radial tolerance 0.01 point");
                }
            }
            "POLYLINE" => {
                let flags = num(e, 70, Some(0.))? as i32;
                if flags & !1 != 0 || num(e, 40, Some(0.))? != 0. || num(e, 41, Some(0.))? != 0. {
                    return Err("Unsupported DXF POLYLINE".into());
                }
                poly = Some((parent, colour, flags == 1, Vec::new()));
            }
            "VERTEX" => {
                if num(e, 40, Some(0.))? != 0. || num(e, 41, Some(0.))? != 0. || num(e, 70, Some(0.))? != 0. {
                    return Err("Unsupported DXF VERTEX flags/width".into());
                }
                poly.as_mut()
                    .ok_or("DXF VERTEX outside POLYLINE")?
                    .3
                    .push((point(e, 10, 20, scale)?, num(e, 42, Some(0.))?));
            }
            "SEQEND" => {
                let (parent, colour, closed, v) = poly.take().ok_or("DXF SEQEND without POLYLINE")?;
                let a = polyline(&mut doc, &v, closed)?;
                emit(&mut doc, parent, a, closed, colour);
                if v.iter().any(|(_, b)| *b != 0.) {
                    report.loss("DXF bulges approximated by cubics; radial tolerance 0.01 point");
                }
            }
            "SPLINE" => {
                let collect = |code| {
                    e.iter()
                        .filter(|p| p.0 == code)
                        .map(|p| p.1.parse::<f32>())
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| "Invalid DXF spline number")
                };
                let xs = collect(10)?;
                let ys = collect(20)?;
                let knots = collect(40)?;
                let degree = num(e, 71, None)?;
                let flags = num(e, 70, Some(0.))?;
                if degree.fract() != 0.
                    || flags.fract() != 0.
                    || flags as i32 & !8 != 0
                    || xs.len() != ys.len()
                    || e.iter().any(|p| p.0 == 11)
                    || num(e, 72, Some(knots.len() as f32))? != knots.len() as f32
                    || num(e, 73, Some(xs.len() as f32))? != xs.len() as f32
                    || collect(41)?.iter().any(|w| *w != 1.)
                {
                    return Err("Only nonrational clamped 2D DXF splines supported".into());
                }
                if xs.iter().chain(&ys).chain(&knots).any(|v| !v.is_finite() || v.abs() > 1e8) {
                    return Err("Invalid DXF spline geometry".into());
                }
                let points: Vec<_> =
                    xs.iter().zip(&ys).map(|(x, y)| [*x as f64 * scale as f64, -*y as f64 * scale as f64]).collect();
                let knots: Vec<_> = knots.iter().map(|v| *v as f64).collect();
                let anchors = crate::dxf_spline::convert(&mut doc, &points, &knots, degree as usize)?;
                emit(&mut doc, parent, anchors, false, colour);
                report.loss("DXF spline programme converted to editable cubic path");
            }
            _ => return Err(format!("Unsupported DXF entity {kind}; no content omitted silently")),
        }
        if doc.ids > 500_000 || doc.paths.len() > 50_000 {
            return Err("DXF geometry budget exceeded".into());
        }
    }
    if poly.is_some() || doc.paths.is_empty() {
        return Err("DXF has no supported artwork or incomplete polyline".into());
    }
    doc.sync_tree();
    report.paths = doc.paths.len();
    Ok((doc, report))
}
