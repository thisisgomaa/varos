use super::*;
fn raster(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let mut v = vec![];
    for y in 0..h {
        for x in 0..w {
            v.extend(f(x, y));
        }
    }
    v
}
fn bw() -> TraceOptions {
    TraceOptions { noise_px: 0, ..TraceOptions::default() }
}
#[test]
fn rectangle_has_four_exact_corners() {
    let p = raster(32, 32, |x, y| if (4..28).contains(&x) && (6..26).contains(&y) { [0, 0, 0, 255] } else { [255; 4] });
    let (paths, r) = trace(&p, 32, 32, &bw()).unwrap();
    assert_eq!(r.paths, 1);
    assert_eq!(r.anchors, 4);
    assert!(paths[0].anchors.iter().all(|a| a.hin.is_none() && a.hout.is_none()));
}
#[test]
fn circle_is_compact_and_within_two_pixels() {
    let p = raster(80, 80, |x, y| {
        if (x as f64 + 0.5 - 40.).hypot(y as f64 + 0.5 - 40.) <= 28. {
            [0, 0, 0, 255]
        } else {
            [255; 4]
        }
    });
    let (paths, r) = trace(&p, 80, 80, &TraceOptions { corners: 20., ..bw() }).unwrap();
    assert!(r.anchors <= 32, "{}", r.anchors);
    assert!(paths[0].anchors.iter().any(|a| a.hout.is_some()));
    let pts = crate::model::Document::ring(&paths[0].anchors, true, 32);
    for p in pts {
        assert!(((p[0] - 40.).hypot(p[1] - 40.) - 28.).abs() < 2.);
    }
}
#[test]
fn holes_and_nested_islands_are_preserved() {
    let p = raster(40, 40, |x, y| {
        let d = (x as f64 - 20.).hypot(y as f64 - 20.);
        if (8. ..=16.).contains(&d) || d <= 3. {
            [0, 0, 0, 255]
        } else {
            [255; 4]
        }
    });
    let (paths, r) = trace(&p, 40, 40, &bw()).unwrap();
    assert_eq!(r.paths, 2);
    assert_eq!(r.holes, 1);
    assert_eq!(paths.iter().map(|p| p.holes.len()).sum::<usize>(), 1);
}
#[test]
fn colors_and_determinism() {
    let p = raster(30, 10, |x, _| match x / 10 {
        0 => [255, 0, 0, 255],
        1 => [0, 255, 0, 255],
        _ => [0, 0, 255, 255],
    });
    let o = TraceOptions { mode: TraceMode::Color { colors: 3 }, ..bw() };
    let a = trace(&p, 30, 10, &o).unwrap();
    let b = trace(&p, 30, 10, &o).unwrap();
    assert_eq!(a.1, b.1);
    assert_eq!(serde_json::to_value(a.0).unwrap(), serde_json::to_value(b.0).unwrap());
    assert_eq!(a.1.paths, 3);
}
#[test]
fn diagonal_pixels_stay_separate() {
    let p = raster(2, 2, |x, y| if x == y { [0, 0, 0, 255] } else { [255; 4] });
    assert_eq!(trace(&p, 2, 2, &bw()).unwrap().1.paths, 2);
}
#[test]
fn noise_merges_into_background() {
    let p = raster(9, 9, |x, y| if x == 4 && y == 4 { [0, 0, 0, 255] } else { [255; 4] });
    assert_eq!(trace(&p, 9, 9, &bw()).unwrap().1.paths, 1);
    assert_eq!(trace(&p, 9, 9, &TraceOptions { noise_px: 2, ..bw() }).unwrap().1.paths, 0);
}
#[test]
fn transparency_and_white_policy() {
    let p = raster(4, 4, |x, _| if x < 2 { [0, 0, 0, 0] } else { [255; 4] });
    let (_, r) = trace(&p, 4, 4, &bw()).unwrap();
    assert_eq!(r.transparent_pixels, 8);
    assert_eq!(r.paths, 0);
    assert_eq!(trace(&p, 4, 4, &TraceOptions { ignore_white: false, ..bw() }).unwrap().1.paths, 1);
}
#[test]
fn grayscale_and_one_color() {
    let p = raster(24, 4, |x, _| [x as u8 * 10, x as u8 * 10, x as u8 * 10, 255]);
    let (_, r) = trace(&p, 24, 4, &TraceOptions { mode: TraceMode::Grayscale, ignore_white: false, ..bw() }).unwrap();
    assert!(r.palette.len() <= 8 && r.palette.len() > 1);
    assert_eq!(trace(&p, 24, 4, &TraceOptions { mode: TraceMode::Color { colors: 1 }, ..bw() }).unwrap().1.paths, 1);
}
#[test]
fn invalid_inputs_are_errors() {
    assert!(trace(&[], 0, 0, &bw()).is_err());
    assert!(trace(&[0; 4], u32::MAX, u32::MAX, &bw()).is_err());
    assert!(trace(&[0; 3], 1, 1, &bw()).is_err());
    assert!(trace(&[0; 4], 1, 1, &TraceOptions { corners: f64::NAN, ..bw() }).is_err());
    assert!(trace(&[0; 4], 1, 1, &TraceOptions { mode: TraceMode::Color { colors: 0 }, ..bw() }).is_err());
}
#[test]
fn insertion_is_one_undo_and_remaps_holes() {
    use crate::{command::EditCommand, editor::Editor};
    let p = raster(8, 8, |x, y| if (2..6).contains(&x) && (2..6).contains(&y) { [255; 4] } else { [0, 0, 0, 255] });
    let (paths, _) = trace(&p, 8, 8, &bw()).unwrap();
    let mut e = Editor::new();
    e.try_execute(EditCommand::InsertTracedPaths { paths }).unwrap();
    assert_eq!(e.doc.paths.len(), 1);
    assert_eq!(e.doc.paths[0].holes.len(), 1);
    let mut ids = std::collections::BTreeSet::new();
    assert!(ids.insert(e.doc.paths[0].id));
    for a in e.doc.paths[0].anchors.iter().chain(e.doc.paths[0].holes.iter().flatten()) {
        assert!(ids.insert(a.id));
    }
    e.execute(EditCommand::Undo);
    assert!(e.doc.paths.is_empty());
    e.execute(EditCommand::Redo);
    assert_eq!(e.doc.paths[0].holes.len(), 1);
}

#[test]
fn threshold_is_an_exact_cutoff() {
    let p = [127, 127, 127, 255, 128, 128, 128, 255];
    let (paths, report) = trace(&p, 2, 1, &bw()).unwrap();
    assert_eq!(report.paths, 1);
    assert_eq!(paths[0].anchors.iter().map(|a| a.p[0]).fold(0., f32::max), 1.);
}
#[test]
fn empty_palette_in_all_modes() {
    for mode in [TraceMode::BlackWhite, TraceMode::Grayscale, TraceMode::Color { colors: 255 }] {
        let (_, report) = trace(&[0; 16], 2, 2, &TraceOptions { mode, ignore_white: false, ..bw() }).unwrap();
        assert_eq!(report.paths, 0);
        assert!(report.palette.is_empty());
    }
}
#[test]
fn one_pixel_has_a_valid_closed_ring() {
    let (paths, report) = trace(&[0, 0, 0, 255], 1, 1, &bw()).unwrap();
    assert_eq!(report.anchors, 4);
    assert!(paths[0].closed);
}
#[test]
fn fidelity_retains_more_detail() {
    let p = raster(80, 80, |x, y| if (x as f64 - 40.).hypot(y as f64 - 40.) < 28. { [0, 0, 0, 255] } else { [255; 4] });
    let (_, lo) = trace(&p, 80, 80, &TraceOptions { paths_fidelity: 0., ..bw() }).unwrap();
    let (_, hi) = trace(&p, 80, 80, &TraceOptions { paths_fidelity: 100., ..bw() }).unwrap();
    assert!(hi.anchors >= lo.anchors);
}
#[test]
fn invalid_insert_is_not_a_partial_edit() {
    use crate::{command::EditCommand, editor::Editor};
    let (mut paths, _) = trace(&[0, 0, 0, 255], 1, 1, &bw()).unwrap();
    paths[0].anchors[0].p[0] = f32::INFINITY;
    let mut e = Editor::new();
    let rev = e.rev;
    assert!(e.try_execute(EditCommand::InsertTracedPaths { paths }).is_err());
    assert_eq!(e.rev, rev);
    assert!(e.doc.paths.is_empty());
    assert!(!e.transaction_open());
}

#[test]
fn exhaustive_three_by_three_masks_preserve_pixel_area() {
    for bits in 0..512u32 {
        let mask: Vec<bool> = (0..9).map(|i| bits & (1 << i) != 0).collect();
        let components = contour::trace_mask(&mask, 3, 3);
        let area2: i64 = components.iter().map(|c| c.outer.area2 + c.holes.iter().map(|h| h.area2).sum::<i64>()).sum();
        assert_eq!(area2, 2 * i64::from(bits.count_ones()), "mask {bits}");
        assert_eq!(components.iter().map(|c| c.pixels).sum::<usize>(), bits.count_ones() as usize);
    }
}
#[test]
fn two_holes_are_attached_to_one_outer_path() {
    let p = raster(24, 12, |x, y| {
        if (3..8).contains(&x) && (3..9).contains(&y) || (15..20).contains(&x) && (3..9).contains(&y) {
            [255; 4]
        } else {
            [0, 0, 0, 255]
        }
    });
    let (paths, r) = trace(&p, 24, 12, &bw()).unwrap();
    assert_eq!(r.paths, 1);
    assert_eq!(r.holes, 2);
    assert_eq!(paths[0].holes.len(), 2);
}

#[test]
fn contour_budget_refuses_before_fitting() {
    assert!(contour::trace_mask_within(&[true, false, false, true], 2, 2, 1).is_err());
    let mask: Vec<bool> =
        (0..8).flat_map(|y| (0..8).map(move |x| !((2..6).contains(&x) && (2..6).contains(&y)))).collect();
    assert!(contour::trace_mask_within(&mask, 8, 8, 1).is_err());
}

fn assert_insert_refused_without_mutation(mut e: crate::editor::Editor, paths: Vec<Path>) {
    let before = e.doc.clone();
    let rev = e.rev;
    let ids = e.allocation_floor();
    let selection = e.objsel.clone();
    let undo = e.history_preview(false).cloned();
    let redo = e.history_preview(true).cloned();
    assert!(e.try_execute(crate::command::EditCommand::InsertTracedPaths { paths }).is_err());
    assert_eq!(e.doc, before);
    assert_eq!(e.rev, rev);
    assert_eq!(e.allocation_floor(), ids);
    assert_eq!(e.objsel, selection);
    assert_eq!(e.history_preview(false), undo.as_ref());
    assert_eq!(e.history_preview(true), redo.as_ref());
    assert!(!e.transaction_open());
}
fn nested_destination(hidden: bool, locked: bool) -> crate::editor::Editor {
    let mut e = crate::editor::Editor::new();
    let root = e.doc.active_layer;
    let mut child = e.doc.node(root).unwrap().clone();
    child.id = 500;
    child.parent = Some(root);
    child.children.clear();
    e.doc.nodes.push(child);
    let parent = e.doc.nodes.iter_mut().find(|n| n.id == root).unwrap();
    parent.children.push(500);
    parent.hidden = hidden;
    parent.locked = locked;
    e.doc.active_layer = 500;
    e.doc.ids = 501;
    e
}
#[test]
fn insertion_rejects_locked_parent_without_mutation() {
    let (paths, _) = trace(&[0, 0, 0, 255], 1, 1, &bw()).unwrap();
    assert_insert_refused_without_mutation(nested_destination(false, true), paths);
}
#[test]
fn insertion_rejects_hidden_parent_without_mutation() {
    let (paths, _) = trace(&[0, 0, 0, 255], 1, 1, &bw()).unwrap();
    assert_insert_refused_without_mutation(nested_destination(true, false), paths);
}
#[test]
fn insertion_accepts_unprotected_sublayer_and_refuses_malformed_ancestors() {
    let (paths, _) = trace(&[0, 0, 0, 255], 1, 1, &bw()).unwrap();
    let mut e = nested_destination(false, false);
    e.try_execute(crate::command::EditCommand::InsertTracedPaths { paths: paths.clone() }).unwrap();
    assert_eq!(e.doc.paths.len(), 1);
    for parent in [500, 999] {
        let mut e = nested_destination(false, false);
        e.doc.nodes[0].parent = Some(parent);
        assert_insert_refused_without_mutation(e, paths.clone());
    }
}
#[test]
fn insertion_requires_no_stroke_even_at_zero_width() {
    for stroke in [crate::model::Paint::Solid([2., 0., 0., 1.]), crate::model::Paint::Solid([1.; 4])] {
        let (mut paths, _) = trace(&[0, 0, 0, 255], 1, 1, &bw()).unwrap();
        paths[0].stroke = stroke;
        assert_eq!(paths[0].stroke_width, 0.);
        assert_insert_refused_without_mutation(crate::editor::Editor::new(), paths);
    }
}
