//! The "add-a-tool" pattern: each tool is a small stateless unit struct implementing `Tool`.
//! A tool defines what a PRESS does (it sets up a Drag or mutates the doc); the shared
//! move/up engine in `editor` handles the rest. Adding a tool = new file + a match arm below.

use crate::editor::{Editor, ToolKind};
use crate::geom::Pt;

pub mod pen;
pub mod direct;
pub mod object;
pub mod shapes;
pub mod convert;
pub mod eyedropper;

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
        ToolKind::Rect | ToolKind::Ellipse | ToolKind::Triangle | ToolKind::Polygon => &shapes::Shapes,
    }
}
