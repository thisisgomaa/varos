//! Platform window chrome — pure data + logic, GPU-free and tested (docs/foundation/MAC_CHROME.md).
//!
//! * `TOPBAR`: how our own top bar sits in the window on this platform (left inset for the macOS
//!   traffic lights, whether we paint our own ─ ☐ ✕ caps).
//! * `caption_hit`: is a physical-px point on the EMPTY part of the bar (the drag band)? Its
//!   exclusions come from ONE list, `TopbarLayout::interactive_rects` — the same rects the bar's
//!   controls and tab chips are hit-tested with — on Windows (`WM_NCHITTEST`) and macOS alike.
//! * `menus()`: the native macOS menu bar as a table — since slice 0.6 it lives in `crate::menus`
//!   (one file per menu) and is re-exported here, so `chrome::menus()` & co. keep their paths.
#![cfg_attr(not(target_os = "macos"), allow(dead_code))] // the menu table is only built on macOS

/// How the top bar fits the window chrome on one platform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TopbarChrome {
    /// Logical height: the 4b band on macOS (52 = 12 + 28 + 12, MAC_CHROME.md §A′).
    pub height: f32,
    /// Logical px before the burger cell (macOS: room for the native traffic lights).
    pub lead: f32,
    /// Logical px between the right-most bar control and the window edge when there are no caps.
    pub right_inset: f32,
    /// Do we paint our own minimize / maximize / close caps (Windows' stripped caption)?
    pub window_caps: bool,
}

/// The table: macOS keeps the native traffic lights (so no caps of ours, and a left inset that clears
/// them — they end ≈ 70 pt from the left); every other platform keeps the original Windows numbers.
pub const fn topbar_chrome(macos: bool) -> TopbarChrome {
    if macos {
        TopbarChrome { height: varos_app::shell::tokens::BAND_H, lead: 78.0, right_inset: 12.0, window_caps: false }
    } else {
        TopbarChrome { height: 46.0, lead: 4.0, right_inset: 0.0, window_caps: true }
    }
}

/// This build's top-bar chrome.
pub const TOPBAR: TopbarChrome = topbar_chrome(cfg!(target_os = "macos"));

/// Logical px between two neighbouring tab chips — the resting layout (`topbar_layout`) and the live
/// drag reflow (`tab_drag_frame`) both space chips with this one number (4b: 2).
pub const TAB_GAP: f32 = 2.0;

/// The actual rectangles painted / hit-tested by the top bar (4b, MAC_CHROME.md §A′). Text widths
/// come from egui's font measurement; all padding, vertical alignment and tab fitting live here.
pub struct TopbarLayout {
    pub caps: Option<[egui::Rect; 3]>,
    /// macOS: the Home chip (28×28 at the traffic-light lead). Windows: the burger cell.
    pub menu: egui::Rect,
    /// The V mark — a 28×28 square at the right zone's right edge. The rest of the right zone, left
    /// of it, is empty band (owner 2026-10-06: no Search there): a press on it drags the window.
    pub brand: egui::Rect,
    /// `(original tab index, its chip rect)`, left → right. Not always a `0..n` prefix: when the
    /// strip overflows, the greedy fit stops early and the ACTIVE tab (spec §4 "Active document name
    /// always matches canvas/layers") takes the last visible slot even if that means displacing
    /// whichever tab the greedy pass had put there (DFS S1 F7).
    pub tabs: Vec<(usize, egui::Rect)>,
    /// The "+N ⌄" button — only when some tabs are not drawn. It lists `hidden`.
    pub overflow: Option<egui::Rect>,
    /// Every tab index NOT drawn as a chip, in tab order.
    pub hidden: Vec<usize>,
    /// The `+` new-document chip — reserved BEFORE tabs are fitted, so it is always placed (F15):
    /// only an unreasonably narrow window ever leaves this `None`.
    pub plus: Option<egui::Rect>,
}

/// A tab chip's width for a name `text_width` wide (measured at Inter 500 12 for EVERY tab, so a
/// chip never changes width when it becomes active).
pub fn tab_width(text_width: f32) -> f32 {
    use varos_app::shell::tokens as t;
    (t::TAB_PAD_L + text_width + t::TAB_TRAIL).clamp(t::TAB_W_MIN, t::TAB_W_MAX)
}

/// The right zone (the V mark at its right edge, empty band before it): the panel column's x-span
/// when one is docked (`right_zone`, from the box tree, last frame), else `BAND_RIGHT_ZONE_W` wide
/// ending `SEAM_GAP` before the right edge (before Windows' caps). Never past the caps / window edge.
/// The tabs stop `BAND_TABS_END_GAP` before it.
pub fn right_zone(
    bar: egui::Rect,
    chrome: TopbarChrome,
    caps_left: Option<f32>,
    column: Option<egui::Rangef>,
) -> egui::Rangef {
    use varos_app::shell::tokens as t;
    let edge = caps_left.unwrap_or(bar.right());
    match column.map(|c| egui::Rangef::new(c.min, c.max.min(edge))).filter(|c| c.span() > 0.0) {
        Some(c) => c,
        None => {
            let right = caps_left.map_or(bar.right() - chrome.right_inset, |l| l - t::SEAM_GAP);
            egui::Rangef::new(right - t::BAND_RIGHT_ZONE_W, right)
        }
    }
}

/// The band's layout. `right_zone` = the panel column's x-span (None on Home / with no right
/// column); `tab_text_widths` = every tab name measured at Inter 500 12.
pub fn topbar_layout(
    bar: egui::Rect,
    chrome: TopbarChrome,
    right_zone: Option<egui::Rangef>,
    tab_text_widths: &[f32],
    active_tab: Option<usize>,
) -> TopbarLayout {
    use egui::{pos2, vec2, Rect};
    use varos_app::shell::tokens as t;
    let cy = bar.center().y;
    let chip = |x: f32, w: f32| Rect::from_min_size(pos2(x, cy - t::BAND_CHIP_H / 2.0), vec2(w, t::BAND_CHIP_H));
    let caps = chrome.window_caps.then(|| {
        [3.0, 2.0, 1.0].map(|i| {
            Rect::from_min_max(
                pos2(bar.right() - i * 42.0, bar.top()),
                pos2(bar.right() - (i - 1.0) * 42.0, bar.bottom()),
            )
        })
    });
    let zone = self::right_zone(bar, chrome, caps.map(|c| c[0].left()), right_zone);
    let brand = chip(zone.max - t::BAND_BRAND, t::BAND_BRAND);
    // macOS: the Home chip on the band's centre line; Windows keeps its full-height burger cell
    let menu = if chrome.window_caps {
        Rect::from_min_size(pos2(bar.left() + chrome.lead, bar.top()), vec2(36.0, bar.height()))
    } else {
        chip(bar.left() + chrome.lead, t::BAND_CHIP_H)
    };
    let start_x = menu.right() + t::BAND_GAP;
    // the tabs end before the right zone (and never closer than BAND_GAP to the V mark)
    let tabs_right = zone.min.min(brand.left() - t::BAND_GAP) - t::BAND_TABS_END_GAP;
    let plus_w = t::BAND_CHIP_H;
    // reserve the `+` chip (and its gap) BEFORE fitting tabs (F15: it must never be starved out)
    let mut fit_right = tabs_right - t::BAND_OVERFLOW_GAP - plus_w;
    let widths: Vec<f32> = tab_text_widths.iter().map(|&w| tab_width(w)).collect();
    let row = |v: &[usize]| v.iter().map(|&i| widths[i] + TAB_GAP).sum::<f32>() - TAB_GAP;
    let all: Vec<usize> = (0..widths.len()).collect();
    let mut drawn: Vec<usize> = Vec::new();
    if all.is_empty() || start_x + row(&all) <= fit_right {
        drawn = all;
    } else {
        // overflow: the "+N ⌄" button needs its room too
        fit_right -= t::BAND_OVERFLOW_W + t::BAND_OVERFLOW_GAP;
        let mut x = start_x;
        for (i, &w) in widths.iter().enumerate() {
            if x + w > fit_right {
                break;
            }
            drawn.push(i);
            x += w + TAB_GAP;
        }
        // F7: the active tab is ALWAYS drawn — it takes the last visible slot, and neighbours before
        // it give way until the row fits again (it alone always stays)
        if let Some(active) = active_tab.filter(|&a| a < widths.len() && !drawn.contains(&a)) {
            drawn.pop();
            drawn.push(active);
            while drawn.len() > 1 && start_x + row(&drawn) > fit_right {
                drawn.remove(drawn.len() - 2);
            }
        }
    }
    let mut tabs: Vec<(usize, Rect)> = Vec::with_capacity(drawn.len());
    let mut x = start_x;
    for &i in &drawn {
        tabs.push((i, chip(x, widths[i])));
        x += widths[i] + TAB_GAP;
    }
    let hidden: Vec<usize> = (0..widths.len()).filter(|i| !drawn.contains(i)).collect();
    let mut next = tabs.last().map_or(start_x, |&(_, r)| r.right() + t::BAND_OVERFLOW_GAP);
    let overflow = (!hidden.is_empty()).then(|| {
        let r = chip(next, t::BAND_OVERFLOW_W);
        next = r.right() + t::BAND_OVERFLOW_GAP;
        r
    });
    // clamp so a wider swapped-in active tab can never push `+` out of the band
    let plus = Some(chip(next.min(tabs_right - plus_w).max(start_x), plus_w));
    TopbarLayout { caps, menu, brand, tabs, overflow, hidden, plus }
}

impl TopbarLayout {
    /// Every bar rect a press BELONGS to (a control, a tab chip's FULL slot — its × lives inside it —
    /// the `+` and "+N" chips, Home / the burger, the V mark, Windows' caps). This one list is
    /// what the bar publishes as the caption exclusions (`caption_exclusions` → `cursors::set_caption`),
    /// so the Windows `WM_NCHITTEST` band and the macOS `caption_drag_hit` test exactly the rects the
    /// strip draws and hit-tests — never a second, hand-kept copy (P15).
    pub fn interactive_rects(&self) -> Vec<egui::Rect> {
        self.interactive_rects_with(self.tabs.iter().map(|&(_, r)| r))
    }

    /// `interactive_rects` with the tab chips' rects supplied by the caller: while a chip is lifted
    /// the strip publishes the rects it actually PAINTS (`TabDragFrame::chip_rects`) instead of the
    /// resting slots — the same "published == painted" invariant (P15, P16 review).
    pub fn interactive_rects_with(&self, chips: impl IntoIterator<Item = egui::Rect>) -> Vec<egui::Rect> {
        let mut out: Vec<egui::Rect> = self.caps.map_or_else(Vec::new, |c| c.to_vec());
        out.extend([self.menu, self.brand]);
        out.extend(chips);
        out.extend(self.overflow);
        out.extend(self.plus);
        out
    }

    /// The tab strip's horizontal extent: from the first drawn chip's left edge to the last one's
    /// right edge. A lifted (dragged) chip is clamped to this span (P16) — exactly the span the
    /// reflowed chips fill, so the lifted chip can always reach the first and the last slot.
    pub fn tab_strip(&self) -> Option<egui::Rangef> {
        let first = self.tabs.first()?.1;
        let last = self.tabs.last()?.1;
        Some(egui::Rangef::new(first.left(), last.right()))
    }
}

/// Logical rects → the physical-px `[l, t, r, b]` exclusions `caption_hit` reads. Rounded OUTWARD
/// (floor / ceil), so a fractional scale factor can only grow a control's no-drag area, never shave a
/// sliver off its edge that would start a window drag.
pub fn caption_exclusions(rects: &[egui::Rect], pixels_per_point: f32) -> Vec<[i32; 4]> {
    rects
        .iter()
        .map(|r| {
            let s = |v: f32, up: bool| {
                (if up { (v * pixels_per_point).ceil() } else { (v * pixels_per_point).floor() }) as i32
            };
            [s(r.left(), false), s(r.top(), false), s(r.right(), true), s(r.bottom(), true)]
        })
        .collect()
}

/// The × hit square of a tab chip: centred where the dirty dot sits (right − `TAB_MARK_INSET`).
pub fn tab_close_rect(tab: egui::Rect) -> egui::Rect {
    use varos_app::shell::tokens as t;
    egui::Rect::from_center_size(
        egui::pos2(tab.right() - t::TAB_MARK_INSET, tab.center().y),
        egui::Vec2::splat(t::TAB_CLOSE_HIT),
    )
}

/// How far (logical pt) past a neighbour's midpoint the lifted chip's leading edge must go before
/// the gap jumps over that neighbour — and, once it has, how far back before it jumps back (P16
/// review). Pointer jitter sitting on a boundary can never toggle the gap.
pub const TAB_DRAG_HYSTERESIS: f32 = 2.0;

/// One frame of a tab drag (P16): where every drawn chip is painted while one of them is LIFTED.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TabDragFrame {
    /// The dragged chip: its resting size, under the pointer (grab offset kept), y locked to the
    /// strip, clamped to the strip's span. Painted last, above the others.
    pub lifted: egui::Rect,
    /// The dragged chip's resting slot — where the press began.
    pub home: egui::Rect,
    /// Every OTHER drawn chip, left → right, as `(original tab index, reflowed rect)`: packed with
    /// `TAB_GAP` from the strip's start, leaving a gap of the lifted chip's width at `landing`.
    pub others: Vec<(usize, egui::Rect)>,
    /// Where the gap is: an insertion slot `0..=others.len()` in `others`' order.
    pub landing: usize,
    /// The gap itself: the rect the lifted chip lands in on release.
    pub gap: egui::Rect,
}

impl TabDragFrame {
    /// The chip rects a press belongs to during this frame: every rect PAINTED (the reflowed chips
    /// and the lifted one) plus the dragged chip's resting slot. The bar publishes these as its caption
    /// exclusions while a chip is lifted (`TopbarLayout::interactive_rects_with`), keeping P15's
    /// "published == painted" — neither the lifted chip nor the press's origin can be a caption spot.
    pub fn chip_rects(&self) -> Vec<egui::Rect> {
        self.others.iter().map(|&(_, r)| r).chain([self.lifted, self.home]).collect()
    }
}

/// The pure geometry of a tab drag (P16). `chips` = the drawn chips at rest (`TopbarLayout::tabs`),
/// `dragged` = the lifted chip's position IN `chips`, `grab_dx` = pointer x − chip left at the press,
/// `pointer_x` = the pointer now, `strip` = the span the lifted chip is clamped to
/// (`TopbarLayout::tab_strip`), `prev_landing` = last frame's `landing` (`dragged` on the first
/// frame: the gap starts at home). `None` only when `dragged` is not a drawn chip.
///
/// A neighbour sits BEFORE the gap while its resting midpoint lies left of the lifted chip's leading
/// edge — the left edge for chips left of home, the right edge for chips right of home — i.e. the gap
/// jumps over a neighbour once the lifted chip covers half of it. One comparison for both
/// directions (`edge > midpoint`), with `TAB_DRAG_HYSTERESIS` against jitter. (The centre would not
/// do: a wide chip clamped at the end of the strip could never pass a narrower end chip's midpoint,
/// so the first / last slot would be unreachable.) No time, no interpolation: the gap is where it is.
pub(crate) fn tab_drag_frame(
    chips: &[(usize, egui::Rect)],
    dragged: usize,
    grab_dx: f32,
    pointer_x: f32,
    strip: egui::Rangef,
    prev_landing: usize,
) -> Option<TabDragFrame> {
    let &(_, home) = chips.get(dragged)?;
    let w = home.width();
    let left = (pointer_x - grab_dx).clamp(strip.min, (strip.max - w).max(strip.min));
    let lifted = egui::Rect::from_min_size(egui::pos2(left, home.top()), home.size());
    // `k` = the neighbour's position among the OTHER chips; it was before the gap iff k < prev_landing
    let before = |k: usize, edge: f32, mid: f32| {
        let h = if k < prev_landing { -TAB_DRAG_HYSTERESIS } else { TAB_DRAG_HYSTERESIS };
        edge > mid + h
    };
    let landing = chips
        .iter()
        .enumerate()
        .filter(|&(k, _)| k != dragged)
        .enumerate()
        .filter(|&(k, (j, &(_, r)))| {
            let edge = if j < dragged { lifted.left() } else { lifted.right() };
            before(k, edge, r.center().x)
        })
        .count();
    let slot_at = |x: f32| egui::Rect::from_min_size(egui::pos2(x, home.top()), home.size());
    let mut x = chips[0].1.left();
    let mut gap = None;
    let mut others = Vec::with_capacity(chips.len() - 1);
    for (k, &(i, r)) in chips.iter().enumerate().filter(|&(k, _)| k != dragged).map(|(_, c)| c).enumerate() {
        if k == landing {
            gap = Some(slot_at(x));
            x += w + TAB_GAP;
        }
        others.push((i, egui::Rect::from_min_size(egui::pos2(x, r.top()), r.size())));
        x += r.width() + TAB_GAP;
    }
    let gap = gap.unwrap_or_else(|| slot_at(x));
    Some(TabDragFrame { lifted, home, others, landing, gap })
}

/// The full order after `Workspace::reorder(dragged, slot)`, as original indices `0..n`.
fn reordered(n: usize, dragged: usize, slot: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let slot = slot.min(n);
    let dest = if slot > dragged { slot - 1 } else { slot };
    let d = order.remove(dragged);
    order.insert(dest, d);
    order
}

/// The FULL-order insertion slot a release commits (`ReorderDocument` → `Workspace::reorder`) for a
/// chip dropped into the gap (P16 review). `n` = tabs in the full order, `dragged` = the lifted tab's
/// full-order index, `others` = the other DRAWN tabs' full-order indices left → right (as painted
/// around the gap), `landing` = the gap's slot among them, `drawn_after(order)` = which tabs the
/// strip would draw for a full order (original indices in their new order) — the real
/// `topbar_layout`, never a second copy of the fitting rules.
///
/// On overflow the drawn chips are not contiguous in the full order (hidden tabs; the active one
/// displaced into the last drawn slot — S1 F7), so "the slot before chip B" can sit behind hidden
/// tabs and the dropped tab would vanish. Instead every slot is tried and the best kept: the
/// dropped tab must be DRAWN afterwards; then its drawn neighbours should be exactly the ones it was
/// dropped between (`others[landing - 1]` / `others[landing]`); then it should sit as close as
/// possible to the gap's position; ties go to the smallest move in the full order (so a drop back
/// into its own gap is a no-op, and hidden tabs are never reshuffled for nothing). When the gap is past
/// a displaced active chip no single move can draw the tab right of it (the active one keeps the
/// last drawn slot), so it lands just left of it — visible, one slot from the drop.
pub(crate) fn visible_drop_slot(
    n: usize,
    dragged: usize,
    others: &[usize],
    landing: usize,
    drawn_after: impl Fn(&[usize]) -> Vec<usize>,
) -> usize {
    let want_left = landing.checked_sub(1).and_then(|l| others.get(l).copied());
    let want_right = others.get(landing).copied();
    let score = |slot: usize| {
        let order = reordered(n, dragged, slot);
        // how far the tab moves in the FULL order — a drop back into its own gap is a no-op
        let moved = order.iter().position(|&i| i == dragged).map_or(usize::MAX, |d| d.abs_diff(dragged));
        let drawn = drawn_after(&order);
        let Some(pos) = drawn.iter().position(|&i| i == dragged) else {
            return (1, 2, usize::MAX, moved);
        };
        let left = pos.checked_sub(1).map(|p| drawn[p]);
        let right = drawn.get(pos + 1).copied();
        let mismatch = usize::from(left != want_left) + usize::from(right != want_right);
        (0, mismatch, pos.abs_diff(landing), moved)
    };
    (0..=n).min_by_key(|&slot| score(slot)).unwrap_or(dragged)
}

/// Is physical-px point (x, y) inside the caption band of height `h` and NOT on one of the bar's
/// interactive rects (`[l, t, r, b]`, physical px, as `cursors::set_caption` receives them)?
pub fn caption_hit(h: i32, excl: &[[i32; 4]], x: i32, y: i32) -> bool {
    y >= 0 && y < h && x >= 0 && !excl.iter().any(|r| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
}

// The native menu bar's table lives in `crate::menus` (slice 0.6 split); these names stay reachable here.
#[cfg(test)]
pub use crate::menus::flat_items;
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))] // like the table: macOS-only users
pub use crate::menus::{egui_key, menus, recent_menu, Accel, Check, Entry, FileCmd, MenuCmd, Native, SnapRow};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_topbar_numbers_are_unchanged_and_mac_clears_the_traffic_lights() {
        let win = topbar_chrome(false);
        assert_eq!(win, TopbarChrome { height: 46.0, lead: 4.0, right_inset: 0.0, window_caps: true });
        let mac = topbar_chrome(true);
        assert_eq!(mac.height, varos_app::shell::tokens::BAND_H, "4b: the 52-pt band");
        assert!(!mac.window_caps, "macOS uses the native traffic lights, never our ─ ☐ ✕");
        assert!(mac.lead >= 72.0, "the three traffic lights end ≈ 70 pt from the left edge");
        assert!(mac.right_inset > 0.0);
        assert_eq!(TOPBAR, topbar_chrome(cfg!(target_os = "macos")));
    }

    /// Every band control's rect (chips' × squares included) for this layout.
    fn controls(layout: &TopbarLayout) -> Vec<egui::Rect> {
        let tab_rects: Vec<egui::Rect> = layout.tabs.iter().map(|&(_, r)| r).collect();
        [layout.menu, layout.brand]
            .into_iter()
            .chain(tab_rects.iter().copied())
            .chain(tab_rects.iter().copied().map(tab_close_rect))
            .chain(layout.overflow)
            .chain(layout.plus)
            .collect()
    }

    /// 4b: every control sits on ONE centre line — the traffic lights' y 26 (`mac_titlebar` puts
    /// the native buttons there).
    #[test]
    fn mac_topbar_controls_centre_on_26() {
        let chrome = topbar_chrome(true);
        // Include a translated bar, minimum window width, overflow tabs and a wide window.
        for origin in [egui::pos2(0.0, 0.0), egui::pos2(31.0, 47.0)] {
            for width in [800.0, 1280.0, 1920.0] {
                let bar = egui::Rect::from_min_size(origin, egui::vec2(width, chrome.height));
                let layout = topbar_layout(bar, chrome, None, &[65.0, 180.0, 300.0, 120.0, 90.0, 220.0], Some(0));
                assert!(layout.caps.is_none());
                assert!(!layout.tabs.is_empty());
                assert!(layout.plus.is_some(), "the + chip is reserved before tabs are fitted (F15)");
                let traffic_light_centre = bar.top() + varos_app::shell::tokens::BAND_H / 2.0;
                assert_eq!(traffic_light_centre - bar.top(), 26.0);
                for rect in controls(&layout) {
                    assert!(bar.contains_rect(rect), "control {rect:?} escapes bar {bar:?}");
                    assert!(
                        (rect.center().y - traffic_light_centre).abs() <= 1.0,
                        "control {rect:?} is not centred on traffic lights at {traffic_light_centre}"
                    );
                }
            }
        }
    }

    /// 4b: band 52 = 12 + 28 + 12 — every control is 28 tall, its top 12 below the band's top.
    #[test]
    fn band_is_12_chip_12() {
        use varos_app::shell::tokens as t;
        assert_eq!(t::BAND_PAD_Y + t::BAND_CHIP_H + t::BAND_PAD_Y, t::BAND_H);
        let chrome = topbar_chrome(true);
        for width in [800.0, 1512.0] {
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, chrome.height));
            let layout = topbar_layout(bar, chrome, None, &[80.0; 9], Some(8));
            assert!(layout.overflow.is_some(), "setup: nine tabs overflow at {width}");
            let chips = [layout.menu, layout.brand]
                .into_iter()
                .chain(layout.tabs.iter().map(|&(_, r)| r))
                .chain(layout.overflow)
                .chain(layout.plus);
            for r in chips {
                assert_eq!((r.top(), r.height()), (t::BAND_PAD_Y, t::BAND_CHIP_H), "{r:?}");
                assert_eq!(bar.bottom() - r.bottom(), t::BAND_PAD_Y, "{r:?}");
            }
            assert_eq!(layout.menu, egui::Rect::from_min_size(egui::pos2(78.0, 12.0), egui::vec2(28.0, 28.0)), "Home");
            assert_eq!(layout.tabs[0].1.left(), 114.0, "the first tab, Home + 8");
        }
    }

    /// 4b: a tab chip is 12 + name (Inter 500 12) + 30, clamped to 88…176.
    #[test]
    fn tab_width_is_the_name_plus_padding_clamped() {
        assert_eq!(tab_width(10.0), 88.0);
        assert_eq!(tab_width(80.0), 122.0);
        assert_eq!(tab_width(400.0), 176.0);
        // the × hit square sits where the dot sits: 16 in from the right edge
        let chip = egui::Rect::from_min_size(egui::pos2(114.0, 12.0), egui::vec2(122.0, 28.0));
        assert_eq!(tab_close_rect(chip).center(), egui::pos2(236.0 - 16.0, 26.0));
    }

    /// The right zone over the panel column: empty band from its left edge, V at its right edge.
    #[test]
    fn right_zone_follows_the_panel_column() {
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1512.0, chrome.height));
        let column = egui::Rangef::new(1212.0, 1500.0);
        let layout = topbar_layout(bar, chrome, Some(column), &[80.0, 80.0], Some(0));
        assert_eq!(right_zone(bar, chrome, None, Some(column)), column, "the mockup: 1212–1500");
        assert_eq!(layout.brand.x_range(), egui::Rangef::new(1472.0, 1500.0), "V: 1472–1500");
        // the editor's column (6-pt side margin) ends 6 from the edge: V follows it, not the fallback
        let editor = egui::Rangef::new(1206.0, 1506.0);
        let l = topbar_layout(bar, chrome, Some(editor), &[], None);
        assert_eq!((right_zone(bar, chrome, None, Some(editor)).min, l.brand.right()), (1206.0, 1506.0));
        // never past the window edge, and an empty span falls back
        let l = topbar_layout(bar, chrome, Some(egui::Rangef::new(1300.0, 1600.0)), &[], None);
        assert_eq!(l.brand.right(), 1512.0);
        let l = topbar_layout(bar, chrome, Some(egui::Rangef::new(1300.0, 1300.0)), &[], None);
        assert_eq!(l.brand.right(), 1500.0, "the fallback");
    }

    #[test]
    fn right_zone_falls_back_to_288_at_the_right_edge() {
        use varos_app::shell::tokens as t;
        let chrome = topbar_chrome(true);
        for width in [800.0, 1512.0, 1920.0] {
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, chrome.height));
            let zone = right_zone(bar, chrome, None, None);
            assert_eq!(zone, egui::Rangef::new(width - 12.0 - t::BAND_RIGHT_ZONE_W, width - 12.0), "{width}");
            let layout = topbar_layout(bar, chrome, None, &[], None);
            assert_eq!(layout.brand.right(), zone.max);
        }
        // Windows: the zone ends a seam before the caps
        let win = topbar_chrome(false);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, win.height));
        let layout = topbar_layout(bar, win, None, &[], None);
        let caps = layout.caps.unwrap();
        assert_eq!(layout.brand.right(), caps[0].left() - t::SEAM_GAP);
    }

    /// Owner 2026-10-06 ("شيل خانة البحث"): the band has NO Search. The slot it used to fill — the
    /// right zone from its left edge up to `BAND_GAP` before the V mark — publishes no rect, so a press
    /// anywhere on it drags the window; the V mark keeps its place at the zone's right edge, and the
    /// tabs still stop `BAND_TABS_END_GAP` before the zone (tab fitting unchanged).
    #[test]
    fn the_band_has_no_search_and_its_old_slot_drags_the_window() {
        use varos_app::shell::tokens as t;
        for chrome in [topbar_chrome(true), topbar_chrome(false)] {
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1512.0, chrome.height));
            for column in [None, Some(egui::Rangef::new(1212.0, 1500.0)), Some(egui::Rangef::new(1100.0, 1500.0))] {
                let layout = topbar_layout(bar, chrome, column, &[90.0; 12], Some(0));
                let zone = right_zone(bar, chrome, layout.caps.map(|c| c[0].left()), column);
                assert_eq!(layout.brand.x_range(), egui::Rangef::new(zone.max - t::BAND_BRAND, zone.max), "V stays");
                // where Search was (4b: zone.min … V − 8, chip height)
                let old = egui::Rect::from_min_max(
                    egui::pos2(zone.min, layout.brand.top()),
                    egui::pos2(layout.brand.left() - t::BAND_GAP, layout.brand.bottom()),
                );
                assert!(old.width() >= 100.0, "setup: a real slot {old:?}");
                for r in layout.interactive_rects() {
                    assert!(!r.intersects(old), "{r:?} is published over the old Search slot {old:?}");
                }
                let plus = layout.plus.expect("+ is placed");
                assert!(plus.right() <= zone.min - t::BAND_TABS_END_GAP, "the tabs stop before the zone");
                for ppp in [1.0, 1.25, 2.0] {
                    let i = old.shrink(0.5);
                    for p in [old.center(), i.left_top(), i.right_top(), i.left_bottom(), i.right_bottom()] {
                        assert!(drags_window(&layout, chrome, ppp, p), "{p:?} (ppp {ppp}) must drag the window");
                    }
                    assert!(!drags_window(&layout, chrome, ppp, layout.brand.center()), "V is still a control");
                }
            }
        }
    }

    #[test]
    fn brand_is_a_28_square_ending_at_the_zone_right() {
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1512.0, chrome.height));
        for column in [None, Some(egui::Rangef::new(1180.0, 1490.0))] {
            let layout = topbar_layout(bar, chrome, column, &[], None);
            let zone = right_zone(bar, chrome, None, column);
            assert_eq!(layout.brand.size(), egui::vec2(28.0, 28.0));
            assert_eq!(layout.brand.right(), zone.max);
        }
    }

    /// 4b: Export / Share / Window / the magnet are gone from the band (and Search, 2026-10-06) — the
    /// published list is exactly caps + Home + V + chips + "+N" + `+`.
    #[test]
    fn the_layout_has_no_export_share_or_window_rect() {
        for chrome in [topbar_chrome(true), topbar_chrome(false)] {
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, chrome.height));
            for n in [2, 12] {
                let layout = topbar_layout(bar, chrome, None, &vec![90.0; n], Some(0));
                let mut want: Vec<egui::Rect> = layout.caps.map_or_else(Vec::new, |c| c.to_vec());
                want.extend([layout.menu, layout.brand]);
                want.extend(layout.tabs.iter().map(|&(_, r)| r));
                want.extend(layout.overflow);
                want.extend(layout.plus);
                assert_eq!(layout.interactive_rects(), want, "{n} tabs");
                assert_eq!(layout.overflow.is_some(), n == 12, "+N only when tabs are hidden");
            }
        }
    }

    /// "+N" sits after the last drawn chip (then `+`), its room reserved before fitting, and lists
    /// every tab not drawn — in tab order, the displaced ones included.
    #[test]
    fn overflow_reserves_room_for_plus_n_and_lists_every_hidden_tab() {
        use varos_app::shell::tokens as t;
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1512.0, chrome.height));
        // nine tabs (the mockup): some hidden; the active last one is drawn
        let widths = [110.0, 90.0, 100.0, 85.0, 75.0, 125.0, 80.0, 115.0, 120.0];
        let column = egui::Rangef::new(1212.0, 1500.0);
        let zone = right_zone(bar, chrome, None, Some(column));
        let layout = topbar_layout(bar, chrome, Some(column), &widths, Some(8));
        let drawn: Vec<usize> = layout.tabs.iter().map(|&(i, _)| i).collect();
        assert!(drawn.contains(&8) && drawn.len() < 9, "setup: overflow with the active tab drawn: {drawn:?}");
        let mut all = drawn.clone();
        all.extend(&layout.hidden);
        all.sort_unstable();
        assert_eq!(all, (0..9).collect::<Vec<_>>(), "drawn + hidden = every tab, once");
        assert!(layout.hidden.windows(2).all(|w| w[0] < w[1]), "hidden in tab order");
        let ov = layout.overflow.expect("+N is shown");
        let last = layout.tabs.last().unwrap().1;
        assert_eq!(ov.left(), last.right() + t::BAND_OVERFLOW_GAP);
        assert_eq!(ov.size(), egui::vec2(t::BAND_OVERFLOW_W, t::BAND_CHIP_H));
        let plus = layout.plus.unwrap();
        assert_eq!(plus.left(), ov.right() + t::BAND_OVERFLOW_GAP, "+ after +N");
        assert!(plus.right() <= zone.min - t::BAND_TABS_END_GAP, "the drag gap before the right zone stays");
        // five tabs at 1512 all fit and leave ≥ 120 pt of empty band (the mockup's drag handle)
        let five = topbar_layout(bar, chrome, Some(column), &widths[..5], Some(1));
        assert!(five.overflow.is_none() && five.hidden.is_empty() && five.tabs.len() == 5);
        assert!(zone.min - five.plus.unwrap().right() >= 120.0);
    }

    /// Whatever the overflow, a displaced active tab that is very wide still never pushes `+` out.
    #[test]
    fn the_active_tab_alone_still_fits_with_plus_n_and_plus() {
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(800.0, chrome.height));
        let layout = topbar_layout(bar, chrome, None, &[400.0; 6], Some(5));
        assert_eq!(layout.tabs.iter().map(|&(i, _)| i).collect::<Vec<_>>(), [5]);
        assert_eq!(layout.hidden, [0, 1, 2, 3, 4]);
        let (ov, plus) = (layout.overflow.unwrap(), layout.plus.unwrap());
        assert!(bar.contains_rect(ov) && bar.contains_rect(plus) && !ov.intersects(plus));
    }

    #[test]
    fn plus_is_always_placed_even_with_overflowing_tabs() {
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, chrome.height));
        let widths = vec![180.0; 20]; // far more than fit
        let layout = topbar_layout(bar, chrome, None, &widths, Some(0));
        assert!(layout.tabs.len() < widths.len(), "the strip really is overflowing here");
        let plus = layout.plus.expect("+ must survive overflow");
        assert!(bar.contains_rect(plus), "+ escapes the bar: {plus:?}");
        // + never overlaps a placed tab
        for &(_, r) in &layout.tabs {
            assert!(!r.intersects(plus), "tab {r:?} overlaps +");
        }
    }

    #[test]
    fn active_tab_is_placed_when_tabs_overflow() {
        let chrome = topbar_chrome(true);
        let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(900.0, chrome.height));
        let widths = vec![180.0; 12];
        let last = widths.len() - 1;
        let layout = topbar_layout(bar, chrome, None, &widths, Some(last));
        // a greedy fit alone would never reach the last tab — confirm this scenario really overflows
        let greedy = topbar_layout(bar, chrome, None, &widths, None);
        assert!(!greedy.tabs.iter().any(|&(i, _)| i == last), "test setup: the last tab must overflow");
        assert!(layout.tabs.iter().any(|&(i, _)| i == last), "the active (last) tab must still be placed");
        let (_, active_rect) = *layout.tabs.last().expect("at least one tab is placed");
        assert!(bar.contains_rect(active_rect), "the active tab's chip escapes the bar");
        assert!(layout.plus.is_some(), "+ still survives once the active tab claims a slot");
    }

    /// Drawn chips `(tab index, rect)` of the given widths, packed from x = 0 with `TAB_GAP`.
    fn chips(widths: &[f32], indices: &[usize]) -> Vec<(usize, egui::Rect)> {
        let mut x = 0.0;
        widths
            .iter()
            .zip(indices)
            .map(|(&w, &i)| {
                let r = egui::Rect::from_min_size(egui::pos2(x, 12.0), egui::vec2(w, 28.0));
                x += w + TAB_GAP;
                (i, r)
            })
            .collect()
    }
    fn span(c: &[(usize, egui::Rect)]) -> egui::Rangef {
        egui::Rangef::new(c[0].1.left(), c.last().unwrap().1.right())
    }
    fn lefts(f: &TabDragFrame) -> Vec<(usize, f32)> {
        f.others.iter().map(|&(i, r)| (i, r.left())).collect()
    }
    /// One drag frame of chip `d` (grab 0) at pointer `x`, the gap last at `prev`.
    fn drag(c: &[(usize, egui::Rect)], d: usize, x: f32, prev: usize) -> TabDragFrame {
        tab_drag_frame(c, d, 0.0, x, span(c), prev).unwrap()
    }

    #[test]
    fn lifted_chip_follows_the_pointer_with_its_grab_offset_and_is_clamped() {
        let c = chips(&[80.0, 80.0, 80.0], &[0, 1, 2]); // [0,80) [82,162) [164,244)
        let f = |x: f32| tab_drag_frame(&c, 1, 10.0, x, span(&c), 1).unwrap();
        assert_eq!(f(100.0).lifted, egui::Rect::from_min_size(egui::pos2(90.0, 12.0), egui::vec2(80.0, 28.0)));
        assert_eq!(f(100.0).home, c[1].1, "the resting slot is kept for the caption exclusions");
        assert_eq!(f(-50.0).lifted.left(), 0.0, "clamped to the strip's start");
        assert_eq!(f(999.0).lifted.right(), 244.0, "clamped to the strip's end");
        assert_eq!(f(100.0).lifted.top(), 12.0, "y is the strip's, never the pointer's");
        assert!(tab_drag_frame(&c, 3, 0.0, 0.0, span(&c), 3).is_none(), "not a drawn chip");
    }

    #[test]
    fn the_gap_jumps_a_neighbour_once_the_leading_edge_clears_its_midpoint_both_ways() {
        // three 80-wide chips (gap 2): midpoints 40, 122, 204; hysteresis 2
        let c = chips(&[80.0, 80.0, 80.0], &[0, 1, 2]);
        assert_eq!(TAB_DRAG_HYSTERESIS, 2.0);

        // chip 0 dragged right: its right edge (x + 80) must clear chip 1's midpoint + 2 = 124
        assert_eq!((drag(&c, 0, 44.0, 0).landing, lefts(&drag(&c, 0, 44.0, 0))), (0, vec![(1, 82.0), (2, 164.0)]));
        let f = drag(&c, 0, 44.1, 0);
        assert_eq!((f.landing, lefts(&f), f.gap.left()), (1, vec![(1, 0.0), (2, 164.0)], 82.0), "1 hops left");
        // …and back only once the same edge is 2 short of the midpoint: 120
        assert_eq!(drag(&c, 0, 40.1, 1).landing, 1, "inside the band: stays");
        assert_eq!(drag(&c, 0, 39.9, 1).landing, 0, "back past it: 1 returns");
        assert_eq!(drag(&c, 0, 126.1, 0).landing, 2, "a fast move clears both neighbours in one frame");

        // chip 2 dragged left: the SAME comparison on its left edge — chip 1 stays before the gap
        // while x > 122 − 2, and gets back before it only once x > 122 + 2
        assert_eq!(drag(&c, 2, 120.1, 2).landing, 2, "inside the band: stays");
        let g = drag(&c, 2, 119.9, 2);
        assert_eq!((g.landing, lefts(&g)), (1, vec![(0, 0.0), (1, 164.0)]), "1 hops right");
        assert_eq!(drag(&c, 2, 123.9, 1).landing, 1, "inside the band: stays");
        assert_eq!(drag(&c, 2, 124.1, 1).landing, 2, "back past it: 1 returns");
        assert_eq!(drag(&c, 2, 37.9, 2).landing, 0, "past chip 0's midpoint too");
    }

    #[test]
    fn pointer_jitter_on_a_boundary_never_toggles_the_gap() {
        let c = chips(&[80.0, 80.0, 80.0], &[0, 1, 2]);
        // chip 0 dragged right; the boundary for chip 1 is at x = 42 (right edge on 122)
        let path = [42.0, 42.9, 41.1, 44.2, 42.0, 43.9, 40.1, 42.0, 39.8, 42.0, 43.5];
        let want = [0, 0, 0, 1, 1, 1, 1, 1, 0, 0, 0];
        let mut prev = 0;
        for (&x, &w) in path.iter().zip(&want) {
            prev = drag(&c, 0, x, prev).landing;
            assert_eq!(prev, w, "pointer at {x}");
        }
        // mirrored: chip 2 dragged left; the boundary for chip 1 is at x = 122
        let path = [122.0, 121.1, 122.9, 119.8, 122.0, 123.9, 124.3, 122.0];
        let want = [2, 2, 2, 1, 1, 1, 2, 2];
        let mut prev = 2;
        for (&x, &w) in path.iter().zip(&want) {
            prev = drag(&c, 2, x, prev).landing;
            assert_eq!(prev, w, "pointer at {x}");
        }
    }

    #[test]
    fn a_wide_chip_reaches_both_ends_past_narrower_ones() {
        // the reason for the leading edge, not the centre: a 220-wide chip clamped at the end has its
        // centre at 188 — it could never pass the 76-wide end chip's midpoint (260) by its centre.
        let c = chips(&[220.0, 76.0], &[0, 1]);
        let f = drag(&c, 0, 9999.0, 0);
        assert_eq!((f.lifted.right(), f.landing), (298.0, 1), "lands after the narrow chip");
        let c = chips(&[76.0, 220.0], &[0, 1]);
        let f = drag(&c, 1, -9999.0, 1);
        assert_eq!((f.lifted.left(), f.landing), (0.0, 0), "lands before the narrow chip");
    }

    #[test]
    fn reflowed_chips_and_the_gap_tile_the_strip_exactly() {
        let c = chips(&[90.0, 150.0, 76.0, 120.0], &[0, 1, 2, 3]);
        let strip = span(&c);
        for dragged in 0..c.len() {
            let mut prev = dragged;
            for x in (-100..700).step_by(7) {
                let f = tab_drag_frame(&c, dragged, 30.0, x as f32, strip, prev).unwrap();
                prev = f.landing;
                assert!(f.lifted.left() >= strip.min && f.lifted.right() <= strip.max);
                assert_eq!(f.gap.width(), c[dragged].1.width());
                let mut row: Vec<egui::Rect> = f.others.iter().map(|&(_, r)| r).collect();
                row.insert(f.landing, f.gap);
                assert_eq!(row[0].left(), strip.min);
                assert_eq!(row.last().unwrap().right(), strip.max, "same span as at rest");
                for w in row.windows(2) {
                    assert_eq!(w[1].left() - w[0].right(), TAB_GAP, "packed with the one gap");
                }
            }
        }
    }

    /// The real `topbar_layout` for `n` tabs every `text` wide in a `width`-wide Mac bar, tab
    /// `active` active: "which tabs are drawn for this full order" — exactly what the strip hands
    /// `visible_drop_slot`.
    fn drawn_for(n: usize, active: usize, width: f32, text: f32) -> impl Fn(&[usize]) -> Vec<usize> {
        move |order: &[usize]| {
            let chrome = topbar_chrome(true);
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, chrome.height));
            let act = order.iter().position(|&i| i == active);
            let l = topbar_layout(bar, chrome, None, &vec![text; n], act);
            l.tabs.iter().map(|&(k, _)| order[k]).collect()
        }
    }
    /// Drop tab `d` into the gap at `landing` among the drawn `others`; the drawn tabs afterwards.
    fn drop_and_draw(n: usize, drawn: &impl Fn(&[usize]) -> Vec<usize>, d: usize, landing: usize) -> Vec<usize> {
        let before = drawn(&(0..n).collect::<Vec<_>>());
        let others: Vec<usize> = before.into_iter().filter(|&i| i != d).collect();
        let slot = visible_drop_slot(n, d, &others, landing, drawn);
        drawn(&reordered(n, d, slot))
    }

    #[test]
    fn overflow_drop_lands_between_its_visible_neighbours_and_stays_visible() {
        // [0..9], 9 active: drawn [0, 1, 2, 9] — 3..8 hidden between 2 and 9
        let (n, drawn) = (10, drawn_for(10, 9, 900.0, 40.0));
        assert_eq!(drawn(&(0..n).collect::<Vec<_>>()), [0, 1, 2, 9], "setup");
        // 0 dropped between 2 and 9 — NOT behind the hidden 3..8 (it used to vanish there)
        assert_eq!(visible_drop_slot(n, 0, &[1, 2, 9], 2, &drawn), 3);
        assert_eq!(drop_and_draw(n, &drawn, 0, 2), [1, 2, 0, 9]);
        assert_eq!(drop_and_draw(n, &drawn, 0, 1), [1, 0, 2, 9], "between 1 and 2");
        // the active 9 dragged to the front / between 1 and 2 / back home
        assert_eq!(drop_and_draw(n, &drawn, 9, 0), [9, 0, 1, 2]);
        assert_eq!(drop_and_draw(n, &drawn, 9, 2), [0, 1, 9, 2]);
        assert_eq!(reordered(n, 9, visible_drop_slot(n, 9, &[0, 1, 2], 3, &drawn)), (0..n).collect::<Vec<_>>());
        // a chip dropped back into its own gap changes nothing
        assert_eq!(reordered(n, 1, visible_drop_slot(n, 1, &[0, 2, 9], 1, &drawn)), (0..n).collect::<Vec<_>>());
        // 20 tabs, 12 active (drawn [0, 1, 2, 12]): 0 dropped between 2 and 12
        let drawn20 = drawn_for(20, 12, 900.0, 40.0);
        assert_eq!(drawn20(&(0..20).collect::<Vec<_>>()), [0, 1, 2, 12], "setup");
        assert_eq!(drop_and_draw(20, &drawn20, 0, 2), [1, 2, 0, 12]);
    }

    #[test]
    fn overflow_drop_after_the_last_visible_tab_stays_visible() {
        // past the displaced active 9 no single move can draw 0 to its right (9 keeps the last drawn
        // slot), so 0 lands just left of it: visible, one slot from the drop
        let (n, drawn) = (10, drawn_for(10, 9, 900.0, 40.0));
        assert_eq!(drop_and_draw(n, &drawn, 0, 3), [1, 2, 0, 9]);
        let drawn20 = drawn_for(20, 12, 900.0, 40.0);
        assert_eq!(drop_and_draw(20, &drawn20, 1, 3), [0, 2, 1, 12]);
        // no overflow: after the last visible tab = the very end, exactly
        let drawn3 = drawn_for(3, 0, 1400.0, 40.0);
        assert_eq!(visible_drop_slot(3, 0, &[1, 2], 2, &drawn3), 3);
        assert_eq!(drop_and_draw(3, &drawn3, 0, 2), [1, 2, 0]);
        assert_eq!(drop_and_draw(3, &drawn3, 2, 0), [2, 0, 1]);
        // a single drawn chip (the active one) has nowhere else to go
        let drawn1 = drawn_for(10, 9, 800.0, 180.0);
        assert_eq!(drawn1(&(0..10).collect::<Vec<_>>()), [9], "setup");
        assert_eq!(reordered(10, 9, visible_drop_slot(10, 9, &[], 0, &drawn1)), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn caption_hit_is_the_empty_band_only() {
        let excl = [[100, 0, 200, 92]];
        assert!(caption_hit(92, &excl, 50, 10));
        assert!(caption_hit(92, &excl, 250, 91));
        assert!(!caption_hit(92, &excl, 150, 10), "an interactive rect is not a drag spot");
        assert!(!caption_hit(92, &excl, 50, 92), "below the band");
        assert!(!caption_hit(0, &[], 50, 0), "no band published yet (splash) → never drag");
    }

    /// P15: the caption predicate exactly as both platforms run it — `interactive_rects` published
    /// through `caption_exclusions` at scale `ppp`, then `caption_hit` on a physical-px point. True =
    /// a press here drags the window.
    fn drags_window(layout: &TopbarLayout, chrome: TopbarChrome, ppp: f32, pos: egui::Pos2) -> bool {
        let excl = caption_exclusions(&layout.interactive_rects(), ppp);
        caption_hit((chrome.height * ppp) as i32, &excl, (pos.x * ppp) as i32, (pos.y * ppp) as i32)
    }

    /// Points on a chip's FULL slot a press can land on: centre, the four inner corners, the ×.
    fn slot_points(r: egui::Rect) -> [egui::Pos2; 6] {
        let i = r.shrink(0.5);
        [r.center(), i.left_top(), i.right_top(), i.left_bottom(), i.right_bottom(), tab_close_rect(r).center()]
    }

    /// P16 review: during a LIVE drag frame the bar publishes `interactive_rects_with(chip_rects)` —
    /// every chip where it is painted (reflowed or lifted) plus the lifted chip's resting slot. None of
    /// those, nor `+`, may ever be a caption spot, for every chip lifted at every pointer x.
    fn assert_live_drag_never_drags_the_window(layout: &TopbarLayout, chrome: TopbarChrome, ppp: f32) {
        let strip = layout.tab_strip().expect("chips are drawn");
        for d in 0..layout.tabs.len() {
            let mut prev = d;
            let mut x = strip.min - 60.0;
            while x < strip.max + 60.0 {
                let f = tab_drag_frame(&layout.tabs, d, 20.0, x, strip, prev).expect("a drawn chip");
                prev = f.landing;
                let excl = caption_exclusions(&layout.interactive_rects_with(f.chip_rects()), ppp);
                let drags = |p: egui::Pos2| {
                    caption_hit((chrome.height * ppp) as i32, &excl, (p.x * ppp) as i32, (p.y * ppp) as i32)
                };
                for r in f.chip_rects() {
                    for pos in slot_points(r) {
                        assert!(!drags(pos), "chip {d} lifted at x {x} (ppp {ppp}): {pos:?} on {r:?} drags");
                    }
                }
                assert!(!drags(layout.plus.expect("+ is placed").center()));
                if let Some(ov) = layout.overflow {
                    assert!(!drags(ov.center()), "+N during a live drag (ppp {ppp})");
                }
                x += 5.5;
            }
        }
    }

    #[test]
    fn a_press_on_any_tab_slot_or_plus_never_drags_the_window() {
        for chrome in [topbar_chrome(true), topbar_chrome(false)] {
            for ppp in [1.0, 1.25, 1.5, 2.0] {
                let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, chrome.height));
                let layout = topbar_layout(bar, chrome, None, &[60.0, 90.0, 120.0], Some(1));
                assert_eq!(layout.tabs.len(), 3, "setup: all three chips drawn");
                for &(i, r) in &layout.tabs {
                    for pos in slot_points(r) {
                        assert!(!drags_window(&layout, chrome, ppp, pos), "tab {i} at {pos:?} (ppp {ppp}) drags");
                    }
                }
                let plus = layout.plus.expect("+ is placed");
                for pos in [plus.center(), plus.shrink(0.5).left_top(), plus.shrink(0.5).right_bottom()] {
                    assert!(!drags_window(&layout, chrome, ppp, pos), "+ at {pos:?} (ppp {ppp}) drags");
                }
                for r in [layout.menu, layout.brand] {
                    assert!(!drags_window(&layout, chrome, ppp, r.center()), "control {r:?} (ppp {ppp}) drags");
                }
                if let Some(caps) = layout.caps {
                    for r in caps {
                        assert!(!drags_window(&layout, chrome, ppp, r.center()), "cap {r:?} drags");
                    }
                }
                assert_live_drag_never_drags_the_window(&layout, chrome, ppp);
            }
        }
    }

    #[test]
    fn a_press_on_empty_bar_space_drags_the_window() {
        for chrome in [topbar_chrome(true), topbar_chrome(false)] {
            for ppp in [1.0, 1.25, 2.0] {
                let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1400.0, chrome.height));
                let layout = topbar_layout(bar, chrome, None, &[60.0, 90.0], Some(0));
                let plus = layout.plus.expect("+ is placed");
                let y = bar.center().y;
                // the open stretch between `+` and the V mark — the main drag handle
                let open = egui::pos2((plus.right() + layout.brand.left()) / 2.0, y);
                assert!(layout.brand.left() - plus.right() > 40.0, "setup: a real empty stretch");
                assert!(drags_window(&layout, chrome, ppp, open), "empty bar at {open:?} (ppp {ppp})");
                // the 2-px gap between two chips is empty bar too
                let gap = egui::pos2((layout.tabs[0].1.right() + layout.tabs[1].1.left()) / 2.0, y);
                assert!(drags_window(&layout, chrome, ppp, gap), "chip gap at {gap:?} (ppp {ppp})");
                // above/below a chip, still inside the band
                let above = egui::pos2(layout.tabs[0].1.center().x, bar.top() + 0.2);
                if layout.tabs[0].1.top() - bar.top() >= 1.0 {
                    assert!(drags_window(&layout, chrome, ppp, above), "band above a chip (ppp {ppp})");
                }
                // below the band is never a caption
                assert!(!drags_window(&layout, chrome, ppp, egui::pos2(open.x, bar.bottom() + 1.0)));
            }
        }
    }

    /// 4b: the 12 pt above AND below a chip are empty band — a press there drags the window, on
    /// every control column (Home, a tab, `+`, V).
    #[test]
    fn the_band_above_and_below_a_chip_drags_the_window() {
        let chrome = topbar_chrome(true);
        for ppp in [1.0, 1.25, 2.0] {
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1512.0, chrome.height));
            let layout = topbar_layout(bar, chrome, None, &[60.0, 90.0], Some(0));
            let columns = [layout.menu, layout.tabs[0].1, layout.plus.unwrap(), layout.brand];
            for r in columns {
                for y in [bar.top() + 0.6, r.top() - 1.0, r.bottom() + 1.0, bar.bottom() - 0.6] {
                    let at = egui::pos2(r.center().x, y);
                    assert!(drags_window(&layout, chrome, ppp, at), "{at:?} over {r:?} (ppp {ppp})");
                }
                assert!(!drags_window(&layout, chrome, ppp, r.center()), "{r:?} itself (ppp {ppp})");
            }
        }
    }

    /// "+N" is a control: a press on it never drags the window, at rest or during a live tab drag.
    #[test]
    fn overflow_rect_never_drags_the_window() {
        for chrome in [topbar_chrome(true), topbar_chrome(false)] {
            let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, chrome.height));
            let layout = topbar_layout(bar, chrome, None, &[120.0; 9], Some(8));
            let ov = layout.overflow.expect("setup: nine tabs overflow");
            for ppp in [1.0, 1.25, 1.5, 2.0] {
                let i = ov.shrink(0.5);
                for pos in [ov.center(), i.left_top(), i.right_top(), i.left_bottom(), i.right_bottom()] {
                    assert!(!drags_window(&layout, chrome, ppp, pos), "+N at {pos:?} (ppp {ppp})");
                }
                assert_live_drag_never_drags_the_window(&layout, chrome, ppp);
            }
        }
    }

    #[test]
    fn overflow_slots_with_eight_tabs_are_still_covered() {
        let chrome = topbar_chrome(true);
        for active in [0, 4, 7] {
            for width in [800.0, 1100.0] {
                let bar = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(width, chrome.height));
                let layout = topbar_layout(bar, chrome, None, &[140.0; 8], Some(active));
                assert!(layout.tabs.len() < 8, "setup: 8 tabs overflow at width {width}");
                assert!(layout.tabs.iter().any(|&(i, _)| i == active), "setup: the active tab is drawn");
                for ppp in [1.0, 2.0] {
                    for &(i, r) in &layout.tabs {
                        for pos in slot_points(r) {
                            assert!(
                                !drags_window(&layout, chrome, ppp, pos),
                                "8 tabs, active {active}, width {width}: chip {i} at {pos:?} (ppp {ppp}) drags"
                            );
                        }
                    }
                    let plus = layout.plus.expect("+ survives overflow");
                    assert!(!drags_window(&layout, chrome, ppp, plus.center()));
                    assert_live_drag_never_drags_the_window(&layout, chrome, ppp);
                }
            }
        }
    }

    #[test]
    fn caption_exclusions_round_outward() {
        let r = egui::Rect::from_min_max(egui::pos2(10.3, 2.6), egui::pos2(20.2, 30.7));
        assert_eq!(caption_exclusions(&[r], 1.0), [[10, 2, 21, 31]]);
        assert_eq!(caption_exclusions(&[r], 2.0), [[20, 5, 41, 62]]);
    }
}
