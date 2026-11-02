//! Transactional panel layout. Presentation and visibility never transfer ownership.
//! Registry-backed ownership, workspace docking and retained floating dimensions.

use super::registry::{self, PanelKind};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_PANELS: usize = 128;
/// Outer host chrome excluded from its layout tree: two side borders, title and foot.
pub const HOST_CHROME: [f32; 2] = [2.0, 26.0];
/// A split has one separator; shared with geometry so remembered sizes use the same space.
pub const SPLIT_GAP: f32 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    Full,
    Names,
    Icons,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Presentation {
    pub mode: Mode,
    pub compact_width: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Group {
    pub id: u64,
    pub panels: Vec<String>,
    pub active: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<Presentation>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Node {
    Group(Group),
    Split { id: u64, axis: Axis, ratio: f32, first: Box<Node>, second: Box<Node> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Host {
    pub id: u64,
    pub position: [f32; 2],
    pub expanded: [f32; 2],
    pub compact_width: f32,
    pub mode: Mode,
    pub hidden: bool,
    #[serde(default)]
    pub dock: Option<Edge>,
    /// Frame-derived Control body height, excluded from persistence and workspace equality.
    #[serde(skip)]
    pub dock_content_height: Option<f32>,
    pub root: Node,
}

impl PartialEq for Host {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.position == other.position
            && self.expanded == other.expanded
            && self.compact_width == other.compact_width
            && self.mode == other.mode
            && self.hidden == other.hidden
            && self.dock == other.dock
            && self.root == other.root
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub version: u32,
    pub next_id: u64,
    pub hosts: Vec<Host>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "camelCase")]
pub enum Source {
    Host(u64),
    Group(u64),
    Panel(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Target {
    Float { position: [f32; 2] },
    Dock { edge: Edge },
    Merge { group: u64, index: usize },
    Split { group: u64, edge: Edge },
    Attach { host: u64, edge: Edge },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    Move { source: Source, target: Target },
    SetMode { host: u64, mode: Mode },
    SetToolsWidth { width: f32 },
    SetColumnMode { group: u64, mode: Mode },
    ResizeColumn { group: u64, width: f32 },
    Resize { host: u64, position: [f32; 2], width: f32, height: f32 },
    FitControlContent { host: u64, height: f32 },
    Hide { host: u64 },
    Raise { host: u64 },
    Show { panel: String },
    Activate { panel: String },
    SplitRatio { split: u64, ratio: f32 },
}

pub fn groups(node: &Node) -> Vec<&Group> {
    match node {
        Node::Group(g) => vec![g],
        Node::Split { first, second, .. } => {
            let mut out = groups(first);
            out.extend(groups(second));
            out
        }
    }
}

/// A column is the largest vertical-only subtree; horizontal branches stay independent.
pub fn has_horizontal(node: &Node) -> bool {
    match node {
        Node::Group(_) => false,
        Node::Split { axis, first, second, .. } => *axis == Axis::Horizontal || has_horizontal(first) || has_horizontal(second),
    }
}

pub fn column_node(node: &Node, group: u64) -> Option<&Node> {
    if !groups(node).iter().any(|g| g.id == group) {
        return None;
    }
    if !has_horizontal(node) {
        return Some(node);
    }
    match node {
        Node::Group(_) => Some(node),
        Node::Split { first, second, .. } => column_node(first, group).or_else(|| column_node(second, group)),
    }
}

/// Replace panel identity `from` with `to` wherever a group holds it.
pub fn rename_panel(node: &mut Node, from: &str, to: &str) {
    match node {
        Node::Group(g) => {
            for panel in &mut g.panels {
                if panel == from {
                    *panel = to.into();
                }
            }
            if g.active == from {
                g.active = to.into();
            }
        }
        Node::Split { first, second, .. } => {
            rename_panel(first, from, to);
            rename_panel(second, from, to);
        }
    }
}

pub fn presentation(host: &Host, group: &Group) -> Presentation {
    group.presentation.unwrap_or(Presentation { mode: host.mode, compact_width: host.compact_width })
}

fn set_presentation(node: &mut Node, value: Option<Presentation>) {
    match node {
        Node::Group(g) => g.presentation = value,
        Node::Split { first, second, .. } => {
            set_presentation(first, value);
            set_presentation(second, value);
        }
    }
}

fn column_mut(node: &mut Node, group: u64) -> Option<&mut Node> {
    if !groups(node).iter().any(|g| g.id == group) {
        return None;
    }
    if !has_horizontal(node) {
        return Some(node);
    }
    match node {
        Node::Group(_) => None,
        Node::Split { first, second, .. } => column_mut(first, group).or_else(|| column_mut(second, group)),
    }
}

/// Project retained full widths without scaling the expanded neighbour of a compact column.
pub fn displayed_width(host: &Host, node: &Node, saved: [f32; 2]) -> f32 {
    if !has_horizontal(node) {
        let mode = groups(node).first().map(|g| presentation(host, g));
        return mode.filter(|p| p.mode != Mode::Full).map_or(saved[0], |p| (p.compact_width - HOST_CHROME[0]).max(1.0));
    }
    match node {
        Node::Group(_) => saved[0],
        Node::Split { axis, ratio, first, second, .. } => {
            let (a, b) = partition(saved, *axis, *ratio);
            let a = displayed_width(host, first, a);
            let b = displayed_width(host, second, b);
            if *axis == Axis::Horizontal { a + b + SPLIT_GAP } else { a.max(b) }
        }
    }
}

fn resize_column(node: &mut Node, group: u64, size: [f32; 2], width: f32) -> [f32; 2] {
    if !groups(node).iter().any(|g| g.id == group) {
        return size;
    }
    if !has_horizontal(node) {
        return [width, size[1]];
    }
    match node {
        Node::Group(_) => size,
        Node::Split { axis, ratio, first, second, .. } => {
            let (a, b) = partition(size, *axis, *ratio);
            let a = resize_column(first, group, a, width);
            let b = resize_column(second, group, b, width);
            let d = if *axis == Axis::Horizontal { 0 } else { 1 };
            let mut total = [a[0].max(b[0]), a[1].max(b[1])];
            total[d] = a[d] + b[d] + SPLIT_GAP;
            *ratio = split_ratio(first, second, a[d], a[d] + b[d]);
            total
        }
    }
}

fn contains_kind(node: &Node, kind: PanelKind) -> bool {
    groups(node).iter().any(|group| group.panels.iter().any(|panel| registry::get(panel).is_some_and(|spec| spec.kind == kind)))
}

/// The column of `root` holding `group` is the Tools column.
pub fn column_has_tools(root: &Node, group: u64) -> bool {
    column_node(root, group).is_some_and(has_tools)
}

/// Tools remain an expanded vertical group even when joined beside ordinary panels.
pub fn has_tools(node: &Node) -> bool {
    contains_kind(node, PanelKind::Tools)
}

fn split_ratio(first: &Node, second: &Node, first_extent: f32, sum: f32) -> f32 {
    if sum <= 0.0 {
        return 0.5;
    }
    let ratio = first_extent / sum;
    if has_tools(first) || has_tools(second) { ratio } else { ratio.clamp(0.05, 0.95) }
}

/// Preserve all sibling extents while bounding the Tools leaf and rebuilding its ancestors.
fn bound_tools(node: &mut Node, size: [f32; 2], width: Option<f32>) -> [f32; 2] {
    if !has_tools(node) {
        return size;
    }
    match node {
        Node::Group(_) => [
            width.unwrap_or(size[0] + HOST_CHROME[0]).clamp(super::metrics::TOOLS_MIN_WIDTH, super::metrics::TOOLS_MAX_WIDTH) - HOST_CHROME[0],
            size[1].max(110.0 - HOST_CHROME[1]),
        ],
        Node::Split { axis, ratio, first, second, .. } => {
            let (a, b) = partition(size, *axis, *ratio);
            let a = bound_tools(first, a, width);
            let b = bound_tools(second, b, width);
            let d = if *axis == Axis::Horizontal { 0 } else { 1 };
            let sum = a[d] + b[d];
            let mut total = [a[0].max(b[0]), a[1].max(b[1])];
            total[d] = sum + SPLIT_GAP;
            *ratio = split_ratio(first, second, a[d], sum);
            total
        }
    }
}

pub(super) fn standalone_control(node: &Node) -> bool {
    matches!(node, Node::Group(group) if group.panels.len() == 1
        && group.panels.first().and_then(|panel| registry::get(panel)).is_some_and(|spec| spec.kind == PanelKind::Control))
}

fn valid_content_height(height: f32) -> bool {
    height.is_finite() && height > 0.0 && height <= 1_000_000.0
}

/// Placement capabilities belong to the content, including content nested inside a host.
pub fn dock_allowed(node: &Node, edge: Edge) -> bool {
    let side = matches!(edge, Edge::Left | Edge::Right);
    (!contains_kind(node, PanelKind::Tools) || side) && (!contains_kind(node, PanelKind::Control) || !side)
}

/// Special bars retain their orientation and ownership independently of the presenter.
/// Only saved logical extents are checked; viewport clipping does not affect this policy.
fn validate_placement(node: &Node, size: [f32; 2], tools_policy: bool) -> Result<(), String> {
    match node {
        Node::Group(group) => {
            let special = group.panels.iter().filter_map(|panel| registry::get(panel)).find(|spec| spec.kind != PanelKind::Standard);
            if let Some(spec) = special {
                if group.panels.len() != 1 {
                    return Err("Tools and Control cannot share a tab group".into());
                }
                let full = expanded_size(size);
                match spec.kind {
                    PanelKind::Tools
                        if tools_policy && !(super::metrics::TOOLS_MIN_WIDTH - 0.01..=super::metrics::TOOLS_MAX_WIDTH + 0.01).contains(&full[0]) =>
                    {
                        return Err("Tools width must remain between 44 and 80 points".into());
                    }
                    PanelKind::Tools if full[0] > full[1] + 0.01 => return Err("Tools must remain vertical".into()),
                    PanelKind::Control if full[1] > full[0] + 0.01 => return Err("Control must remain horizontal".into()),
                    _ => {}
                }
            }
        }
        Node::Split { axis, ratio, first, second, .. } => {
            if contains_kind(node, PanelKind::Control) {
                return Err("Control must remain a standalone bar".into());
            }
            if *axis != Axis::Horizontal && contains_kind(node, PanelKind::Tools) {
                return Err("Tools can join other panel groups only side by side".into());
            }
            let (a, b) = partition(size, *axis, *ratio);
            validate_placement(first, a, tools_policy)?;
            validate_placement(second, b, tools_policy)?;
        }
    }
    Ok(())
}

fn group_mut(node: &mut Node, id: u64) -> Option<&mut Group> {
    match node {
        Node::Group(g) => (g.id == id).then_some(g),
        Node::Split { first, second, .. } => group_mut(first, id).or_else(|| group_mut(second, id)),
    }
}

/// Replace a nested leaf and propagate only its dimensional change through its ancestors.
fn replace_group_sized(node: Node, id: u64, replacement: Node, size: [f32; 2], replacement_size: [f32; 2]) -> (Node, [f32; 2]) {
    match node {
        Node::Group(g) if g.id == id => (replacement, replacement_size),
        Node::Split { id: split, axis, ratio, first, second } => {
            let (a, b) = partition(size, axis, ratio);
            let in_first = groups(&first).iter().any(|g| g.id == id);
            let (first, a, second, b) = if in_first {
                let (n, s) = replace_group_sized(*first, id, replacement, a, replacement_size);
                (n, s, *second, b)
            } else {
                let (n, s) = replace_group_sized(*second, id, replacement, b, replacement_size);
                (*first, a, n, s)
            };
            let dimension = if axis == Axis::Horizontal { 0 } else { 1 };
            let sum = a[dimension] + b[dimension];
            let mut total = [a[0].max(b[0]), a[1].max(b[1])];
            total[dimension] = sum + SPLIT_GAP;
            let ratio = split_ratio(&first, &second, a[dimension], sum);
            (Node::Split { id: split, axis, ratio, first: Box::new(first), second: Box::new(second) }, total)
        }
        other => (other, size),
    }
}

fn content_size(expanded: [f32; 2]) -> [f32; 2] {
    [(expanded[0] - HOST_CHROME[0]).max(1.0), (expanded[1] - HOST_CHROME[1]).max(1.0)]
}

fn expanded_size(content: [f32; 2]) -> [f32; 2] {
    [content[0] + HOST_CHROME[0], content[1] + HOST_CHROME[1]]
}

fn partition(size: [f32; 2], axis: Axis, ratio: f32) -> ([f32; 2], [f32; 2]) {
    let dimension = if axis == Axis::Horizontal { 0 } else { 1 };
    let available = (size[dimension] - SPLIT_GAP).max(0.0);
    let mut first = size;
    let mut second = size;
    first[dimension] = available * ratio;
    second[dimension] = available - first[dimension];
    (first, second)
}

fn group_size(node: &Node, group: u64, size: [f32; 2]) -> Option<[f32; 2]> {
    match node {
        Node::Group(g) => (g.id == group).then_some(size),
        Node::Split { axis, ratio, first, second, .. } => {
            let (a, b) = partition(size, *axis, *ratio);
            group_size(first, group, a).or_else(|| group_size(second, group, b))
        }
    }
}

fn remove(node: Node, source: &Source, size: [f32; 2]) -> Option<(Node, [f32; 2])> {
    match node {
        Node::Group(mut g) => {
            if *source == Source::Group(g.id) {
                return None;
            }
            if let Source::Panel(panel) = source {
                g.panels.retain(|p| p != panel);
            }
            if g.panels.is_empty() {
                return None;
            }
            if !g.panels.contains(&g.active) {
                g.active = g.panels.first().cloned().unwrap_or_default();
            }
            Some((Node::Group(g), size))
        }
        Node::Split { id, axis, ratio, first, second } => {
            let (a, b) = partition(size, axis, ratio);
            match (remove(*first, source, a), remove(*second, source, b)) {
                (Some((a, a_size)), Some((b, b_size))) => {
                    let d = if axis == Axis::Horizontal { 0 } else { 1 };
                    let sum = a_size[d] + b_size[d];
                    let mut total = [a_size[0].max(b_size[0]), a_size[1].max(b_size[1])];
                    total[d] = sum + SPLIT_GAP;
                    let ratio = split_ratio(&a, &b, a_size[d], sum);
                    Some((Node::Split { id, axis, ratio, first: Box::new(a), second: Box::new(b) }, total))
                }
                (Some(n), None) | (None, Some(n)) => Some(n),
                (None, None) => None,
            }
        }
    }
}

impl Layout {
    pub fn column_key(&self, group: u64) -> Option<u64> {
        let host = self.source_host(&Source::Group(group))?;
        groups(column_node(&host.root, group)?).first().map(|g| g.id)
    }

    pub fn column_mode(&self, group: u64) -> Option<Mode> {
        let host = self.source_host(&Source::Group(group))?;
        groups(column_node(&host.root, group)?).first().map(|g| presentation(host, g).mode)
    }

    /// Width includes the same two border points as an independent floating host.
    pub fn column_width(&self, group: u64) -> Option<f32> {
        let host = self.source_host(&Source::Group(group))?;
        let g = groups(column_node(&host.root, group)?).first().copied()?;
        let p = presentation(host, g);
        Some(if p.mode == Mode::Full { group_size(&host.root, g.id, content_size(host.expanded))?[0] + HOST_CHROME[0] } else { p.compact_width })
    }

    /// The column holding `group` is the Tools column (which switches width, never collapses).
    pub fn column_has_tools(&self, group: u64) -> bool {
        self.source_host(&Source::Group(group)).is_some_and(|host| column_has_tools(&host.root, group))
    }

    /// The compact mode the column holding `group` collapses to: a docked column at least
    /// [`NAMES_MIN_WIDTH`](super::metrics::NAMES_MIN_WIDTH) wide shows names, anything else icons.
    pub fn collapsed_mode(&self, group: u64) -> Mode {
        let Some(host) = self.source_host(&Source::Group(group)) else { return Mode::Icons };
        let width = column_node(&host.root, group).and_then(|column| groups(column).first().map(|g| presentation(host, g).compact_width));
        if host.dock.is_some() && width.is_some_and(|width| width >= super::metrics::NAMES_MIN_WIDTH) { Mode::Names } else { Mode::Icons }
    }

    /// The two floating blocks every workspace starts from (Layers, then Swatches 328 points to
    /// its right) at `origin`; every other panel has one hidden owner until opened.
    fn starter(origin: [f32; 2]) -> Self {
        let block = |id: u64, group: u64, panel: &str, x: f32| Host {
            id,
            position: [x, origin[1]],
            expanded: [300.0, 380.0],
            compact_width: super::metrics::COMPACT,
            mode: Mode::Full,
            hidden: false,
            dock: None,
            dock_content_height: None,
            root: Node::Group(Group { id: group, panels: vec![panel.into()], active: panel.into(), presentation: None }),
        };
        Self { version: 1, next_id: 10, hosts: vec![block(1, 2, "layers", origin[0]), block(3, 4, "swatches", origin[0] + 328.0)] }
    }

    /// The two-block layout the model and geometry tests start from.
    #[cfg(test)]
    pub fn new_slice() -> Self {
        Self::starter([72.0, 72.0])
    }

    /// The Design workspace: the starter pair, every registered panel with a hidden owner, and
    /// Tools and Control docked at the left and top.
    pub fn new_workspace() -> Self {
        let mut layout = Self::starter([112.0, 152.0]);
        for spec in super::registry::BUILTINS.iter() {
            // The built-in registry is bounded and valid; keep defaults if allocation is exhausted.
            if layout.ensure_panel(spec.id).is_err() {
                break;
            }
        }
        for (panel, edge) in [("tools", Edge::Left), ("control", Edge::Top)] {
            if let Some(host) = layout.owner(panel).map(|h| h.id)
                && let Ok(h) = layout.host_mut(host)
            {
                h.hidden = false;
                h.dock = Some(edge);
            }
        }
        layout
    }

    /// A practical starter arrangement; the original Design baseline remains unchanged.
    pub fn studio_workspace() -> Result<Self, String> {
        let mut layout = Self::new_workspace();
        fn owner(layout: &Layout, panel: &str) -> Result<u64, String> {
            layout.owner(panel).map(|host| host.id).ok_or_else(|| format!("Missing Studio panel `{panel}`"))
        }
        for host in &mut layout.hosts {
            host.hidden = true;
        }
        layout.apply(&Action::SetToolsWidth { width: super::metrics::TOOLS_MAX_WIDTH })?;
        for panel in ["tools", "control"] {
            layout.apply(&Action::Show { panel: panel.into() })?;
        }
        let full = owner(&layout, "properties")?;
        let layers = owner(&layout, "layers")?;
        for (host, height) in [(full, 420.0), (layers, 380.0)] {
            layout.apply(&Action::Resize { host, position: [100.0, 100.0], width: 320.0, height })?;
        }
        layout.apply(&Action::Move { source: Source::Host(layers), target: Target::Attach { host: full, edge: Edge::Bottom } })?;
        layout.apply(&Action::Move { source: Source::Host(full), target: Target::Dock { edge: Edge::Right } })?;
        let icons = owner(&layout, "color")?;
        for panel in ["swatches", "gradient", "stroke", "appearance"] {
            let source = owner(&layout, panel)?;
            layout.apply(&Action::Move { source: Source::Host(source), target: Target::Attach { host: icons, edge: Edge::Bottom } })?;
        }
        layout.apply(&Action::SetMode { host: icons, mode: Mode::Icons })?;
        layout.apply(&Action::Move { source: Source::Host(icons), target: Target::Dock { edge: Edge::Right } })?;
        layout.validate()?;
        Ok(layout)
    }

    /// Register a real lazy panel (including library/editor instances) without duplicating it.
    pub fn ensure_panel(&mut self, panel: &str) -> Result<(), String> {
        if self.owner(panel).is_some() {
            return Ok(());
        }
        let spec = super::registry::get(panel).ok_or("Unknown panel")?;
        if self.hosts.iter().flat_map(|h| groups(&h.root)).map(|g| g.panels.len()).sum::<usize>() >= MAX_PANELS {
            return Err("Panel instance limit reached".into());
        }
        let mut next = self.clone();
        let id = next.fresh()?;
        let group = next.fresh()?;
        next.hosts.push(Host {
            id,
            position: [100.0, 100.0],
            expanded: spec.default_size,
            compact_width: super::metrics::COMPACT,
            mode: Mode::Full,
            hidden: true,
            dock: None,
            dock_content_height: None,
            root: Node::Group(Group { id: group, panels: vec![panel.into()], active: panel.into(), presentation: None }),
        });
        next.validate()?;
        *self = next;
        Ok(())
    }

    pub fn host(&self, id: u64) -> Option<&Host> {
        self.hosts.iter().find(|h| h.id == id)
    }
    pub fn group(&self, id: u64) -> Option<&Group> {
        self.hosts.iter().flat_map(|h| groups(&h.root)).find(|g| g.id == id)
    }
    pub fn owner(&self, panel: &str) -> Option<&Host> {
        self.hosts.iter().find(|h| groups(&h.root).iter().any(|g| g.panels.iter().any(|p| p == panel)))
    }
    pub fn group_of(&self, panel: &str) -> Option<&Group> {
        self.hosts.iter().flat_map(|h| groups(&h.root)).find(|g| g.panels.iter().any(|p| p == panel))
    }
    pub fn expanded_group_size(&self, group: u64) -> Option<[f32; 2]> {
        let h = self.source_host(&Source::Group(group))?;
        group_size(&h.root, group, content_size(h.expanded)).map(expanded_size)
    }

    /// Remembered Tools group width, including the two host borders.
    pub fn tools_width(&self) -> Option<f32> {
        self.expanded_group_size(self.group_of("tools")?.id).map(|size| size[0])
    }

    /// Upgrade old compact bars and oversized Tools hosts without altering ordinary panels.
    pub fn normalize_tools(&mut self) -> Result<(), String> {
        self.validate_tools_policy(false)?;
        let mut next = self.clone();
        for host in &mut next.hosts {
            if standalone_control(&host.root) {
                host.mode = Mode::Full;
                set_presentation(&mut host.root, None);
            }
            let special: Vec<_> = groups(&host.root)
                .iter()
                .filter(|g| g.panels.iter().filter_map(|p| registry::get(p)).any(|s| s.kind != PanelKind::Standard))
                .map(|g| g.id)
                .collect();
            for id in special {
                if let Some(g) = group_mut(&mut host.root, id) {
                    g.presentation = None;
                }
            }
        }
        next.bound_tools_hosts(None);
        next.validate()?;
        *self = next;
        Ok(())
    }

    fn bound_tools_hosts(&mut self, width: Option<f32>) {
        for host in &mut self.hosts {
            if has_tools(&host.root) {
                host.mode = Mode::Full;
                host.expanded = expanded_size(bound_tools(&mut host.root, content_size(host.expanded), width));
            }
        }
    }

    pub fn source_host(&self, source: &Source) -> Option<&Host> {
        match source {
            Source::Host(id) => self.host(*id),
            Source::Group(id) => self.hosts.iter().find(|h| groups(&h.root).iter().any(|g| g.id == *id)),
            Source::Panel(panel) => self.owner(panel),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        self.validate_tools_policy(true)
    }

    fn validate_tools_policy(&self, tools_policy: bool) -> Result<(), String> {
        if self.version != 1 || self.hosts.is_empty() || self.hosts.len() > MAX_PANELS {
            return Err("Invalid panel layout version or host count".into());
        }
        let mut ids = BTreeSet::new();
        let mut panels = BTreeSet::new();
        fn check(n: &Node, depth: usize, ids: &mut BTreeSet<u64>, panels: &mut BTreeSet<String>, tools_policy: bool) -> Result<(), String> {
            if depth > 16 {
                return Err("Panel nesting is too deep".into());
            }
            match n {
                Node::Group(g) => {
                    if g.presentation.is_some_and(|p| !p.compact_width.is_finite() || !(44.0..=320.0).contains(&p.compact_width)) {
                        return Err("Invalid column presentation".into());
                    }
                    if tools_policy
                        && g.presentation.is_some_and(|p| p.mode != Mode::Full)
                        && g.panels.iter().filter_map(|p| registry::get(p)).any(|s| s.kind != PanelKind::Standard)
                    {
                        return Err("Special bars cannot use compact presentation".into());
                    }
                    if !ids.insert(g.id) || g.panels.is_empty() || !g.panels.contains(&g.active) {
                        return Err("Invalid panel group".into());
                    }
                    for p in &g.panels {
                        if !super::registry::contains(p) || panels.len() >= MAX_PANELS || !panels.insert(p.clone()) {
                            return Err("Unknown or repeated panel owner".into());
                        }
                    }
                }
                Node::Split { id, ratio, first, second, .. } => {
                    let valid_ratio = if has_tools(n) { *ratio > 0.0 && *ratio < 1.0 } else { (0.05..=0.95).contains(ratio) };
                    if !ids.insert(*id) || !ratio.is_finite() || !valid_ratio {
                        return Err("Invalid panel split".into());
                    }
                    check(first, depth + 1, ids, panels, tools_policy)?;
                    check(second, depth + 1, ids, panels, tools_policy)?;
                }
            }
            Ok(())
        }
        for host in &self.hosts {
            if !ids.insert(host.id)
                || !host.position.iter().chain(host.expanded.iter()).all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
                || host.expanded.iter().any(|v| *v < 1.0)
                || !host.compact_width.is_finite()
                || !(44.0..=320.0).contains(&host.compact_width)
            {
                return Err("Invalid panel host geometry".into());
            }
            check(&host.root, 0, &mut ids, &mut panels, tools_policy)?;
            if host.dock_content_height.is_some_and(|height| !valid_content_height(height) || !standalone_control(&host.root)) {
                return Err("Invalid fitted Control content height".into());
            }
            if tools_policy && (has_tools(&host.root) || standalone_control(&host.root)) && host.mode != Mode::Full {
                return Err("Tools and Control must remain expanded".into());
            }
            validate_placement(&host.root, content_size(host.expanded), tools_policy)?;
            if host.dock.is_some_and(|edge| !dock_allowed(&host.root, edge)) {
                return Err("Tools dock only at the sides; Control docks only at the top or bottom".into());
            }
        }
        if !registry::required().all(|p| panels.contains(p)) || ids.last().is_some_and(|id| *id >= self.next_id) {
            return Err("Panel layout is missing an owner or valid identity counter".into());
        }
        Ok(())
    }

    /// Invalid actions leave both layout and identity allocation untouched.
    pub fn apply(&mut self, action: &Action) -> Result<(), String> {
        self.validate()?;
        let mut next = self.clone();
        let stack_presentation = next.prepare_column_move(action);
        next.apply_inner(action)?;
        if let Some((group, presentation)) = stack_presentation
            && let Some(host) = next.source_host(&Source::Group(group)).map(|h| h.id)
            && let Some(column) = next.host_mut(host).ok().and_then(|h| column_mut(&mut h.root, group))
        {
            set_presentation(column, Some(presentation));
        }
        if matches!(action, Action::Move { .. } | Action::Resize { .. } | Action::SplitRatio { .. }) {
            next.bound_tools_hosts(None);
        }
        next.validate()?;
        *self = next;
        Ok(())
    }

    /// Carry effective presentation across host boundaries; vertical stacking adopts its destination column.
    fn prepare_column_move(&mut self, action: &Action) -> Option<(u64, Presentation)> {
        let Action::Move { source, target } = action else { return None };
        let source_host = self.source_host(source)?.id;
        let (destination, group, vertical) = match target {
            Target::Attach { host, edge } => {
                let h = self.host(*host)?;
                (*host, groups(&h.root).first()?.id, matches!(edge, Edge::Top | Edge::Bottom))
            }
            Target::Merge { group, .. } | Target::Split { group, .. } => {
                let h = self.source_host(&Source::Group(*group))?;
                (h.id, *group, matches!(target, Target::Split { edge: Edge::Top | Edge::Bottom, .. }))
            }
            _ => return None,
        };
        let affected = [source_host, destination];
        let needs =
            affected.iter().filter_map(|id| self.host(*id)).any(|h| h.mode != Mode::Full || groups(&h.root).iter().any(|g| g.presentation.is_some()));
        if !needs {
            return None;
        }
        let h = self.host(destination)?;
        let preferred = presentation(h, self.group(group)?);
        for id in affected {
            if let Ok(h) = self.host_mut(id) {
                let values: Vec<_> = groups(&h.root).iter().map(|g| (g.id, presentation(h, g))).collect();
                for (group, value) in values {
                    if let Some(g) = group_mut(&mut h.root, group) {
                        g.presentation = Some(value);
                    }
                }
            }
        }
        vertical.then_some((group, preferred))
    }

    fn fresh(&mut self) -> Result<u64, String> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or("Panel identity limit reached")?;
        Ok(id)
    }

    fn host_mut(&mut self, id: u64) -> Result<&mut Host, String> {
        self.hosts.iter_mut().find(|h| h.id == id).ok_or_else(|| "Unknown panel host".into())
    }

    fn apply_inner(&mut self, action: &Action) -> Result<(), String> {
        match action {
            Action::Move { source, target } => self.move_item(source, target),
            Action::SplitRatio { split, ratio } => {
                if !ratio.is_finite() || *ratio <= 0.0 || *ratio >= 1.0 {
                    return Err("Invalid split ratio".into());
                }
                fn set(node: &mut Node, split: u64, value: f32) -> Result<bool, String> {
                    let tools = has_tools(node);
                    match node {
                        Node::Group(_) => Ok(false),
                        Node::Split { id, ratio, first, second, .. } => {
                            if *id == split {
                                if !tools && !(0.05..=0.95).contains(&value) {
                                    return Err("Invalid split ratio".into());
                                }
                                *ratio = value;
                                Ok(true)
                            } else {
                                Ok(set(first, split, value)? || set(second, split, value)?)
                            }
                        }
                    }
                }
                for host in &mut self.hosts {
                    if set(&mut host.root, *split, *ratio)? {
                        return Ok(());
                    }
                }
                Err("Unknown divider".into())
            }
            Action::SetMode { host, mode } => {
                let h = self.host_mut(*host)?;
                if (has_tools(&h.root) || standalone_control(&h.root)) && *mode != Mode::Full {
                    return Err("Tools and Control must remain expanded".into());
                }
                h.mode = *mode;
                set_presentation(&mut h.root, None);
                Ok(())
            }
            Action::SetColumnMode { group, mode } => {
                let id = self.source_host(&Source::Group(*group)).ok_or("Unknown column")?.id;
                let h = self.host_mut(id)?;
                let column = column_node(&h.root, *group).ok_or("Unknown column")?;
                if (has_tools(column) || standalone_control(column)) && *mode != Mode::Full {
                    return Err("Tools and Control must remain expanded".into());
                }
                if has_tools(column) || standalone_control(column) {
                    return Ok(());
                }
                let compact_width = groups(column).first().map(|g| presentation(h, g).compact_width).unwrap_or(44.0);
                let compact_width = match mode {
                    Mode::Icons => super::metrics::COMPACT,
                    Mode::Names => compact_width.max(72.0),
                    Mode::Full => compact_width,
                };
                if !has_horizontal(&h.root) {
                    h.mode = *mode;
                    h.compact_width = compact_width;
                    set_presentation(&mut h.root, None);
                } else {
                    set_presentation(column_mut(&mut h.root, *group).ok_or("Unknown column")?, Some(Presentation { mode: *mode, compact_width }));
                }
                Ok(())
            }
            Action::ResizeColumn { group, width } => {
                if !width.is_finite() || *width < 1.0 {
                    return Err("Invalid column width".into());
                }
                let id = self.source_host(&Source::Group(*group)).ok_or("Unknown column")?.id;
                let mode = self.column_mode(*group).ok_or("Unknown column")?;
                let h = self.host_mut(id)?;
                let column = column_node(&h.root, *group).ok_or("Unknown column")?;
                if has_tools(column) {
                    self.bound_tools_hosts(Some(*width));
                    return Ok(());
                }
                if standalone_control(column) {
                    return Err("Control uses its dedicated width policy".into());
                }
                if mode == Mode::Full {
                    let minimum = groups(column)
                        .iter()
                        .flat_map(|g| g.panels.iter())
                        .filter_map(|p| registry::get(p))
                        .map(|s| s.min_size[0])
                        .fold(1.0, f32::max);
                    h.expanded = expanded_size(resize_column(&mut h.root, *group, content_size(h.expanded), width.max(minimum) - HOST_CHROME[0]));
                } else {
                    let compact_width = width.clamp(44.0, 320.0);
                    let mode = if compact_width >= 72.0 { Mode::Names } else { Mode::Icons };
                    if !has_horizontal(&h.root) {
                        h.mode = mode;
                        h.compact_width = compact_width;
                        set_presentation(&mut h.root, None);
                    } else {
                        set_presentation(column_mut(&mut h.root, *group).ok_or("Unknown column")?, Some(Presentation { mode, compact_width }));
                    }
                }
                Ok(())
            }
            Action::SetToolsWidth { width } => {
                if !width.is_finite() {
                    return Err("Invalid Tools width".into());
                }
                if self.owner("tools").is_none() {
                    return Err("Missing Tools owner".into());
                }
                self.bound_tools_hosts(Some(*width));
                Ok(())
            }
            Action::FitControlContent { host, height } => {
                if !valid_content_height(*height) {
                    return Err("Invalid fitted Control content height".into());
                }
                let h = self.host_mut(*host)?;
                if !standalone_control(&h.root) {
                    return Err("Only standalone Control content can set a fitted height".into());
                }
                h.dock_content_height = Some(*height);
                Ok(())
            }
            Action::Resize { host, position, width, height } => {
                if !position.iter().chain([width, height]).all(|v| v.is_finite()) || *width < 1.0 || *height < 1.0 {
                    return Err("Invalid panel dimensions".into());
                }
                let h = self.host_mut(*host)?;
                h.position = *position;
                if h.mode == Mode::Full {
                    h.expanded = [*width, *height];
                } else {
                    h.compact_width = width.clamp(44.0, 320.0);
                    h.mode = if h.compact_width >= 72.0 { Mode::Names } else { Mode::Icons };
                }
                Ok(())
            }
            Action::Raise { host } => self.raise(*host),
            Action::Hide { host } => {
                self.host_mut(*host)?.hidden = true;
                Ok(())
            }
            Action::Show { panel } | Action::Activate { panel } => {
                let group = self.group_of(panel).ok_or("Unknown panel")?.id;
                let host = self.owner(panel).ok_or("Unknown panel owner")?.id;
                let h = self.host_mut(host)?;
                if matches!(action, Action::Show { .. }) {
                    h.hidden = false;
                }
                group_mut(&mut h.root, group).ok_or("Missing panel group")?.active = panel.clone();
                if matches!(action, Action::Show { .. }) { self.raise(host) } else { Ok(()) }
            }
        }
    }

    fn raise(&mut self, host: u64) -> Result<(), String> {
        let index = self.hosts.iter().position(|h| h.id == host).ok_or("Unknown panel host")?;
        let item = self.hosts.remove(index);
        self.hosts.push(item);
        Ok(())
    }

    fn move_item(&mut self, source: &Source, target: &Target) -> Result<(), String> {
        let old_host = self.source_host(source).ok_or("Unknown drag source")?.clone();
        if let Target::Dock { edge } = target {
            // Detaching uses exactly the same ownership transaction as a free float.
            self.move_item(source, &Target::Float { position: old_host.position })?;
            let panel = groups(&old_host.root).first().and_then(|g| g.panels.first()).cloned();
            let moved = match source {
                Source::Panel(p) => self.owner(p).map(|h| h.id),
                Source::Group(g) => self.source_host(&Source::Group(*g)).map(|h| h.id),
                Source::Host(_) => panel.as_deref().and_then(|p| self.owner(p)).map(|h| h.id),
            }
            .ok_or("Missing dock payload")?;
            let host = self.host_mut(moved)?;
            if !dock_allowed(&host.root, *edge) {
                return Err("Tools dock only at the sides; Control docks only at the top or bottom".into());
            }
            host.dock = Some(*edge);
            return Ok(());
        }
        if let Target::Float { position } = target {
            if !position.iter().all(|v| v.is_finite()) {
                return Err("Invalid floating position".into());
            }
            let whole = matches!(source, Source::Host(_))
                || match &old_host.root {
                    Node::Group(g) => {
                        matches!(source, Source::Group(id) if *id == g.id)
                            || matches!(source, Source::Panel(p) if g.panels.len() == 1 && g.panels.contains(p))
                    }
                    _ => false,
                };
            if whole {
                let h = self.host_mut(old_host.id)?;
                h.position = *position;
                h.dock = None;
                h.hidden = false;
                return self.raise(old_host.id);
            }
        }
        if let (Source::Panel(panel), Target::Merge { group, index }) = (source, target)
            && self.group_of(panel).is_some_and(|g| g.id == *group)
        {
            let g = group_mut(&mut self.host_mut(old_host.id)?.root, *group).ok_or("Missing group")?;
            if *index > g.panels.len() {
                return Err("Invalid tab insertion index".into());
            }
            let from = g.panels.iter().position(|p| p == panel).ok_or("Missing tab")?;
            g.panels.retain(|p| p != panel);
            let at = if from < *index { index.saturating_sub(1) } else { *index };
            g.panels.insert(at, panel.clone());
            g.active = panel.clone();
            return Ok(());
        }
        // Derive retained dimensions from the expanded tree, even while its host is compact.
        let old_content = content_size(old_host.expanded);
        let payload_size = match source {
            Source::Host(_) => old_content,
            Source::Group(id) => group_size(&old_host.root, *id, old_content).ok_or("Unknown group size")?,
            Source::Panel(panel) => {
                let group = self.group_of(panel).ok_or("Unknown panel")?.id;
                group_size(&old_host.root, group, old_content).ok_or("Unknown panel size")?
            }
        };
        let payload = match source {
            Source::Host(_) => old_host.root.clone(),
            Source::Group(id) => Node::Group(self.group(*id).ok_or("Unknown group")?.clone()),
            Source::Panel(panel) => {
                let g = self.group_of(panel).ok_or("Unknown panel")?.clone();
                if g.panels.len() == 1 {
                    Node::Group(g)
                } else {
                    Node::Group(Group { id: self.fresh()?, panels: vec![panel.clone()], active: panel.clone(), presentation: g.presentation })
                }
            }
        };
        let mut hosts = Vec::new();
        for mut host in std::mem::take(&mut self.hosts) {
            if matches!(source, Source::Host(id) if *id == host.id) {
                continue;
            }
            if host.id != old_host.id {
                hosts.push(host);
                continue;
            }
            if let Some((root, size)) = remove(host.root, source, content_size(host.expanded)) {
                host.root = root;
                host.expanded = expanded_size(size);
                hosts.push(host);
            }
        }
        self.hosts = hosts;
        match target {
            Target::Dock { .. } => return Err("Dock target was not resolved".into()),
            Target::Float { position } => {
                let id = self.fresh()?;
                self.hosts.push(Host {
                    id,
                    position: *position,
                    expanded: expanded_size(payload_size),
                    root: payload,
                    hidden: false,
                    dock: None,
                    ..old_host
                });
            }
            Target::Merge { group, index } => {
                let Node::Group(incoming) = payload else {
                    return Err("Drop a multi-group block on an edge to keep its groups".into());
                };
                let h = self.hosts.iter_mut().find(|h| groups(&h.root).iter().any(|g| g.id == *group)).ok_or("Drop target no longer exists")?;
                let g = group_mut(&mut h.root, *group).ok_or("Missing destination group")?;
                if *index > g.panels.len() {
                    return Err("Invalid tab insertion index".into());
                }
                g.panels.splice(*index..*index, incoming.panels);
                g.active = incoming.active;
                h.hidden = false;
            }
            Target::Split { group, edge } => {
                let old = self.group(*group).ok_or("Drop target no longer exists")?.clone();
                let destination = self.source_host(&Source::Group(*group)).ok_or("Missing destination host")?;
                let old_size = group_size(&destination.root, *group, content_size(destination.expanded)).ok_or("Missing destination size")?;
                let (replacement, size) = self.split(Node::Group(old), old_size, payload, payload_size, *edge)?;
                let h = self.hosts.iter_mut().find(|h| groups(&h.root).iter().any(|g| g.id == *group)).ok_or("Missing destination host")?;
                let (root, total) = replace_group_sized(h.root.clone(), *group, replacement, content_size(h.expanded), size);
                h.root = root;
                h.hidden = false;
                h.expanded = expanded_size(total);
            }
            Target::Attach { host, edge } => {
                let destination = self.host(*host).ok_or("Drop target no longer exists")?;
                let old = destination.root.clone();
                let old_size = content_size(destination.expanded);
                let (replacement, size) = self.split(old, old_size, payload, payload_size, *edge)?;
                let h = self.host_mut(*host)?;
                h.root = replacement;
                h.hidden = false;
                h.expanded = expanded_size(size);
            }
        }
        Ok(())
    }

    fn split(
        &mut self,
        existing: Node,
        existing_size: [f32; 2],
        incoming: Node,
        incoming_size: [f32; 2],
        edge: Edge,
    ) -> Result<(Node, [f32; 2]), String> {
        let axis = if matches!(edge, Edge::Left | Edge::Right) { Axis::Horizontal } else { Axis::Vertical };
        let dimension = if axis == Axis::Horizontal { 0 } else { 1 };
        let (first, first_size, second, second_size) = if matches!(edge, Edge::Left | Edge::Top) {
            (incoming, incoming_size, existing, existing_size)
        } else {
            (existing, existing_size, incoming, incoming_size)
        };
        let mut size = [first_size[0].max(second_size[0]), first_size[1].max(second_size[1])];
        let sum = first_size[dimension] + second_size[dimension];
        size[dimension] = sum + SPLIT_GAP;
        let ratio = split_ratio(&first, &second, first_size[dimension], sum);
        Ok((Node::Split { id: self.fresh()?, axis, ratio, first: Box::new(first), second: Box::new(second) }, size))
    }
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "size_tests.rs"]
mod size_tests;

#[cfg(test)]
#[path = "extended_tests.rs"]
mod extended_tests;
