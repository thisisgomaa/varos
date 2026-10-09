//! Lane F: typed Bridge invocations to portable selection-bound action steps.
use crate::dto::{Operation, Paint};
use varos_core::actions::Step;
pub(crate) fn steps(operation: &Operation) -> Result<Vec<Step>, String> {
    fn colour(paint: &Paint) -> Result<Option<varos_core::geom::Rgba>, String> {
        match paint {
            Paint::None => Ok(None),
            Paint::Solid(text)
                if text.len() == 9 && text.starts_with('#') && text[1..].bytes().all(|b| b.is_ascii_hexdigit()) =>
            {
                let mut rgba = [0.; 4];
                for (i, value) in rgba.iter_mut().enumerate() {
                    *value =
                        u8::from_str_radix(&text[1 + i * 2..3 + i * 2], 16).map_err(|e| e.to_string())? as f32 / 255.;
                }
                Ok(Some(rgba))
            }
            _ => Err("Unsupported action paint".into()),
        }
    }
    Ok(match operation {
        Operation::Move { delta, .. } => vec![Step::Nudge { delta_pt: *delta }],
        Operation::SetPaint { fill, stroke, stroke_width, opacity, stroke_style, .. } => {
            if stroke_style.is_some() {
                return Err("Stroke style recording is unavailable".into());
            }
            let mut steps = vec![];
            for (fill_target, paint) in [(true, fill), (false, stroke)] {
                if !matches!(paint, Paint::Unchanged) {
                    steps.push(Step::Paint { fill: fill_target, colour: colour(paint)? });
                }
            }
            if let Some(value) = stroke_width {
                steps.push(Step::StrokeWidth { value: *value });
            }
            if let Some(value) = opacity {
                steps.push(Step::Opacity { value: *value });
            }
            steps
        }
        Operation::Rotate { degrees, .. } => vec![Step::Rotation { degrees: *degrees }],
        Operation::Delete { .. } => vec![Step::Delete {}],
        Operation::Group { local: None, .. } => vec![Step::Group {}],
        Operation::Ungroup { .. } => vec![Step::Ungroup {}],
        _ => return Err("Document operation is not recordable".into()),
    })
}
