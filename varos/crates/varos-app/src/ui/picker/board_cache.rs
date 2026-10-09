//! Outline membership is expensive: scan only for a visible Board drawer, once per key.
use super::*;
#[derive(Default)]
pub(crate) struct BoardColors {
    key: Option<(u64, usize, Vec<[f32; 4]>)>,
    colors: Vec<Rgba>,
    #[cfg(test)]
    pub(crate) scans: usize,
}
impl BoardColors {
    pub(crate) fn read(&mut self, ed: &Editor, visible: bool) -> Vec<Rgba> {
        if !visible {
            return vec![];
        }
        let key = (ed.rev, ed.doc.active, ed.doc.artboards.iter().map(|a| [a.x, a.y, a.w, a.h]).collect());
        if self.key.as_ref() != Some(&key) {
            self.colors = ed.board_colors();
            self.key = Some(key);
            #[cfg(test)]
            {
                self.scans += 1;
            }
        }
        self.colors.clone()
    }
}
