//! The "add-a-tool" pattern: each tool is a small stateless unit struct implementing `Tool`.
//! A tool defines what a PRESS does (it sets up a Drag or mutates the doc); the shared
//! move/up engine in `editor` handles the rest. Adding a tool = new file + a match arm below.

use crate::editor::{Editor, ToolKind};
use crate::geom::Pt;

pub mod anchor_edit;
pub mod convert;
pub mod direct;
pub mod eyedropper;
pub mod object;
pub mod pen;
pub mod rotate;
pub mod select_transform;
pub mod shapes;

pub trait Tool {
    fn down(&self, ed: &mut Editor, pos: Pt);
}

pub fn get(kind: ToolKind) -> &'static dyn Tool {
    match kind {
        // ---- Lane D: handled before stateless tool dispatch ----
        ToolKind::RoundedRect
        | ToolKind::Star
        | ToolKind::Line
        | ToolKind::Arc
        | ToolKind::Spiral
        | ToolKind::RectGrid
        | ToolKind::PolarGrid
        | ToolKind::Pencil
        | ToolKind::Smooth
        | ToolKind::PathEraser
        | ToolKind::Join
        | ToolKind::Curvature => &object::Object,
        ToolKind::Pen => &pen::Pen,
        ToolKind::Direct => &direct::Direct,
        ToolKind::Object | ToolKind::FreeTransform => &object::Object,
        ToolKind::Text | ToolKind::Hand | ToolKind::Zoom => &object::Object, // view gestures are owned by the app
        ToolKind::Lasso => &anchor_edit::Lasso,
        ToolKind::AddAnchor => &anchor_edit::Add,
        ToolKind::DeleteAnchor => &anchor_edit::Delete,
        ToolKind::Convert => &convert::Convert,
        ToolKind::Eyedropper => &eyedropper::Eyedropper,
        ToolKind::Rotate | ToolKind::Scale | ToolKind::Reflect | ToolKind::Shear | ToolKind::MagicWand => {
            &rotate::Transform
        }
        ToolKind::Rect | ToolKind::Ellipse | ToolKind::Triangle | ToolKind::Polygon => &shapes::Shapes,
        // The Artboard tool is handled by `Editor::ab_down` before `get` is ever called — this arm only
        // keeps the match exhaustive (the value is never used).
        ToolKind::ShapeBuilder | ToolKind::Scissors | ToolKind::Knife | ToolKind::Eraser | ToolKind::Artboard => {
            &object::Object
        }
    }
}
