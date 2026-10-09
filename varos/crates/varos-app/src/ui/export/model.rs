//! Export checklist and persisted Minimal preferences. No renderer or event loop.
use crate::{
    app_command::SessionId,
    file_jobs::{CancelFlag, ExportJob, ScreenJob},
};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use varos_app::storage::layout::ExportPreferences;
use varos_core::model::Document;
use varos_raster::export::{self, Asset, Format, Options, Scope};

thread_local! { static PREFERENCES: std::cell::RefCell<BTreeMap<String, ExportPreferences>> = const { std::cell::RefCell::new(BTreeMap::new()) }; }
pub fn preferences() -> BTreeMap<String, ExportPreferences> {
    PREFERENCES.with(|p| p.borrow().clone())
}
pub fn restore_preferences(value: BTreeMap<String, ExportPreferences>) {
    PREFERENCES.with(|p| *p.borrow_mut() = value);
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cards {
    pub assets: Vec<Asset>,
    pub checked: Vec<bool>,
    pub anchor: Option<usize>,
}
impl Cards {
    pub fn new(assets: Vec<Asset>) -> Self {
        Self { checked: vec![true; assets.len()], assets, anchor: None }
    }
    fn identities(&self) -> Vec<String> {
        self.assets
            .iter()
            .map(|asset| {
                asset
                    .page
                    .artboard
                    .and_then(|i| asset.doc.artboards.get(i))
                    .map(|ab| format!("artboard:{}", ab.id))
                    .unwrap_or_else(|| asset.name.clone())
            })
            .collect()
    }
    fn remembered(&self) -> Vec<(String, bool)> {
        self.identities().into_iter().zip(self.checked.iter().copied()).collect()
    }
    fn restore_checks(&mut self, checks: &[(String, bool)]) {
        for (id, checked) in self.identities().iter().zip(&mut self.checked) {
            if let Some((_, on)) = checks.iter().find(|(key, _)| key == id) {
                *checked = *on;
            }
        }
    }
    pub fn count(&self) -> usize {
        self.checked.iter().filter(|v| **v).count()
    }
    pub fn click(&mut self, index: usize, shift: bool, double: bool) {
        if index >= self.checked.len() {
            return;
        }
        if double {
            self.checked.fill(false);
            self.checked[index] = true;
        } else if shift {
            if let Some(anchor) = self.anchor {
                let on = self.checked[anchor];
                for i in anchor.min(index)..=anchor.max(index) {
                    self.checked[i] = on;
                }
            } else {
                self.checked[index] = !self.checked[index];
            }
        } else {
            self.checked[index] = !self.checked[index];
        }
        self.anchor = Some(index);
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Minimal {
    pub boards: Cards,
    pub selection: Cards,
    pub selection_tab: bool,
    pub list: bool,
    pub folder: String,
    pub options: Options,
    pub key: String,
    pub remaining: usize,
    pub destinations: Vec<PathBuf>,
    pub report: varos_core::ExportReport,
    pub preview_id: u64,
}
impl Minimal {
    pub fn new(doc: &Document, selection: &HashSet<u32>, selection_tab: bool) -> Self {
        let boards = export::plan(doc, &if doc.artboards.is_empty() { Scope::WholeBoard } else { Scope::AllArtboards })
            .unwrap_or_default();
        let mut seen = HashSet::new();
        let mut assets = vec![];
        for path in &doc.paths {
            if !selection.contains(&path.id) {
                continue;
            }
            let unit = doc.unit_of(path.id).unwrap_or(path.id);
            if !seen.insert(unit) {
                continue;
            }
            let members: HashSet<_> = doc
                .group_members(path.id)
                .into_iter()
                .filter(|id| selection.contains(id))
                .chain(std::iter::once(path.id))
                .collect();
            if let Ok(mut planned) = export::plan(doc, &Scope::Selection(members)) {
                if let Some(mut asset) = planned.pop() {
                    asset.name = doc
                        .node(unit)
                        .map(|n| n.name.as_str())
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("Asset {unit}"));
                    assets.push(asset);
                }
            }
        }
        let selection = Cards::new(assets);
        let selection_tab = selection_tab && !selection.assets.is_empty();
        let folder = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join("Desktop/Export")
            .to_string_lossy()
            .into_owned();
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        if let Ok(bytes) = serde_json::to_vec(doc) {
            bytes.hash(&mut hasher);
        }
        let mut ids: Vec<_> = selection.assets.iter().map(|a| &a.name).collect();
        ids.sort();
        ids.hash(&mut hasher);
        Self {
            boards: Cards::new(boards),
            selection,
            selection_tab,
            list: false,
            folder,
            options: Options::default(),
            key: String::new(),
            remaining: 0,
            destinations: vec![],
            report: Default::default(),
            preview_id: hasher.finish(),
        }
    }
    pub fn cards(&self) -> &Cards {
        if self.selection_tab {
            &self.selection
        } else {
            &self.boards
        }
    }
    pub fn cards_mut(&mut self) -> &mut Cards {
        if self.selection_tab {
            &mut self.selection
        } else {
            &mut self.boards
        }
    }
    pub fn selection_reason(&self) -> Option<&'static str> {
        self.selection.assets.is_empty().then_some("Select something to export it.")
    }
    pub fn remember(&self) {
        if self.key.is_empty() {
            return;
        }
        let p = ExportPreferences {
            selection_tab: self.selection_tab,
            list: self.list,
            folder: self.folder.clone(),
            checked: self.cards().checked.clone(),
            artboard_cards: self.boards.remembered(),
            selection_cards: self.selection.remembered(),
            format: self.options.format.extension().into(),
            scale: self.options.scale,
            transparent: self.options.transparent,
            quality: self.options.quality,
            advanced: false,
        };
        PREFERENCES.with(|prefs| prefs.borrow_mut().insert(self.key.clone(), p));
    }
    pub fn restore(&mut self, key: String) {
        self.key = key;
        let p = PREFERENCES.with(|prefs| prefs.borrow().get(&self.key).cloned());
        if let Some(p) = p {
            self.selection_tab = p.selection_tab && self.selection_reason().is_none();
            self.list = p.list;
            self.folder = p.folder;
            self.boards.restore_checks(&p.artboard_cards);
            self.selection.restore_checks(&p.selection_cards);
            self.options = Options {
                format: Format::parse(&p.format).unwrap_or(Format::Pdf),
                scale: p.scale,
                transparent: p.transparent,
                quality: p.quality,
            };
        }
    }
    pub fn jobs(&self, sid: SessionId, ticket: u64, cancel: CancelFlag) -> Vec<ScreenJob> {
        self.cards()
            .assets
            .iter()
            .zip(&self.cards().checked)
            .filter(|(_, on)| **on)
            .map(|(asset, _)| {
                screen_job(
                    sid,
                    ticket,
                    asset.clone(),
                    self.options.clone(),
                    PathBuf::from(&self.folder).join(export::file_name(&asset.name, "", self.options.format, 1)),
                    cancel.clone(),
                    true,
                )
            })
            .collect()
    }
}
pub fn screen_job(
    sid: SessionId,
    ticket: u64,
    asset: Asset,
    options: Options,
    dest: PathBuf,
    cancel: CancelFlag,
    collision_names: bool,
) -> ScreenJob {
    let plan = varos_pdf::ExportPlan {
        scope: varos_pdf::ExportScope::ArtworkBounds,
        pages: vec![varos_pdf::PageSpec { rect: asset.page.rect, background: asset.page.background }],
    };
    ScreenJob {
        job: ExportJob { sid, ticket, dest, doc: Arc::clone(&asset.doc), plan, replace_confirmed: false, cancel },
        asset,
        options,
        collision_names,
        additional: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toggles_range_double_and_disable() {
        let mut cards = Cards { assets: vec![], checked: vec![false; 4], anchor: None };
        cards.click(0, false, false);
        cards.click(2, true, false);
        assert_eq!(cards.checked, [true, true, true, false]);
        cards.click(3, false, true);
        assert_eq!(cards.checked, [false, false, false, true]);
        let minimal = Minimal::new(&Document::default(), &HashSet::new(), true);
        assert_eq!(minimal.selection_reason(), Some("Select something to export it."));
        assert!(!minimal.selection_tab);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    #[test]
    fn preferences_roundtrip_and_one_card_one_job() {
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-cli/tests/fixtures/v3_nested_group.vrs");
        let doc = varos_pdf::load_vrs(&fixture).unwrap();
        let mut model = Minimal::new(&doc, &HashSet::new(), false);
        model.restore("fixture-document".into());
        model.boards.checked[1] = false;
        model.folder = "/tmp/Export".into();
        model.options.format = Format::Svg;
        model.remember();
        let serialized = serde_json::to_vec(&preferences()).unwrap();
        restore_preferences(serde_json::from_slice(&serialized).unwrap());
        let mut back = Minimal::new(&doc, &HashSet::new(), false);
        back.restore("fixture-document".into());
        assert_eq!(back.boards.checked, model.boards.checked);
        assert_eq!(back.folder, model.folder);
        assert_eq!(back.options, model.options);
        let jobs = back.jobs(SessionId(1), 1, Default::default());
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job.plan.pages[0].rect, [0.0, 0.0, 100.0, 100.0]);
    }
    #[test]
    fn app_job_bridge_and_shared_bytes_identical_and_collisions_numbered() {
        let fixture =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-cli/tests/fixtures/v3_nested_group.vrs");
        let doc = varos_pdf::load_vrs(&fixture).unwrap();
        let dir =
            std::env::temp_dir().join(format!("varos-screen-parity-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::create_dir_all(&dir).unwrap();
        for format in [Format::Svg, Format::Png] {
            let mut model = Minimal::new(&doc, &HashSet::new(), false);
            model.boards.checked[1] = false;
            model.folder = dir.to_string_lossy().into_owned();
            model.options.format = format;
            let jobs = model.jobs(SessionId(1), 42, Default::default());
            let output =
                export::encode(&jobs[0].asset, &jobs[0].options, &std::sync::atomic::AtomicBool::new(false)).unwrap();
            let dest = jobs[0].job.dest.clone();
            let done = crate::file_jobs::execute(
                crate::file_jobs::FileJob::Screen(Box::new(jobs[0].clone())),
                &mut crate::file_ports::DiskStore,
            );
            assert!(
                matches!(done,crate::file_jobs::FileDone::Exported(ref done) if done.result == crate::file_jobs::ExportResult::Exported),
                "{done:?}"
            );
            assert_eq!(std::fs::read(&dest).unwrap(), output.bytes);
            let request = varos_bridge::mcp::decode_tool(if format == Format::Svg { "export_svg" } else { "export_raster" },serde_json::json!({"api":"1.2","board":"b1","request_id":"r1","expected_rev":0,"path":dest,"scope":format!("artboard:{}",doc.artboards[0].id)})).unwrap();
            let file = match request {
                varos_bridge::Request::ExportSvg(file) | varos_bridge::Request::ExportRaster(file) => file,
                _ => unreachable!(),
            };
            let bridge = crate::export_ui::bridge_job(
                SessionId(1),
                43,
                &doc,
                &HashSet::new(),
                dest.clone(),
                &file,
                if format == Format::Svg { "export_svg" } else { "export_raster" },
            )
            .unwrap();
            assert_eq!(
                export::encode(&bridge.asset, &bridge.options, &std::sync::atomic::AtomicBool::new(false))
                    .unwrap()
                    .bytes,
                output.bytes
            );
            let done = crate::file_jobs::execute(
                crate::file_jobs::FileJob::Screen(Box::new(jobs[0].clone())),
                &mut crate::file_ports::DiskStore,
            );
            let crate::file_jobs::FileDone::Exported(done) = done else { panic!("unexpected completion") };
            assert!(done.job.dest.file_name().unwrap().to_string_lossy().contains(" 2."));
            assert_eq!(std::fs::read(dest).unwrap(), output.bytes);
            let cancel = CancelFlag::default();
            cancel.cancel();
            let cancelled = crate::file_jobs::execute(
                crate::file_jobs::FileJob::Screen(Box::new(model.jobs(SessionId(1), 44, cancel).remove(0))),
                &mut crate::file_ports::DiskStore,
            );
            assert!(
                matches!(cancelled,crate::file_jobs::FileDone::Exported(done) if done.result == crate::file_jobs::ExportResult::Cancelled)
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
