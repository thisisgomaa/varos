//! Integration w2 review (Astra, FIX-THEN-MERGE) regressions.
use varos_core::{
    model::{Paint, ShapeKind},
    scene::{build_scene, Prim},
    EditCommand, Editor,
};

fn stroked(gradient: bool) -> Editor {
    let mut ed = Editor::new();
    ed.try_execute(EditCommand::AddShape {
        kind: ShapeKind::Rect,
        bounds: [10., 10., 110., 90.],
        parent: None,
        fill: None,
        stroke: Some([0., 0., 0., 1.]),
        stroke_width: 2.,
        opacity: 1.,
        name: None,
    })
    .unwrap();
    let p = ed.doc.paths.last_mut().unwrap();
    // Far beyond the canvas element cap: the hotfix gives up and draws the native fallback.
    p.stroke_style.dash = vec![0.0001, 0.0001];
    if gradient {
        p.stroke = Paint::Gradient(varos_core::gradient::Gradient::default());
    }
    ed
}

fn fallback_strokes(ed: &Editor) -> (Vec<[f32; 4]>, bool) {
    let scene = build_scene(ed, 1.);
    assert!(scene.errors.is_empty(), "{:?}", scene.errors);
    let colours = scene
        .content
        .iter()
        .flat_map(|g| g.prims())
        .filter_map(|p| match p {
            Prim::Stroke { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    (colours, scene.report.notes.iter().any(|n| n.kind == "stroke_simplified"))
}

/// P1: a gradient stroke over the canvas budget stays visible (native fallback in the gradient's
/// representative colour) exactly like a solid one, with the simplified note.
#[test]
fn over_budget_gradient_stroke_draws_the_native_fallback() {
    let (solid, solid_note) = fallback_strokes(&stroked(false));
    assert!(!solid.is_empty() && solid_note, "solid baseline");
    let ed = stroked(true);
    let (gradient, note) = fallback_strokes(&ed);
    assert_eq!(gradient.len(), solid.len(), "gradient fallback mirrors the solid fallback");
    assert!(note, "the muted simplified note is recorded");
    let expected = ed.doc.paths.last().unwrap().stroke.resolved(&ed.doc).representative().unwrap();
    assert!(gradient.iter().all(|c| *c == expected));
}
