use super::Tool;
use crate::editor::Editor;
use crate::geom::Pt;

pub struct Eyedropper;
impl Tool for Eyedropper {
    fn down(&self, ed: &mut Editor, pos: Pt) {
        if let Some(pid) = ed.path_under(pos) { ed.eyedrop(pid); }
    }
}
