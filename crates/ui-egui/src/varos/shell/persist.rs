//! The shell's saved state: `varos_shell` in `ui.json` (version 3: the boxes, the Board's strip,
//! floating panels and floating strips), apart from craft-ui's `docking` layout, which it never
//! touches. Versions 1 and 2 load (no floats then).
//!
//! Every panel has at most one owner: a box of the tree, the Board's strip, a floating panel or a
//! floating strip ([`VarosShell::invariants`]).

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::floats::{Float, FloatStrip};
use super::strip::{Group, StripRef};
use super::tree::{BOX_MIN, Node, PanelBox};

/// The saved layout's version.
pub(crate) const VERSION: u32 = 3;
/// Floating panels' box ids start here (the tree's start at 1), so the two never meet.
const FLOAT_IDS: u32 = 1 << 20;

/// Preferences › Varos shell (new) and the shell's box layout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VarosShell {
    /// The window's central region is the Varos shell (off: craft-ui's dock, as upstream).
    pub enabled: bool,
    /// The layout's version ([`VERSION`]).
    pub version: u32,
    /// The boxes.
    pub tree: Node,
    /// The sole Board owns this strip; it is independent of ordinary panel boxes.
    pub strip: super::strip::Strip,
    /// Floating panels, bottom to top as first drawn.
    #[serde(default)]
    pub floats: Vec<Float>,
    /// Floating strips (a lone floating icon is a floating strip of one icon).
    #[serde(default)]
    pub float_strips: Vec<FloatStrip>,
    /// The panel whose flyout is open (beside the strip that owns it); not saved.
    #[serde(skip)]
    pub open: Option<String>,
    /// The drag under way; not saved.
    #[serde(skip)]
    pub drag: Option<super::drag::Drag>,
}

impl Default for VarosShell {
    fn default() -> Self {
        Self {
            enabled: false,
            version: VERSION,
            strip: super::strip::Strip::default_for(&Node::default_layout()),
            tree: Node::default_layout(),
            floats: Vec::new(),
            float_strips: Vec::new(),
            open: None,
            drag: None,
        }
    }
}

/// A panel id the app knows (an icon panel, a dock tab or a library panel).
pub(crate) fn known(id: &str) -> bool {
    crate::menus::normalize_panel(id) == Some(id)
}

/// `varos_shell` read from what `ui.json` holds: the switch kept whatever the layout; a layout of
/// another version, or one that doesn't read, is the default; unknown panel ids are dropped.
pub(crate) fn from_value(v: &Value) -> VarosShell {
    let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let version = v.get("version").and_then(Value::as_u64);
    let tree = matches!(version, Some(1..=3))
        .then(|| v.get("tree").and_then(|t| Node::deserialize(t).ok()))
        .flatten()
        .map_or_else(Node::default_layout, |t| t.sanitized(&known));
    let strip = v.get("strip").and_then(|s| super::strip::Strip::deserialize(s).ok()).unwrap_or_else(|| super::strip::Strip::default_for(&tree));
    let current = version == Some(u64::from(VERSION));
    // Each float and floating strip reads alone: one that doesn't read is dropped, not the rest.
    let list = |key: &str| if current { v.get(key).and_then(Value::as_array).cloned().unwrap_or_default() } else { Vec::new() };
    let floats = list("floats").into_iter().filter_map(|f| Float::deserialize(f).ok()).collect();
    let float_strips = list("float_strips").into_iter().filter_map(|s| FloatStrip::deserialize(s).ok()).collect();
    let mut shell = VarosShell { enabled, version: VERSION, tree, strip, floats, float_strips, open: None, drag: None };
    shell.sanitize(!current);
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
    /// Close `panel`'s tab (in a box or a floating panel): it goes back to the Board's strip.
    pub fn close_tab(&mut self, panel: &str) -> bool {
        if !self.tree.remove_tab(panel) && !self.take_float_tab(panel) {
            return false;
        }
        self.strip.append(panel);
        true
    }

    /// Take `panel` out of whichever floating panel holds it (the float goes with its last tab).
    fn take_float_tab(&mut self, panel: &str) -> bool {
        let Some(f) = self.floats.iter_mut().find(|f| f.panels.tabs.iter().any(|t| t == panel)) else { return false };
        let b = &mut f.panels;
        if let Some(i) = b.tabs.iter().position(|t| t == panel) {
            b.tabs.remove(i);
            if b.active > i || b.active >= b.tabs.len() {
                b.active = b.active.saturating_sub(1);
            }
        }
        self.floats.retain(|f| !f.panels.tabs.is_empty());
        true
    }

    /// Take `panel` from its owner, whoever it is: a box (its box goes with its last tab), the
    /// Board's strip, a floating panel or a floating strip (each goes when left empty).
    pub fn take(&mut self, panel: &str) {
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

    /// Floating panel `id`'s `×`: its panels go back to the Board's strip.
    pub(crate) fn close_float(&mut self, id: u32) {
        let Some(i) = self.floats.iter().position(|f| f.panels.id == id) else { return };
        for p in self.floats.remove(i).panels.tabs {
            self.strip.append(&p);
        }
    }

    /// Floating panel `id`'s `<<`: a lone icon (a floating strip of one group) where it was.
    pub(crate) fn float_to_icon(&mut self, id: u32) {
        let Some(i) = self.floats.iter().position(|f| f.panels.id == id) else { return };
        let f = self.floats.remove(i);
        let r = f.rect();
        let temporary = f.panels.tabs.iter().all(|p| !super::strip::Strip::is_default(p));
        let group = Group { panels: f.panels.tabs, temporary, width: r.width(), height: r.height() };
        let id = self.fresh_strip_id();
        self.float_strips.push(FloatStrip { id, pos: [r.left(), r.top()], groups: vec![group] });
    }

    /// Floating strip `id`'s `×`: its panels go back to the Board's strip.
    pub(crate) fn close_float_strip(&mut self, id: u32) {
        let Some(i) = self.float_strips.iter().position(|s| s.id == id) else { return };
        for p in self.float_strips.remove(i).groups.into_iter().flat_map(|g| g.panels) {
            self.strip.append(&p);
        }
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
    fn sanitize(&mut self, legacy: bool) {
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
            let size = egui::vec2(ok(r.width(), super::floats::DEFAULT_SIZE.x), ok(r.height(), super::floats::DEFAULT_SIZE.y))
                .clamp(BOX_MIN, egui::vec2(4000.0, 4000.0));
            f.set_rect(egui::Rect::from_min_size(egui::pos2(ok(r.left(), 0.0), ok(r.top(), 0.0)), size));
        }
        self.floats.retain(|f| !f.panels.tabs.is_empty());
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

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_switch_and_the_layout_round_trip_through_ui_json() {
        let mut ui = crate::state::UiState::default();
        assert!(!ui.varos_shell.enabled, "off by default");
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
    fn a_bad_or_foreign_layout_never_fails_the_ui_state() {
        for raw in [json!(null), json!(3), json!({"enabled": true, "tree": "x", "version": 1}), json!({"enabled": true, "version": 99, "tree": {}})] {
            let ui: crate::state::UiState = serde_json::from_value(json!({"varos_shell": raw})).unwrap();
            assert_eq!(ui.varos_shell.tree, Node::default_layout(), "{raw}");
        }
        let ui: crate::state::UiState = serde_json::from_value(json!({"varos_shell": {"enabled": true}})).unwrap();
        assert!(ui.varos_shell.enabled, "the switch is kept without a layout");
        let ui: crate::state::UiState = serde_json::from_value(json!({})).unwrap();
        assert_eq!(ui.varos_shell, VarosShell::default());
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
