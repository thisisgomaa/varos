//! Lane B provisional gradient annotator: all positions authored in local gradient space.
use crate::{
    editor::{Editor, ToolKind},
    geom::{dist, Pt},
    gradient::{Gradient, GradientKind},
    model::Paint,
    scene::{Prim, ACCENT, WHITE},
};
#[derive(Clone, Default)]
pub struct State {
    pub drag: Option<Gesture>,
    pub geometry: std::collections::HashMap<u32, [Paint; 2]>,
}
#[derive(Clone)]
pub struct Gesture {
    pid: u32,
    origin: Pt,
    base: Gradient,
    handle: Handle,
}
#[derive(Clone, Copy)]
enum Handle {
    Start,
    End,
    Aspect,
    Focal,
    Stop(usize),
    Midpoint(usize),
    New,
}
fn point(g: &Gradient, t: Pt) -> Pt {
    let [a, b, c, d, e, f] = g.placement;
    [a * t[0] + c * t[1] + e, b * t[0] + d * t[1] + f]
}
fn handles(g: &Gradient, ppu: f32) -> Vec<(Handle, Pt)> {
    let length = g.placement[0].hypot(g.placement[1]).max(1e-6);
    let side = [-g.placement[1] / length * 12. / ppu.max(0.001), g.placement[0] / length * 12. / ppu.max(0.001)];
    let shifted = |p: Pt, k: f32| [p[0] + side[0] * k, p[1] + side[1] * k];
    let mut out = vec![(Handle::Start, point(g, [0., 0.])), (Handle::End, point(g, [1., 0.]))];
    for (i, s) in g.stops.iter().enumerate() {
        out.push((Handle::Stop(i), shifted(point(g, [s.offset, 0.]), 1.)));
        if let Some(next) = g.stops.get(i + 1) {
            out.push((
                Handle::Midpoint(i),
                shifted(point(g, [s.offset + (next.offset - s.offset) * s.midpoint, 0.]), -1.),
            ));
        }
    }
    if g.kind == GradientKind::Radial {
        out.push((Handle::Aspect, point(g, [0., 1.])));
        out.push((Handle::Focal, point(g, g.focal)));
    }
    out
}
fn selected(ed: &Editor) -> Option<(u32, Gradient)> {
    let ids = ed.selected_pids();
    ed.doc.paths.iter().find(|p| ids.contains(&p.id)).and_then(|p| {
        let paint = match ed.paint {
            crate::editor::PaintTarget::Fill => p.appearance().fill(),
            crate::editor::PaintTarget::Stroke => p.appearance().stroke(),
        };
        if let Paint::Gradient(g) = paint.resolved(&ed.doc) {
            Some((p.id, g))
        } else {
            None
        }
    })
}
pub fn down(ed: &mut Editor, pos: Pt) -> bool {
    if ed.gesture != ToolKind::Gradient {
        return false;
    }
    if ed.selected_pids().is_empty() {
        if let Some(pid) = ed.path_under(pos) {
            ed.execute_ui(crate::EditCommand::SelectPaths(vec![pid]));
        }
    }
    let existing = selected(ed);
    let Some(pid) = existing.as_ref().map(|(pid, _)| *pid).or_else(|| ed.selected_pids().into_iter().min()) else {
        return true;
    };
    let xf = ed.doc.unit_xform(pid);
    let local = xf.inverse_apply(pos);
    let (base, handle) = if let Some((_, g)) = existing {
        let h = if ed.mods.alt
            && g.kind == GradientKind::Radial
            && dist(xf.apply(point(&g, g.focal)), pos) < 8. / ed.ppu.max(0.001)
        {
            Handle::Focal
        } else {
            handles(&g, ed.ppu)
                .into_iter()
                .filter(|(_, p)| dist(xf.apply(*p), pos) < 8. / ed.ppu.max(0.001))
                .min_by(|a, b| dist(a.1, local).total_cmp(&dist(b.1, local)))
                .map(|x| x.0)
                .unwrap_or(Handle::New)
        };
        (g, h)
    } else {
        (Gradient::default(), Handle::New)
    };
    ed.gradient_tool.drag = Some(Gesture { pid, origin: local, base, handle });
    true
}
pub fn movement(ed: &mut Editor, pos: Pt) -> bool {
    let Some(drag) = ed.gradient_tool.drag.clone() else { return false };
    let p = ed.doc.unit_xform(drag.pid).inverse_apply(pos);
    let mut g = drag.base.clone();
    let [a, b, c, d, e, f] = g.placement;
    match drag.handle {
        Handle::New => {
            let v = [p[0] - drag.origin[0], p[1] - drag.origin[1]];
            if v[0].hypot(v[1]) < 0.01 {
                return true;
            }
            g.placement = [v[0], v[1], -v[1], v[0], drag.origin[0], drag.origin[1]];
        }
        Handle::Start => {
            let end = [e + a, f + b];
            g.placement = [end[0] - p[0], end[1] - p[1], c, d, p[0], p[1]];
        }
        Handle::End => {
            g.placement = [p[0] - e, p[1] - f, c, d, e, f];
        }
        Handle::Aspect => {
            g.placement = [a, b, p[0] - e, p[1] - f, e, f];
        }
        Handle::Focal => {
            let det = a * d - b * c;
            let q = [(d * (p[0] - e) - c * (p[1] - f)) / det, (-b * (p[0] - e) + a * (p[1] - f)) / det];
            let k = (0.98 / q[0].hypot(q[1])).min(1.);
            g.focal = q.map(|v| v * k);
        }
        Handle::Stop(i) => {
            let t = ((p[0] - e) * a + (p[1] - f) * b) / (a * a + b * b);
            let lo = i.checked_sub(1).and_then(|j| g.stops.get(j)).map_or(0., |s| s.offset);
            let hi = g.stops.get(i + 1).map_or(1., |s| s.offset);
            g.stops[i].offset = t.clamp(lo, hi);
        }
        Handle::Midpoint(i) => {
            let t = ((p[0] - e) * a + (p[1] - f) * b) / (a * a + b * b);
            let span = g.stops[i + 1].offset - g.stops[i].offset;
            if span > 0. {
                g.stops[i].midpoint = ((t - g.stops[i].offset) / span).clamp(0.01, 0.99);
            }
        }
    }
    ed.execute_ui(crate::EditCommand::Colour(crate::colour_commands::ColourCommand::Live {
        target: ed.paint,
        paint: Paint::Gradient(g),
    }));
    true
}
pub fn up(ed: &mut Editor) -> bool {
    if ed.gradient_tool.drag.take().is_none() {
        return false;
    }
    ed.execute_ui(crate::EditCommand::Colour(crate::colour_commands::ColourCommand::Commit));
    true
}
pub fn overlay(ed: &Editor) -> Vec<Prim> {
    if ed.tool != ToolKind::Gradient {
        return vec![];
    }
    let Some((pid, g)) = selected(ed) else { return vec![] };
    let xf = ed.doc.unit_xform(pid);
    let mut out = vec![Prim::Stroke {
        pts: vec![xf.apply(point(&g, [0., 0.])), xf.apply(point(&g, [1., 0.]))],
        width: 1.,
        color: ACCENT,
        clip: None,
    }];
    for (handle, p) in handles(&g, ed.ppu) {
        let p = xf.apply(p);
        out.push(match handle {
            Handle::Midpoint(_) => Prim::Square { c: p, half: 3., color: WHITE },
            _ => Prim::Disc { c: p, r: 4., color: ACCENT },
        });
    }
    out
}
