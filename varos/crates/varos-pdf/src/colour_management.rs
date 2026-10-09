//! Lane C: PDF process and named colourants, built directly on pdf-writer.
use crate::write::Alloc;
use pdf_writer::{Content, Finish, Name, Pdf, Ref, Str};
use varos_core::{
    colour_management::{Cmyk, Colour},
    model::{Document, Paint},
    ExportNote,
};
fn spot_key(name: &str) -> String {
    let mut s = "Spot".to_owned();
    for b in name.bytes() {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
pub(crate) struct Resources {
    pub spaces: Vec<(String, Ref)>,
    pub intent: Option<Ref>,
}
pub(crate) fn resources(doc: &Document, pdf: &mut Pdf, ids: &mut Alloc) -> Resources {
    let mut spaces = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for paint in doc.paths.iter().flat_map(|p| [&p.fill, &p.stroke]).chain(doc.swatches.iter().map(|s| &s.paint)) {
        if let Paint::Managed(m) = paint.resolved_ref(doc) {
            if let Colour::Spot { name, alt, .. } = &m.colour {
                if !seen.insert(name.clone()) {
                    continue;
                }
                let fun = ids.next();
                let mut d = pdf.indirect(fun).dict();
                d.pair(Name(b"FunctionType"), 2);
                d.insert(Name(b"Domain")).array().items([0., 1.]);
                d.insert(Name(b"C0")).array().items([0.; 4]);
                d.insert(Name(b"C1")).array().items(alt.channels());
                d.pair(Name(b"N"), 1);
                d.finish();
                let space = ids.next();
                pdf.indirect(space)
                    .array()
                    .item(Name(b"Separation"))
                    .item(Name(name.as_bytes()))
                    .item(Name(b"DeviceCMYK"))
                    .item(fun);
                spaces.push((spot_key(name), space));
            }
        }
    }
    let mut intent = None;
    if let Some(profile) = &doc.output_profile {
        // Model validation ran before writing. A matching source process profile is explicit.
        if let (Ok(parsed), Ok(bytes)) = (profile.parse(), profile.bytes()) {
            let n = match parsed.color_space {
                moxcms::DataColorSpace::Rgb => 3,
                moxcms::DataColorSpace::Cmyk => 4,
                moxcms::DataColorSpace::Gray => 1,
                _ => 0,
            };
            if n > 0 {
                let stream = ids.next();
                pdf.stream(stream, &bytes).pair(Name(b"N"), n);
                let space = ids.next();
                pdf.indirect(space).array().item(Name(b"ICCBased")).item(stream);
                spaces.push((format!("ICC{n}"), space));
                let output = ids.next();
                pdf.indirect(output)
                    .dict()
                    .pair(Name(b"Type"), Name(b"OutputIntent"))
                    .pair(Name(b"S"), Name(b"GTS_PDFX"))
                    .pair(Name(b"OutputConditionIdentifier"), Str(profile.name.as_bytes()))
                    .pair(Name(b"DestOutputProfile"), stream);
                intent = Some(output);
            }
        }
    }
    Resources { spaces, intent }
}
pub(crate) fn set(c: &mut Content, doc: &Document, paint: &Paint, rgba: [f32; 4], stroke: bool) {
    let colour = match paint.resolved_ref(doc) {
        Paint::Managed(m)
            if doc.colour_mode == varos_core::colour_management::ColourMode::Cmyk
                && matches!(m.colour, Colour::Rgb { .. }) =>
        {
            let Cmyk { c, m, y, k } = Cmyk::from_rgb(rgba);
            Some(Colour::Cmyk { c, m, y, k })
        }
        Paint::Managed(m) => Some(m.colour.clone()),
        Paint::Solid(_) if doc.colour_mode == varos_core::colour_management::ColourMode::Cmyk => {
            let Cmyk { c, m, y, k } = Cmyk::from_rgb(rgba);
            Some(Colour::Cmyk { c, m, y, k })
        }
        _ => None,
    };
    let Some(colour) = colour else {
        if stroke {
            c.set_stroke_rgb(rgba[0], rgba[1], rgba[2]);
        } else {
            c.set_fill_rgb(rgba[0], rgba[1], rgba[2]);
        }
        return;
    };
    let (space, values) = match colour {
        Colour::Rgb { r, g, b } => ("DeviceRGB".to_owned(), vec![r, g, b]),
        Colour::Cmyk { c, m, y, k } => ("DeviceCMYK".to_owned(), vec![c, m, y, k]),
        Colour::Gray { value } => ("DeviceGray".to_owned(), vec![value]),
        Colour::Spot { name, tint, .. } => (spot_key(&name), vec![tint]),
    };
    let space = if !space.starts_with("Spot")
        && doc.output_profile.as_ref().is_some_and(|p| {
            p.data.get(32..40).is_some_and(|s| match values.len() {
                3 => s.eq_ignore_ascii_case("52474220"),
                4 => s.eq_ignore_ascii_case("434d594b"),
                1 => s.eq_ignore_ascii_case("47524159"),
                _ => false,
            })
        }) {
        format!("ICC{}", values.len())
    } else {
        space
    };
    if stroke {
        c.set_stroke_color_space(Name(space.as_bytes())).set_stroke_color(values);
    } else {
        c.set_fill_color_space(Name(space.as_bytes())).set_fill_color(values);
    }
}
pub(crate) fn notes(doc: &Document) -> Vec<ExportNote> {
    let mut notes = Vec::new();
    for p in &doc.paths {
        for (slot, paint) in [("fill", &p.fill), ("stroke", &p.stroke)] {
            let message = match paint.resolved_ref(doc) {
                Paint::Solid(_) if doc.colour_mode == varos_core::colour_management::ColourMode::Cmyk => {
                    Some(if doc.output_profile.is_some() {
                        format!("PDF {slot}: sRGB explicitly converted to output CMYK with moxcms.")
                    } else {
                        format!("PDF {slot}: sRGB converted to CMYK with unprofiled naive conversion; press accuracy is unverified.")
                    })
                }
                Paint::Managed(m)
                    if doc.colour_mode == varos_core::colour_management::ColourMode::Cmyk
                        && matches!(m.colour, Colour::Rgb { .. }) =>
                {
                    Some(format!(
                        "PDF {slot}: source RGB converted to CMYK ({}).",
                        if doc.output_profile.is_some() {
                            "moxcms output profile"
                        } else {
                            "naive unprofiled conversion"
                        }
                    ))
                }
                Paint::Managed(m) => Some(format!(
                    "PDF {slot}: source {:?} retained; on-screen unprofiled sRGB approximation.",
                    m.colour
                )),
                _ => None,
            };
            if let Some(message) = message {
                notes.push(ExportNote { kind: "colour_conversion".into(), object_id: Some(p.id), message });
            }
        }
    }
    notes
}
/// Process conversion is local to this write; authored model bytes remain untouched.
pub(crate) fn prepared(doc: &Document) -> Result<std::borrow::Cow<'_, Document>, String> {
    use varos_core::colour_management::{ColourMode, ManagedColour};
    if doc.colour_mode != ColourMode::Cmyk {
        return Ok(std::borrow::Cow::Borrowed(doc));
    }
    let Some(profile) = &doc.output_profile else {
        return Ok(std::borrow::Cow::Borrowed(doc));
    };
    let output = profile.parse()?;
    if output.color_space != moxcms::DataColorSpace::Cmyk {
        return Err("CMYK export requires a CMYK output profile or an explicitly unprofiled document".into());
    }
    let needs_rgb = doc
        .paths
        .iter()
        .flat_map(|p| [&p.fill, &p.stroke])
        .chain(doc.swatches.iter().map(|s| &s.paint))
        .any(|p| matches!(p, Paint::Solid(_) | Paint::Managed(ManagedColour { colour: Colour::Rgb { .. }, .. })));
    if !needs_rgb {
        return Ok(std::borrow::Cow::Borrowed(doc));
    }
    let transform = moxcms::ColorProfile::new_srgb()
        .create_transform_f32(moxcms::Layout::Rgb, &output, moxcms::Layout::Rgba, Default::default())
        .map_err(|e| e.to_string())?;
    let convert = |paint: &mut Paint| -> Result<(), String> {
        let rgb = match paint {
            Paint::Solid(c) => Some(*c),
            Paint::Managed(m) if matches!(m.colour, Colour::Rgb { .. }) => Some(m.rgba()),
            _ => None,
        };
        if let Some(rgb) = rgb {
            let mut ink = [0.; 4];
            transform.transform(&rgb[..3], &mut ink).map_err(|e| e.to_string())?;
            let [c, m, y, k] = ink.map(|v| v.clamp(0., 1.));
            *paint = Paint::Managed(ManagedColour { colour: Colour::Cmyk { c, m, y, k }, alpha: rgb[3] });
        }
        Ok(())
    };
    let mut converted = doc.clone();
    for path in &mut converted.paths {
        convert(&mut path.fill)?;
        convert(&mut path.stroke)?;
    }
    for swatch in &mut converted.swatches {
        convert(&mut swatch.paint)?;
    }
    Ok(std::borrow::Cow::Owned(converted))
}
