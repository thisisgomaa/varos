//! Slice 0.7 document view commands. Artboard fit/conversion adapts VectorCraft
//! engine/src/cmd/layer.rs:142 and menucmds.rs:150-155 @ a469568 (MIT OR Apache-2.0).
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewAction {
    MakeGuides,
    ReleaseGuides,
    ClearGuides,
    ToggleGrid,
    Grid { spacing: f32, subdivisions: u32 },
    GuidePosition { index: usize, position: f32 },
    FitArtboard { id: u32, selected: bool },
    ConvertArtboards,
    ReorderArtboard { id: u32, position: usize },
}
impl Editor {
    pub fn view_command(&mut self, action: ViewAction) {
        match action {
            ViewAction::ToggleGrid => self.doc.snap.show_grid = !self.doc.snap.show_grid,
            ViewAction::Grid { spacing, subdivisions } => {
                self.doc.snap.grid_spacing = spacing;
                self.doc.snap.grid_subdivisions = subdivisions;
            }
            ViewAction::ReorderArtboard { id, position } => {
                let _ = self.artboard_reorder(id, position);
            }
            ViewAction::GuidePosition { index, position } => {
                if self.doc.guides_locked || self.doc.guides.get(index).is_none_or(|g| g.pos == position) {
                    return;
                }
                self.begin();
                self.doc.guides[index].pos = position;
                self.dirty = true;
                self.commit();
            }
            ViewAction::MakeGuides | ViewAction::ReleaseGuides => {
                let ids = self.selected_pids();
                if ids.is_empty() {
                    return;
                }
                let before = self.doc.guide_paths.clone();
                let make = action == ViewAction::MakeGuides;
                self.begin();
                if make {
                    for p in &self.doc.paths {
                        if ids.contains(&p.id)
                            && !self.doc.guide_paths.contains(&p.id)
                            && !self.doc.is_mask_source(p.id)
                        {
                            self.doc.guide_paths.push(p.id);
                        }
                    }
                } else {
                    self.doc.guide_paths.retain(|id| !ids.contains(id));
                }
                self.dirty = self.doc.guide_paths != before;
                self.commit();
            }
            ViewAction::ClearGuides => {
                if self.doc.guides.is_empty() && self.doc.guide_paths.is_empty() {
                    return;
                }
                self.begin();
                self.doc.paths.retain(|p| !self.doc.guide_paths.contains(&p.id));
                self.doc.guide_paths.clear();
                self.doc.guides.clear();
                self.doc.sync_tree();
                self.dirty = true;
                self.commit();
                self.prune_inert_selection();
            }
            ViewAction::FitArtboard { id, selected } => {
                if let Some(b) = self.artwork_bounds(selected) {
                    let _ = self.artboard_set_rect(id, [b.0, b.1, (b.2 - b.0).max(1.0), (b.3 - b.1).max(1.0)]);
                }
            }
            ViewAction::ConvertArtboards => {
                let units = self.objsel_units();
                let boards: Vec<_> = units.iter().filter_map(|id| self.wave_unit_bbox(*id)).collect();
                if boards.is_empty() {
                    return;
                }
                let ids = self.selected_pids();
                self.begin();
                for b in boards {
                    let id = self.doc.nid();
                    self.doc.artboards.push(Artboard {
                        id,
                        x: b.0,
                        y: b.1,
                        w: (b.2 - b.0).max(1.0),
                        h: (b.3 - b.1).max(1.0),
                        ..Default::default()
                    });
                }
                self.doc.paths.retain(|p| !ids.contains(&p.id));
                self.doc.sync_tree();
                self.dirty = true;
                self.commit();
                self.escape();
            }
        }
    }
    pub fn artwork_bounds(&self, selected: bool) -> Option<(f32, f32, f32, f32)> {
        let ids = self.selected_pids();
        self.doc
            .paint_list()
            .filter(|(_, p)| !self.doc.eff_hidden(p.id) && (!selected || ids.contains(&p.id)) && !p.anchors.is_empty())
            .map(|(pi, _)| self.doc.outline_bbox(pi))
            .reduce(|a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)))
    }
    pub fn document_grid_step(&self) -> f32 {
        self.doc.snap.grid_spacing / self.doc.snap.grid_subdivisions.max(1) as f32
    }
}
