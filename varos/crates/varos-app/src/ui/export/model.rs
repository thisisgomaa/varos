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
    identities: Vec<String>,
    pub checked: Vec<bool>,
    pub anchor: Option<usize>,
}
impl Cards {
    pub fn new(assets: Vec<Asset>) -> Self {
        let identities = assets
            .iter()
            .map(|asset| {
                asset
                    .page
                    .artboard
                    .and_then(|i| asset.doc.artboards.get(i))
                    .map(|ab| format!("artboard:{}", ab.id))
                    .unwrap_or_else(|| "whole-board".into())
            })
            .collect();
        Self { checked: vec![true; assets.len()], assets, identities, anchor: None }
    }
    fn remembered(&self) -> Vec<(String, bool)> {
        self.identities.iter().cloned().zip(self.checked.iter().copied()).collect()
    }
    fn restore_checks(&mut self, checks: &[(String, bool)]) {
        for (id, checked) in self.identities.iter().zip(&mut self.checked) {
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
    pub blobs: Arc<varos_core::images::BlobStore>,
    pub preview_id: u64,
    pub previews: super::previews::Previews,
    pub preferences_dirty: bool,
    pub advanced: bool,
    pub revealed_folder: bool,
    pub screen_settings: varos_raster::screens::Advanced,
}
impl Minimal {
    pub fn new(doc: &Document, selection: &HashSet<u32>, selection_tab: bool) -> Self {
        let boards = export::plan(doc, &if doc.artboards.is_empty() { Scope::WholeBoard } else { Scope::AllArtboards })
            .unwrap_or_default();
        let mut seen = HashSet::new();
        let mut assets = vec![];
        let mut selection_ids = vec![];
        for pid in doc.paths.iter().map(|p| p.id).chain(doc.images.iter().map(|i| i.id)) {
            if !selection.contains(&pid) {
                continue;
            }
            let unit = doc.unit_of(pid).unwrap_or(pid);
            if !seen.insert(unit) {
                continue;
            }
            let members: HashSet<_> = doc
                .group_members(pid)
                .into_iter()
                .filter(|id| selection.contains(id))
                .chain(std::iter::once(pid))
                .collect();
            if let Ok(mut planned) = export::plan(doc, &Scope::Selection(members)) {
                if let Some(mut asset) = planned.pop() {
                    asset.name = doc
                        .node(unit)
                        .map(|n| n.name.as_str())
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("Asset {unit}"));
                    selection_ids.push(format!("selection:{unit}"));
                    assets.push(asset);
                }
            }
        }
        let mut selection = Cards::new(assets);
        selection.identities = selection_ids;
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
        let mut ids: Vec<_> = selection.identities.iter().collect();
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
            blobs: Default::default(),
            preview_id: hasher.finish(),
            previews: Default::default(),
            preferences_dirty: false,
            advanced: false,
            revealed_folder: false,
            screen_settings: Default::default(),
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
    pub fn remember(&mut self) {
        if !self.preferences_dirty || self.key.is_empty() {
            return;
        }
        self.preferences_dirty = false;
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
            advanced: self.advanced,
            screen_settings: self.screen_settings.clone(),
        };
        PREFERENCES.with(|prefs| prefs.borrow_mut().insert(self.key.clone(), p));
    }
    pub fn restore(&mut self, key: String) {
        self.key = key;
        let p = PREFERENCES.with(|prefs| prefs.borrow().get(&self.key).cloned());
        if let Some(p) = p {
            self.selection_tab = p.selection_tab && self.selection_reason().is_none();
            self.advanced = p.advanced;
            self.screen_settings = p.screen_settings;
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
        if self.advanced {
            let range = if self.selection_tab || self.screen_settings.whole_board {
                vec![true; self.cards().assets.len()]
            } else {
                match self.screen_settings.checks(self.boards.assets.len()) {
                    Ok(range) => range,
                    Err(_) => return vec![],
                }
            };
            let assets: Vec<_> = if self.screen_settings.whole_board && !self.selection_tab {
                self.cards()
                    .assets
                    .first()
                    .and_then(|a| export::plan(&a.doc, &Scope::WholeBoard).ok())
                    .unwrap_or_default()
            } else {
                self.cards()
                    .assets
                    .iter()
                    .zip(&self.cards().checked)
                    .zip(range)
                    .filter_map(|((a, on), ranged)| (*on && ranged).then_some(a.clone()))
                    .collect()
            };
            let mut settings = self.screen_settings.clone();
            // Range refers to the original artboard grid, before checkbox filtering.
            settings.range.clear();
            if self.selection_tab {
                settings.pdf_single = false;
            }
            return settings
                .expand(&assets, &self.options)
                .unwrap_or_default()
                .into_iter()
                .map(|p| planned_screen_job(sid, ticket, p, PathBuf::from(&self.folder), cancel.clone(), true))
                .collect();
        }
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
/// Advanced expansion already resolved colour and bleed into each page rectangle.
/// Keep PDF bleed metadata empty so Press options cannot expand the same bleed twice.
pub fn planned_screen_job(
    sid: SessionId,
    ticket: u64,
    p: varos_raster::screens::Planned,
    folder: PathBuf,
    cancel: CancelFlag,
    collision_names: bool,
) -> ScreenJob {
    let mut job = screen_job(sid, ticket, p.asset, p.options, folder.join(p.relative), cancel, collision_names);
    job.svg_options = p.svg;
    job.folder_root = Some(folder);
    if !p.pages.is_empty() {
        job.job.plan.pages = p
            .pages
            .iter()
            .map(|a| varos_pdf::PageSpec {
                rect: a.page.rect,
                background: a.page.background,
                bleed: 0.,
                bleed_edges: [0.; 4],
            })
            .collect();
    } else {
        for page in &mut job.job.plan.pages {
            page.bleed = 0.;
            page.bleed_edges = [0.; 4];
        }
    }
    job
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
        pages: vec![varos_pdf::PageSpec {
            rect: asset.page.rect,
            background: asset.page.background,
            bleed: asset.page.artboard.and_then(|index| asset.doc.artboards.get(index)).map_or(0.0, |ab| ab.bleed),
            bleed_edges: asset
                .page
                .artboard
                .and_then(|index| asset.doc.artboards.get(index))
                .map_or([0.0; 4], varos_core::document_setup::bleed),
        }],
    };
    ScreenJob {
        job: ExportJob {
            blobs: Default::default(),
            pdf_options: Default::default(),
            sid,
            ticket,
            dest,
            doc: Arc::clone(&asset.doc),
            plan,
            replace_confirmed: false,
            cancel,
        },
        asset,
        options,
        collision_names,
        additional: vec![],
        svg_options: Default::default(),
        additional_jobs: vec![],
        folder_root: None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn selected_image_has_a_real_export_card() {
        let mut ed = varos_core::Editor::new();
        let bytes = varos_core::images::codec::encode_png(&varos_core::images::Pixels {
            budget: None,
            width: 2,
            height: 2,
            rgba: std::sync::Arc::from([255, 0, 0, 255].repeat(4)),
        })
        .unwrap();
        let id =
            varos_core::images::links::place_bytes(&mut ed, &bytes, [0.; 2], None, Default::default(), None).unwrap().0;
        let model = super::Minimal::new(&ed.doc, &std::collections::HashSet::from([id]), true);
        assert!(model.selection_tab);
        assert_eq!(model.selection.assets.len(), 1);
    }

    use super::*;
    #[test]
    fn toggles_range_double_and_disable() {
        let mut cards = Cards { assets: vec![], identities: vec![], checked: vec![false; 4], anchor: None };
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
        model.preferences_dirty = true;
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
            let output = export::encode_with_svg_options(
                &jobs[0].asset,
                &jobs[0].options,
                &std::sync::atomic::AtomicBool::new(false),
                &jobs[0].svg_options,
            )
            .unwrap();
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
                export::encode_with_svg_options(
                    &bridge.asset,
                    &bridge.options,
                    &std::sync::atomic::AtomicBool::new(false),
                    &bridge.svg_options
                )
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

#[cfg(test)]
mod fix_round_tests {
    use super::*;
    #[test]
    fn duplicate_selection_names_restore_by_id_after_rename_and_reorder() {
        let mut editor = varos_core::editor::Editor::new();
        for x in [0.0, 50.0] {
            editor
                .try_execute_created(varos_core::EditCommand::AddShape {
                    kind: varos_core::model::ShapeKind::Rect,
                    bounds: [x, 0.0, 20.0, 20.0],
                    parent: None,
                    fill: Some([1.0; 4]),
                    stroke: None,
                    stroke_width: 0.0,
                    opacity: 1.0,
                    name: Some("Rectangle".into()),
                })
                .unwrap();
        }
        for node in &mut editor.doc.nodes {
            node.name = "Rectangle".into();
        }
        let selected = editor.doc.paths.iter().map(|p| p.id).collect();
        let mut model = Minimal::new(&editor.doc, &selected, true);
        assert_eq!(model.selection.assets.len(), 2);
        assert_eq!(model.selection.assets[0].name, model.selection.assets[1].name);
        model.restore("duplicate-names-fix".into());
        model.selection.click(0, false, false);
        model.preferences_dirty = true;
        model.remember();
        let checks = model.selection.remembered();
        editor.doc.paths.reverse();
        for node in &mut editor.doc.nodes {
            node.name = "Renamed".into();
        }
        let mut back = Minimal::new(&editor.doc, &selected, true);
        back.restore("duplicate-names-fix".into());
        for (id, checked) in back.selection.remembered() {
            assert_eq!(checks.iter().find(|(key, _)| *key == id).unwrap().1, checked);
        }
        assert_eq!(back.selection.count(), 1);
    }
    #[test]
    fn repaint_without_edits_does_not_rebuild_preferences() {
        let mut model = Minimal::new(&Document::default(), &HashSet::new(), false);
        model.restore("dirty-export-fix".into());
        model.folder = "/tmp/first".into();
        model.preferences_dirty = true;
        model.remember();
        assert!(!model.preferences_dirty);
        model.folder = "/tmp/unmarked".into();
        model.remember();
        assert_eq!(preferences()["dirty-export-fix"].folder, "/tmp/first");
        model.preferences_dirty = true;
        model.remember();
        assert_eq!(preferences()["dirty-export-fix"].folder, "/tmp/unmarked");
    }
}
#[cfg(test)]
mod lane_c_tests {
    use super::*;
    #[test]
    fn advanced_preferences_and_card_row_jobs_roundtrip() {
        let settings = varos_core::new_document::Settings { count: 2, bleed: 3., ..Default::default() };
        let doc = settings.document().unwrap();
        let mut m = Minimal::new(&doc, &HashSet::new(), false);
        m.restore("lane-c-job".into());
        m.advanced = true;
        m.screen_settings.preset(2);
        m.screen_settings.prefix = "brand-".into();
        m.screen_settings.subfolders = varos_raster::screens::Subfolders::Format;
        m.preferences_dirty = true;
        m.remember();
        let mut back = Minimal::new(&doc, &HashSet::new(), false);
        back.restore("lane-c-job".into());
        assert!(back.advanced);
        assert_eq!(back.screen_settings, m.screen_settings);
        let jobs = back.jobs(SessionId(1), 42, Default::default());
        assert_eq!(jobs.len(), 6);
        assert!(jobs[2].job.dest.ends_with("png/brand-Artboard 1@2x.png"));
        back.screen_settings.rows = vec![varos_raster::screens::Row { format: "pdf".into(), ..Default::default() }];
        back.screen_settings.pdf_single = true;
        let jobs = back.jobs(SessionId(1), 43, Default::default());
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job.plan.pages.len(), 2);
    }
    #[test]
    fn selection_single_pdf_setting_exports_each_snapshot_separately() {
        let mut editor = varos_core::Editor::new();
        for x in [0., 40.] {
            editor
                .try_execute(varos_core::EditCommand::AddShape {
                    kind: varos_core::model::ShapeKind::Rect,
                    bounds: [x, 0., x + 20., 20.],
                    parent: None,
                    fill: Some([1., 0., 0., 1.]),
                    stroke: None,
                    stroke_width: 0.,
                    opacity: 1.,
                    name: None,
                })
                .unwrap();
        }
        let selected = editor.doc.paths.iter().map(|p| p.id).collect();
        let mut m = Minimal::new(&editor.doc, &selected, true);
        m.advanced = true;
        m.screen_settings.pdf_single = true;
        m.screen_settings.rows = vec![varos_raster::screens::Row { format: "pdf".into(), ..Default::default() }];
        let jobs = m.jobs(SessionId(1), 42, Default::default());
        assert_eq!(jobs.len(), 2);
        for job in jobs {
            assert_eq!(job.job.plan.pages.len(), 1);
            let (bytes, _) = varos_pdf::export_pdf_with_options(
                &job.job.doc,
                &job.job.plan,
                &job.job.pdf_options,
                job.job.cancel.flag(),
            )
            .unwrap();
            assert!(bytes.starts_with(b"%PDF"));
            assert_eq!(job.job.doc.paths.iter().filter(|p| !p.hidden).count(), 1);
        }
    }
    #[test]
    fn advanced_worker_keeps_prefix_suffix_and_numbers_collisions() {
        let doc = varos_core::new_document::Settings::category(3).document().unwrap();
        let mut m = Minimal::new(&doc, &HashSet::new(), false);
        m.advanced = true;
        m.screen_settings.prefix = "icon-".into();
        m.screen_settings.rows[0].format = "svg".into();
        m.screen_settings.rows[0].suffix = "@2x".into();
        let dir = std::env::temp_dir().join(format!("lane-c-names-{}", varos_app::storage::checksum::new_nonce()));
        m.folder = dir.to_string_lossy().into_owned();
        for n in 1..=2 {
            let job = m.jobs(SessionId(1), n, Default::default()).remove(0);
            let expected = varos_raster::export::encode_with_svg_options(
                &job.asset,
                &job.options,
                job.job.cancel.flag(),
                &job.svg_options,
            )
            .unwrap()
            .bytes;
            let request: varos_bridge::dto::FileEffect = serde_json::from_value(serde_json::json!({
                "api":"1.2", "board":"b1", "request_id":"svg", "expected_rev":0,
                "path":"/tmp/shared.svg", "scope":"all_visible_artboards", "options":{"screens":m.screen_settings}
            }))
            .unwrap();
            let bridge = crate::export_ui::bridge_job(
                SessionId(1),
                n,
                &doc,
                &HashSet::new(),
                "/tmp/shared.svg".into(),
                &request,
                "export_svg",
            )
            .unwrap();
            assert_eq!(
                expected,
                varos_raster::export::encode_with_svg_options(
                    &bridge.asset,
                    &bridge.options,
                    bridge.job.cancel.flag(),
                    &bridge.svg_options,
                )
                .unwrap()
                .bytes
            );
            let done = crate::file_jobs::execute(
                crate::file_jobs::FileJob::Screen(Box::new(job)),
                &mut crate::file_ports::DiskStore,
            );
            let crate::file_jobs::FileDone::Exported(done) = done else { panic!("Expected export") };
            assert!(matches!(done.result, crate::file_jobs::ExportResult::Exported));
            assert_eq!(std::fs::read(&done.job.dest).unwrap(), expected);
            assert_eq!(
                done.job.dest.file_name().unwrap().to_string_lossy(),
                if n == 1 { "icon-Artboard 1@2x.svg" } else { "icon-Artboard 1@2x 2.svg" }
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod resume_tests {
    use super::*;
    #[test]
    fn restored_range_applies_to_grid_before_checklist_filtering() {
        let doc = varos_core::new_document::Settings { count: 3, ..Default::default() }.document().unwrap();
        let mut m = Minimal::new(&doc, &HashSet::new(), false);
        m.advanced = true;
        m.screen_settings.range = "2-3".into();
        m.boards.checked[1] = false;
        let jobs = m.jobs(SessionId(1), 1, Default::default());
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].asset.name, "Artboard 3");
    }
    #[test]
    fn advanced_pdf_resolves_bleed_once_and_invalid_range_cannot_export() {
        let doc = varos_core::new_document::Settings { count: 2, bleed: 3., ..Default::default() }.document().unwrap();
        let mut m = Minimal::new(&doc, &HashSet::new(), false);
        m.advanced = true;
        m.screen_settings.rows[0].format = "pdf".into();
        for bleed in [false, true] {
            m.screen_settings.include_bleed = bleed;
            let jobs = m.jobs(SessionId(1), 1, Default::default());
            let page = &jobs[0].job.plan.pages[0];
            assert_eq!(page.bleed_edges, [0.; 4]);
            let expected = doc.artboards[0].w + if bleed { 2. * doc.artboards[0].bleed } else { 0. };
            assert!((page.rect[2] - expected).abs() < 0.001);
            let request: varos_bridge::dto::FileEffect = serde_json::from_value(serde_json::json!({
                "api":"1.2", "board":"b1", "request_id":"r1", "expected_rev":0,
                "path":"/tmp/export.pdf", "scope":"all_visible_artboards", "options":{"screens":m.screen_settings}
            }))
            .unwrap();
            let bridge = crate::export_ui::bridge_job(
                SessionId(1),
                1,
                &doc,
                &HashSet::new(),
                "/tmp/export.pdf".into(),
                &request,
                "export_raster",
            )
            .unwrap();
            assert_eq!(bridge.job.plan.pages, jobs[0].job.plan.pages);
        }
        m.screen_settings.range = "999".into();
        assert!(m.jobs(SessionId(1), 1, Default::default()).is_empty());
    }
}
