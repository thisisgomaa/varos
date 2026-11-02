//! One measured layout supplies canvas bounds, painting, hit tests and drop targets.
use super::{
    model::{Axis, Edge, Group, HOST_CHROME, Host, Layout, Mode, Node, content_size, has_tools, partition},
    paint, registry,
};

use super::metrics::{DROP_LINE, HEADER_CONTROL, TAB_MAX_WIDTH, TAB_MIN_WIDTH};

/// Compact panel rows sit this far inside their column on each side.
const COMPACT_ROW_INSET: f32 = 3.0;
use egui::{Rect, pos2, vec2};

/// Every tab's width, from its measured, localized label. The presenter measures them with the UI
/// fonts (`paint::tab_widths`); the model only reads them and never estimates text. A panel
/// that was not measured (a layout checked without fonts) gets the narrowest tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TabWidths(pub(super) std::collections::BTreeMap<String, f32>);

impl TabWidths {
    fn width(&self, panel: &str) -> f32 {
        self.0.get(panel).copied().unwrap_or(TAB_MIN_WIDTH).clamp(TAB_MIN_WIDTH, TAB_MAX_WIDTH)
    }
}

/// What one measurement needs besides the layout: the tab widths, and whether the host is a
/// compact column's flyout, whose group headers also keep a close button.
#[derive(Clone, Copy)]
struct Chrome<'a> {
    tabs: &'a TabWidths,
    flyout: bool,
}

#[derive(Clone, Debug)]
pub struct GroupRect {
    pub id: u64,
    pub mode: Mode,
    pub column: u64,
    pub rect: Rect,
    pub tabs: Vec<(String, Rect)>,
    pub overflow: Vec<String>,
    pub body: Rect,
    /// The strip that drags the group: a compact group's top strip, a special bar's top strip, or
    /// a full group's header between its last tab and its controls.
    pub grip: Rect,
    /// A full group's header controls, measured with its tabs, right to left: the panel menu,
    /// a flyout's close button, and the tab overflow menu while the tabs don't fit.
    pub menu: Option<Rect>,
    pub close: Option<Rect>,
    pub overflow_button: Option<Rect>,
}
#[derive(Clone, Debug)]
pub struct DividerRect {
    pub id: u64,
    pub axis: Axis,
    pub rect: Rect,
    pub parent: Rect,
    pub min_ratio: f32,
    pub max_ratio: f32,
}
#[derive(Clone, Debug)]
pub struct ColumnRect {
    pub key: u64,
    pub rect: Rect,
    pub mode: Mode,
}
#[derive(Clone, Debug)]
pub struct HostRect {
    pub id: u64,
    pub rect: Rect,
    pub groups: Vec<GroupRect>,
    pub columns: Vec<ColumnRect>,
    pub dividers: Vec<DividerRect>,
}

pub fn measure(layout: &Layout, region: Rect, tabs: &TabWidths) -> Vec<HostRect> {
    arrange(layout, region, tabs).0
}

/// Docked hosts reserve space; floating hosts always use the entire workspace coordinate system.
pub fn arrange(layout: &Layout, region: Rect, tabs: &TabWidths) -> (Vec<HostRect>, Rect) {
    let chrome = Chrome { tabs, flyout: false };
    let mut board = region;
    let mut docks = std::collections::BTreeMap::new();
    for edge in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
        // Raising a window changes z-order only, never the position of another dock.
        let mut hosts: Vec<_> = layout.hosts.iter().filter(|h| !h.hidden && h.dock == Some(edge)).collect();
        hosts.sort_by_key(|h| h.id);
        for host in hosts {
            let mut rect = board;
            if matches!(edge, Edge::Left | Edge::Right) {
                let wanted = if has_overrides(host) {
                    visible_size(host)[0]
                } else if host.mode == Mode::Full {
                    let minimum = if has_tools(&host.root) {
                        tools_minimum_width(&host.root, host.expanded[0] - HOST_CHROME[0]) + HOST_CHROME[0]
                    } else {
                        minimum_width(&host.root)
                    };
                    host.expanded[0].max(minimum)
                } else {
                    host.compact_width
                };
                let width = wanted.min((board.width() - 160.0).max(24.0)).min(board.width()).max(0.0);
                if edge == Edge::Left {
                    rect.max.x = rect.left() + width;
                    board.min.x = rect.right();
                } else {
                    rect.min.x = rect.right() - width;
                    board.max.x = rect.left();
                }
            } else {
                let wanted = if has_overrides(host) {
                    visible_size(host)[1] - if host.dock.is_some() { paint::FOOT } else { 0.0 }
                } else if host.mode == Mode::Full {
                    let fitted = if is_control(host) { Some(control_height(host)) } else { None };
                    fitted.unwrap_or_else(|| host.expanded[1].max(minimum_height(&host.root)) - docked_bar_trim(host))
                } else {
                    compact_height(host)
                };
                let height = wanted.min((board.height() - 120.0).max(24.0)).min(board.height()).max(0.0);
                if edge == Edge::Top {
                    rect.max.y = rect.top() + height;
                    board.min.y = rect.bottom();
                } else {
                    rect.min.y = rect.bottom() - height;
                    board.max.y = rect.top();
                }
            }
            docks.insert(host.id, rect);
        }
    }
    let hosts = layout
        .hosts
        .iter()
        .filter(|h| !h.hidden)
        .map(|h| docks.get(&h.id).map_or_else(|| measure_host(h, region, tabs), |r| in_rect(h, *r, chrome)))
        .collect();
    let mut hosts: Vec<HostRect> = hosts;
    hosts.sort_by_key(|h| layout.host(h.id).is_some_and(|h| h.dock.is_none()));
    (hosts, board)
}

fn compact_height(host: &Host) -> f32 {
    paint::TITLE + footer(host) + HOST_CHROME[0] + super::model::groups(&host.root).iter().map(|g| compact_group_height(g)).sum::<f32>()
}

/// A compact group: its grip strip and one row per panel.
fn compact_group_height(group: &Group) -> f32 {
    paint::GRIP + 2.0 * super::metrics::COMPACT_PAD + group.panels.len() as f32 * paint::ROW
}

/// Stack `groups` as compact rows from the top of `rect`, all in column `column`.
fn push_compact(groups: &[&Group], rect: Rect, column: u64, mode: Mode, out: &mut Vec<GroupRect>) {
    let mut y = rect.top();
    for group in groups {
        let r = Rect::from_min_size(pos2(rect.left(), y), vec2(rect.width(), compact_group_height(group)));
        let tabs = group
            .panels
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let row = r.min + vec2(COMPACT_ROW_INSET, paint::GRIP + super::metrics::COMPACT_PAD + i as f32 * paint::ROW);
                (p.clone(), Rect::from_min_size(row, vec2((r.width() - 2.0 * COMPACT_ROW_INSET).max(0.0), paint::ROW)))
            })
            .collect();
        let grip = Rect::from_min_size(r.min, vec2(r.width(), paint::GRIP));
        out.push(GroupRect {
            id: group.id,
            mode,
            column,
            rect: r,
            tabs,
            overflow: Vec::new(),
            body: Rect::NOTHING,
            grip,
            menu: None,
            close: None,
            overflow_button: None,
        });
        y += r.height();
    }
}

/// Control has inline dragging and no generic host title, in either placement.
pub fn is_control(host: &Host) -> bool {
    super::model::standalone_control(&host.root)
}

pub fn title_height(host: &Host) -> f32 {
    if is_control(host) { 0.0 } else { paint::TITLE }
}

fn control_height(host: &Host) -> f32 {
    host.dock_content_height.map_or(super::metrics::CONTROL_INITIAL_HEIGHT, |height| height + 2.0 * super::metrics::BORDER)
}

pub fn control_grip(rect: Rect) -> Rect {
    Rect::from_min_max(
        rect.min + vec2(super::metrics::BORDER, super::metrics::BORDER),
        pos2((rect.left() + super::metrics::BORDER + super::metrics::CONTROL_GRIP).min(rect.right()), rect.bottom() - super::metrics::BORDER),
    )
}

/// An edge-integrated host has no floating bottom trim.
pub fn footer(host: &Host) -> f32 {
    if is_control(host) {
        super::metrics::BORDER
    } else if host.dock.is_some() {
        0.0
    } else {
        paint::FOOT
    }
}

/// A standalone docked bar is already identified by its tools. Groups and floating bars
/// retain normal tabs so they can be selected, rearranged and detached individually.
fn standalone_docked_bar(host: &Host) -> bool {
    host.dock.is_some()
        && matches!(&host.root, Node::Group(g) if g.panels.len() == 1
            && registry::get(&g.active).is_some_and(|s| s.kind != registry::PanelKind::Standard))
}

/// Keep the remembered floating size while removing its unused tab/footer space at an edge.
pub fn docked_bar_trim(host: &Host) -> f32 {
    if host.mode == Mode::Full && standalone_docked_bar(host) { paint::TABS + paint::FOOT } else { 0.0 }
}

pub fn measure_host(host: &Host, region: Rect, tabs: &TabWidths) -> HostRect {
    place_host(host, region, Chrome { tabs, flyout: false })
}

/// [`measure_host`] for a compact column's flyout (a temporary full host): its group headers also
/// keep the flyout's close button, and its tabs are measured with that reserve.
pub fn measure_flyout(host: &Host, region: Rect, tabs: &TabWidths) -> HostRect {
    place_host(host, region, Chrome { tabs, flyout: true })
}

fn place_host(host: &Host, region: Rect, chrome: Chrome) -> HostRect {
    let height = if has_overrides(host) && !is_control(host) {
        visible_size(host)[1]
    } else if is_control(host) {
        control_height(host)
    } else if host.mode == Mode::Full {
        host.expanded[1]
    } else {
        compact_height(host)
    };
    let width = if has_overrides(host) {
        visible_size(host)[0]
    } else if host.mode == Mode::Full {
        host.expanded[0]
    } else {
        host.compact_width
    };
    let size = vec2(width.min(region.width()).max(1.0), height.min(region.height()).max(1.0));
    let min = region.min + vec2(host.position[0], host.position[1]);
    let min = pos2(
        min.x.clamp(region.left(), (region.right() - size.x).max(region.left())),
        min.y.clamp(region.top(), (region.bottom() - size.y).max(region.top())),
    );
    in_rect(host, Rect::from_min_size(min, size), chrome)
}

fn in_rect(host: &Host, rect: Rect, chrome: Chrome) -> HostRect {
    let control = is_control(host);
    let inset = if control { vec2(1.0 + super::metrics::CONTROL_GRIP, 1.0) } else { vec2(1.0, title_height(host)) };
    let inner = Rect::from_min_max(rect.min + inset, rect.max - vec2(1.0, footer(host)));
    let mut out = HostRect { id: host.id, rect, groups: Vec::new(), columns: Vec::new(), dividers: Vec::new() };
    if has_overrides(host) {
        column_tree(host, &host.root, inner, content_size(host.expanded), chrome, &mut out);
    } else if host.mode == Mode::Full {
        tree(&host.root, inner, content_size(host.expanded), chrome, &mut out.groups, &mut out.dividers);
    } else {
        let groups = super::model::groups(&host.root);
        let column = groups.first().map_or(0, |g| g.id);
        push_compact(&groups, inner, column, host.mode, &mut out.groups);
    }
    if out.columns.is_empty() {
        if host.mode != Mode::Full && !has_overrides(host) {
            if let Some(first) = out.groups.first() {
                let viewport = Rect::from_min_max(rect.min, pos2(rect.right(), (rect.bottom() - footer(host)).max(rect.top())));
                out.columns.push(ColumnRect { key: first.column, rect: viewport, mode: host.mode });
            }
        } else {
            for group in &mut out.groups {
                if let Some(column) = super::model::column_node(&host.root, group.id) {
                    let key = super::model::groups(column).first().map_or(group.id, |g| g.id);
                    group.column = key;
                    if let Some(existing) = out.columns.iter_mut().find(|c| c.key == key) {
                        existing.rect = existing.rect.union(group.rect);
                    } else {
                        out.columns.push(ColumnRect { key, rect: group.rect, mode: group.mode });
                    }
                }
            }
        }
    }
    for column in &mut out.columns {
        column.rect.min.y = rect.top();
    }
    // Special bars never have a name tab or panel menu, floating or docked.
    {
        for group in &mut out.groups {
            if group.mode == Mode::Full
                && super::model::groups(&host.root).iter().any(|g| {
                    g.id == group.id && g.panels.len() == 1 && registry::get(&g.active).is_some_and(|spec| spec.kind != registry::PanelKind::Standard)
                })
            {
                group.tabs.clear();
                group.overflow.clear();
                group.body = group.rect;
                if matches!(host.root, Node::Split { .. }) {
                    group.body.min.y = (group.body.top() + paint::GRIP).min(group.body.bottom());
                }
                group.grip = Rect::from_min_max(group.rect.min, pos2(group.rect.right(), group.body.top()));
                (group.menu, group.close, group.overflow_button) = (None, None, None);
            }
        }
    }
    out
}

fn has_overrides(host: &Host) -> bool {
    super::model::groups(&host.root).iter().any(|g| g.presentation.is_some())
}

fn projected_height(host: &Host, node: &Node, saved: [f32; 2]) -> f32 {
    if !super::model::has_horizontal(node) {
        let groups = super::model::groups(node);
        let mode = groups.first().map_or(host.mode, |g| super::model::presentation(host, g).mode);
        return if mode == Mode::Full { saved[1] } else { groups.iter().map(|g| compact_group_height(g)).sum() };
    }
    match node {
        Node::Group(_) => saved[1],
        Node::Split { axis, ratio, first, second, .. } => {
            let (a, b) = partition(saved, *axis, *ratio);
            let a = projected_height(host, first, a);
            let b = projected_height(host, second, b);
            if *axis == Axis::Horizontal { a.max(b) } else { a + b + super::model::SPLIT_GAP }
        }
    }
}

fn visible_size(host: &Host) -> [f32; 2] {
    let saved = content_size(host.expanded);
    [super::model::displayed_width(host, &host.root, saved) + HOST_CHROME[0], projected_height(host, &host.root, saved) + paint::TITLE + paint::FOOT]
}

fn column_tree(host: &Host, node: &Node, rect: Rect, saved: [f32; 2], chrome: Chrome, out: &mut HostRect) {
    // Expanded subtrees retain the established content minimums and Tools bounds.
    // Presentation overrides must not change their divider limits after re-expansion.
    if super::model::groups(node).iter().all(|g| super::model::presentation(host, g).mode == Mode::Full) {
        let before = out.groups.len();
        tree(node, rect, saved, chrome, &mut out.groups, &mut out.dividers);
        for group in out.groups.iter_mut().skip(before) {
            let Some(column) = super::model::column_node(node, group.id) else { continue };
            let Some(first) = super::model::groups(column).first().copied() else { continue };
            group.column = first.id;
            if let Some(existing) = out.columns.iter_mut().find(|c| c.key == first.id) {
                existing.rect = existing.rect.union(group.rect);
            } else {
                out.columns.push(ColumnRect { key: first.id, rect: group.rect, mode: Mode::Full });
            }
        }
        return;
    }
    if !super::model::has_horizontal(node) {
        let groups = super::model::groups(node);
        let Some(first) = groups.first() else { return };
        let p = super::model::presentation(host, first);
        let key = first.id;
        out.columns.push(ColumnRect { key, rect, mode: p.mode });
        if p.mode == Mode::Full {
            let before = out.groups.len();
            tree(node, rect, saved, chrome, &mut out.groups, &mut out.dividers);
            for g in out.groups.iter_mut().skip(before) {
                g.column = key;
                g.mode = Mode::Full;
            }
        } else {
            push_compact(&groups, rect, key, p.mode, &mut out.groups);
        }
        return;
    }
    if let Node::Split { id, axis, ratio, first, second } = node {
        let d = if *axis == Axis::Horizontal { 0 } else { 1 };
        let (sa, sb) = partition(saved, *axis, *ratio);
        let available = (if d == 0 { rect.width() } else { rect.height() } - super::model::SPLIT_GAP).max(0.0);
        let desired_a = if d == 0 { super::model::displayed_width(host, first, sa) } else { projected_height(host, first, sa) };
        let desired_b = if d == 0 { super::model::displayed_width(host, second, sb) } else { projected_height(host, second, sb) };
        let first_compact = super::model::groups(first).iter().all(|g| super::model::presentation(host, g).mode != Mode::Full);
        let second_compact = super::model::groups(second).iter().all(|g| super::model::presentation(host, g).mode != Mode::Full);
        let extent = if d == 0 && second_compact && !first_compact {
            (available - desired_b).max(0.0)
        } else if d == 1 {
            (available * ratio).clamp(0.0, available)
        } else {
            desired_a.min(available)
        };
        let mut a = rect;
        let mut b = rect;
        let mut divider = rect;
        if d == 0 {
            a.max.x = rect.left() + extent;
            b.min.x = a.right() + super::model::SPLIT_GAP;
            divider.min.x = a.right();
            divider.max.x = b.left();
        } else {
            a.max.y = rect.top() + extent;
            b.min.y = a.bottom() + super::model::SPLIT_GAP;
            divider.min.y = a.bottom();
            divider.max.y = b.top();
        }
        out.dividers.push(DividerRect { id: *id, axis: *axis, rect: divider, parent: rect, min_ratio: 0.05, max_ratio: 0.95 });
        column_tree(host, first, a, sa, chrome, out);
        column_tree(host, second, b, sb, chrome, out);
    }
}

fn tree(node: &Node, rect: Rect, saved: [f32; 2], chrome: Chrome, out: &mut Vec<GroupRect>, dividers: &mut Vec<DividerRect>) {
    match node {
        Node::Group(group) => {
            let tab_width = |panel: &str| chrome.tabs.width(panel);
            // The header's controls keep their slots, right to left: the panel menu, a flyout's
            // close button and, with several tabs, the drag strip, which the tab overflow menu
            // joins when the tabs don't fit.
            let several = group.panels.len() > 1;
            let controls = 1.0 + f32::from(u8::from(chrome.flyout));
            let available = (rect.width() - (controls + f32::from(u8::from(several))) * HEADER_CONTROL).max(1.0);
            let total: f32 = group.panels.iter().map(|p| tab_width(p)).sum();
            let overflowed = several && total > available;
            let available = (available - if overflowed { HEADER_CONTROL } else { 0.0 }).max(1.0);
            let active = group.panels.iter().position(|p| p == &group.active).unwrap_or(0);
            let mut start = 0;
            while start < active && group.panels.iter().skip(start).take(active - start + 1).map(|p| tab_width(p)).sum::<f32>() > available {
                start += 1;
            }
            let mut x = rect.left();
            let mut tabs = Vec::new();
            for p in group.panels.iter().skip(start) {
                let width = tab_width(p).min(available);
                if x + width > rect.left() + available + 0.1 && !tabs.is_empty() {
                    break;
                }
                tabs.push((p.clone(), Rect::from_min_size(pos2(x, rect.top()), vec2(width, paint::TABS))));
                x += width;
            }
            let slot = |from_right: f32| {
                Rect::from_min_size(pos2(rect.right() - from_right * HEADER_CONTROL, rect.top()), vec2(HEADER_CONTROL, paint::TABS))
            };
            let controls = controls + f32::from(u8::from(overflowed));
            let grip = Rect::from_min_max(
                pos2(tabs.last().map_or(rect.left(), |(_, r)| r.right()), rect.top()),
                pos2(rect.right() - controls * HEADER_CONTROL, rect.top() + paint::TABS),
            );
            out.push(GroupRect {
                id: group.id,
                mode: Mode::Full,
                column: group.id,
                rect,
                tabs,
                overflow: if overflowed { group.panels.clone() } else { Vec::new() },
                body: Rect::from_min_max(rect.min + vec2(0.0, paint::TABS), rect.max),
                grip,
                menu: Some(slot(1.0)),
                close: chrome.flyout.then(|| slot(2.0)),
                overflow_button: overflowed.then(|| slot(controls)),
            });
        }
        Node::Split { id, axis, ratio, first, second } => {
            let dimension = if *axis == Axis::Horizontal { 0 } else { 1 };
            let total = if dimension == 0 { rect.width() } else { rect.height() };
            let available = (total - super::model::SPLIT_GAP).max(1.0);
            let a_min = content_min(first)[dimension];
            let b_min = content_min(second)[dimension];
            let tools_split = *axis == Axis::Horizontal && has_tools(node);
            let (saved_a, saved_b) = partition(saved, *axis, *ratio);
            let (min_ratio, max_ratio, actual) = if tools_split {
                // A larger dock or a viewport constraint must not scale the Tools leaf.
                // Extra space goes to ordinary groups; nested Tools branches retain their
                // remembered width until their own children are measured below.
                let first_tools = has_tools(first);
                let wanted = if first_tools { saved_a[0] } else { available - saved_b[0] };
                let lower = a_min.max(available - maximum_width(second)).clamp(0.0, available);
                let upper = maximum_width(first).min(available - b_min).clamp(0.0, available);
                let extent = if matches!(first.as_ref(), Node::Group(_)) && first_tools {
                    saved_a[0].clamp(0.0, available)
                } else if matches!(second.as_ref(), Node::Group(_)) && has_tools(second) {
                    (available - saved_b[0]).clamp(0.0, available)
                } else if lower <= upper {
                    wanted.clamp(lower, upper)
                } else if first_tools {
                    saved_a[0].min(available)
                } else {
                    (available - saved_b[0]).max(0.0)
                };
                (lower / available, upper / available, extent / available)
            } else {
                let min = (a_min / available).clamp(0.05, 0.95);
                let max = (1.0 - b_min / available).clamp(0.05, 0.95);
                let actual = if min <= max { ratio.clamp(min, max) } else { (a_min / (a_min + b_min)).clamp(0.05, 0.95) };
                (min, max, actual)
            };
            let mut a = rect;
            let mut b = rect;
            let mut divider = rect;
            if dimension == 0 {
                let x = rect.left() + available * actual;
                a.max.x = x;
                b.min.x = x + super::model::SPLIT_GAP;
                divider.min.x = x;
                divider.max.x = b.min.x;
            } else {
                let y = rect.top() + available * actual;
                a.max.y = y;
                b.min.y = y + super::model::SPLIT_GAP;
                divider.min.y = y;
                divider.max.y = b.min.y;
            }
            dividers.push(DividerRect {
                id: *id,
                axis: *axis,
                rect: divider,
                parent: rect,
                min_ratio: if min_ratio <= max_ratio { min_ratio } else { 0.05 },
                max_ratio: if min_ratio <= max_ratio { max_ratio } else { 0.95 },
            });
            tree(first, a, saved_a, chrome, out, dividers);
            tree(second, b, saved_b, chrome, out, dividers);
        }
    }
}

pub fn edge_line(rect: Rect, edge: Edge) -> Rect {
    match edge {
        Edge::Left => Rect::from_min_size(rect.min, vec2(DROP_LINE, rect.height())),
        Edge::Right => Rect::from_min_size(pos2(rect.right() - DROP_LINE, rect.top()), vec2(DROP_LINE, rect.height())),
        Edge::Top => Rect::from_min_size(rect.min, vec2(rect.width(), DROP_LINE)),
        Edge::Bottom => Rect::from_min_size(pos2(rect.left(), rect.bottom() - DROP_LINE), vec2(rect.width(), DROP_LINE)),
    }
}
fn content_min(node: &Node) -> [f32; 2] {
    match node {
        Node::Group(g) => g
            .panels
            .iter()
            .filter_map(|p| registry::get(p))
            .fold([1.0, 1.0], |a, s| [(s.min_size[0] - HOST_CHROME[0]).max(a[0]), (s.min_size[1] - HOST_CHROME[1]).max(a[1])]),
        Node::Split { axis, ratio, first, second, .. } => {
            let a = content_min(first);
            let b = content_min(second);
            let mut size = [a[0].max(b[0]), a[1].max(b[1])];
            let d = if *axis == Axis::Horizontal { 0 } else { 1 };
            size[d] = if has_tools(node) {
                a[d] + b[d] + super::model::SPLIT_GAP
            } else {
                (a[d] / ratio.max(0.05)).max(b[d] / (1.0 - ratio).max(0.05)) + super::model::SPLIT_GAP
            };
            size
        }
    }
}
/// A dock must reserve the selected Tools width as well as ordinary body minima.
fn tools_minimum_width(node: &Node, saved_width: f32) -> f32 {
    if !has_tools(node) {
        return content_min(node)[0];
    }
    match node {
        Node::Group(_) => saved_width.clamp(super::metrics::TOOLS_MIN_WIDTH - HOST_CHROME[0], super::metrics::TOOLS_MAX_WIDTH - HOST_CHROME[0]),
        Node::Split { ratio, first, second, .. } => {
            let available = (saved_width - super::model::SPLIT_GAP).max(0.0);
            let a = available * ratio;
            tools_minimum_width(first, a) + tools_minimum_width(second, available - a) + super::model::SPLIT_GAP
        }
    }
}

fn maximum_width(node: &Node) -> f32 {
    if matches!(node, Node::Group(_)) && has_tools(node) { super::metrics::TOOLS_MAX_WIDTH - HOST_CHROME[0] } else { f32::INFINITY }
}

pub fn minimum_width(node: &Node) -> f32 {
    content_min(node)[0] + HOST_CHROME[0]
}
pub fn minimum_height(node: &Node) -> f32 {
    content_min(node)[1] + HOST_CHROME[1]
}

/// Scrolling compact groups changes only measured rectangles, never ownership or saved positions.
pub fn scroll_compact(measured: &mut [HostRect], _layout: &Layout, offsets: &std::collections::BTreeMap<u64, f32>) {
    for host in measured {
        for column in &host.columns {
            if column.mode == Mode::Full {
                continue;
            }
            let max = column_scroll_max(host, column);
            let offset = offsets
                .get(&column.key)
                .or_else(|| (host.columns.len() == 1).then(|| offsets.get(&host.id)).flatten())
                .copied()
                .unwrap_or(0.0)
                .clamp(0.0, max);
            for group in host.groups.iter_mut().filter(|g| g.column == column.key) {
                group.rect = group.rect.translate(vec2(0.0, -offset));
                group.grip = group.grip.translate(vec2(0.0, -offset));
                for (_, rect) in &mut group.tabs {
                    *rect = rect.translate(vec2(0.0, -offset));
                }
            }
        }
    }
}
pub fn compact_column_scroll_max(host: &HostRect, key: u64) -> f32 {
    host.columns.iter().find(|c| c.key == key && c.mode != Mode::Full).map_or(0.0, |column| column_scroll_max(host, column))
}

/// How far a compact column's rows reach past its viewport below the title.
fn column_scroll_max(host: &HostRect, column: &ColumnRect) -> f32 {
    let total: f32 = host.groups.iter().filter(|g| g.column == column.key).map(|g| g.rect.height()).sum();
    (total - (column.rect.height() - paint::TITLE)).max(0.0)
}
