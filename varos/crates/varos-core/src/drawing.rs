//! Lane D: original gesture orchestration over the attributed core geometry/fitter.
use crate::{
    editor::{Drag, Editor, ToolKind},
    geom::{self, Pt},
    model::Path,
    EditCommand,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Rectangle,
    Ellipse,
    RoundedRectangle,
    Polygon,
    Star,
    Line,
    Arc,
    Spiral,
    RectangularGrid,
    PolarGrid,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ShapeSpec {
    pub kind: Shape,
    pub origin: Pt,
    pub size: Pt,
    pub radius: f32,
    pub sides: usize,
    pub inner_ratio: f32,
    pub rotation: f32,
    pub sweep: f32,
    pub turns: f32,
    pub decay: f32,
    pub rows: usize,
    pub columns: usize,
    pub centre: bool,
}
impl Default for ShapeSpec {
    fn default() -> Self {
        Self {
            kind: Shape::RoundedRectangle,
            origin: [0., 0.],
            size: [100., 100.],
            radius: 12.,
            sides: 5,
            inner_ratio: 0.5,
            rotation: 0.,
            sweep: 90.,
            turns: 3.,
            decay: 0.8,
            rows: 4,
            columns: 4,
            centre: false,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub fidelity: f32,
    pub smoothness: f32,
    pub endpoint_distance: f32,
    pub brush_radius: f32,
}
impl Default for Options {
    fn default() -> Self {
        Self { fidelity: 2., smoothness: 0.5, endpoint_distance: 6., brush_radius: 8. }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Shape { spec: ShapeSpec },
    Pencil { points: Vec<Pt>, options: Options },
    Smooth { points: Vec<Pt>, options: Options },
    PathErase { points: Vec<Pt>, options: Options },
    Join { points: Vec<Pt>, options: Options },
    Curvature { points: Vec<Pt>, closed: bool },
    Options { options: Options },
}
#[derive(Clone, Default)]
pub struct State {
    pub options: Options,
    pub shape: ShapeSpec,
    pub dialog: Option<ShapeSpec>,
    pub options_requested: bool,
    pub samples: Vec<Pt>,
    pub start: Option<Pt>,
    pub preview: Vec<Path>,
    pub curvature: Vec<Pt>,
    pub star_outer: Option<f32>,
}
pub fn shape_kind(t: ToolKind) -> Option<Shape> {
    Some(match t {
        ToolKind::Rect => Shape::Rectangle,
        ToolKind::Ellipse => Shape::Ellipse,
        ToolKind::Polygon => Shape::Polygon,
        ToolKind::RoundedRect => Shape::RoundedRectangle,
        ToolKind::Star => Shape::Star,
        ToolKind::Line => Shape::Line,
        ToolKind::Arc => Shape::Arc,
        ToolKind::Spiral => Shape::Spiral,
        ToolKind::RectGrid => Shape::RectangularGrid,
        ToolKind::PolarGrid => Shape::PolarGrid,
        _ => return None,
    })
}
pub fn owns(t: ToolKind) -> bool {
    (shape_kind(t).is_some() && !matches!(t, ToolKind::Rect | ToolKind::Ellipse))
        || matches!(
            t,
            ToolKind::Pencil | ToolKind::Smooth | ToolKind::PathEraser | ToolKind::Join | ToolKind::Curvature
        )
}
/// Numeric sheets and dragging share this pure geometry entry point.
pub fn shape_paths(s: ShapeSpec) -> Vec<Path> {
    use geom::shapes as g;
    let a = if s.centre { [s.origin[0] - s.size[0] * 0.5, s.origin[1] - s.size[1] * 0.5] } else { s.origin };
    let b = geom::add(a, s.size);
    let bounds = [a[0], a[1], b[0], b[1]];
    let c = geom::scale(geom::add(a, b), 0.5);
    let r = [s.size[0].abs() * 0.5, s.size[1].abs() * 0.5];
    vec![match s.kind {
        Shape::Rectangle => g::rounded_rectangle(bounds, [0.; 4]),
        Shape::RoundedRectangle => g::rounded_rectangle(bounds, [s.radius; 4]),
        Shape::Ellipse => g::arc(c, r, 0., std::f32::consts::TAU, g::ArcClosure::Chord),
        Shape::Polygon => g::polygon(c, r[0].max(r[1]), s.sides, s.rotation.to_radians()),
        Shape::Star => g::star(c, r[0].max(r[1]), r[0].max(r[1]) * s.inner_ratio, s.sides, s.rotation.to_radians()),
        Shape::Line => g::line(a, b),
        Shape::Arc => g::arc(c, r, s.rotation.to_radians(), s.sweep.to_radians(), g::ArcClosure::Open),
        Shape::Spiral => g::spiral(c, r[0].max(r[1]), s.turns, s.decay, (s.turns.abs() * 16.).ceil() as usize),
        Shape::RectangularGrid => return g::rectangular_grid(s.rows, s.columns, bounds),
        Shape::PolarGrid => return g::polar_grid(s.rows, s.columns, c, r),
    }]
}
fn valid_options(o: Options) -> bool {
    [o.fidelity, o.smoothness, o.endpoint_distance, o.brush_radius].iter().all(|v| v.is_finite())
        && (0.01..=100.).contains(&o.fidelity)
        && (0. ..=1.).contains(&o.smoothness)
        && (0. ..=1000.).contains(&o.endpoint_distance)
        && (0.01..=1000.).contains(&o.brush_radius)
}
pub fn check(ed: &Editor, action: &Action) -> Result<(), String> {
    let (pts, options) = match action {
        Action::Shape { spec: s } => {
            if !s
                .origin
                .iter()
                .chain(s.size.iter())
                .chain([s.radius, s.inner_ratio, s.rotation, s.sweep, s.turns, s.decay].iter())
                .all(|v| v.is_finite() && v.abs() <= 1.0e7)
                || (if s.kind == Shape::Line {
                    geom::length(s.size) < 0.001
                } else {
                    s.size.iter().any(|v| v.abs() < 0.001)
                })
                || s.radius < 0.
                || !(2..=1000).contains(&s.sides)
                || !(0. ..=1.).contains(&s.inner_ratio)
                || !(0.0001..=1.).contains(&s.decay)
                || s.turns.abs() > 100.
                || !(1..=1000).contains(&s.rows)
                || !(1..=1000).contains(&s.columns)
            {
                return Err("invalid shape parameters".into());
            }
            let paths = shape_paths(*s);
            check_insert(ed, &paths)?;
            return allocation_check(ed);
        }
        Action::Options { options } => {
            return if valid_options(*options) { Ok(()) } else { Err("invalid drawing options".into()) }
        }
        Action::Pencil { points, options }
        | Action::Smooth { points, options }
        | Action::PathErase { points, options }
        | Action::Join { points, options } => (points, Some(*options)),
        Action::Curvature { points, .. } => (points, None),
    };
    if pts.len() < 2
        || pts.len() > 16384
        || pts.iter().flatten().any(|v| !v.is_finite() || v.abs() > 1.0e7)
        || options.is_some_and(|o| !valid_options(o))
    {
        return Err("drawing requires 2–16384 finite points and valid options".into());
    }
    if matches!(action, Action::Pencil { .. } | Action::Curvature { .. }) {
        if pts.windows(2).all(|w| w[0] == w[1]) {
            return Err("drawing needs distinct points".into());
        }
        if matches!(action, Action::Curvature { closed: true, .. }) && pts.len() < 3 {
            return Err("closed curvature needs at least three points".into());
        }
        // A fitted stroke cannot add more anchors than the input polyline.
        let limits = crate::format::Limits::DEFAULT;
        let count: usize =
            ed.doc.paths.iter().map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum();
        if count + pts.len() > limits.max_anchors {
            return Err("drawing exceeds document anchor limit".into());
        }
        let dummy = geom::shapes::line(pts[0], pts[pts.len() - 1]);
        check_insert(ed, &[dummy])?;
    }
    if matches!(action, Action::PathErase { .. } | Action::Join { .. })
        && selected(ed).iter().any(|id| ed.doc.is_mask_source(*id))
    {
        return Err("path eraser and join cannot replace mask sources".into());
    }
    allocation_check(ed)
}
fn check_insert(ed: &Editor, paths: &[Path]) -> Result<(), String> {
    let mut parent = Some(ed.doc.active_layer);
    let mut remaining = ed.doc.nodes.len();
    while let Some(id) = parent {
        if remaining == 0 {
            return Err("drawing destination has cyclic ancestors".into());
        }
        remaining -= 1;
        let node = ed.doc.node(id).ok_or("unknown drawing destination")?;
        if node.locked || node.hidden {
            return Err("drawing destination must be visible and unlocked".into());
        }
        parent = node.parent;
    }
    let limits = crate::format::Limits::DEFAULT;
    if ed.doc.paths.len() + paths.len() > limits.max_paths || ed.doc.nodes.len() + paths.len() > limits.max_nodes {
        return Err("drawing exceeds document path/node limit".into());
    }
    let count: usize =
        ed.doc.paths.iter().chain(paths).map(|p| p.anchors.len() + p.holes.iter().map(Vec::len).sum::<usize>()).sum();
    if count > limits.max_anchors {
        return Err("drawing exceeds document anchor limit".into());
    }
    Ok(())
}
fn allocation_check(ed: &Editor) -> Result<(), String> {
    let limits = crate::format::Limits::DEFAULT;
    if u64::from(ed.allocation_floor()) + (limits.max_nodes + limits.max_paths + limits.max_anchors) as u64
        >= u64::from(u32::MAX)
    {
        Err("drawing id arena exhausted".into())
    } else {
        Ok(())
    }
}
fn paint(ed: &Editor, p: &mut Path) {
    p.fill = crate::model::Paint::from_opt(if p.closed { ed.cur_fill } else { None });
    p.stroke = crate::model::Paint::from_opt(ed.cur_stroke);
    p.stroke_width = ed.cur_sw;
}
fn insert(ed: &mut Editor, mut p: Path) -> u32 {
    p.id = ed.doc.nid();
    for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
        if a.id == 0 {
            a.id = ed.doc.nid();
        }
    }
    let id = p.id;
    ed.doc.paths.push(p);
    id
}
fn selected(ed: &Editor) -> Vec<u32> {
    let mut ids: Vec<_> = ed
        .selected_pids()
        .into_iter()
        .filter(|p| {
            ed.doc.pidx(*p).is_some_and(|i| {
                ed.in_isolation(*p) && !ed.doc.eff_locked(ed.doc.paths[i].id) && !ed.doc.eff_hidden(ed.doc.paths[i].id)
            })
        })
        .collect();
    ids.sort_unstable();
    ids
}
fn near_walk(p: Pt, points: &[Pt], radius: f32) -> bool {
    points.windows(2).any(|w| {
        let d = geom::sub(w[1], w[0]);
        let l = d[0] * d[0] + d[1] * d[1];
        let t = if l > 0. { ((p[0] - w[0][0]) * d[0] + (p[1] - w[0][1]) * d[1]) / l } else { 0. };
        geom::dist(p, geom::add(w[0], geom::scale(d, t.clamp(0., 1.)))) <= radius
    })
}
fn curvature_path(points: &[Pt], closed: bool) -> Path {
    // Through-points are retained exactly; core smoothing supplies interpolating handles.
    let mut p = geom::fit::fit_points(points, 0.01);
    p.anchors =
        points.iter().map(|q| crate::model::Anchor { id: 0, p: *q, hin: None, hout: None, smooth: false }).collect();
    p.closed = closed;
    geom::edit::smooth(&p, 1., 0..points.len())
}
pub fn apply(ed: &mut Editor, action: Action) {
    if let Err(e) = check(ed, &action) {
        ed.stroke_error = Some(e);
        return;
    }
    if let Action::Options { options } = action {
        ed.drawing.options = options;
        ed.drawing.options_requested = true;
        return;
    }
    ed.begin();
    match action {
        Action::Shape { spec } => {
            ed.objsel.clear();
            for mut p in shape_paths(spec) {
                paint(ed, &mut p);
                let id = insert(ed, p);
                ed.objsel.insert(id);
            }
        }
        Action::Pencil { points, options } => {
            let mut p = geom::fit::fit_points(&points, options.fidelity as f64);
            p = geom::edit::smooth(&p, options.smoothness, 0..p.anchors.len());
            let candidate = ed
                .doc
                .paths
                .iter()
                .enumerate()
                .filter(|(i, p)| {
                    !p.closed
                        && ed.in_isolation(p.id)
                        && p.holes.is_empty()
                        && !p.anchors.is_empty()
                        && !ed.doc.eff_locked(ed.doc.paths[*i].id)
                        && !ed.doc.eff_hidden(ed.doc.paths[*i].id)
                })
                .flat_map(|(i, p)| [(i, false, p.anchors[0].p), (i, true, p.anchors[p.anchors.len() - 1].p)])
                .map(|(i, end, q)| (geom::dist(ed.doc.unit_xform(ed.doc.paths[i].id).apply(q), points[0]), i, end))
                .filter(|(d, _, _)| *d <= options.endpoint_distance)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, i, end)) = candidate {
                ed.bake_unit_of(ed.doc.paths[i].id);
                if !end {
                    ed.doc.paths[i].anchors.reverse();
                    for a in &mut ed.doc.paths[i].anchors {
                        std::mem::swap(&mut a.hin, &mut a.hout);
                    }
                }
                if let Some(a) = ed.doc.paths[i].anchors.last_mut() {
                    a.hout = p.anchors[0].hout.map(|h| geom::add(h, geom::sub(a.p, p.anchors[0].p)));
                }
                for mut a in p.anchors.into_iter().skip(1) {
                    a.id = ed.doc.nid();
                    ed.doc.paths[i].anchors.push(a);
                }
            } else {
                paint(ed, &mut p);
                let id = insert(ed, p);
                ed.objsel.clear();
                ed.objsel.insert(id);
            }
        }
        Action::Curvature { points, closed } => {
            let mut p = curvature_path(&points, closed);
            paint(ed, &mut p);
            let id = insert(ed, p);
            ed.objsel.clear();
            ed.objsel.insert(id);
        }
        Action::Smooth { points, options } => {
            for id in selected(ed) {
                let Some(i) = ed.doc.pidx(id) else { continue };
                let xf = ed.doc.unit_xform(id);
                let local: Vec<_> = points.iter().map(|p| xf.inverse_apply(*p)).collect();
                let source = &ed.doc.paths[i];
                let smooth = geom::edit::smooth(source, options.smoothness, 0..usize::MAX);
                for (a, b) in ed.doc.paths[i].anchors.iter_mut().zip(smooth.anchors) {
                    if near_walk(a.p, &local, options.brush_radius) {
                        *a = b;
                    }
                }
                for (ring, new) in ed.doc.paths[i].holes.iter_mut().zip(smooth.holes) {
                    for (a, b) in ring.iter_mut().zip(new) {
                        if near_walk(a.p, &local, options.brush_radius) {
                            *a = b;
                        }
                    }
                }
            }
        }
        Action::PathErase { points, options } => erase(ed, &points, options.brush_radius),
        Action::Join { points, options } => {
            let candidates: Vec<_> = selected(ed)
                .into_iter()
                .filter(|id| {
                    let Some(i) = ed.doc.pidx(*id) else { return false };
                    let p = &ed.doc.paths[i];
                    let xf = ed.doc.unit_xform(*id);
                    !p.closed
                        && ed.in_isolation(p.id)
                        && p.holes.is_empty()
                        && p.anchors.first().zip(p.anchors.last()).is_some_and(|(a, b)| {
                            near_walk(xf.apply(a.p), &points, options.brush_radius)
                                || near_walk(xf.apply(b.p), &points, options.brush_radius)
                        })
                })
                .collect();
            let close_single = candidates.len() == 1
                && candidates.first().and_then(|id| ed.doc.pidx(*id)).is_some_and(|i| {
                    let p = &ed.doc.paths[i];
                    let xf = ed.doc.unit_xform(p.id);
                    p.anchors.first().zip(p.anchors.last()).is_some_and(|(a, b)| {
                        near_walk(xf.apply(a.p), &points, options.brush_radius)
                            && near_walk(xf.apply(b.p), &points, options.brush_radius)
                    })
                });
            if candidates.len() > 1 || close_single {
                for id in &candidates {
                    ed.bake_unit_of(*id);
                }
                let paths: Vec<_> =
                    candidates.iter().filter_map(|id| ed.doc.pidx(*id)).map(|i| ed.doc.paths[i].clone()).collect();
                let joined = geom::edit::join_open_paths(&paths, options.endpoint_distance);
                ed.doc.paths.retain(|p| !candidates.contains(&p.id));
                for mut p in joined {
                    for a in &mut p.anchors {
                        if a.id == 0 {
                            a.id = ed.doc.nid();
                        }
                    }
                    ed.doc.paths.push(p);
                }
            }
        }
        Action::Options { .. } => {}
    }
    ed.dirty = true;
    ed.finish_document_setup();
}
fn walk_distance(p: Pt, points: &[Pt]) -> f32 {
    points
        .windows(2)
        .map(|w| {
            let d = geom::sub(w[1], w[0]);
            let l = d[0] * d[0] + d[1] * d[1];
            let t = if l > 0. { ((p[0] - w[0][0]) * d[0] + (p[1] - w[0][1]) * d[1]) / l } else { 0. };
            geom::dist(p, geom::add(w[0], geom::scale(d, t.clamp(0., 1.))))
        })
        .fold(f32::INFINITY, f32::min)
}
fn clip_curve(c: kurbo::CubicBez, points: &[Pt], radius: f32, depth: usize, out: &mut Vec<Option<kurbo::CubicBez>>) {
    use kurbo::ParamCurve;
    let mid = c.eval(0.5);
    let distance = walk_distance([mid.x as f32, mid.y as f32], points) as f64;
    let bound = [c.p0, c.p1, c.p2, c.p3].iter().map(|p| p.distance(mid)).fold(0., f64::max);
    if distance - bound > radius as f64 {
        out.push(Some(c));
        return;
    }
    if distance + bound < radius as f64 {
        out.push(None);
        return;
    }
    if depth >= 16 || bound < 0.001 {
        out.push((distance >= radius as f64).then_some(c));
        return;
    }
    clip_curve(c.subsegment(0. ..0.5), points, radius, depth + 1, out);
    clip_curve(c.subsegment(0.5..1.), points, radius, depth + 1, out);
}
fn erase(ed: &mut Editor, points: &[Pt], radius: f32) {
    for id in selected(ed) {
        let Some(i) = ed.doc.pidx(id) else { continue };
        let source = ed.doc.paths[i].clone();
        let source_node = ed.doc.node_of_path(id);
        let xf = ed.doc.unit_xform(id);
        let local: Vec<_> = points.iter().map(|p| xf.inverse_apply(*p)).collect();
        let mut pieces = vec![];
        for c in geom::edit::curves(&source.anchors, source.closed) {
            clip_curve(c, &local, radius, 0, &mut pieces);
        }
        let mut removed = pieces.iter().any(Option::is_none);
        // Compound paths retain their separate rings; erase them into independent stroke remnants.
        for ring in &source.holes {
            let start = pieces.len();
            pieces.push(None);
            for c in geom::edit::curves(ring, true) {
                clip_curve(c, &local, radius, 0, &mut pieces);
            }
            removed |= pieces[start + 1..].iter().any(Option::is_none);
        }
        if !removed {
            continue;
        }
        let mut runs: Vec<Vec<kurbo::CubicBez>> = vec![vec![]];
        for p in pieces {
            if let Some(c) = p {
                if let Some(run) = runs.last_mut() {
                    run.push(c);
                }
            } else if runs.last().is_some_and(|r| !r.is_empty()) {
                runs.push(vec![]);
            }
        }
        if source.closed && source.holes.is_empty() && runs.len() > 1 && !near_walk(source.anchors[0].p, &local, radius)
        {
            if let Some(mut last) = runs.pop() {
                last.extend(runs.remove(0));
                runs.insert(0, last);
            }
        }
        ed.doc.paths.remove(i);
        let mut first = true;
        for run in runs.into_iter().filter(|r| !r.is_empty()) {
            let mut p = source.clone();
            p.anchors = geom::fit::cubics_to_path(&run).anchors;
            p.holes.clear();
            p.closed = false;
            p.fill = crate::model::Paint::None;
            // Drop intermediate subdivision boundaries on straight runs only in future Simplify.
            for a in &mut p.anchors {
                a.id = ed.doc.nid();
            }
            if first {
                p.id = source.id;
                first = false;
                ed.doc.paths.push(p);
            } else {
                let new = insert(ed, p);
                ed.doc.sync_tree();
                if let (Some(new_node), Some(source_node)) = (ed.doc.node_of_path(new), source_node) {
                    ed.doc.move_node_to(new_node, source_node, crate::model::DropPos::After);
                }
                if let Some(node) = ed.doc.unit_of(new) {
                    ed.doc.set_node_xform(node, xf);
                }
            }
        }
    }
}
pub fn down(ed: &mut Editor, pos: Pt) -> bool {
    if !owns(ed.gesture) {
        return false;
    }
    if ed.gesture == ToolKind::Curvature {
        if ed.drawing.curvature.len() > 2 && geom::dist(pos, ed.drawing.curvature[0]) * ed.ppu < 6. {
            let points = std::mem::take(&mut ed.drawing.curvature);
            ed.drawing.preview.clear();
            ed.execute_ui(EditCommand::Drawing(Action::Curvature { points, closed: true }));
            return true;
        }
        ed.drawing.curvature.push(pos);
        ed.drawing.preview =
            if ed.drawing.curvature.len() > 1 { vec![curvature_path(&ed.drawing.curvature, false)] } else { vec![] };
        ed.commit();
        return true;
    }
    ed.drawing.star_outer = None;
    ed.drawing.start = Some(pos);
    ed.drawing.samples = vec![pos];
    ed.drag = Drag::Drawing;
    true
}
fn drag_spec(ed: &Editor, pos: Pt) -> ShapeSpec {
    let start = ed.drawing.start.unwrap_or(pos);
    let mut delta = geom::sub(pos, start);
    let kind = shape_kind(ed.gesture).unwrap_or(ed.drawing.shape.kind);
    let radial = matches!(kind, Shape::Polygon | Shape::Star | Shape::Spiral);
    if ed.mods.shift && !radial {
        if kind == Shape::Line {
            let length = geom::dist(start, pos);
            let angle = (delta[1].atan2(delta[0]) / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
            delta = [angle.cos() * length, angle.sin() * length];
        } else {
            let d = delta[0].abs().max(delta[1].abs());
            delta = [d * delta[0].signum(), d * delta[1].signum()];
        }
    }
    if radial {
        let radius = if kind == Shape::Star {
            ed.drawing.star_outer.unwrap_or(geom::length(delta))
        } else {
            geom::length(delta)
        };
        let mut rotation = delta[1].atan2(delta[0]).to_degrees() + 90.;
        if ed.mods.shift {
            rotation = (rotation / 45.).round() * 45.;
        }
        return ShapeSpec { kind, origin: start, size: [radius * 2.; 2], centre: true, rotation, ..ed.drawing.shape };
    }
    let centre = ed.mods.alt;
    if centre {
        delta = geom::scale(delta, 2.);
    }

    ShapeSpec { kind, origin: start, size: delta, centre, ..ed.drawing.shape }
}
pub fn movement(ed: &mut Editor, pos: Pt) -> bool {
    if ed.drawing.start.is_none() {
        return false;
    }
    if ed.gesture == ToolKind::Star {
        let start = ed.drawing.start.unwrap_or(pos);
        if ed.mods.alt {
            let outer = *ed.drawing.star_outer.get_or_insert(geom::dist(start, ed.cursor).max(0.001));
            let before = geom::dist(start, ed.cursor);
            let delta = geom::dist(start, pos) - before;
            ed.drawing.shape.inner_ratio = (ed.drawing.shape.inner_ratio + delta / outer).clamp(0.01, 1.);
        } else {
            ed.drawing.star_outer = None;
        }
    }
    if shape_kind(ed.gesture).is_some() {
        ed.drawing.preview = shape_paths(drag_spec(ed, pos));
    } else if ed.drawing.samples.last().is_none_or(|q| geom::dist(*q, pos) * ed.ppu >= 0.5)
        && ed.drawing.samples.len() < 16384
    {
        ed.drawing.samples.push(pos);
        if ed.gesture == ToolKind::Pencil {
            ed.drawing.preview = vec![geom::fit::fit_points(&ed.drawing.samples, ed.drawing.options.fidelity as f64)];
        }
    }
    true
}
pub fn up(ed: &mut Editor) -> bool {
    let Some(start) = ed.drawing.start else {
        // Preserve the existing Rectangle/Ellipse drag engine and expose its click sheet.
        if matches!(ed.gesture, ToolKind::Rect | ToolKind::Ellipse) {
            if let Drag::Shape { start, .. } = ed.drag {
                if geom::dist(start, ed.cursor) * ed.ppu < 3. {
                    ed.drawing.dialog = Some(ShapeSpec {
                        kind: shape_kind(ed.gesture).unwrap_or(Shape::Rectangle),
                        origin: start,
                        ..ed.drawing.shape
                    });
                }
            }
        }
        return ed.gesture == ToolKind::Curvature;
    };
    let spec = drag_spec(ed, ed.cursor);
    ed.drawing.start = None;
    ed.drag = Drag::None;
    ed.drawing.preview.clear();
    let mut points = std::mem::take(&mut ed.drawing.samples);
    if points.last() != Some(&ed.cursor) {
        points.push(ed.cursor);
    }
    let options = ed.drawing.options;
    let action = if shape_kind(ed.gesture).is_some() {
        if geom::dist(start, ed.cursor) * ed.ppu < 3. {
            ed.drawing.dialog = Some(ShapeSpec { size: ed.drawing.shape.size, ..spec });
            ed.commit();
            return true;
        }
        Action::Shape { spec }
    } else {
        match ed.gesture {
            ToolKind::Pencil => Action::Pencil { points, options },
            ToolKind::Smooth => Action::Smooth { points, options },
            ToolKind::PathEraser => Action::PathErase { points, options },
            ToolKind::Join => Action::Join { points, options },
            _ => {
                ed.commit();
                return true;
            }
        }
    };
    if points_are_stationary(&action) {
        ed.commit();
    } else {
        ed.execute_ui(EditCommand::Drawing(action));
        // A refused pointer edit still has to settle its untouched down snapshot.
        ed.commit();
    }
    true
}
fn points_are_stationary(action: &Action) -> bool {
    match action {
        Action::Pencil { points, .. }
        | Action::Smooth { points, .. }
        | Action::PathErase { points, .. }
        | Action::Join { points, .. } => points.windows(2).all(|w| w[0] == w[1]),
        _ => false,
    }
}
pub fn finish(ed: &mut Editor, cancel: bool) {
    // Never commit another tool's pending transaction.
    if ed.drawing.curvature.is_empty() && ed.drawing.start.is_none() && ed.drawing.dialog.is_none() {
        return;
    }
    let points = std::mem::take(&mut ed.drawing.curvature);
    if matches!(ed.drag, Drag::Drawing) {
        ed.drag = Drag::None;
    }
    ed.drawing.start = None;
    ed.drawing.star_outer = None;
    ed.drawing.samples.clear();
    ed.drawing.preview.clear();
    if !cancel && points.len() > 1 {
        ed.execute_ui(EditCommand::Drawing(Action::Curvature { points, closed: false }));
    } else {
        ed.commit();
    }
    if cancel {
        ed.drawing.dialog = None;
    }
}
pub fn arrow(ed: &mut Editor, up: bool) -> bool {
    if ed.drawing.start.is_none() {
        return false;
    }
    let s = &mut ed.drawing.shape;
    match shape_kind(ed.gesture) {
        Some(Shape::RoundedRectangle) => s.radius = (s.radius + if up { 1. } else { -1. }).max(0.),
        Some(Shape::Polygon | Shape::Star) => {
            s.sides = if up {
                (s.sides + 1).min(1000)
            } else {
                s.sides.saturating_sub(1).max(if ed.gesture == ToolKind::Star { 2 } else { 3 })
            }
        }
        _ => return false,
    }
    movement(ed, ed.cursor);
    true
}

/// Refresh shape previews on a modifier-only frame without sampling a freehand stroke.
pub fn refresh(ed: &mut Editor) {
    if ed.drawing.start.is_some() && shape_kind(ed.gesture).is_some() {
        ed.drawing.preview = shape_paths(drag_spec(ed, ed.cursor));
    }
}
