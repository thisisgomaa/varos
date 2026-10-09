//! Lane B: resolved paint becomes a world-space scene primitive once.
use crate::{
    geom::Pt,
    model::{Paint, Xform},
    scene::Prim,
};
pub fn prim(rings: Vec<Vec<Pt>>, paint: &Paint, xf: Xform) -> Prim {
    match paint {
        Paint::Gradient(g) => Prim::GradientFill { rings, gradient: g.transformed(xf), opacity: 1.0, stroke: false },
        _ => Prim::Fill { rings, color: paint.solid().unwrap_or([0.; 4]) },
    }
}
