//! The "add-a-tool" pattern: each tool is a small stateless unit struct implementing `Tool`.
//! A tool defines what a PRESS does (it sets up a Drag or mutates the doc); the shared
//! move/up engine in `editor` handles the rest. Adding a tool = new file + a match arm below.

use crate::editor::{Editor, ToolKind};
use crate::geom::Pt;

pub mod convert;
pub mod direct;
pub mod eyedropper;
pub mod object;
pub mod pen;
pub mod rotate;
pub mod shapes;

pub trait Tool {
    fn down(&self, ed: &mut Editor, pos: Pt);
}

pub fn get(kind: ToolKind) -> &'static dyn Tool {
    match kind {
        ToolKind::Pen => &pen::Pen,
        ToolKind::Direct => &direct::Direct,
        ToolKind::Object => &object::Object,
        ToolKind::Convert => &convert::Convert,
        ToolKind::Eyedropper => &eyedropper::Eyedropper,
        ToolKind::Rotate | ToolKind::Scale => &rotate::Transform,
        ToolKind::Rect | ToolKind::Ellipse | ToolKind::Triangle | ToolKind::Polygon => &shapes::Shapes,
        // The Artboard tool is handled by `Editor::ab_down` before `get` is ever called — this arm only
        // keeps the match exhaustive (the value is never used).
        ToolKind::Artboard => &object::Object,
    }
}
