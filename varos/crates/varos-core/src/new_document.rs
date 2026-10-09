// Adapted from VectorCraft engine/src/cmd/newdoc.rs:19,125 @ a469568 (MIT OR Apache-2.0).
// Copyright (c) 2026 ArtCraft Team and the VectorCraft contributors. See NOTICE.
//! New document settings are pure and shared by the sheet, Bridge and CLI.
use crate::{
    model::{Artboard, Document},
    units::{to_pt, DocUnits, Unit},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    #[default]
    Grid,
    Row,
    Column,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub width: f32,
    pub height: f32,
    pub units: Unit,
    pub count: usize,
    pub columns: usize,
    pub spacing: f32,
    pub layout: Layout,
    pub bleed: f32,
    pub ppi: f32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 210.,
            height: 297.,
            units: Unit::Mm,
            count: 1,
            columns: 3,
            spacing: 20.,
            layout: Layout::Grid,
            bleed: 0.,
            ppi: 300.,
        }
    }
}
impl Settings {
    pub fn category(category: usize) -> Self {
        match category {
            1 => Self { width: 1440., height: 900., units: Unit::Px, ppi: 72., ..Self::default() },
            2 => Self { width: 390., height: 844., units: Unit::Px, ppi: 72., ..Self::default() },
            3 => Self { width: 1080., height: 1080., units: Unit::Px, ppi: 72., ..Self::default() },
            _ => Self::default(),
        }
    }
    /// Change the display unit without resizing the configured physical document.
    pub fn set_units(&mut self, units: Unit) {
        for value in [&mut self.width, &mut self.height, &mut self.spacing, &mut self.bleed] {
            *value = crate::units::from_pt(to_pt(*value, self.units, self.ppi), units, self.ppi);
        }
        self.units = units;
    }
    pub fn document(&self) -> Result<Document, String> {
        if !crate::document_setup::valid_ppi(self.ppi)
            || !(1..=100).contains(&self.count)
            || !(1..=100).contains(&self.columns)
        {
            return Err("Use 1–100 artboards/columns and 1–9600 ppi".into());
        }
        let w = to_pt(self.width, self.units, self.ppi);
        let h = to_pt(self.height, self.units, self.ppi);
        let bleed = to_pt(self.bleed, self.units, self.ppi);
        let gap = to_pt(self.spacing, self.units, self.ppi);
        if [w, h, bleed, gap].iter().any(|v| !v.is_finite())
            || w <= 0.
            || h <= 0.
            || w > 1e6
            || h > 1e6
            || gap < 0.
            || gap > 7200.
            || !(0.0..=7200.0).contains(&bleed)
        {
            return Err("Invalid size, spacing or bleed".into());
        }
        let mut doc = Document { units: DocUnits { display: self.units, ppi: self.ppi }, ..Document::default() };
        for i in 0..self.count {
            let (x, y) = match self.layout {
                Layout::Row => (i, 0),
                Layout::Column => (0, i),
                Layout::Grid => (i % self.columns, i / self.columns),
            };
            doc.artboards.push(Artboard {
                x: x as f32 * (w + gap),
                y: y as f32 * (h + gap),
                w,
                h,
                bleed,
                name: format!("Artboard {}", i + 1),
                ..Artboard::default()
            });
        }
        doc.assign_artboard_ids();
        doc.sync_tree();
        Ok(doc)
    }
}
