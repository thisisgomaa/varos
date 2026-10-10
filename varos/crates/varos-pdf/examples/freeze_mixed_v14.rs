//! One-shot generation of the frozen mixed format-14 fixture (integration w3): ONE document that
//! carries every wave-2 and wave-3 model addition — placed image, gradient fill + Live Corners, an
//! appearance stack (extra stroke entry) with a live effect, a global CMYK swatch referenced by a
//! path, a live Repeat node, and styled area text bound to a shape. Tests compare against these
//! bytes and never regenerate them.
use std::{collections::BTreeMap, sync::Arc};
use varos_core::{
    appearance_edits::AppearanceEdit,
    colour_management::{Colour, ManagedColour},
    effects::{self, Effect},
    format::{encode_model, Limits},
    images::{codec, links, Pixels, PlacementMode},
    live::{self, Axis, Kind, Repeat},
    live_corners::{CornerParam, Kind as CornerKind},
    model::{Paint, ShapeKind},
    typography::{self, Binding, CharacterStyle, ParagraphStyle},
    EditCommand, Editor,
};

fn rect(ed: &mut Editor, bounds: [f32; 4], fill: [f32; 4]) -> Result<u32, Box<dyn std::error::Error>> {
    ed.try_execute(EditCommand::AddShape {
        kind: ShapeKind::Rect,
        bounds,
        parent: None,
        fill: Some(fill),
        stroke: None,
        stroke_width: 0.,
        opacity: 1.,
        name: None,
    })?;
    Ok(ed.doc.paths.last().ok_or("no path")?.id)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v14-mixed");
    let mut ed = Editor::new();
    // v6 image
    let png =
        codec::encode_png(&Pixels { budget: None, width: 2, height: 2, rgba: Arc::from([255, 0, 0, 255].repeat(4)) })?;
    links::place_bytes(&mut ed, &png, [10., 10.], None, PlacementMode::Embed, None)?;
    // v7 gradient + v9 corners + v10 appearance stack + v11 live effect, on one path
    let a = rect(&mut ed, [40., 40., 140., 100.], [0., 0., 1., 1.])?;
    let path = ed.doc.paths.iter_mut().find(|p| p.id == a).ok_or("no path")?;
    path.fill = Paint::Gradient(varos_core::gradient::Gradient::default());
    path.corners = vec![CornerParam { radius: 8., kind: CornerKind::Round }; path.anchors.len()];
    ed.try_execute(EditCommand::Appearance(AppearanceEdit::AddStroke {
        path: a,
        paint: Paint::Solid([0.1, 0.1, 0.1, 1.]),
        width: 3.,
    }))?;
    ed.try_execute(EditCommand::LiveEffects(effects::Action::Set {
        ids: vec![a],
        effects: vec![Effect::ZigZag { size: 2., ridges: 3, smooth: false }],
    }))?;
    // v12 managed colour: a global CMYK swatch, referenced by a path
    let swatch = ed.doc.nid();
    ed.doc.swatches.push(varos_core::swatches::Swatch {
        id: swatch,
        name: "Process Cyan".into(),
        paint: Paint::Managed(ManagedColour { colour: Colour::Cmyk { c: 1., m: 0., y: 0., k: 0. }, alpha: 1. }),
        global: true,
        group: String::new(),
    });
    let b = rect(&mut ed, [160., 40., 220., 100.], [0., 0., 0., 1.])?;
    ed.doc.paths.iter_mut().find(|p| p.id == b).ok_or("no path")?.fill = Paint::SwatchRef { id: swatch };
    // v13 live node: a mirrored repeat
    let c = rect(&mut ed, [40., 120., 80., 150.], [0., 0.6, 0.2, 1.])?;
    ed.try_execute(EditCommand::Live(live::Action::Make {
        paths: vec![c],
        kind: Kind::Repeat { repeat: Repeat::Mirror { axis: Axis::Vertical } },
    }))?;
    // v8 text + v14 typography: named styles + OpenType features, area text flowed into a shape
    let area = rect(&mut ed, [20., 170., 220., 260.], [0.95, 0.95, 0.95, 1.])?;
    let text = varos_text_layout::default_text("Varos v14 نص عربي", [24., 190.])?;
    ed.try_execute(EditCommand::AddText { text, parent: None })?;
    let tid = ed.doc.text_boxes.last().ok_or("no text")?.id;
    let style = ed.doc.text_boxes.last().ok_or("no text")?.runs[0].style.clone();
    let para = ed.doc.text_boxes.last().ok_or("no text")?.para.clone();
    let len = ed.doc.text_boxes.last().ok_or("no text")?.source().chars().count();
    for action in [
        typography::Action::DefineCharacter {
            name: "Body".into(),
            definition: CharacterStyle { parent: None, style: Some(style) },
        },
        typography::Action::DefineParagraph {
            name: "Lead".into(),
            definition: ParagraphStyle { parent: None, style: Some(para) },
        },
        typography::Action::ApplyCharacter { text: tid, start: 0, end: len, name: "Body".into() },
        typography::Action::ApplyParagraph { text: tid, name: "Lead".into() },
        typography::Action::Features { text: tid, features: BTreeMap::from([("liga".to_owned(), 1)]) },
        typography::Action::Bind { text: tid, binding: Some(Binding::Area { path: area, inset: 4. }) },
    ] {
        ed.try_execute(EditCommand::Typography(action))?;
    }
    let json = encode_model(&ed.doc, &Limits::DEFAULT)?;
    std::fs::write(root.join("mixed.json"), json.as_bytes())?;
    std::fs::write(root.join("mixed.vrs"), varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Limits::DEFAULT)?)?;
    Ok(())
}
