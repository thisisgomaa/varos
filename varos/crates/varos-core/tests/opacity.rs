//! Object-opacity → Group building (pure, render-agnostic logic — allowed per the math-test rule).
//! Locks the decision behind Ahmed's "#1 most dangerous" bug: an object with opacity<1 that has BOTH a
//! fill and a stroke must become ONE isolated layer (so fill+stroke composite as a unit, then fade
//! together — no double-blend). A translucent object with only a fill (or only a stroke) has no overlap to
//! double-blend, so it just folds opacity into that colour's alpha and stays on the fast opaque path.
//! Layers keep their z-position (opaque-behind stays behind). This is the core half of the renderer fix.

use varos_core::geom::Rgba;
use varos_core::model::{Anchor, Path};
use varos_core::scene::{build_scene, Group, Prim};
use varos_core::editor::Editor;

fn anc(id: u32, x: f32, y: f32) -> Anchor { Anchor { id, p: [x, y], hin: None, hout: None, smooth: false } }
fn rect(id: u32, base: u32, fill: Option<Rgba>, stroke: Option<Rgba>) -> Path {
    Path::new(id, vec![anc(base, 0.0, 0.0), anc(base + 1, 100.0, 0.0), anc(base + 2, 100.0, 100.0), anc(base + 3, 0.0, 100.0)],
              true, fill, stroke, 4.0)
}
/// An editor with NO artboards (so `content` groups come only from the paths we add — no page-fill noise).
fn bare() -> Editor { let mut ed = Editor::new(); ed.doc.artboards.clear(); ed.ppu = 1.0; ed }

fn n_isolated(groups: &[Group]) -> usize { groups.iter().filter(|g| matches!(g, Group::Isolated { .. })).count() }

const BLUE: Rgba = [0.2, 0.4, 0.8, 1.0];
const BLACK: Rgba = [0.0, 0.0, 0.0, 1.0];

#[test]
fn opaque_fill_and_stroke_never_isolate() {
    let mut ed = bare();
    ed.doc.paths.push(rect(1, 1, Some(BLUE), Some(BLACK)));   // opacity defaults to 1.0
    let s = build_scene(&ed, 1.0);
    assert_eq!(n_isolated(&s.content), 0, "a fully-opaque object must stay on the fast opaque path");
}

#[test]
fn translucent_fill_plus_stroke_becomes_one_isolated_layer() {
    let mut ed = bare();
    ed.doc.paths.push(rect(1, 1, Some(BLUE), Some(BLACK)));
    ed.doc.paths[0].opacity = 0.5;
    let s = build_scene(&ed, 1.0);
    assert_eq!(n_isolated(&s.content), 1, "opacity<1 with BOTH fill+stroke must isolate as a group");
    let iso = s.content.iter().find_map(|g| match g { Group::Isolated { opacity, prims } => Some((opacity, prims)), _ => None }).unwrap();
    assert!((iso.0 - 0.5).abs() < 1e-6, "the layer carries the object opacity (0.5), got {}", iso.0);
    // inside the layer the colours are FULL alpha (opacity is applied once, at composite) and fill precedes stroke
    let has_full_fill = iso.1.iter().any(|p| matches!(p, Prim::Fill { color, .. } if (color[3] - 1.0).abs() < 1e-6));
    let has_full_stroke = iso.1.iter().any(|p| matches!(p, Prim::Stroke { color, .. } if (color[3] - 1.0).abs() < 1e-6));
    assert!(has_full_fill && has_full_stroke, "layer prims keep full colour alpha (no per-prim opacity baked in)");
    assert!(matches!(iso.1[0], Prim::Fill { .. }), "fill draws before stroke inside the layer");
}

#[test]
fn translucent_fill_only_folds_opacity_no_layer() {
    let mut ed = bare();
    ed.doc.paths.push(rect(1, 1, Some(BLUE), None));   // fill only — no overlap, no double-blend
    ed.doc.paths[0].opacity = 0.5;
    let s = build_scene(&ed, 1.0);
    assert_eq!(n_isolated(&s.content), 0, "a single-primitive translucent object must NOT allocate a layer");
    // opacity folded into the fill's own alpha: 1.0 * 0.5 = 0.5
    let fill_a = s.content.iter().flat_map(|g| g.prims()).find_map(|p| match p { Prim::Fill { color, .. } => Some(color[3]), _ => None }).unwrap();
    assert!((fill_a - 0.5).abs() < 1e-6, "opacity folds into the single fill's alpha, got {}", fill_a);
}

#[test]
fn opaque_objects_paint_fill_then_own_stroke_in_z_order() {
    // Illustrator stacking: each object paints fill → ITS stroke at its own z-slot, so an object above
    // covers the stroke of the one below. (The old renderer floated ALL strokes above ALL fills.)
    let mut ed = bare();
    ed.doc.paths.push(rect(1, 1, Some(BLUE), Some(BLACK)));
    ed.doc.paths.push(rect(2, 10, Some(BLUE), Some(BLACK)));
    let s = build_scene(&ed, 1.0);
    assert_eq!(s.content.len(), 1, "two opaque objects share one opaque run");
    let kinds: Vec<char> = s.content[0].prims().iter()
        .map(|p| match p { Prim::Fill { .. } => 'F', Prim::Stroke { .. } => 'S', _ => '?' }).collect();
    assert_eq!(kinds, ['F', 'S', 'F', 'S'], "per-object paint order (fill, own stroke, next object…), got {:?}", kinds);
}

#[test]
fn translucent_stroke_on_filled_object_becomes_knockout() {
    // colour-level stroke alpha < 1 on a filled object → the band must knock out the fill beneath it
    // (blend against what's BEHIND the object), so scene building routes it to Group::Knockout.
    let mut ed = bare();
    ed.doc.paths.push(rect(1, 1, Some(BLUE), Some([0.0, 0.0, 0.0, 0.5])));
    let s = build_scene(&ed, 1.0);
    assert!(s.content.iter().any(|g| matches!(g, Group::Knockout(_))), "fill + translucent stroke must emit a Knockout group");
    assert_eq!(n_isolated(&s.content), 0, "colour alpha alone (object opacity = 1) needs no isolated layer");
}

#[test]
fn isolated_layer_keeps_its_z_position() {
    let mut ed = bare();
    ed.doc.paths.push(rect(1, 1, Some(BLACK), None));          // opaque, BEHIND
    ed.doc.paths.push(rect(2, 10, Some(BLUE), Some(BLACK)));   // translucent fill+stroke, IN FRONT
    ed.doc.paths[1].opacity = 0.5;
    let s = build_scene(&ed, 1.0);
    // the opaque object must come first (drawn, then the layer composites on top) — z-order preserved
    assert!(matches!(s.content.first(), Some(Group::Opaque(_))), "opaque-behind renders before the layer");
    assert!(matches!(s.content.last(), Some(Group::Isolated { .. })), "the translucent-in-front object is the last group");
    assert_eq!(n_isolated(&s.content), 1);
}
