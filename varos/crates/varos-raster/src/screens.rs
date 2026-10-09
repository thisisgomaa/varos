//! Pure Advanced export state and card × row job expansion shared with headless hosts.
use crate::export::{Asset, Format, Options};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::PathBuf};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Row {
    pub scale: f32,
    pub suffix: String,
    pub format: String,
    pub svg: varos_core::svg::options::Options,
}
impl Default for Row {
    fn default() -> Self {
        Self { scale: 1., suffix: String::new(), format: "png".into(), svg: Default::default() }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Subfolders {
    #[default]
    None,
    Scale,
    Format,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Advanced {
    pub rows: Vec<Row>,
    pub prefix: String,
    pub open_folder: bool,
    pub subfolders: Subfolders,
    pub pdf_single: bool,
    pub include_bleed: bool,
    pub include_colour: bool,
    pub whole_board: bool,
    pub range: String,
}
impl Default for Advanced {
    fn default() -> Self {
        Self {
            rows: vec![Row::default()],
            prefix: String::new(),
            open_folder: false,
            subfolders: Subfolders::None,
            pdf_single: false,
            include_bleed: false,
            include_colour: true,
            whole_board: false,
            range: String::new(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct Planned {
    pub asset: Asset,
    pub options: Options,
    pub relative: PathBuf,
    pub pages: Vec<Asset>,
    pub svg: varos_core::svg::options::Options,
}
impl Advanced {
    pub fn preset(&mut self, index: usize) {
        self.rows = match index {
            0 => [1., 2., 3.]
                .into_iter()
                .map(|scale| Row {
                    scale,
                    suffix: if scale == 1. { String::new() } else { format!("@{scale}x") },
                    ..Row::default()
                })
                .collect(),
            1 => [(1., "-mdpi"), (1.5, "-hdpi"), (2., "-xhdpi"), (3., "-xxhdpi"), (4., "-xxxhdpi")]
                .into_iter()
                .map(|(scale, suffix)| Row { scale, suffix: suffix.into(), ..Row::default() })
                .collect(),
            _ => vec![
                Row::default(),
                Row { scale: 2., suffix: "@2x".into(), ..Row::default() },
                Row { format: "svg".into(), ..Row::default() },
            ],
        };
    }
    pub fn checks(&self, count: usize) -> Result<Vec<bool>, String> {
        if self.range.trim().is_empty() {
            return Ok(vec![true; count]);
        }
        let mut checked = vec![false; count];
        for part in self.range.split(',') {
            let mut ends = part.trim().split('-');
            let a = ends.next().unwrap_or_default().trim().parse::<usize>().map_err(|_| "Use ranges such as 1-3,5")?;
            let b =
                ends.next().map(|s| s.trim().parse::<usize>()).transpose().map_err(|_| "Invalid range")?.unwrap_or(a);
            if ends.next().is_some() || a == 0 || b < a || b > count {
                return Err("Artboard range is outside the document".into());
            }
            checked[a - 1..b].fill(true);
        }
        Ok(checked)
    }
    pub fn expand(&self, assets: &[Asset], base: &Options) -> Result<Vec<Planned>, String> {
        if self.rows.is_empty() || self.rows.len() > 32 {
            return Err("Use 1–32 export rows".into());
        }
        if self.prefix.contains(['/', '\\']) || self.rows.iter().any(|r| r.suffix.contains(['/', '\\'])) {
            return Err("Prefix and suffix cannot contain path separators".into());
        }
        let checks = self.checks(assets.len())?;
        let selected: Vec<_> = assets.iter().zip(checks).filter_map(|(a, on)| on.then_some(a)).collect();
        let mut names = HashSet::new();
        let mut jobs = vec![];
        for row in &self.rows {
            row.svg.validate()?;
            let format = Format::parse(&row.format)?;
            let options = Options { format, scale: row.scale, transparent: !self.include_colour, ..base.clone() };
            options.validate()?;
            let mut pages: Vec<_> = selected
                .iter()
                .map(|asset| {
                    let mut a = (*asset).clone();
                    if !self.include_colour {
                        a.page.background = None;
                    }
                    if self.include_bleed {
                        if let Some(ab) = a.page.artboard.and_then(|i| a.doc.artboards.get(i)) {
                            let [top, right, bottom, left] = varos_core::document_setup::bleed(ab);
                            a.page.rect[0] -= left;
                            a.page.rect[1] -= top;
                            a.page.rect[2] += left + right;
                            a.page.rect[3] += top + bottom;
                        }
                    }
                    a
                })
                .collect();
            if pages.is_empty() {
                continue;
            }
            let single = format == Format::Pdf && self.pdf_single;
            if single && pages.iter().any(|a| a.doc != pages[0].doc) {
                return Err(
                    "Single PDF requires one shared document snapshot; export Selection as separate files".into()
                );
            }
            let targets = if single { vec![pages[0].clone()] } else { pages.clone() };
            for asset in targets {
                let stem = format!("{}{}", self.prefix, if single { "Artboards" } else { &asset.name });
                let mut n = 1;
                let dir = match self.subfolders {
                    Subfolders::None => PathBuf::new(),
                    Subfolders::Scale => PathBuf::from(format!("{}x", row.scale)),
                    Subfolders::Format => PathBuf::from(format.extension()),
                };
                let relative = loop {
                    let p = dir.join(crate::export::file_name(&stem, &row.suffix, format, n));
                    if names.insert(p.clone()) {
                        break p;
                    }
                    n += 1;
                };
                jobs.push(Planned {
                    asset,
                    options: options.clone(),
                    relative,
                    pages: if single { std::mem::take(&mut pages) } else { vec![] },
                    svg: row.svg.clone(),
                });
            }
        }
        Ok(jobs)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use varos_core::model::{Artboard, Document};
    fn assets() -> Vec<Asset> {
        let mut d = Document {
            artboards: vec![
                Artboard {
                    w: 100.,
                    h: 100.,
                    name: "One".into(),
                    bleed: 3.,
                    page_color: Some([1.; 4]),
                    ..Default::default()
                },
                Artboard { x: 200., w: 100., h: 100., name: "Two".into(), ..Default::default() },
            ],
            ..Default::default()
        };
        d.assign_artboard_ids();
        crate::export::plan(&d, &crate::export::Scope::AllArtboards).unwrap()
    }
    #[test]
    fn card_row_names_suffixes_subfolders_and_collisions() {
        let mut s = Advanced { prefix: "brand-".into(), subfolders: Subfolders::Scale, ..Default::default() };
        s.preset(0);
        let jobs = s.expand(&assets(), &Default::default()).unwrap();
        assert_eq!(jobs.len(), 6);
        assert_eq!(jobs[0].relative, PathBuf::from("1x/brand-One.png"));
        assert_eq!(jobs[2].relative, PathBuf::from("2x/brand-One@2x.png"));
        s.rows = vec![Row::default(), Row::default()];
        s.subfolders = Subfolders::Format;
        let jobs = s.expand(&assets(), &Default::default()).unwrap();
        assert_eq!(jobs[2].relative, PathBuf::from("png/brand-One 2.png"));
    }
    #[test]
    fn range_bleed_colour_pdf_single_and_presets() {
        let mut s = Advanced { range: "2".into(), include_bleed: true, include_colour: false, ..Default::default() };
        assert_eq!(s.expand(&assets(), &Default::default()).unwrap().len(), 1);
        s.range = "1-2".into();
        let j = s.expand(&assets(), &Default::default()).unwrap();
        assert_eq!(j[0].asset.page.rect, [-3., -3., 106., 106.]);
        assert_eq!(j[0].asset.page.background, None);
        s.rows = vec![Row { format: "pdf".into(), ..Default::default() }];
        s.pdf_single = true;
        let j = s.expand(&assets(), &Default::default()).unwrap();
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].pages.len(), 2);
        s.preset(1);
        assert_eq!(s.rows.len(), 5);
        s.preset(2);
        assert_eq!(s.rows.len(), 3);
        assert_eq!(s.rows[2].format, "svg");
    }
    #[test]
    fn refuses_invalid_rows_ranges_and_path_separators() {
        let mut s = Advanced::default();
        for range in ["0", "1-3", "3-1", "1--2"] {
            s.range = range.into();
            assert!(s.expand(&assets(), &Default::default()).is_err());
        }
        s.range.clear();
        s.prefix = "../".into();
        assert!(s.expand(&assets(), &Default::default()).is_err());
        s.prefix.clear();
        s.rows[0].scale = f32::NAN;
        assert!(s.expand(&assets(), &Default::default()).is_err());
    }
    #[test]
    fn single_pdf_refuses_different_page_snapshots() {
        let mut assets = assets();
        std::sync::Arc::make_mut(&mut assets[1].doc).artboards[0].name = "Different snapshot".into();
        let s = Advanced {
            rows: vec![Row { format: "pdf".into(), ..Default::default() }],
            pdf_single: true,
            ..Default::default()
        };
        assert!(s.expand(&assets, &Default::default()).unwrap_err().contains("shared document snapshot"));
        assert_eq!(Advanced { pdf_single: false, ..s }.expand(&assets, &Default::default()).unwrap().len(), 2);
    }
    #[test]
    fn persisted_settings_roundtrip() {
        let mut s = Advanced::default();
        s.preset(2);
        s.prefix = "icon-".into();
        let b = serde_json::to_vec(&s).unwrap();
        assert_eq!(serde_json::from_slice::<Advanced>(&b).unwrap(), s);
    }
}
