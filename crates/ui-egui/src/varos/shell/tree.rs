//! The shell's split tree of boxes: pure data and the split / merge / ratio arithmetic, no egui
//! state (only its `Rect` and `Vec2` maths).
//!
//! A [`Node`] is either a [`Node::Split`] of two nodes along an [`Axis`] at a ratio, or a
//! [`Node::Leaf`] holding one [`PanelBox`]: the Board (the canvas, [`BOARD`]) or an ordinary box of
//! panel tabs. Every panel id appears at most once in the whole tree; the Board exactly once.

use egui::{Rect, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};

/// The Board's tab id: the box that holds the canvas. Not a panel id (`window.panel` never names it).
pub(crate) const BOARD: &str = "board";
/// The void between two boxes (and around them): the old shell's `SEAM_GAP`.
pub(crate) const SEAM: f32 = 1.0;
/// The smallest an ordinary box may be made by a divider (height; width comes from [`panel_min_width`]).
pub(crate) const BOX_MIN: Vec2 = vec2(180.0, 120.0);

/// The pointer must pass the column's minimum by this much before the azure collapse hint shows.
pub(crate) const COLLAPSE_HINT: f32 = 16.0;
/// On release, the column collapses only once the pointer is this far past the minimum.
pub(crate) const COLLAPSE_RELEASE: f32 = 32.0;

/// The narrowest a panel's column may be drawn. Dividers stop here; dragging past it collapses
/// the column into the strip.
pub(crate) fn panel_min_width(id: &str) -> f32 {
    let kind = id.split_once('@').map(|(kind, _)| kind).unwrap_or(id);
    match kind {
        "color" => 240.0,
        "properties" | "character" | "paragraph" => 300.0,
        "layers" => 200.0,
        _ => 180.0,
    }
}
/// The smallest the Board may be made by a divider.
pub(crate) const BOARD_MIN: Vec2 = vec2(240.0, 180.0);
/// The Board's share of the window's width in the default layout.
pub(crate) const BOARD_RATIO: f32 = 0.72;
/// A box collapsed to its tab row ([`PanelBox::collapsed`]) is this tall: the tab row's height.
pub(crate) const ROW: f32 = 36.0;
/// No ratio is stored outside this range (a hand-edited file, a NaN).
const RATIO_RANGE: std::ops::RangeInclusive<f32> = 0.02..=0.98;
/// The Design layout's Pages box: the top fifth of the left column (a header and a few rows).
const PAGES_RATIO: f32 = 0.2;

/// The way a split lays out its two children.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// Side by side: `a` on the left, `b` on the right; the divider between them is vertical.
    Horizontal,
    /// Stacked: `a` on top, `b` below; the divider between them is horizontal.
    Vertical,
}

impl Axis {
    /// `v`'s component along the axis.
    pub(crate) fn of(self, v: Vec2) -> f32 {
        match self {
            Axis::Horizontal => v.x,
            Axis::Vertical => v.y,
        }
    }
}

/// One box: the Board, or panels as tabs with one in front.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelBox {
    /// Stable for the box's life (egui ids, hit rects); unique in the tree.
    #[serde(default)]
    pub id: u32,
    /// Panel ids in tab order; `[BOARD]` for the Board.
    pub tabs: Vec<String>,
    /// The tab in front.
    #[serde(default)]
    pub active: usize,
    /// Shown as its tab row only (a double-click on a tab); its sibling takes the rest. Never the
    /// Board.
    #[serde(default)]
    pub collapsed: bool,
}

impl PanelBox {
    pub(crate) fn is_board(&self) -> bool {
        self.tabs.iter().any(|t| t == BOARD)
    }

    /// The panel in front.
    pub(crate) fn front(&self) -> Option<&str> {
        self.tabs.get(self.active).or(self.tabs.first()).map(String::as_str)
    }

    fn min_size(&self) -> Vec2 {
        if self.is_board() {
            BOARD_MIN
        } else {
            let width = self.tabs.iter().map(|id| panel_min_width(id)).fold(180.0, f32::max);
            if self.collapsed { vec2(width, ROW) } else { vec2(width, BOX_MIN.y) }
        }
    }

    /// Drawn as its tab row only.
    pub(crate) fn is_collapsed(&self) -> bool {
        self.collapsed && !self.is_board()
    }
}

/// A node of the split tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Node {
    Split {
        axis: Axis,
        /// `a`'s share of the room left after the seam.
        ratio: f32,
        /// The ratio a double-click on the divider puts back.
        #[serde(default = "half")]
        reset: f32,
        a: Box<Node>,
        b: Box<Node>,
    },
    Leaf(PanelBox),
}

fn half() -> f32 {
    0.5
}

/// Where a split sits in the tree: from the root, `0` for its `a`, `1` for its `b`.
pub type Path = Vec<u8>;

/// A divider: the seam between a split's two children.
#[derive(Clone, Debug, PartialEq)]
pub struct Divider {
    /// The split it belongs to.
    pub path: Path,
    pub axis: Axis,
    /// The seam itself (its hit area).
    pub rect: Rect,
    /// The whole split's rect.
    pub span: Rect,
}

/// The tree laid out in a rect: every box's rect (tree order) and every divider.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub boxes: Vec<(u32, Rect)>,
    pub dividers: Vec<Divider>,
}

impl Layout {
    #[cfg(test)]
    pub(crate) fn rect_of(&self, id: u32) -> Option<Rect> {
        self.boxes.iter().find(|(b, _)| *b == id).map(|(_, r)| *r)
    }
}

/// The smallest non-zero id not in `used` (a box tree never holds `u32::MAX` boxes).
fn free_id(used: &[u32]) -> u32 {
    (1..=u32::MAX).find(|i| !used.contains(i)).unwrap_or(u32::MAX)
}

fn leaf(id: u32, tabs: &[&str]) -> Node {
    Node::Leaf(PanelBox { id, tabs: tabs.iter().map(|t| (*t).to_string()).collect(), active: 0, collapsed: false })
}

fn split(axis: Axis, ratio: f32, a: Node, b: Node) -> Node {
    Node::Split { axis, ratio, reset: ratio, a: Box::new(a), b: Box::new(b) }
}

/// The ratio range that keeps both sides at least their minimum (`a_min`, `b_min` along the axis)
/// in `room` (the split's extent less the seam); when both can't fit, the ratio that shares the
/// shortfall in proportion.
fn ratio_bounds(room: f32, a_min: f32, b_min: f32) -> (f32, f32) {
    if room <= 0.0 {
        return (0.5, 0.5);
    }
    let lo = a_min / room;
    let hi = 1.0 - b_min / room;
    if lo <= hi {
        (lo, hi)
    } else {
        let r = a_min / (a_min + b_min).max(1.0);
        (r, r)
    }
}

impl Node {
    /// Design: Pages over Layers | Artboards on the left, Board in the middle, Properties on the
    /// right (the simple-shell mockup's columns; every group stays a dockable box).
    pub fn default_layout() -> Node {
        split(
            Axis::Horizontal,
            232.0 / 1360.0,
            split(Axis::Vertical, PAGES_RATIO, leaf(4, &["pages"]), leaf(2, &["layers", "artboards"])),
            split(Axis::Horizontal, 880.0 / 1128.0, leaf(1, &[BOARD]), leaf(3, &["properties"])),
        )
    }

    /// The Board (left, 72 %) beside a column of two boxes: `top`'s tabs over `bottom`'s (the
    /// workspace presets' shape).
    pub fn board_and_column(top: &[&str], bottom: &[&str]) -> Node {
        split(Axis::Horizontal, BOARD_RATIO, leaf(1, &[BOARD]), split(Axis::Vertical, 0.5, leaf(2, top), leaf(3, bottom)))
    }

    /// Every box, in tree order (a before b).
    pub fn boxes(&self) -> Vec<&PanelBox> {
        let mut out = Vec::new();
        self.walk(&mut |b| out.push(b));
        out
    }

    fn walk<'a>(&'a self, f: &mut impl FnMut(&'a PanelBox)) {
        match self {
            Node::Leaf(b) => f(b),
            Node::Split { a, b, .. } => {
                a.walk(f);
                b.walk(f);
            }
        }
    }

    fn walk_mut(&mut self, f: &mut impl FnMut(&mut PanelBox)) {
        match self {
            Node::Leaf(b) => f(b),
            Node::Split { a, b, .. } => {
                a.walk_mut(f);
                b.walk_mut(f);
            }
        }
    }

    pub fn find_box(&self, id: u32) -> Option<&PanelBox> {
        self.boxes().into_iter().find(|b| b.id == id)
    }

    fn box_mut_rec(&mut self, id: u32) -> Option<&mut PanelBox> {
        match self {
            Node::Leaf(b) => (b.id == id).then_some(b),
            Node::Split { a, b, .. } => match a.box_mut_rec(id) {
                Some(found) => Some(found),
                None => b.box_mut_rec(id),
            },
        }
    }

    /// The box whose tabs hold `panel`.
    pub fn find_tab(&self, panel: &str) -> Option<u32> {
        self.boxes().into_iter().find(|b| b.tabs.iter().any(|t| t == panel)).map(|b| b.id)
    }

    /// The smallest box id no box has (never 0, never past `u32::MAX`).
    fn next_id(&self) -> u32 {
        let used: Vec<u32> = self.boxes().iter().map(|b| b.id).collect();
        free_id(&used)
    }

    /// Bring `panel` to the front of its box; false when no box holds it.
    pub fn activate(&mut self, panel: &str) -> bool {
        let Some(id) = self.find_tab(panel) else { return false };
        self.box_mut_rec(id).is_some_and(|b| match b.tabs.iter().position(|t| t == panel) {
            Some(i) => {
                b.active = i;
                true
            }
            None => false,
        })
    }

    /// Put tab `index` of box `id` in front.
    pub fn set_active(&mut self, id: u32, index: usize) {
        if let Some(b) = self.box_mut_rec(id)
            && index < b.tabs.len()
        {
            b.active = index;
        }
    }

    /// Add `panel` as the last tab of box `id`, in front. A panel already in box `id` only comes to
    /// the front; one in another box moves (atomically: it is never lost on the way).
    pub fn add_tab(&mut self, id: u32, panel: &str) -> bool {
        if self.find_tab(panel) == Some(id) {
            return self.activate(panel);
        }
        let end = self.find_box(id).map_or(0, |b| b.tabs.len());
        self.insert_tab(id, end, panel)
    }

    /// Put `panel` at `index` (clamped) among box `id`'s tabs, in front: a reorder when it is
    /// already there, else a move from its box (or a new tab). Box `id` must exist and not be the
    /// Board. The tree stays sound: one owner per panel, no empty box.
    pub fn insert_tab(&mut self, id: u32, index: usize, panel: &str) -> bool {
        if panel == BOARD || self.find_box(id).is_none_or(PanelBox::is_board) {
            return false;
        }
        self.take_tab(panel);
        let Some(b) = self.box_mut_rec(id) else { return false };
        let at = index.min(b.tabs.len());
        b.tabs.insert(at, panel.to_string());
        b.active = at;
        self.prune();
        true
    }

    /// Split box `id` along `axis`: `panel` (moved out of whichever box holds it, or new) becomes a
    /// box of its own beside it, after it (right or below) or before it (left or above), with
    /// `ratio` of the room. → the new box's id. Nothing happens (`None`) for the Board's tab, a box
    /// that doesn't exist, or a box's only tab split off itself.
    #[allow(dead_code)] // Varos: stage 3's drop on a box edge.
    pub fn split_box(&mut self, id: u32, axis: Axis, panel: &str, after: bool, ratio: f32) -> Option<u32> {
        let target = self.find_box(id)?;
        if panel == BOARD || (target.tabs.len() == 1 && target.tabs[0] == panel) {
            return None;
        }
        let new_id = self.next_id();
        self.take_tab(panel);
        let ratio = if ratio.is_finite() { ratio.clamp(*RATIO_RANGE.start(), *RATIO_RANGE.end()) } else { 0.5 };
        let fresh = Node::Leaf(PanelBox { id: new_id, tabs: vec![panel.to_string()], active: 0, collapsed: false });
        let done = self.replace_leaf(id, &mut |old| {
            if after { split(axis, 1.0 - ratio, old, fresh.clone()) } else { split(axis, ratio, fresh.clone(), old) }
        });
        self.prune();
        done.then_some(new_id)
    }

    /// Replace the leaf of box `id` by what `with` makes of it.
    fn replace_leaf(&mut self, id: u32, with: &mut dyn FnMut(Node) -> Node) -> bool {
        match self {
            Node::Leaf(b) if b.id == id => {
                let old = std::mem::replace(self, leaf(0, &[]));
                *self = with(old);
                true
            }
            Node::Leaf(_) => false,
            Node::Split { a, b, .. } => a.replace_leaf(id, with) || b.replace_leaf(id, with),
        }
    }

    /// Take `panel` out of its box, leaving an empty box in place (the caller prunes).
    fn take_tab(&mut self, panel: &str) -> bool {
        let Some(id) = self.find_tab(panel) else { return false };
        let Some(b) = self.box_mut_rec(id) else { return false };
        let Some(i) = b.tabs.iter().position(|t| t == panel) else { return false };
        b.tabs.remove(i);
        if b.active > i || b.active >= b.tabs.len() {
            b.active = b.active.saturating_sub(1);
        }
        true
    }

    /// Drop the empty boxes (their siblings take their space).
    fn prune(&mut self) {
        let tree = std::mem::replace(self, leaf(0, &[]));
        *self = tree.pruned().unwrap_or_else(Node::default_layout);
    }

    /// Take `panel` out of its box. A box left with no tabs goes and its sibling takes its space.
    /// The Board never goes. False when no box holds it.
    pub fn remove_tab(&mut self, panel: &str) -> bool {
        if panel == BOARD || !self.take_tab(panel) {
            return false;
        }
        self.prune();
        true
    }

    /// Show box `id` as its tab row only, or whole again. False for the Board, no such box, or a
    /// box with nothing above or below it to take its room (beside the Board alone, say): it
    /// would leave a hole, so it stays whole.
    pub fn toggle_collapsed(&mut self, id: u32) -> bool {
        let Some(b) = self.box_mut_rec(id).filter(|b| !b.is_board()) else { return false };
        b.collapsed = !b.collapsed;
        if b.collapsed && !self.shows_collapsed(id) {
            self.expand(id);
            return false;
        }
        true
    }

    /// Is box `id` laid out as its tab row only (collapsed, and another box takes its room).
    pub(crate) fn shows_collapsed(&self, id: u32) -> bool {
        let big = Rect::from_min_size(pos2(0.0, 0.0), vec2(8000.0, 8000.0));
        self.find_box(id).is_some_and(|b| b.collapsed) && self.layout(big).boxes.iter().any(|(b, r)| *b == id && r.height() <= ROW + 0.5)
    }

    /// Show box `id` whole (a panel in it asked for by the Window menu).
    pub fn expand(&mut self, id: u32) {
        if let Some(b) = self.box_mut_rec(id) {
            b.collapsed = false;
        }
    }

    /// The height this node needs when it is all collapsed boxes (their tab rows, stacked or side
    /// by side); `None` when some box in it is whole.
    fn collapsed_height(&self) -> Option<f32> {
        match self {
            Node::Leaf(b) => b.is_collapsed().then_some(ROW),
            Node::Split { axis, a, b, .. } => {
                let (a, b) = (a.collapsed_height()?, b.collapsed_height()?);
                Some(match axis {
                    Axis::Vertical => a + SEAM + b,
                    Axis::Horizontal => a.max(b),
                })
            }
        }
    }

    /// What must always hold: one Board, each panel once, no empty box, unique non-zero ids, every
    /// front tab in range. Checked by the tests and, in debug builds, after every drop.
    pub(crate) fn invariants(&self) -> Result<(), String> {
        let boxes = self.boxes();
        let boards = boxes.iter().filter(|b| b.is_board()).count();
        if boards != 1 {
            return Err(format!("{boards} Boards"));
        }
        let mut panels: Vec<&str> = boxes.iter().flat_map(|b| b.tabs.iter().map(String::as_str)).collect();
        let n = panels.len();
        panels.sort_unstable();
        panels.dedup();
        if panels.len() != n {
            return Err("a panel in two places".into());
        }
        let mut ids: Vec<u32> = boxes.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != boxes.len() || ids.contains(&0) {
            return Err(format!("box ids {ids:?}"));
        }
        if let Some(b) = boxes.iter().find(|b| b.tabs.is_empty() || b.active >= b.tabs.len()) {
            return Err(format!("box {} is empty or fronts no tab", b.id));
        }
        Ok(())
    }

    /// This node without its empty boxes: a split that lost one child is replaced by the other.
    fn pruned(self) -> Option<Node> {
        match self {
            Node::Leaf(b) if b.tabs.is_empty() => None,
            Node::Leaf(b) => Some(Node::Leaf(b)),
            Node::Split { axis, ratio, reset, a, b } => match (a.pruned(), b.pruned()) {
                (Some(a), Some(b)) => Some(Node::Split { axis, ratio, reset, a: Box::new(a), b: Box::new(b) }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            },
        }
    }

    /// The smallest this node can be laid out at, its seams included.
    pub fn min_size(&self) -> Vec2 {
        match self {
            Node::Leaf(b) => b.min_size(),
            Node::Split { axis, a, b, .. } => {
                let (a, b) = (a.min_size(), b.min_size());
                match axis {
                    Axis::Horizontal => vec2(a.x + SEAM + b.x, a.y.max(b.y)),
                    Axis::Vertical => vec2(a.x.max(b.x), a.y + SEAM + b.y),
                }
            }
        }
    }

    /// The split at `path`.
    pub fn split_at(&self, path: &[u8]) -> Option<&Node> {
        match (path.split_first(), self) {
            (None, Node::Split { .. }) => Some(self),
            (Some((0, rest)), Node::Split { a, .. }) => a.split_at(rest),
            (Some((1, rest)), Node::Split { b, .. }) => b.split_at(rest),
            _ => None,
        }
    }

    fn split_at_mut(&mut self, path: &[u8]) -> Option<&mut Node> {
        match path.split_first() {
            None => matches!(self, Node::Split { .. }).then_some(self),
            Some((0, rest)) => match self {
                Node::Split { a, .. } => a.split_at_mut(rest),
                Node::Leaf(_) => None,
            },
            Some((1, rest)) => match self {
                Node::Split { b, .. } => b.split_at_mut(rest),
                Node::Leaf(_) => None,
            },
            Some(_) => None,
        }
    }

    /// The split's stored ratio.
    pub fn ratio(&self, path: &[u8]) -> Option<f32> {
        match self.split_at(path)? {
            Node::Split { ratio, .. } => Some(*ratio),
            Node::Leaf(_) => None,
        }
    }

    /// `ratio` kept in the range that leaves both of the split's sides their minimum size when
    /// the split is `extent` long along its axis.
    pub fn clamp_ratio(&self, path: &[u8], ratio: f32, extent: f32) -> f32 {
        match self.split_at(path) {
            Some(Node::Split { axis, a, b, .. }) => {
                let (lo, hi) = ratio_bounds(extent - SEAM, axis.of(a.min_size()), axis.of(b.min_size()));
                if ratio.is_finite() { ratio.clamp(lo, hi) } else { lo.midpoint(hi) }
            }
            _ => ratio,
        }
    }

    /// Set the split's ratio as given (the caller clamps it).
    pub fn set_ratio(&mut self, path: &[u8], value: f32) {
        if let Some(Node::Split { ratio, .. }) = self.split_at_mut(path)
            && value.is_finite()
        {
            *ratio = value.clamp(*RATIO_RANGE.start(), *RATIO_RANGE.end());
        }
    }

    /// Put the split's default ratio back (a double-click on its divider).
    pub fn reset_ratio(&mut self, path: &[u8]) {
        if let Some(Node::Split { ratio, reset, .. }) = self.split_at_mut(path) {
            *ratio = *reset;
        }
    }

    /// How far `center` wants a horizontal split's side past its minimum, and `true` when that side
    /// is `a`. None while both sides still fit, the axis is vertical, or the short side is the Board.
    /// The layout itself stays clamped at the minimum; the hint starts at 16 pt and the release
    /// collapses only at 32 pt ([`COLLAPSE_HINT`], [`COLLAPSE_RELEASE`]).
    pub fn side_overshoot(&self, path: &[u8], center: f32, span: Rect) -> Option<(bool, f32)> {
        let Node::Split { axis, a, b, .. } = self.split_at(path)? else { return None };
        if *axis != Axis::Horizontal {
            return None;
        }
        let start = axis.of(span.min.to_vec2());
        let room = (axis.of(span.size()) - SEAM).max(0.0);
        let a_len = center - start - SEAM / 2.0;
        let a_min = axis.of(a.min_size());
        let b_min = axis.of(b.min_size());
        let past_a = a_min - a_len;
        let past_b = b_min - (room - a_len);
        if past_a > 0.0 && !a.contains_board() {
            Some((true, past_a))
        } else if past_b > 0.0 && !b.contains_board() {
            Some((false, past_b))
        } else {
            None
        }
    }

    /// Replace the split at `path` with the child that stays (`take_a` removes `a`). The removed
    /// subtree comes back; the tree is left unchanged when `path` is not a split.
    pub fn detach_child(&mut self, path: &[u8], take_a: bool) -> Option<Node> {
        fn walk(node: &mut Node, path: &[u8], take_a: bool) -> Option<Node> {
            if path.is_empty() {
                let current = std::mem::replace(node, Node::Leaf(PanelBox { id: 0, tabs: Vec::new(), active: 0, collapsed: false }));
                let Node::Split { a, b, .. } = current else {
                    *node = current;
                    return None;
                };
                if take_a {
                    *node = *b;
                    Some(*a)
                } else {
                    *node = *a;
                    Some(*b)
                }
            } else {
                match (path.first().copied(), node) {
                    (Some(0), Node::Split { a, .. }) => walk(a, &path[1..], take_a),
                    (Some(1), Node::Split { b, .. }) => walk(b, &path[1..], take_a),
                    _ => None,
                }
            }
        }
        walk(self, path, take_a)
    }

    fn contains_board(&self) -> bool {
        self.boxes().iter().any(|b| b.is_board())
    }

    /// Ordinary boxes' tabs, top to bottom (the Board is skipped).
    pub(crate) fn panel_groups(&self) -> Vec<Vec<String>> {
        let mut out = Vec::new();
        fn walk(node: &Node, out: &mut Vec<Vec<String>>) {
            match node {
                Node::Leaf(b) if !b.is_board() && !b.tabs.is_empty() => out.push(b.tabs.clone()),
                Node::Split { a, b, .. } => {
                    walk(a, out);
                    walk(b, out);
                }
                Node::Leaf(_) => {}
            }
        }
        walk(self, &mut out);
        out
    }

    /// Put `column` beside the Board (`right`: the Board stays on the left). The Board's share of
    /// that split is `board_ratio`.
    pub fn attach_beside_board(&mut self, column: Node, right: bool, board_ratio: f32) -> bool {
        fn walk(node: &mut Node, column: &mut Option<Node>, right: bool, ratio: f32) -> bool {
            match node {
                Node::Leaf(b) if b.is_board() => {
                    let board = std::mem::replace(node, Node::Leaf(PanelBox { id: 0, tabs: Vec::new(), active: 0, collapsed: false }));
                    let Some(col) = column.take() else {
                        *node = board;
                        return false;
                    };
                    let ratio = ratio.clamp(0.05, 0.95);
                    *node = if right {
                        Node::Split { axis: Axis::Horizontal, ratio, reset: ratio, a: Box::new(board), b: Box::new(col) }
                    } else {
                        let share = 1.0 - ratio;
                        Node::Split { axis: Axis::Horizontal, ratio: share, reset: share, a: Box::new(col), b: Box::new(board) }
                    };
                    true
                }
                Node::Split { a, b, .. } => walk(a, column, right, ratio) || walk(b, column, right, ratio),
                Node::Leaf(_) => false,
            }
        }
        let mut column = Some(column);
        walk(self, &mut column, right, board_ratio)
    }

    /// The split's ratio that puts its divider's centre at `center` along its axis in `span`.
    pub fn ratio_for(&self, path: &[u8], center: f32, span: Rect) -> f32 {
        let Some(Node::Split { axis, .. }) = self.split_at(path) else { return 0.5 };
        let start = axis.of(span.min.to_vec2());
        let room = axis.of(span.size()) - SEAM;
        let raw = if room > 0.0 { (center - start - SEAM / 2.0) / room } else { 0.5 };
        self.clamp_ratio(path, raw, axis.of(span.size()))
    }

    /// Lay the tree out in `rect`: each split gives its `a` its share of the room left by the seam,
    /// kept within both sides' minimum sizes; positions are whole points.
    pub fn layout(&self, rect: Rect) -> Layout {
        let mut out = Layout::default();
        self.place(rect, false, &mut Vec::new(), &mut out);
        out
    }

    /// Place this node in `rect`. `shrink`: its parent gives the room it doesn't need to a
    /// sibling, so a collapsed part of it takes only its rows; else it fills `rect` (a collapsed box
    /// with nothing to take its room shows whole rather than leave a hole).
    fn place(&self, rect: Rect, shrink: bool, path: &mut Path, out: &mut Layout) {
        match self {
            // A collapsed box is its tab row at the top of its room.
            Node::Leaf(b) if b.is_collapsed() && shrink => {
                out.boxes.push((b.id, Rect::from_min_size(rect.min, vec2(rect.width(), ROW.min(rect.height())))))
            }
            Node::Leaf(b) => out.boxes.push((b.id, rect)),
            Node::Split { axis, ratio, a, b, .. } => {
                let extent = axis.of(rect.size());
                let room = (extent - SEAM).max(0.0);
                let r = self.clamp_ratio(&[], *ratio, extent);
                // Stacked under or over collapsed boxes, the other side takes all the room but
                // their rows; both collapsed, the lower one fills unless this split shrinks too.
                // Side by side and both collapsed, the split is as tall as its taller side: only
                // that side keeps to its rows, the shorter one fills the height rather than leave a
                // hole under it (it shows whole).
                let (a_len, shrink_a, shrink_b) = match (axis, a.collapsed_height(), b.collapsed_height()) {
                    (Axis::Vertical, Some(h), None) => (h.min(room), true, false),
                    (Axis::Vertical, None, Some(h)) => ((room - h).max(0.0), false, true),
                    (Axis::Vertical, Some(h), Some(_)) => (h.min(room), true, shrink),
                    (Axis::Horizontal, Some(ha), Some(hb)) => ((room * r).round(), shrink && ha >= hb - 0.5, shrink && hb >= ha - 0.5),
                    _ => ((room * r).round(), false, false),
                };
                let (ra, seam, rb) = match axis {
                    Axis::Horizontal => {
                        let x = rect.left() + a_len;
                        (
                            Rect::from_min_max(rect.min, pos2(x, rect.bottom())),
                            Rect::from_min_max(pos2(x, rect.top()), pos2(x + SEAM, rect.bottom())),
                            Rect::from_min_max(pos2((x + SEAM).min(rect.right()), rect.top()), rect.max),
                        )
                    }
                    Axis::Vertical => {
                        let y = rect.top() + a_len;
                        (
                            Rect::from_min_max(rect.min, pos2(rect.right(), y)),
                            Rect::from_min_max(pos2(rect.left(), y), pos2(rect.right(), y + SEAM)),
                            Rect::from_min_max(pos2(rect.left(), (y + SEAM).min(rect.bottom())), rect.max),
                        )
                    }
                };
                out.dividers.push(Divider { path: path.clone(), axis: *axis, rect: seam, span: rect });
                path.push(0);
                a.place(ra, shrink_a, path, out);
                path.pop();
                path.push(1);
                b.place(rb, shrink_b, path, out);
                path.pop();
            }
        }
    }

    /// This tree made sound after loading: only tabs `known` says are panels (and the Board),
    /// each once; no empty box; unique box ids; ratios in range; the active tab in range. A tree
    /// with no Board is replaced by the default layout.
    pub fn sanitized(self, known: &dyn Fn(&str) -> bool) -> Node {
        let mut tree = self;
        let mut seen: Vec<String> = Vec::new();
        let mut board = false;
        tree.walk_mut(&mut |b| {
            let is_board = b.is_board() && !board;
            let front = b.front().map(str::to_owned);
            let tabs = std::mem::take(&mut b.tabs);
            if is_board {
                board = true;
                b.tabs = vec![BOARD.to_string()];
            } else {
                for t in tabs {
                    if t != BOARD && known(&t) && !seen.contains(&t) {
                        seen.push(t.clone());
                        b.tabs.push(t);
                    }
                }
            }
            b.active = front.and_then(|f| b.tabs.iter().position(|t| *t == f)).unwrap_or(0);
        });
        if !board {
            return Node::default_layout();
        }
        let Some(mut tree) = tree.pruned() else { return Node::default_layout() };
        // Box ids: unique and non-zero, the first box keeping a repeated one.
        let mut ids: Vec<u32> = Vec::new();
        tree.walk_mut(&mut |b| {
            if b.id == 0 || ids.contains(&b.id) {
                b.id = 0;
            } else {
                ids.push(b.id);
            }
        });
        tree.walk_mut(&mut |b| {
            if b.id == 0 {
                b.id = free_id(&ids);
                ids.push(b.id);
            }
        });
        tree.fix_ratios();
        tree
    }

    fn fix_ratios(&mut self) {
        if let Node::Split { ratio, reset, a, b, .. } = self {
            let fix = |r: f32, fallback: f32| if r.is_finite() { r.clamp(*RATIO_RANGE.start(), *RATIO_RANGE.end()) } else { fallback };
            *reset = fix(*reset, 0.5);
            *ratio = fix(*ratio, *reset);
            a.fix_ratios();
            b.fix_ratios();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known(id: &str) -> bool {
        ["layers", "artboards", "properties", "color", "swatches", "stroke", "gradient"].contains(&id)
    }

    fn historical_layout() -> Node {
        Node::board_and_column(&["layers", "artboards"], &["properties", "color", "swatches"])
    }

    fn tabs(tree: &Node) -> Vec<Vec<&str>> {
        tree.boxes().iter().map(|b| b.tabs.iter().map(String::as_str).collect()).collect()
    }

    #[test]
    fn simple_default_layout_matches_the_editor_columns() {
        let tree = Node::default_layout();
        assert_eq!(tabs(&tree), [vec!["pages"], vec!["layers", "artboards"], vec![BOARD], vec!["properties"]]);
        let layout = tree.layout(Rect::from_min_size(egui::Pos2::ZERO, vec2(1360.0, 836.0)));
        let pages = layout.rect_of(4).unwrap();
        let layers = layout.rect_of(2).unwrap();
        assert_eq!((pages.left(), pages.top(), pages.width()), (layers.left(), 0.0, layers.width()));
        assert_eq!(layers.top() - pages.bottom(), 1.0, "one hairline between Pages and Layers");
        assert!((pages.height() - 835.0 * PAGES_RATIO).abs() <= 1.0, "{pages:?}");
        assert_eq!(layers.bottom(), 836.0);
        let left = pages.union(layers);
        let board = layout.rect_of(1).unwrap();
        let right = layout.rect_of(3).unwrap();
        assert!((left.width() - 232.0).abs() <= 1.0);
        assert!((right.width() - 300.0).abs() <= 1.0, "Properties' column stops at 300: {right:?}");
        assert_eq!(left.left(), 0.0);
        assert_eq!(right.right(), 1360.0);
        assert_eq!(board.left() - left.right(), 1.0);
        assert_eq!(right.left() - board.right(), 1.0);
        assert_eq!(left.height(), 836.0);
        assert_eq!(right.height(), 836.0);
    }

    #[test]
    fn the_historical_layout_is_board_then_layers_over_properties() {
        let tree = historical_layout();
        assert_eq!(tabs(&tree), [vec![BOARD], vec!["layers", "artboards"], vec!["properties", "color", "swatches"]]);
        let Node::Split { axis, ratio, a, b, .. } = &tree else { panic!("a split") };
        assert_eq!((*axis, *ratio), (Axis::Horizontal, BOARD_RATIO));
        assert!(matches!(&**a, Node::Leaf(bx) if bx.is_board()));
        assert!(matches!(&**b, Node::Split { axis: Axis::Vertical, .. }));
        let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 900.0));
        let layout = tree.layout(rect);
        let board = layout.rect_of(1).unwrap();
        assert!((board.width() - (1400.0 - SEAM) * BOARD_RATIO).abs() < 1.0, "{board:?}");
        assert_eq!(layout.dividers.len(), 2);
        // The boxes and seams tile the rect exactly.
        let area: f32 = layout.boxes.iter().map(|(_, r)| r.area()).sum::<f32>() + layout.dividers.iter().map(|d| d.rect.area()).sum::<f32>();
        assert!((area - rect.area()).abs() < 1.0);
    }

    #[test]
    fn removing_the_last_tab_merges_the_box_away() {
        let mut tree = historical_layout();
        assert!(tree.remove_tab("layers"));
        assert!(tree.remove_tab("artboards"));
        assert_eq!(tabs(&tree), [vec![BOARD], vec!["properties", "color", "swatches"]]);
        assert!(matches!(&tree, Node::Split { axis: Axis::Horizontal, b, .. } if matches!(&**b, Node::Leaf(_))));
        for p in ["properties", "color", "swatches"] {
            assert!(tree.remove_tab(p));
        }
        assert_eq!(tabs(&tree), [vec![BOARD]]);
        assert!(!tree.remove_tab(BOARD), "the Board never goes");
        assert!(!tree.remove_tab("stroke"));
    }

    #[test]
    fn tabs_activate_add_and_remove_keep_the_front_tab() {
        let mut tree = historical_layout();
        assert!(tree.add_tab(3, "stroke"));
        assert_eq!(tree.find_box(3).unwrap().front(), Some("stroke"));
        assert!(tree.activate("color"));
        assert_eq!(tree.find_box(3).unwrap().front(), Some("color"));
        assert!(tree.remove_tab("properties"));
        assert_eq!(tree.find_box(3).unwrap().front(), Some("color"), "the front tab stays in front");
        assert!(tree.remove_tab("color"));
        assert_eq!(tree.find_box(3).unwrap().front(), Some("swatches"));
        assert!(!tree.add_tab(1, "gradient"), "the Board takes no tabs");
        // Adding a panel already in a box moves it.
        assert!(tree.add_tab(2, "stroke"));
        assert_eq!(tabs(&tree)[1], ["layers", "artboards", "stroke"]);
        assert_eq!(tabs(&tree)[2], ["swatches"]);
    }

    #[test]
    fn ratios_clamp_to_the_minimum_box_sizes() {
        let tree = historical_layout();
        let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(1000.0, 700.0));
        // Far right: the right column keeps 190 points.
        let r = tree.ratio_for(&[], 5000.0, rect);
        let layout = {
            let mut t = tree.clone();
            t.set_ratio(&[], r);
            t.layout(rect)
        };
        let right = layout.rect_of(3).unwrap();
        // Properties is in that column: 300, not a generic box.
        assert!((right.width() - 300.0).abs() < 1.0, "{right:?}");
        // Far left: the Board keeps 240.
        let r = tree.ratio_for(&[], -5000.0, rect);
        let mut t = tree.clone();
        t.set_ratio(&[], r);
        assert!((t.layout(rect).rect_of(1).unwrap().width() - BOARD_MIN.x).abs() < 1.0);
        // The vertical split keeps 120 points for each box.
        let column = t.layout(rect).rect_of(2).unwrap().union(t.layout(rect).rect_of(3).unwrap());
        let r = t.ratio_for(&[1], 0.0, column);
        t.set_ratio(&[1], r);
        assert!((t.layout(rect).rect_of(2).unwrap().height() - BOX_MIN.y).abs() < 1.0);
        // A stored ratio out of bounds is laid out within them anyway (a window made smaller).
        let mut t = tree.clone();
        t.set_ratio(&[], 0.98);
        assert!(t.layout(rect).rect_of(3).unwrap().width() >= 300.0 - 0.5);
        // A window too small for both: no negative sizes.
        let tiny = Rect::from_min_size(egui::Pos2::ZERO, vec2(200.0, 100.0));
        assert!(t.layout(tiny).boxes.iter().all(|(_, r)| r.width() >= 0.0 && r.height() >= 0.0));
        // The divider centre maps back to the same ratio.
        let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 900.0));
        let d = &tree.layout(rect).dividers[0];
        let r = tree.ratio_for(&[], d.rect.center().x, rect);
        assert!((r - BOARD_RATIO).abs() < 0.002, "{r}");
    }

    #[test]
    fn reset_puts_the_default_ratio_back() {
        let mut tree = historical_layout();
        tree.set_ratio(&[], 0.4);
        tree.set_ratio(&[1], 0.8);
        tree.reset_ratio(&[]);
        tree.reset_ratio(&[1]);
        assert_eq!(tree.ratio(&[]), Some(BOARD_RATIO));
        assert_eq!(tree.ratio(&[1]), Some(0.5));
        assert_eq!(tree.ratio(&[0]), None, "the Board is no split");
    }

    #[test]
    fn serde_round_trip_and_sanitizing() {
        let mut tree = historical_layout();
        tree.add_tab(3, "stroke");
        tree.set_ratio(&[], 0.6);
        let json = serde_json::to_value(&tree).unwrap();
        let back: Node = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(back, tree);
        assert_eq!(back.clone().sanitized(&known), tree, "a sound tree is unchanged");
        // Unknown ids are dropped, duplicates kept once, a box left empty goes, the front tab kept.
        let raw = serde_json::json!({"split": {"axis": "horizontal", "ratio": 7.0, "a": {"leaf": {"id": 1, "tabs": ["board"]}},
            "b": {"split": {"axis": "vertical", "ratio": 0.3, "a": {"leaf": {"id": 2, "tabs": ["bogus", "nope"]}},
            "b": {"leaf": {"id": 2, "tabs": ["bogus", "color", "layers", "color"], "active": 3}}}}}});
        let loaded: Node = serde_json::from_value(raw).unwrap();
        let clean = loaded.sanitized(&known);
        assert_eq!(tabs(&clean), [vec![BOARD], vec!["color", "layers"]]);
        assert_eq!(clean.find_box(2).unwrap().front(), Some("color"));
        assert!(clean.ratio(&[]).unwrap() <= 0.98);
        // No Board: the default layout.
        let raw = serde_json::json!({"leaf": {"id": 4, "tabs": ["layers"]}});
        let loaded: Node = serde_json::from_value(raw).unwrap();
        assert_eq!(loaded.sanitized(&known), Node::default_layout());
        // Duplicate box ids get fresh ones.
        let raw = serde_json::json!({"split": {"axis": "horizontal", "ratio": 0.5, "a": {"leaf": {"id": 1, "tabs": ["board"]}},
            "b": {"leaf": {"id": 1, "tabs": ["layers"]}}}});
        let loaded: Node = serde_json::from_value(raw).unwrap();
        let ids: Vec<u32> = loaded.sanitized(&known).boxes().iter().map(|b| b.id).collect();
        assert_eq!(ids, [1, 2]);
    }

    #[test]
    fn the_minimum_size_adds_up_the_seams() {
        let tree = historical_layout();
        // The right column's width is Properties' 300 (the wider of the two stacked boxes).
        assert_eq!(tree.min_size(), vec2(BOARD_MIN.x + SEAM + 300.0, (BOX_MIN.y * 2.0 + SEAM).max(BOARD_MIN.y)));
    }

    #[test]
    fn box_ids_never_overflow_and_take_the_smallest_free_one() {
        let raw = serde_json::json!({"split": {"axis": "horizontal", "ratio": 0.7, "a": {"leaf": {"id": u32::MAX, "tabs": ["board"]}},
            "b": {"leaf": {"id": u32::MAX, "tabs": ["layers"]}}}});
        let loaded: Node = serde_json::from_value(raw).unwrap();
        let mut tree = loaded.sanitized(&known);
        tree.invariants().unwrap();
        assert_eq!(tree.boxes().iter().map(|b| b.id).collect::<Vec<_>>(), [u32::MAX, 1]);
        let id = tree.split_box(1, Axis::Vertical, "color", true, 0.5).unwrap();
        assert_eq!(id, 2);
        tree.invariants().unwrap();
        // Only the Board, at u32::MAX: a new box still gets an id.
        let raw = serde_json::json!({"leaf": {"id": u32::MAX, "tabs": ["board"]}});
        let mut tree = serde_json::from_value::<Node>(raw).unwrap().sanitized(&known);
        let id = tree.split_box(u32::MAX, Axis::Horizontal, "stroke", true, BOARD_RATIO).unwrap();
        assert_eq!(id, 1);
        assert!(tree.add_tab(id, "stroke"));
        tree.invariants().unwrap();
    }

    #[test]
    fn adding_a_tab_to_its_own_box_only_brings_it_forward() {
        let mut tree = historical_layout();
        let id = tree.split_box(3, Axis::Vertical, "stroke", true, 0.4).unwrap();
        assert_eq!(tabs(&tree)[3], ["stroke"]);
        // Its only tab added to its own box: nothing lost.
        assert!(tree.add_tab(id, "stroke"));
        assert_eq!(tree.find_tab("stroke"), Some(id));
        tree.invariants().unwrap();
        // A tab already in the box with others comes forward without moving.
        assert!(tree.add_tab(3, "properties"));
        assert_eq!(tabs(&tree)[2], ["properties", "color", "swatches"]);
        assert_eq!(tree.find_box(3).unwrap().front(), Some("properties"));
        // A box's only tab moved to another box: the box goes, the panel arrives.
        assert!(tree.add_tab(2, "stroke"));
        assert_eq!(tree.find_box(id), None);
        assert_eq!(tabs(&tree)[1], ["layers", "artboards", "stroke"]);
        tree.invariants().unwrap();
    }

    #[test]
    fn insert_tab_reorders_and_moves() {
        let mut tree = historical_layout();
        assert!(tree.insert_tab(3, 0, "swatches"));
        assert_eq!(tabs(&tree)[2], ["swatches", "properties", "color"]);
        assert_eq!(tree.find_box(3).unwrap().front(), Some("swatches"));
        assert!(tree.insert_tab(3, 99, "swatches"));
        assert_eq!(tabs(&tree)[2], ["properties", "color", "swatches"]);
        assert!(tree.insert_tab(2, 1, "color"));
        assert_eq!(tabs(&tree)[1], ["layers", "color", "artboards"]);
        assert_eq!(tabs(&tree)[2], ["properties", "swatches"]);
        assert!(tree.insert_tab(3, 1, "gradient"), "a new panel");
        assert_eq!(tabs(&tree)[2], ["properties", "gradient", "swatches"]);
        assert!(!tree.insert_tab(1, 0, "stroke"), "the Board takes no tabs");
        assert!(!tree.insert_tab(42, 0, "stroke"), "no such box");
        assert!(!tree.insert_tab(3, 0, BOARD));
        tree.invariants().unwrap();
    }

    #[test]
    fn a_collapsed_box_keeps_its_row_and_its_sibling_takes_the_rest() {
        let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 900.0));
        let mut tree = historical_layout();
        assert!(tree.toggle_collapsed(2));
        let layout = tree.layout(rect);
        let (top, bottom) = (layout.rect_of(2).unwrap(), layout.rect_of(3).unwrap());
        assert_eq!(top.height(), ROW);
        assert!((bottom.top() - top.bottom() - SEAM).abs() < 0.5, "{top:?} {bottom:?}");
        assert_eq!(bottom.bottom(), rect.bottom());
        assert_eq!(top.width(), bottom.width(), "it keeps its width");
        let back: Node = serde_json::from_value(serde_json::to_value(&tree).unwrap()).unwrap();
        assert!(back.find_box(2).unwrap().collapsed, "saved");
        assert!(!tree.toggle_collapsed(1), "the Board never collapses");
        assert!(tree.toggle_collapsed(2));
        assert_eq!(tree.layout(rect), historical_layout().layout(rect), "whole again");
        // The lower box collapsed: the upper one takes the rest.
        assert!(tree.toggle_collapsed(3));
        let layout = tree.layout(rect);
        assert_eq!(layout.rect_of(3).unwrap().height(), ROW);
        assert_eq!(layout.rect_of(3).unwrap().bottom(), rect.bottom());
        assert!((layout.rect_of(3).unwrap().top() - layout.rect_of(2).unwrap().bottom() - SEAM).abs() < 0.5);
        tree.invariants().unwrap();
    }

    #[test]
    fn collapsing_never_leaves_a_hole_beside_or_under_a_box() {
        let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 900.0));
        // Beside the Board alone, nothing takes the room: the box stays whole.
        let mut tree = historical_layout();
        tree.remove_tab("layers");
        tree.remove_tab("artboards");
        assert!(!tree.toggle_collapsed(3), "nothing above or below it");
        assert!(!tree.find_box(3).unwrap().collapsed);
        assert_eq!(tree.layout(rect).rect_of(3).unwrap().height(), rect.height());
        // Nested stacks: two collapsed boxes under a whole one; it takes the room of both.
        let mut tree = historical_layout();
        let low = tree.split_box(3, Axis::Vertical, "stroke", true, 0.5).unwrap();
        assert!(tree.toggle_collapsed(3) && tree.toggle_collapsed(low));
        let layout = tree.layout(rect);
        let (whole, mid, bottom) = (layout.rect_of(2).unwrap(), layout.rect_of(3).unwrap(), layout.rect_of(low).unwrap());
        assert_eq!((mid.height(), bottom.height()), (ROW, ROW));
        assert_eq!(bottom.bottom(), rect.bottom());
        assert!((mid.top() - whole.bottom() - SEAM).abs() < 0.5 && (bottom.top() - mid.bottom() - SEAM).abs() < 0.5, "{whole:?} {mid:?} {bottom:?}");
        // Side by side over a box: one alone would leave a hole under it, so it stays whole; both
        // collapsed (a saved layout), their row is one tab row and the box below takes the rest.
        let mut tree = historical_layout();
        let right = tree.split_box(2, Axis::Horizontal, "stroke", true, 0.5).unwrap();
        assert!(!tree.toggle_collapsed(2));
        for id in [2, right] {
            tree.box_mut_rec(id).unwrap().collapsed = true;
        }
        let layout = tree.layout(rect);
        let (a, b, below) = (layout.rect_of(2).unwrap(), layout.rect_of(right).unwrap(), layout.rect_of(3).unwrap());
        assert_eq!((a.height(), b.height()), (ROW, ROW));
        assert!((below.top() - a.bottom() - SEAM).abs() < 0.5 && below.bottom() == rect.bottom(), "{below:?}");
        // A collapsed box whose neighbour goes shows whole again.
        let mut tree = historical_layout();
        assert!(tree.toggle_collapsed(2));
        for p in ["properties", "color", "swatches"] {
            tree.remove_tab(p);
        }
        assert_eq!(tree.layout(rect).rect_of(2).unwrap().height(), rect.height());
        assert!(!tree.shows_collapsed(2));
    }

    /// Stage-3 P2: an all-collapsed side-by-side subtree of unequal heights (one box beside a
    /// stacked pair) above a whole box. The pair keeps its two rows; the single box beside it
    /// fills the pair's height instead of leaving a hole under its row; the boxes and seams tile the
    /// rect exactly.
    #[test]
    fn an_all_collapsed_row_beside_a_collapsed_pair_leaves_no_hole() {
        let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 900.0));
        // Board | (box 2 | (box 4 / box 5)) / box 3.
        let mut tree = historical_layout();
        let pair = tree.split_box(2, Axis::Horizontal, "stroke", true, 0.5).unwrap();
        let low = tree.split_box(pair, Axis::Vertical, "color", true, 0.5).unwrap();
        for id in [2, pair, low] {
            tree.box_mut_rec(id).unwrap().collapsed = true;
        }
        tree.invariants().unwrap();
        let layout = tree.layout(rect);
        let (single, top, bottom, below) =
            (layout.rect_of(2).unwrap(), layout.rect_of(pair).unwrap(), layout.rect_of(low).unwrap(), layout.rect_of(3).unwrap());
        assert_eq!((top.height(), bottom.height()), (ROW, ROW), "the pair keeps its rows");
        assert_eq!(single.top(), top.top());
        assert_eq!(single.bottom(), bottom.bottom(), "no hole under the single box: {single:?} beside {top:?} / {bottom:?}");
        assert!((below.top() - bottom.bottom() - SEAM).abs() < 0.5 && below.bottom() == rect.bottom(), "{below:?}");
        let area: f32 = layout.boxes.iter().map(|(_, r)| r.area()).sum::<f32>() + layout.dividers.iter().map(|d| d.rect.area()).sum::<f32>();
        assert!((area - rect.area()).abs() < 1.0, "the boxes and seams tile the rect: {area} of {}", rect.area());
        // The single box doesn't show as its row: the gesture that would leave it so is refused.
        assert!(!tree.shows_collapsed(2));
        tree.box_mut_rec(2).unwrap().collapsed = false;
        assert!(!tree.toggle_collapsed(2), "refused, as beside the Board");
        assert!(!tree.find_box(2).unwrap().collapsed);
        // Equal heights (two single boxes) still share one row, as before.
        let mut tree = historical_layout();
        let right = tree.split_box(2, Axis::Horizontal, "stroke", true, 0.5).unwrap();
        for id in [2, right] {
            tree.box_mut_rec(id).unwrap().collapsed = true;
        }
        let layout = tree.layout(rect);
        assert_eq!((layout.rect_of(2).unwrap().height(), layout.rect_of(right).unwrap().height()), (ROW, ROW));
    }

    #[test]
    fn split_box_keeps_the_tree_sound() {
        let mut tree = historical_layout();
        // Properties split off its own box, to the left.
        let id = tree.split_box(3, Axis::Horizontal, "properties", false, 0.3).unwrap();
        tree.invariants().unwrap();
        assert_eq!(tree.find_tab("properties"), Some(id));
        let rect = Rect::from_min_size(egui::Pos2::ZERO, vec2(1400.0, 900.0));
        let layout = tree.layout(rect);
        let (new, old) = (layout.rect_of(id).unwrap(), layout.rect_of(3).unwrap());
        assert!(new.right() < old.left(), "before it: {new:?} {old:?}");
        // A box's only tab split off itself: nothing.
        assert_eq!(tree.split_box(id, Axis::Vertical, "properties", true, 0.5), None);
        // Layers moved out of box 2 below the Board: box 2 keeps Artboards.
        let below = tree.split_box(1, Axis::Vertical, "layers", true, 0.25).unwrap();
        tree.invariants().unwrap();
        let layout = tree.layout(rect);
        assert!(layout.rect_of(below).unwrap().top() > layout.rect_of(1).unwrap().bottom());
        assert_eq!(tree.find_box(2).unwrap().tabs, ["artboards"]);
        // Artboards (box 2's last tab) split beside box 3: box 2 goes.
        let last = tree.split_box(3, Axis::Vertical, "artboards", true, 0.5).unwrap();
        assert_eq!(tree.find_box(2), None);
        assert_eq!(tree.find_tab("artboards"), Some(last));
        tree.invariants().unwrap();
        assert_eq!(tree.split_box(77, Axis::Vertical, "stroke", true, 0.5), None, "no such box");
        assert_eq!(tree.split_box(1, Axis::Vertical, BOARD, true, 0.5), None);
        tree.invariants().unwrap();
    }
}
