//! Lane D: API 1.2 drawing verbs; deterministic core command shared with the CLI apply host.
use crate::dto::{Error, Operation};
pub(crate) const VERBS: &[&str] =
    &["shape_tool", "pencil", "smooth_path", "path_erase", "join_tool", "curvature", "drawing_options"];
use std::collections::BTreeSet;
use varos_core::{drawing::Action, EditCommand, Editor};
pub(crate) fn apply(
    ed: &mut Editor,
    op: &Operation,
    paths: Vec<u32>,
    affected: &mut BTreeSet<String>,
) -> Result<Option<u32>, Error> {
    let action = match op {
        Operation::ShapeTool { spec } => Action::Shape { spec: *spec },
        Operation::Pencil { points, options } => Action::Pencil { points: points.clone(), options: *options },
        Operation::SmoothPath { points, options, .. } => Action::Smooth { points: points.clone(), options: *options },
        Operation::PathErase { points, options, .. } => Action::PathErase { points: points.clone(), options: *options },
        Operation::JoinTool { points, options, .. } => Action::Join { points: points.clone(), options: *options },
        Operation::Curvature { points, closed } => Action::Curvature { points: points.clone(), closed: *closed },
        Operation::DrawingOptions { options } => Action::Options { options: *options },
        _ => return Err(Error::new("invalid_argument", "unknown drawing operation")),
    };
    let before: BTreeSet<_> = ed.doc.paths.iter().map(|p| p.id).collect();
    ed.try_execute(EditCommand::SelectPaths(paths.clone())).map_err(|e| Error::new("invalid_argument", e))?;
    ed.try_execute(EditCommand::Drawing(action)).map_err(|e| Error::new("invalid_argument", e))?;
    affected.extend(paths.iter().map(|p| format!("path:{p}")));
    affected.extend(ed.doc.paths.iter().filter(|p| !before.contains(&p.id)).map(|p| format!("path:{}", p.id)));
    Ok(None)
}
pub(crate) fn schemas(defs: &mut serde_json::Map<String, serde_json::Value>, ops: &mut Vec<serde_json::Value>) {
    use serde_json::json;
    let point =
        json!({"type":"array","minItems":2,"maxItems":2,"items":{"type":"number","minimum":-1e7,"maximum":1e7}});
    let samples = json!({"type":"array","minItems":2,"maxItems":16384,"items":point});
    let options = json!({"type":"object","additionalProperties":false,"properties":{"fidelity":{"type":"number","minimum":0.01,"maximum":100},"smoothness":{"type":"number","minimum":0,"maximum":1},"endpoint_distance":{"type":"number","minimum":0,"maximum":1000},"brush_radius":{"type":"number","minimum":0.01,"maximum":1000}}});
    let spec = json!({"type":"object","additionalProperties":false,"properties":{"kind":{"enum":["rectangle","ellipse","rounded_rectangle","polygon","star","line","arc","spiral","rectangular_grid","polar_grid"]},"origin":point,"size":point,"radius":{"type":"number","minimum":0},"sides":{"type":"integer","minimum":2,"maximum":1000},"inner_ratio":{"type":"number","minimum":0,"maximum":1},"rotation":{"type":"number"},"sweep":{"type":"number"},"turns":{"type":"number","minimum":-100,"maximum":100},"decay":{"type":"number","minimum":0.0001,"maximum":1},"rows":{"type":"integer","minimum":1,"maximum":1000},"columns":{"type":"integer","minimum":1,"maximum":1000},"centre":{"type":"boolean"}}});
    let ids = json!({"type":"array","minItems":1,"maxItems":1000,"items":{"type":"string"}});
    for (verb, fields, required, description) in [
        (
            "shape_tool",
            json!({"spec":spec}),
            vec!["spec"],
            "Draw a numeric shape, arc, spiral or grid as ordinary paths.",
        ),
        (
            "pencil",
            json!({"points":samples,"options":options}),
            vec!["points", "options"],
            "Fit a Pencil stroke; continue an open endpoint within the specified distance.",
        ),
        (
            "smooth_path",
            json!({"ids":ids,"points":samples,"options":options}),
            vec!["ids", "points", "options"],
            "Smooth the targeted path anchors touched by the stroke.",
        ),
        (
            "path_erase",
            json!({"ids":ids,"points":samples,"options":options}),
            vec!["ids", "points", "options"],
            "Erase portions of targeted strokes, preserving remaining curves.",
        ),
        (
            "join_tool",
            json!({"ids":ids,"points":samples,"options":options}),
            vec!["ids", "points", "options"],
            "Join open targeted paths touched by the stroke.",
        ),
        (
            "curvature",
            json!({"points":samples,"closed":{"type":"boolean"}}),
            vec!["points", "closed"],
            "Draw an interpolating path through clicked points.",
        ),
        (
            "drawing_options",
            json!({"options":options}),
            vec!["options"],
            "Set session Pencil fidelity/smoothness and stroke brush options.",
        ),
    ] {
        let mut properties = fields.as_object().cloned().unwrap_or_default();
        properties.insert("verb".into(), json!({"const":verb}));
        let mut required = required;
        required.insert(0, "verb");
        defs.insert(verb.into(),json!({"type":"object","additionalProperties":false,"properties":properties,"required":required,"description":description}));
        ops.push(json!({"$ref":format!("#/$defs/{verb}")}));
    }
}
