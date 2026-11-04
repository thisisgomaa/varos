//! The shell's saved state: `varos_shell` in `ui.json` (version 7: the boxes, the Board's strip,
//! floating panels and floating strips, the Properties editors' section views, the maximized box
//! and the workspaces), apart from craft-ui's `docking` layout, which it never touches. Versions
//! 1 through 6 load through the migrations, each a pure function of the saved JSON. Version 7
//! turns the shell on once for a saved file that still has the old default `enabled: false` and
//! never recorded an explicit choice.
//!
//! Every panel has at most one owner: a box of the tree, the Board's strip, a floating panel or a
//! floating strip ([`VarosShell::invariants`]). A panel id is a panel the app knows or a second
//! (third, …) Properties editor, `properties@2` ([`instance_kind`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

use super::floats::{Float, FloatStrip};
use super::strip::{Group, StripRef};
use super::tree::{self, Axis, BOX_MIN, Node, PanelBox};
use super::workspace::Arrangement;
use crate::panels::properties::PropertiesView;

/// The saved layout's version.
pub(crate) const VERSION: u32 = 7;
/// Floating panels' box ids start here (the tree's start at 1), so the two never meet.
const FLOAT_IDS: u32 = 1 << 20;
/// The Properties panel: the one editor that has more than one instance.
pub(crate) const PROPERTIES: &str = "properties";

/// Preferences › Varos shell (new) and the shell's box layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VarosShell {
    /// The window's central region is the Varos shell (off: craft-ui's dock, as upstream).
    pub enabled: bool,
    /// The user has set the switch. Absent on a version-6 file, so a saved `enabled: false` that
    /// was only the old default turns on once ([`migrate_6_to_7`]). An explicit off writes this.
    #[serde(default)]
    pub enabled_chosen: bool,
    /// The layout's version ([`VERSION`]).
    pub version: u32,
    /// The boxes.
    pub tree: Node,
    /// Board tools: None docks; a position floats. Independent of the legacy toolbar.
    #[serde(default)]
    pub tools_pos: Option<[f32; 2]>,
    #[serde(default = "default_columns")]
    pub tools_columns: u8,
    /// The sole Board owns this strip; it is independent of ordinary panel boxes.
    pub strip: super::strip::Strip,
    /// Floating panels, bottom to top as first drawn.
    #[serde(default)]
    pub floats: Vec<Float>,
    /// Floating strips (a lone floating icon is a floating strip of one icon).
    #[serde(default)]
    pub float_strips: Vec<FloatStrip>,
    /// Each Properties editor's section view (order, collapsed sections), by its panel id
    /// (`properties`, `properties@2`, …). The pin is not saved.
    #[serde(default)]
    pub views: BTreeMap<String, PropertiesView>,
    /// The box that fills the central region (the rest of the tree is kept as it is, hidden).
    #[serde(default)]
    pub maximized: Option<u32>,
    /// The workspace shown (a preset or one of [`Self::workspaces`]).
    #[serde(default = "default_workspace")]
    pub workspace: String,
    /// The user's own workspaces, as saved.
    #[serde(default)]
    pub workspaces: Vec<Arrangement>,
    /// The layouts the workspaces were left in (the changes made in them since they were saved).
    #[serde(default)]
    pub workspace_changes: Vec<Arrangement>,
    /// The panel whose flyout is open (beside the strip that owns it); not saved.
    #[serde(skip)]
    pub open: Option<String>,
    /// The drag under way; not saved.
    #[serde(skip)]
    pub drag: Option<super::drag::Drag>,
    /// Columns collapsed into the Board's strip by dragging a divider past their minimum width,
    /// newest last. Expanding the strip past ~120 pt restores the last one.
    #[serde(default)]
    pub collapsed_columns: Vec<CollapsedColumn>,
    /// Where a panel was when it last left a strip, a pill or a box. Not saved: a launch has no
    /// drag to undo. Restored by closing a float, collapsing to an icon, or returning a pill.
    #[serde(skip)]
    pub(crate) homes: BTreeMap<String, PanelHome>,
}

/// The place a panel left, so closing a float, collapsing to an icon or returning a pill puts it
/// back there instead of at the end of the Board strip.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PanelHome {
    /// A strip (the Board's, or a floating pill) and the index in that group.
    Strip {
        strip: StripRef,
        index: usize,
        /// The group's other panels, so the group can be found after indexes shift.
        peers: Vec<String>,
        /// A panel of the previous group, if any.
        after_group: Option<String>,
        /// A panel of the next group, if any.
        before_group: Option<String>,
        /// A floating pill's top-left, so a removed pill can be rebuilt.
        pos: Option<[f32; 2]>,
    },
    /// A tab of a docked or floating box.
    Tabs { box_id: u32, index: usize },
}

impl PanelHome {
    fn index(&self) -> usize {
        match self {
            PanelHome::Strip { index, .. } | PanelHome::Tabs { index, .. } => *index,
        }
    }
}

/// Where a strip panel goes back: the arguments of [`VarosShell::place_in_strip`] as one value.
struct StripSlot<'a> {
    strip: StripRef,
    index: usize,
    peers: &'a [String],
    after_group: Option<&'a str>,
    before_group: Option<&'a str>,
    pos: Option<[f32; 2]>,
}

fn strip_home(groups: &[Group], strip: StripRef, group: usize, panel: &str, pos: Option<[f32; 2]>) -> Option<PanelHome> {
    let g = groups.get(group)?;
    let index = g.panels.iter().position(|p| p == panel).unwrap_or(0);
    let peers = g.panels.iter().filter(|p| p.as_str() != panel).cloned().collect();
    let after_group = group.checked_sub(1).and_then(|i| groups.get(i)).and_then(|g| g.panels.first().cloned());
    let before_group = groups.get(group + 1).and_then(|g| g.panels.first().cloned());
    Some(PanelHome::Strip { strip, index, peers, after_group, before_group, pos })
}

/// A dock column that was collapsed into the strip, so dragging the strip back open can rebuild it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CollapsedColumn {
    /// Each inner list is one box's tabs, top to bottom.
    pub boxes: Vec<Vec<String>>,
    /// The column sat to the right of its sibling (the Board, in the usual layout).
    pub right: bool,
}

pub(crate) fn default_columns() -> u8 {
    1
}

fn default_workspace() -> String {
    super::workspace::DESIGN.to_string()
}

impl Default for VarosShell {
    fn default() -> Self {
        Self {
            enabled: true,
            enabled_chosen: false,
            version: VERSION,
            strip: super::strip::Strip::default_for(&Node::default_layout()),
            tree: Node::default_layout(),
            tools_pos: None,
            tools_columns: 1,
            floats: Vec::new(),
            float_strips: Vec::new(),
            views: BTreeMap::new(),
            maximized: None,
            workspace: default_workspace(),
            workspaces: Vec::new(),
            workspace_changes: Vec::new(),
            open: None,
            drag: None,
            collapsed_columns: Vec::new(),
            homes: BTreeMap::new(),
        }
    }
}

/// The panel kind of panel id `id`: `properties` for `properties@2`, else the id itself.
pub(crate) fn instance_kind(id: &str) -> &str {
    if is_extra_instance(id) { id.split_once('@').map_or(id, |(kind, _)| kind) } else { id }
}

/// Is `id` a second (third, …) Properties editor, `properties@<n>` with `n` ≥ 2.
pub(crate) fn is_extra_instance(id: &str) -> bool {
    id.split_once('@')
        .is_some_and(|(kind, n)| matches!(kind, PROPERTIES | "control") && n.parse::<u32>().is_ok_and(|n| n >= 2) && !n.starts_with('0'))
}

/// Is `id` a Properties editor (the panel or another instance of it).
pub(crate) fn is_properties(id: &str) -> bool {
    instance_kind(id) == PROPERTIES
}

/// A panel id the app knows (an icon panel, a dock tab or a library panel), or another Properties
/// editor.
pub(crate) fn known(id: &str) -> bool {
    id == "control" || crate::menus::normalize_panel(id) == Some(id) || is_extra_instance(id)
}

/// Version 1 (stage 1: the boxes only) → 2: the Board's strip holds the panels no box has.
pub(crate) fn migrate_1_to_2(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.insert("version".into(), json!(2));
        if !o.contains_key("strip") {
            let tree = o.get("tree").and_then(|t| Node::deserialize(t).ok()).map_or_else(Node::default_layout, |t| t.sanitized(&known));
            o.insert("strip".into(), serde_json::to_value(super::strip::Strip::legacy_default_for(&tree)).unwrap_or(Value::Null));
        }
    }
    v
}

/// Version 2 (stage 2: boxes and the Board's strip) → 3: no floating panels nor floating strips
/// yet (whatever a version-2 file says of them is not stage 2's).
pub(crate) fn migrate_2_to_3(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.insert("version".into(), json!(3));
        o.insert("floats".into(), json!([]));
        o.insert("float_strips".into(), json!([]));
    }
    v
}

/// Version 3 (stage 3: floats, floating strips, collapsed boxes) → 4: default section views,
/// nothing maximized, the Design workspace and no saved workspaces.
pub(crate) fn migrate_3_to_4(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.insert("version".into(), json!(4));
        o.insert("views".into(), json!({}));
        o.insert("maximized".into(), Value::Null);
        o.insert("workspace".into(), json!(super::workspace::DESIGN));
        o.insert("workspaces".into(), json!([]));
        o.insert("workspace_changes".into(), json!([]));
    }
    v
}

/// Version 4 → 5: tools begin docked on the Board, never at the legacy toolbar position.
pub(crate) fn migrate_4_to_5(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.insert("version".into(), json!(5));
        o.insert("tools_pos".into(), Value::Null);
    }
    v
}

/// Version 6 → 7: a saved `enabled: false` with no explicit choice was the old default (the key
/// was always written). Turn the shell on once. A file that already records `enabled_chosen`
/// keeps the choice, so a later explicit off stays off.
pub(crate) fn migrate_6_to_7(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        let chosen = o.get("enabled_chosen").and_then(Value::as_bool).unwrap_or(false);
        let enabled = o.get("enabled").and_then(Value::as_bool).unwrap_or(true);
        if !enabled && !chosen {
            o.insert("enabled".into(), json!(true));
        }
        o.insert("version".into(), json!(7));
    }
    v
}

/// Version 5 → 6 preserves every owner and adds the per-workspace tools grid.
pub(crate) fn migrate_5_to_6(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        o.insert("version".into(), json!(6));
        o.entry("tools_columns").or_insert(json!(1));
        for key in ["workspaces", "workspace_changes"] {
            if let Some(items) = o.get_mut(key).and_then(Value::as_array_mut) {
                for item in items {
                    if let Some(a) = item.as_object_mut() {
                        a.entry("tools_columns").or_insert(json!(1));
                    }
                }
            }
        }
    }
    v
}

/// `varos_shell` read from what `ui.json` holds: the switch kept whatever the layout; an older
/// version migrated step by step; a layout of an unknown version, or one that doesn't read, is the
/// default; unknown panel ids are dropped; a tree with no Board is the default layout.
pub(crate) fn from_value(v: &Value) -> VarosShell {
    let version = v.get("version").and_then(Value::as_u64);
    let mut v = v.clone();
    if version == Some(1) {
        v = migrate_1_to_2(v);
    }
    if matches!(version, Some(1 | 2)) {
        v = migrate_2_to_3(v);
    }
    if matches!(version, Some(1..=3)) {
        v = migrate_3_to_4(v);
    }
    if matches!(version, Some(1..=4)) {
        v = migrate_4_to_5(v);
    }
    if matches!(version, Some(1..=5)) {
        v = migrate_5_to_6(v);
    }
    if matches!(version, Some(1..=6)) {
        v = migrate_6_to_7(v);
    }
    // Absent means on: fresh configs and older ui.json files that never wrote the key. Read after
    // the migration so a version-6 `enabled: false` without a choice has already been turned on.
    let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let enabled_chosen = v.get("enabled_chosen").and_then(Value::as_bool).unwrap_or(false);
    let readable = matches!(version, Some(1..=7));
    let tree =
        readable.then(|| v.get("tree").and_then(|t| Node::deserialize(t).ok())).flatten().map_or_else(Node::default_layout, |t| t.sanitized(&known));
    let strip =
        v.get("strip").and_then(|s| super::strip::Strip::deserialize(s).ok()).unwrap_or_else(|| super::strip::Strip::legacy_default_for(&tree));
    // Each float, floating strip, view and workspace reads alone: one that doesn't read is dropped,
    // not the rest.
    let list = |key: &str| if readable { v.get(key).and_then(Value::as_array).cloned().unwrap_or_default() } else { Vec::new() };
    let floats = list("floats").into_iter().filter_map(|f| Float::deserialize(f).ok()).collect();
    let float_strips = list("float_strips").into_iter().filter_map(|s| FloatStrip::deserialize(s).ok()).collect();
    let views = v
        .get("views")
        .and_then(Value::as_object)
        .filter(|_| readable)
        .map(|m| m.iter().filter_map(|(k, view)| Some((k.clone(), PropertiesView::deserialize(view).ok()?))).collect())
        .unwrap_or_default();
    let maximized = v.get("maximized").and_then(Value::as_u64).and_then(|m| u32::try_from(m).ok());
    let workspace = v.get("workspace").and_then(Value::as_str).filter(|w| !w.trim().is_empty()).map_or_else(default_workspace, str::to_owned);
    let arrangements =
        |key: &str| list(key).into_iter().filter_map(|a| Arrangement::deserialize(a).ok()).map(Arrangement::sanitized).collect::<Vec<_>>();
    let (workspaces, workspace_changes) = (arrangements("workspaces"), arrangements("workspace_changes"));
    let mut shell = VarosShell {
        enabled,
        enabled_chosen,
        version: VERSION,
        tree,
        tools_columns: v.get("tools_columns").and_then(Value::as_u64).unwrap_or(1).clamp(1, 3) as u8,
        tools_pos: v
            .get("tools_pos")
            .filter(|_| readable)
            .and_then(|p| serde_json::from_value::<[f32; 2]>(p.clone()).ok())
            .filter(|p| p.iter().all(|v| v.is_finite())),
        strip,
        floats,
        float_strips,
        views,
        maximized,
        workspace,
        workspaces,
        workspace_changes,
        open: None,
        drag: None,
        collapsed_columns: v.get("collapsed_columns").and_then(|c| Vec::<CollapsedColumn>::deserialize(c).ok()).unwrap_or_default(),
        homes: BTreeMap::new(),
    };
    // Stage-2 state keeps its own strip arrangement rules (see `Strip::sanitize_owned`).
    shell.sanitize(matches!(version, Some(1 | 2)) || !readable);
    shell.sanitize_workspaces();
    shell
}

/// Defaults for a historical preferences file with no shell key.
pub(crate) fn historical_default() -> VarosShell {
    let mut shell = VarosShell::default();
    shell.strip.remove("ai");
    shell
}

/// Serde's reader for `UiState::varos_shell`: never fails the whole UI state.
pub(crate) fn lenient<'de, D: Deserializer<'de>>(d: D) -> Result<VarosShell, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(from_value(&v))
}

impl VarosShell {
    /// Atomic ownership transfer from strip to box.
    #[cfg(test)]
    pub fn add_tab(&mut self, target: u32, panel: &str) -> bool {
        if !self.tree.add_tab(target, panel) {
            return false;
        }
        self.strip.remove(panel);
        if self.open.as_deref() == Some(panel) {
            self.open = None;
        }
        true
    }
    /// Close `panel`'s tab (in a box or a floating panel): it goes back to the Board's strip; a
    /// second Properties editor goes for good (its view with it).
    pub fn close_tab(&mut self, panel: &str) -> bool {
        if !self.tree.remove_tab(panel) && !self.take_float_tab(panel) {
            return false;
        }
        if is_extra_instance(panel) {
            self.views.remove(panel);
        } else {
            self.strip.append(panel);
        }
        true
    }

    /// Does some owner (a box, a strip, a floating panel or strip) hold `panel`.
    pub(crate) fn owns(&self, panel: &str) -> bool {
        self.tree.find_tab(panel).is_some() || self.float_of(panel).is_some() || self.in_a_strip(panel)
    }

    /// An id for a new Properties editor: `properties@<n>`, the smallest `n` ≥ 2 no owner has.
    pub(crate) fn fresh_properties_id(&self) -> String {
        (2..=u32::MAX).map(|n| format!("{PROPERTIES}@{n}")).find(|id| !self.owns(id)).unwrap_or_else(|| format!("{PROPERTIES}@{}", u32::MAX))
    }

    /// The box that fills the central region, while it exists.
    pub(crate) fn fresh_instance_id(&self, kind: &str) -> String {
        if kind == PROPERTIES {
            return self.fresh_properties_id();
        }
        (2..=u32::MAX).map(|n| format!("{kind}@{n}")).find(|id| !self.owns(id)).unwrap_or_else(|| format!("{kind}@{}", u32::MAX))
    }

    pub(crate) fn maximized_box(&self) -> Option<u32> {
        self.maximized.filter(|id| self.tree.find_box(*id).is_some())
    }

    /// Box `id` fills the central region, or (maximized already) the layout comes back as it was:
    /// the tree itself never changes. False when the tree has no box `id`.
    pub(crate) fn toggle_maximized(&mut self, id: u32) -> bool {
        if self.maximized_box() == Some(id) {
            self.maximized = None;
            return true;
        }
        if self.tree.find_box(id).is_none() {
            return false;
        }
        self.maximized = Some(id);
        true
    }

    /// Drop tab homes that name floating panel `id`. Float ids are reused, so a home left
    /// behind would insert into whichever float is given that id next.
    fn forget_float_tabs(&mut self, id: u32) {
        self.homes.retain(|_, home| !matches!(home, PanelHome::Tabs { box_id, .. } if *box_id == id));
    }

    /// Take `panel` out of whichever floating panel holds it (the float goes with its last tab).
    fn take_float_tab(&mut self, panel: &str) -> bool {
        let Some(pos) = self.floats.iter().position(|f| f.panels.tabs.iter().any(|t| t == panel)) else { return false };
        let id = self.floats[pos].panels.id;
        let b = &mut self.floats[pos].panels;
        if let Some(i) = b.tabs.iter().position(|t| t == panel) {
            b.tabs.remove(i);
            if b.active > i || b.active >= b.tabs.len() {
                b.active = b.active.saturating_sub(1);
            }
        }
        if self.floats[pos].panels.tabs.is_empty() {
            self.floats.remove(pos);
            self.forget_float_tabs(id);
        }
        true
    }

    /// Take `panel` from its owner, whoever it is: a box (its box goes with its last tab), the
    /// Board's strip, a floating panel or a floating strip (each goes when left empty). The place
    /// it leaves is remembered, so a later close puts it back there.
    pub fn take(&mut self, panel: &str) {
        self.remember_home(panel);
        self.tree.remove_tab(panel);
        self.strip.remove(panel);
        self.take_float_tab(panel);
        for s in &mut self.float_strips {
            for g in &mut s.groups {
                g.panels.retain(|p| p != panel);
            }
            s.groups.retain(|g| !g.panels.is_empty());
        }
        self.float_strips.retain(|s| !s.groups.is_empty());
    }

    /// Record where `panel` sits, overwriting an older record. Called before it is removed.
    fn remember_home(&mut self, panel: &str) {
        let Some(home) = self.locate(panel) else { return };
        self.homes.insert(panel.to_string(), home);
    }

    fn locate(&self, panel: &str) -> Option<PanelHome> {
        if let Some(group) = self.strip.group(panel) {
            return strip_home(&self.strip.groups, StripRef::Board, group, panel, None);
        }
        if let Some(id) = self.float_strip_of(panel) {
            let s = self.float_strips.iter().find(|s| s.id == id)?;
            let group = s.groups.iter().position(|g| g.panels.iter().any(|p| p == panel))?;
            return strip_home(&s.groups, StripRef::Float(id), group, panel, Some(s.pos));
        }
        if let Some(id) = self.tree.find_tab(panel) {
            let index = self.tree.find_box(id)?.tabs.iter().position(|t| t == panel)?;
            return Some(PanelHome::Tabs { box_id: id, index });
        }
        if let Some(id) = self.float_of(panel) {
            let f = self.floats.iter().find(|f| f.panels.id == id)?;
            let index = f.panels.tabs.iter().position(|t| t == panel)?;
            return Some(PanelHome::Tabs { box_id: id, index });
        }
        None
    }

    /// Put `panels` back where each one left, lowest index first so a shared group rebuilds in order.
    fn place_all(&mut self, mut panels: Vec<String>) {
        panels.sort_by_key(|p| self.homes.get(p).map(PanelHome::index).unwrap_or(usize::MAX));
        for panel in panels {
            self.place_back(&panel);
        }
    }

    /// Put `panel` back at its recorded origin. With none, it joins the Board strip.
    fn place_back(&mut self, panel: &str) {
        let Some(home) = self.homes.remove(panel) else {
            self.strip.append(panel);
            return;
        };
        match home {
            PanelHome::Tabs { box_id, index } => {
                let before = self.tab_at(box_id, index).filter(|name| name != panel);
                let lives = self.tree.find_box(box_id).is_some() || self.floats.iter().any(|f| f.panels.id == box_id);
                if !lives || !self.insert_tabs(box_id, before.as_deref(), &[panel.to_string()]) {
                    self.strip.append(panel);
                }
            }
            PanelHome::Strip { strip, index, peers, after_group, before_group, pos } => {
                let slot = StripSlot { strip, index, peers: &peers, after_group: after_group.as_deref(), before_group: before_group.as_deref(), pos };
                if !self.place_in_strip(slot, panel) {
                    self.strip.append(panel);
                }
            }
        }
    }

    fn tab_at(&self, box_id: u32, index: usize) -> Option<String> {
        let tabs = if let Some(b) = self.tree.find_box(box_id) { &b.tabs } else { &self.floats.iter().find(|f| f.panels.id == box_id)?.panels.tabs };
        tabs.get(index).cloned()
    }

    fn place_in_strip(&mut self, slot: StripSlot<'_>, panel: &str) -> bool {
        let StripSlot { strip, index, peers, after_group, before_group, pos } = slot;
        if self.strip_groups(strip).is_none() {
            let StripRef::Float(id) = strip else { return false };
            let pos = pos.unwrap_or([80.0, 80.0]);
            self.float_strips.push(super::floats::FloatStrip {
                id,
                pos,
                groups: vec![Group {
                    temporary: !super::strip::Strip::is_default(panel),
                    panels: vec![panel.to_string()],
                    width: 240.0,
                    height: 360.0,
                }],
            });
            return true;
        }
        if let Some(groups) = self.strip_groups_mut(strip)
            && let Some(g) = groups.iter_mut().find(|g| g.panels.iter().any(|p| peers.iter().any(|q| q == p)))
        {
            let at = index.min(g.panels.len());
            g.panels.insert(at, panel.to_string());
            return true;
        }
        let Some(groups) = self.strip_groups_mut(strip) else { return false };
        let at = before_group
            .and_then(|b| groups.iter().position(|g| g.panels.iter().any(|p| p == b)))
            .or_else(|| after_group.and_then(|a| groups.iter().position(|g| g.panels.iter().any(|p| p == a)).map(|i| i + 1)))
            .unwrap_or(groups.len());
        groups.insert(
            at.min(groups.len()),
            Group { temporary: !super::strip::Strip::is_default(panel), panels: vec![panel.to_string()], width: 240.0, height: 360.0 },
        );
        true
    }

    /// The floating panel holding `panel`.
    pub(crate) fn float_of(&self, panel: &str) -> Option<u32> {
        self.floats.iter().find(|f| f.panels.tabs.iter().any(|t| t == panel)).map(|f| f.panels.id)
    }

    pub(crate) fn float_mut(&mut self, id: u32) -> Option<&mut Float> {
        self.floats.iter_mut().find(|f| f.panels.id == id)
    }

    /// The floating strip holding `panel`.
    pub(crate) fn float_strip_of(&self, panel: &str) -> Option<u32> {
        self.float_strips.iter().find(|s| s.groups.iter().any(|g| g.panels.iter().any(|p| p == panel))).map(|s| s.id)
    }

    /// Strip `sref`'s groups (none for a floating strip that is gone).
    pub(crate) fn strip_groups(&self, sref: StripRef) -> Option<&Vec<Group>> {
        match sref {
            StripRef::Board => Some(&self.strip.groups),
            StripRef::Float(id) => self.float_strips.iter().find(|s| s.id == id).map(|s| &s.groups),
        }
    }

    pub(crate) fn strip_groups_mut(&mut self, sref: StripRef) -> Option<&mut Vec<Group>> {
        match sref {
            StripRef::Board => Some(&mut self.strip.groups),
            StripRef::Float(id) => self.float_strips.iter_mut().find(|s| s.id == id).map(|s| &mut s.groups),
        }
    }

    /// Is `panel` an icon in some strip (the Board's or a floating one).
    pub(crate) fn in_a_strip(&self, panel: &str) -> bool {
        self.strip.group(panel).is_some() || self.float_strip_of(panel).is_some()
    }

    /// Put tab `index` of box `id` (of the tree or a floating panel) in front.
    pub(crate) fn set_active(&mut self, id: u32, index: usize) {
        if let Some(f) = self.float_mut(id) {
            if index < f.panels.tabs.len() {
                f.panels.active = index;
            }
        } else {
            self.tree.set_active(id, index);
        }
    }

    /// Bring `panel` to the front of its floating panel, shown whole; false when no float holds it.
    pub(crate) fn activate_float_tab(&mut self, panel: &str) -> bool {
        let Some(f) = self.floats.iter_mut().find(|f| f.panels.tabs.iter().any(|t| t == panel)) else { return false };
        f.panels.active = f.panels.tabs.iter().position(|t| t == panel).unwrap_or(0);
        f.panels.collapsed = false;
        true
    }

    /// Collapse box `id` (of the tree or a floating panel) to its tab row, or show it whole again.
    pub(crate) fn toggle_collapsed(&mut self, id: u32) -> bool {
        match self.float_mut(id) {
            Some(f) => {
                f.panels.collapsed = !f.panels.collapsed;
                true
            }
            None => self.tree.toggle_collapsed(id),
        }
    }

    /// Insert `panels` into box `id`'s tabs (of the tree or a floating panel) before tab `before`
    /// (at the end: `None`), the first in front. False when the box doesn't take them.
    pub(crate) fn insert_tabs(&mut self, id: u32, before: Option<&str>, panels: &[String]) -> bool {
        if let Some(f) = self.float_mut(id) {
            let tabs = &mut f.panels.tabs;
            let at = before.and_then(|b| tabs.iter().position(|t| t == b)).unwrap_or(tabs.len());
            for (k, p) in panels.iter().enumerate() {
                tabs.insert(at + k, p.clone());
            }
            f.panels.active = at;
            return true;
        }
        let Some(b) = self.tree.find_box(id) else { return false };
        let at = before.and_then(|x| b.tabs.iter().position(|t| t == x)).unwrap_or(b.tabs.len());
        for (k, p) in panels.iter().enumerate() {
            if !self.tree.insert_tab(id, at + k, p) {
                return false;
            }
        }
        self.tree.set_active(id, at);
        true
    }

    /// Floating panel `id`'s `×`: each panel goes
    /// back where it left (a strip group, a pill, or the tab it was dragged out of).
    pub(crate) fn close_float(&mut self, id: u32) {
        let Some(i) = self.floats.iter().position(|f| f.panels.id == id) else { return };
        let panels = self.floats.remove(i).panels.tabs;
        // The id is reused by the next float. A tab home that still names it would restore
        // into that new float.
        self.forget_float_tabs(id);
        self.place_all(panels);
        self.settle();
    }

    /// A panel menu's Collapse to icon: floats collapse in place. Docked panels that left a strip
    /// or pill go back there; remaining docked tabs become one icon group. A docked box
    /// disappears (its sibling takes the room).
    pub(crate) fn collapse_box_to_strip(&mut self, id: u32) -> bool {
        if self.floats.iter().any(|f| f.panels.id == id) {
            self.float_to_icon(id);
            return true;
        }
        let Some(b) = self.tree.find_box(id).cloned() else { return false };
        if b.is_board() || b.tabs.is_empty() {
            return false;
        }
        let tabs = b.tabs.clone();
        for panel in &tabs {
            self.tree.remove_tab(panel);
        }
        // A tab dragged in from another box, or reordered here, has a tab home. Restoring
        // those sends them back. Only a strip or pill home is put back; the rest of the box,
        // in its current order, becomes one icon group.
        let (restore, fresh): (Vec<String>, Vec<String>) = tabs.into_iter().partition(|p| matches!(self.homes.get(p), Some(PanelHome::Strip { .. })));
        self.place_all(restore);
        if !fresh.is_empty() {
            self.strip.groups.push(Group {
                temporary: fresh.iter().all(|p| !super::strip::Strip::is_default(p)),
                panels: fresh,
                width: 260.0,
                height: 360.0,
            });
        }
        self.settle();
        true
    }

    /// The divider was released past this side's minimum: the whole column becomes icons in the
    /// Board's strip, remembered so the strip's edge can open it again.
    pub(crate) fn collapse_side(&mut self, path: &[u8], take_a: bool) -> bool {
        let Some(node) = self.tree.split_at(path) else { return false };
        let Node::Split { axis, a, b, .. } = node else { return false };
        if *axis != Axis::Horizontal {
            return false;
        }
        let side = if take_a { a.as_ref() } else { b.as_ref() };
        if side.boxes().iter().any(|bx| bx.is_board()) {
            return false;
        }
        let groups = side.panel_groups();
        if groups.is_empty() || self.tree.detach_child(path, take_a).is_none() {
            return false;
        }
        for tabs in &groups {
            self.strip.groups.push(Group {
                temporary: tabs.iter().all(|p| !super::strip::Strip::is_default(p)),
                panels: tabs.clone(),
                width: tabs.iter().map(|p| tree::panel_min_width(p)).fold(240.0, f32::max),
                height: 360.0,
            });
        }
        self.collapsed_columns.push(CollapsedColumn { boxes: groups, right: !take_a });
        self.settle();
        true
    }

    /// The last collapsed column opens again beside the Board.
    pub(crate) fn restore_collapsed(&mut self) -> bool {
        let Some(col) = self.collapsed_columns.pop() else { return false };
        let boxes: Vec<Vec<String>> = col
            .boxes
            .into_iter()
            .map(|tabs| tabs.into_iter().filter(|p| self.strip.group(p).is_some()).collect())
            .filter(|tabs: &Vec<String>| !tabs.is_empty())
            .collect();
        if boxes.is_empty() {
            return false;
        }
        for tabs in &boxes {
            for panel in tabs {
                self.strip.remove(panel);
            }
        }
        let Some(column) = column_node(self, &boxes) else { return false };
        let restored = self.tree.attach_beside_board(column, col.right, 0.72);
        self.settle();
        restored
    }

    /// Collapse a floating group in place, preserving its expanded dimensions.
    pub(crate) fn float_to_icon(&mut self, id: u32) {
        let Some(i) = self.floats.iter().position(|f| f.panels.id == id) else { return };
        let f = self.floats.remove(i);
        self.forget_float_tabs(id);
        let strip_id = self.fresh_strip_id();
        self.float_strips.push(FloatStrip {
            id: strip_id,
            pos: [f.rect[0], f.rect[1]],
            groups: vec![Group { temporary: false, panels: f.panels.tabs, width: f.rect[2], height: f.rect[3] }],
        });
        self.settle();
    }

    /// Dock a floating group at the rail, independently of its previous tab location.
    pub(crate) fn dock_float(&mut self, id: u32) {
        let Some(i) = self.floats.iter().position(|f| f.panels.id == id) else { return };
        let f = self.floats.remove(i);
        self.forget_float_tabs(id);
        self.strip.groups.push(Group { panels: f.panels.tabs, temporary: false, width: f.rect[2], height: f.rect[3] });
        self.settle();
    }

    /// A pill's Collapse to icon: each panel goes back where it left. A home that names this pill
    /// is dropped first, so closing the pill does not rebuild it.
    pub(crate) fn collapse_float_strip(&mut self, id: u32) {
        let Some(i) = self.float_strips.iter().position(|s| s.id == id) else { return };
        let panels: Vec<String> = self.float_strips[i].groups.iter().flat_map(|g| g.panels.clone()).collect();
        self.float_strips.remove(i);
        for panel in &panels {
            if matches!(self.homes.get(panel), Some(PanelHome::Strip { strip: StripRef::Float(fid), .. }) if *fid == id) {
                self.homes.remove(panel);
            }
        }
        self.place_all(panels);
        self.settle();
    }

    /// A pill's Close: the panels leave the layout. They are not docked to the Board strip.
    pub(crate) fn dismiss_float_strip(&mut self, id: u32) {
        let Some(i) = self.float_strips.iter().position(|s| s.id == id) else { return };
        let panels: Vec<String> = self.float_strips[i].groups.iter().flat_map(|g| g.panels.clone()).collect();
        self.float_strips.remove(i);
        for panel in &panels {
            self.homes.remove(panel);
            if self.open.as_deref() == Some(panel) {
                self.open = None;
            }
        }
        self.settle();
    }

    /// Floating strip `id`'s `>>`: each of its groups becomes a floating panel (the group's
    /// remembered size, its open panel in front), stacked down from where the strip was.
    pub(crate) fn expand_float_strip(&mut self, id: u32) {
        let Some(i) = self.float_strips.iter().position(|s| s.id == id) else { return };
        let s = self.float_strips.remove(i);
        for (k, g) in s.groups.into_iter().enumerate() {
            let active = g.panels.iter().position(|p| self.open.as_deref() == Some(p)).unwrap_or(0);
            let id = self.fresh_float_id();
            let panels = PanelBox { id, tabs: g.panels, active, collapsed: false };
            let offset = 24.0 * k as f32;
            let size = egui::vec2(g.width, g.height).max(BOX_MIN);
            let mut f = Float { panels, rect: [0.0; 4], owner_area: 0 };
            f.set_rect(egui::Rect::from_min_size(egui::pos2(s.pos[0] + offset, s.pos[1] + offset), size));
            self.floats.push(f);
        }
    }

    /// A box id for a new floating panel: no box of the tree nor float has it.
    pub(crate) fn fresh_float_id(&self) -> u32 {
        (FLOAT_IDS..=u32::MAX).find(|i| self.tree.find_box(*i).is_none() && !self.floats.iter().any(|f| f.panels.id == *i)).unwrap_or(u32::MAX)
    }

    /// An id for a new floating strip.
    pub(crate) fn fresh_strip_id(&self) -> u32 {
        (1..=u32::MAX).find(|i| !self.float_strips.iter().any(|s| s.id == *i)).unwrap_or(u32::MAX)
    }

    /// After a change: a flyout only for a strip's panel; floats' ids apart from the tree's.
    pub(crate) fn settle(&mut self) {
        if let Some(open) = self.open.clone()
            && !self.in_a_strip(&open)
        {
            self.open = None;
        }
        for i in 0..self.floats.len() {
            let id = self.floats[i].panels.id;
            let clash = self.tree.find_box(id).is_some() || self.floats[..i].iter().any(|f| f.panels.id == id) || id == 0;
            if clash {
                let fresh = self.fresh_float_id();
                self.floats[i].panels.id = fresh;
            }
        }
    }

    /// Made sound after loading: the tree first, then floating panels, floating strips and the
    /// Board's strip keep only known panels no earlier owner has; empty ones go; sizes and
    /// positions finite; ids unique. `legacy`: state of an older version (see
    /// [`super::strip::Strip::sanitize_owned`]).
    pub(crate) fn sanitize(&mut self, legacy: bool) {
        let mut seen: Vec<String> = self.tree.boxes().iter().flat_map(|b| b.tabs.clone()).collect();
        let mut keep = |p: &String| {
            known(p) && !seen.contains(p) && {
                seen.push(p.clone());
                true
            }
        };
        for f in &mut self.floats {
            let front = f.panels.front().map(str::to_owned);
            f.panels.tabs.retain(&mut keep);
            f.panels.active = front.and_then(|x| f.panels.tabs.iter().position(|t| *t == x)).unwrap_or(0);
            let r = f.rect();
            let ok = |v: f32, fallback: f32| if v.is_finite() { v } else { fallback };
            let min_w = f.panels.tabs.iter().map(|t| tree::panel_min_width(t)).fold(180.0, f32::max);
            let size = egui::vec2(ok(r.width(), super::floats::DEFAULT_SIZE.x), ok(r.height(), super::floats::DEFAULT_SIZE.y))
                .clamp(egui::vec2(min_w, BOX_MIN.y), egui::vec2(4000.0, 4000.0));
            f.set_rect(egui::Rect::from_min_size(egui::pos2(ok(r.left(), 0.0), ok(r.top(), 0.0)), size));
        }
        let gone: Vec<u32> = self.floats.iter().filter(|f| f.panels.tabs.is_empty()).map(|f| f.panels.id).collect();
        self.floats.retain(|f| !f.panels.tabs.is_empty());
        for id in gone {
            self.forget_float_tabs(id);
        }
        for s in &mut self.float_strips {
            for g in &mut s.groups {
                g.panels.retain(&mut keep);
                g.width = if g.width.is_finite() { g.width.clamp(190.0, 1200.0) } else { 260.0 };
                g.height = if g.height.is_finite() { g.height.clamp(120.0, 1600.0) } else { 360.0 };
            }
            s.groups.retain(|g| !g.panels.is_empty());
            s.pos = s.pos.map(|v| if v.is_finite() { v } else { 0.0 });
        }
        self.float_strips.retain(|s| !s.groups.is_empty());
        let mut ids = Vec::new();
        for s in &mut self.float_strips {
            if s.id == 0 || ids.contains(&s.id) {
                s.id = (1..=u32::MAX).find(|i| !ids.contains(i)).unwrap_or(u32::MAX);
            }
            ids.push(s.id);
        }
        // Box ids: a float's never a tree box's, nor another float's.
        let mut used: Vec<u32> = self.tree.boxes().iter().map(|b| b.id).collect();
        for f in &mut self.floats {
            if f.panels.id < FLOAT_IDS || used.contains(&f.panels.id) {
                f.panels.id = (FLOAT_IDS..=u32::MAX).find(|i| !used.contains(i)).unwrap_or(u32::MAX);
            }
            used.push(f.panels.id);
        }
        let elsewhere: Vec<String> = self
            .floats
            .iter()
            .flat_map(|f| f.panels.tabs.clone())
            .chain(self.float_strips.iter().flat_map(|s| s.groups.iter().flat_map(|g| g.panels.clone())))
            .collect();
        self.strip.sanitize_owned(&self.tree, &|p| elsewhere.iter().any(|e| e == p), legacy);
        self.collapsed_columns = std::mem::take(&mut self.collapsed_columns)
            .into_iter()
            .filter_map(|col| {
                let boxes: Vec<Vec<String>> = col
                    .boxes
                    .into_iter()
                    .map(|tabs| tabs.into_iter().filter(|p| self.strip.group(p).is_some()).collect())
                    .filter(|tabs: &Vec<String>| !tabs.is_empty())
                    .collect();
                if boxes.is_empty() { None } else { Some(CollapsedColumn { boxes, right: col.right }) }
            })
            .collect();
        // Views only for the Properties editors that are somewhere; the maximized box only while
        // the tree has it.
        let views = std::mem::take(&mut self.views);
        self.views = views.into_iter().filter(|(id, _)| is_properties(id) && self.owns(id)).map(|(id, v)| (id, v.sanitized())).collect();
        self.maximized = self.maximized_box();
    }

    /// What must always hold: the tree's invariants; each panel in one place among the boxes, the
    /// strips and the floats; no empty floating panel, strip or group; box ids unique across the
    /// tree and the floats; a flyout only for a strip's panel.
    pub(crate) fn invariants(&self) -> Result<(), String> {
        self.tree.invariants()?;
        let mut owners: Vec<&str> = self.tree.boxes().iter().flat_map(|b| b.tabs.iter().map(String::as_str)).collect();
        owners.extend(self.strip.groups.iter().flat_map(|g| g.panels.iter().map(String::as_str)));
        owners.extend(self.floats.iter().flat_map(|f| f.panels.tabs.iter().map(String::as_str)));
        owners.extend(self.float_strips.iter().flat_map(|s| s.groups.iter().flat_map(|g| g.panels.iter().map(String::as_str))));
        let n = owners.len();
        owners.sort_unstable();
        owners.dedup();
        if owners.len() != n {
            return Err("a panel has two owners".into());
        }
        if self.strip.groups.iter().chain(self.float_strips.iter().flat_map(|s| &s.groups)).any(|g| g.panels.is_empty()) {
            return Err("an empty strip group".into());
        }
        if self.float_strips.iter().any(|s| s.groups.is_empty()) {
            return Err("an empty floating strip".into());
        }
        if let Some(f) = self.floats.iter().find(|f| f.panels.tabs.is_empty() || f.panels.active >= f.panels.tabs.len()) {
            return Err(format!("floating panel {} is empty or fronts no tab", f.panels.id));
        }
        let mut ids: Vec<u32> = self.tree.boxes().iter().map(|b| b.id).chain(self.floats.iter().map(|f| f.panels.id)).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != n {
            return Err(format!("box ids {ids:?} repeat"));
        }
        let mut strips: Vec<u32> = self.float_strips.iter().map(|s| s.id).collect();
        strips.sort_unstable();
        strips.dedup();
        if strips.len() != self.float_strips.len() {
            return Err("floating strip ids repeat".into());
        }
        if let Some(open) = &self.open
            && !self.in_a_strip(open)
        {
            return Err(format!("a flyout for {open}, no strip's panel"));
        }
        Ok(())
    }
}

/// A vertical stack of boxes, one per tab group, with ids the shell does not already use.
fn column_node(shell: &VarosShell, boxes: &[Vec<String>]) -> Option<Node> {
    let mut used: Vec<u32> = shell.tree.boxes().iter().map(|b| b.id).chain(shell.floats.iter().map(|f| f.panels.id)).collect();
    let mut leaves = Vec::new();
    for tabs in boxes {
        let id = (1..u32::MAX).find(|i| !used.contains(i)).unwrap_or(u32::MAX);
        used.push(id);
        leaves.push(Node::Leaf(PanelBox { id, tabs: tabs.clone(), active: 0, collapsed: false }));
    }
    let mut iter = leaves.into_iter();
    let mut node = iter.next()?;
    for next in iter {
        node = Node::Split { axis: Axis::Vertical, ratio: 0.5, reset: 0.5, a: Box::new(node), b: Box::new(next) };
    }
    Some(node)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_switch_and_the_layout_round_trip_through_ui_json() {
        let mut ui = crate::state::UiState::default();
        assert!(ui.varos_shell.enabled, "on by default");
        ui.varos_shell.enabled = true;
        ui.varos_shell.add_tab(3, "stroke");
        ui.varos_shell.tree.set_ratio(&[], 0.61);
        let saved = serde_json::to_value(&ui).unwrap();
        assert_eq!(saved["varos_shell"]["enabled"], json!(true));
        assert_eq!(saved["varos_shell"]["version"], json!(VERSION));
        let back: crate::state::UiState = serde_json::from_value(saved).unwrap();
        assert_eq!(back.varos_shell, ui.varos_shell);
        assert!(back.docking.is_none(), "craft-ui's layout is its own");
    }

    #[test]
    fn the_shell_is_on_when_the_key_is_absent_and_a_chosen_off_stays_off() {
        let missing: crate::state::UiState = serde_json::from_value(json!({"varos_shell": {"version": 6}})).unwrap();
        assert!(missing.varos_shell.enabled, "an older ui.json without the key loads on");
        let forgotten: crate::state::UiState = serde_json::from_value(json!({"varos_shell": {"enabled": false, "version": 6}})).unwrap();
        assert!(forgotten.varos_shell.enabled, "version 6 wrote false as the default; the migration turns it on");
        assert!(!forgotten.varos_shell.enabled_chosen);
        let chosen: crate::state::UiState =
            serde_json::from_value(json!({"varos_shell": {"enabled": false, "enabled_chosen": true, "version": 7}})).unwrap();
        assert!(!chosen.varos_shell.enabled, "an explicit off stays off");
        assert!(chosen.varos_shell.enabled_chosen);
    }

    #[test]
    fn a_bad_or_foreign_layout_never_fails_the_ui_state() {
        for raw in [json!(null), json!(3), json!({"enabled": true, "tree": "x", "version": 1}), json!({"enabled": true, "version": 99, "tree": {}})] {
            let ui: crate::state::UiState = serde_json::from_value(json!({"varos_shell": raw})).unwrap();
            assert_eq!(ui.varos_shell.tree, Node::default_layout(), "{raw}");
        }
        let ui: crate::state::UiState = serde_json::from_value(json!({"varos_shell": {"enabled": true}})).unwrap();
        assert!(ui.varos_shell.enabled, "the switch is kept without a layout");
        let ui: crate::state::UiState = serde_json::from_value(json!({})).unwrap();
        assert_eq!(ui.varos_shell, historical_default());
    }

    const V1: &str = include_str!("fixtures/v1.json");
    const V2: &str = include_str!("fixtures/v2.json");
    const V3: &str = include_str!("fixtures/v3.json");
    const V6: &str = include_str!("fixtures/v6.json");

    #[test]
    fn version_6_false_without_a_choice_turns_on_and_a_chosen_off_stays_off() {
        let raw = fixture(V6);
        assert_eq!((raw["version"].clone(), raw["enabled"].clone()), (json!(6), json!(false)));
        assert!(raw.get("enabled_chosen").is_none(), "the fixture has no choice marker");
        let on = from_value(&raw);
        on.invariants().unwrap_or_else(|e| panic!("{e}"));
        assert!(on.enabled, "the old default off becomes on");
        assert!(!on.enabled_chosen);
        assert_eq!(on.version, VERSION);
        let mut marked = raw.clone();
        marked.as_object_mut().unwrap().insert("enabled_chosen".into(), json!(true));
        let off = from_value(&marked);
        assert!(!off.enabled, "an explicit off on a version-6 file stays off");
        assert!(off.enabled_chosen);
    }

    fn fixture(raw: &str) -> Value {
        serde_json::from_str(raw).unwrap()
    }

    /// Every panel some owner holds.
    fn owned(shell: &VarosShell) -> Vec<String> {
        let mut v: Vec<String> = shell.tree.boxes().iter().flat_map(|b| b.tabs.clone()).collect();
        v.extend(shell.strip.groups.iter().flat_map(|g| g.panels.clone()));
        v.extend(shell.floats.iter().flat_map(|f| f.panels.tabs.clone()));
        v.extend(shell.float_strips.iter().flat_map(|s| s.groups.iter().flat_map(|g| g.panels.clone())));
        v.sort();
        v
    }

    #[test]
    fn each_migration_is_one_pure_step() {
        let v1 = fixture(V1);
        let v2 = migrate_1_to_2(v1.clone());
        assert_eq!(migrate_1_to_2(v1.clone()), v2, "the same file, the same result");
        assert_eq!(v2["version"], json!(2));
        let strip = super::super::strip::Strip::deserialize(&v2["strip"]).unwrap();
        let tree = Node::deserialize(&v1["tree"]).unwrap();
        assert_eq!(strip, super::super::strip::Strip::legacy_default_for(&tree), "the original panels no box has");
        assert!(strip.group("ai").is_none());
        assert_eq!(v2["tree"], v1["tree"], "the boxes as they were");
        let v3 = migrate_2_to_3(v2.clone());
        assert_eq!((v3["version"].clone(), v3["floats"].clone(), v3["float_strips"].clone()), (json!(3), json!([]), json!([])));
        assert_eq!((v3["tree"].clone(), v3["strip"].clone()), (v2["tree"].clone(), v2["strip"].clone()));
        let v4 = migrate_3_to_4(v3.clone());
        assert_eq!(v4["version"], json!(4));
        assert_eq!((v4["views"].clone(), v4["maximized"].clone()), (json!({}), Value::Null));
        assert_eq!((v4["workspace"].clone(), v4["workspaces"].clone(), v4["workspace_changes"].clone()), (json!("Design"), json!([]), json!([])));
        assert_eq!((v4["tree"].clone(), v4["floats"].clone()), (v3["tree"].clone(), v3["floats"].clone()));
        // A version-2 file's strip is kept by the first step.
        let v2_file = fixture(V2);
        assert_eq!(migrate_1_to_2(v2_file.clone())["strip"], v2_file["strip"]);
    }

    /// Layouts saved by stages 1, 2 and 3 open without loss: the boxes as saved, every panel still
    /// somewhere, the strip's mode and sizes, floats, floating strips and collapsed boxes.
    #[test]
    fn stage_1_2_and_3_layouts_open_without_loss() {
        for (raw, version) in [(V1, 1), (V2, 2), (V3, 3)] {
            let v = fixture(raw);
            let shell = from_value(&v);
            shell.invariants().unwrap_or_else(|e| panic!("v{version}: {e}"));
            assert_eq!(shell.version, VERSION);
            assert!(shell.enabled, "v{version}");
            assert_eq!(shell.tree, Node::deserialize(&v["tree"]).unwrap().sanitized(&known), "v{version}: the boxes as saved");
            let mut saved: Vec<String> = Node::deserialize(&v["tree"]).unwrap().boxes().iter().flat_map(|b| b.tabs.clone()).collect();
            for key in ["strip"] {
                if let Some(groups) = v[key]["groups"].as_array() {
                    saved.extend(groups.iter().flat_map(|g| g["panels"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string())));
                }
            }
            for f in v["floats"].as_array().into_iter().flatten() {
                saved.extend(f["box"]["tabs"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()));
            }
            for s in v["float_strips"].as_array().into_iter().flatten() {
                saved.extend(s["groups"][0]["panels"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()));
            }
            let now = owned(&shell);
            for p in &saved {
                assert!(now.contains(p), "v{version}: {p} was lost");
            }
            assert_eq!((shell.workspace.as_str(), shell.maximized), (super::super::workspace::DESIGN, None));
            assert!(shell.views.is_empty() && shell.workspaces.is_empty());
        }
        let v1 = from_value(&fixture(V1));
        assert_eq!(v1.tree.ratio(&[]), Some(0.65));
        assert_eq!(v1.tree.find_box(3).unwrap().front(), Some("stroke"));
        let v2 = from_value(&fixture(V2));
        assert_eq!(v2.strip.mode, super::super::strip::Mode::Labels);
        assert_eq!(v2.strip.groups[0].width, 280.0);
        assert!(v2.strip.group("tabs").is_some() && v2.strip.group("separations").is_some());
        let v3 = from_value(&fixture(V3));
        let file = fixture(V3);
        assert_eq!(v3.floats, vec![Float::deserialize(&file["floats"][0]).unwrap()]);
        assert_eq!(v3.float_strips, vec![FloatStrip::deserialize(&file["float_strips"][0]).unwrap()]);
        assert!(v3.tree.find_box(2).unwrap().collapsed);
        assert_eq!(v3.strip.groups[2].width, 310.0);
        // Saved again, each is the current version and reads back the same.
        for raw in [V1, V2, V3] {
            let shell = from_value(&fixture(raw));
            assert_eq!(from_value(&serde_json::to_value(&shell).unwrap()), shell);
        }
    }

    /// Unknown or removed panel ids go from boxes, strips, floats and floating strips, unknown
    /// sections from the section views, views of no editor go; a layout without a Board is the
    /// default one.
    #[test]
    fn unknown_ids_are_dropped_everywhere_and_a_layout_without_a_board_is_the_default() {
        let mut v = fixture(V3);
        v["version"] = json!(4);
        v["tree"]["split"]["b"]["split"]["b"]["leaf"]["tabs"].as_array_mut().unwrap().push(json!("removedPanel"));
        v["strip"]["groups"][0]["panels"].as_array_mut().unwrap().push(json!("oldPanel"));
        v["strip"]["groups"].as_array_mut().unwrap().push(json!({"panels": ["gone"], "temporary": true, "width": 260.0, "height": 360.0}));
        v["floats"][0]["box"]["tabs"].as_array_mut().unwrap().push(json!("nope"));
        v["floats"].as_array_mut().unwrap().push(json!({"box": {"id": 1048577, "tabs": ["nothing"]}, "rect": [0.0, 0.0, 200.0, 200.0]}));
        v["float_strips"][0]["groups"][0]["panels"].as_array_mut().unwrap().push(json!("whatever"));
        v["views"] = json!({
            "properties": {"collapsed": ["transform", "noSuchSection"], "order": ["quickActions", "bogus", "transform"]},
            "properties@7": {"collapsed": ["align"]},
            "layers": {"collapsed": []}
        });
        v["maximized"] = json!(99);
        let shell = from_value(&v);
        shell.invariants().unwrap();
        let all = owned(&shell);
        for gone in ["removedPanel", "oldPanel", "gone", "nope", "nothing", "whatever"] {
            assert!(!all.iter().any(|p| p == gone), "{gone} dropped");
        }
        assert_eq!(shell.floats.len(), 1, "the float left empty goes");
        assert_eq!(shell.floats[0].panels.tabs, ["gradient"]);
        assert_eq!(shell.float_strips[0].groups[0].panels, ["transparency"]);
        assert_eq!(shell.views.keys().collect::<Vec<_>>(), ["properties"], "no view for an editor that is nowhere, nor for a panel that isn't one");
        let view = &shell.views["properties"];
        use crate::panels::properties::SectionId;
        assert_eq!(view.collapsed.iter().copied().collect::<Vec<_>>(), [SectionId::Transform]);
        let at = |id: SectionId| view.order.iter().position(|o| *o == id).unwrap();
        assert!(at(SectionId::QuickActions) < at(SectionId::Transform), "the saved order kept: {:?}", view.order);
        assert_eq!(view.order.len(), SectionId::ALL.len(), "every section once");
        assert_eq!(shell.maximized, None, "no such box");
        // No Board: the default layout (whatever the version).
        for version in [1, 2, 3, 4] {
            let mut v = fixture(V3);
            v["version"] = json!(version);
            v["tree"] = json!({"split": {"axis": "vertical", "ratio": 0.5, "a": {"leaf": {"id": 2, "tabs": ["layers"]}}, "b": {"leaf": {"id": 3, "tabs": ["color"]}}}});
            let shell = from_value(&v);
            assert_eq!(shell.tree, Node::default_layout(), "v{version}");
            shell.invariants().unwrap();
        }
    }

    #[test]
    fn unknown_panels_are_dropped_on_load() {
        let mut shell = VarosShell::default();
        shell.tree.add_tab(2, "stroke");
        let mut saved = serde_json::to_value(&shell).unwrap();
        let tabs = saved["tree"]["split"]["b"]["split"]["a"]["leaf"]["tabs"].as_array_mut().unwrap();
        tabs.push(json!("noSuchPanel"));
        let back = from_value(&saved);
        assert_eq!(back.tree, shell.tree);
        assert_eq!(back.tree.find_tab("noSuchPanel"), None);
    }
}
