//! Phase 1 PDF settings. Built for pdf-writer; no third-party preset code copied.
use crate::{export_pdf_bytes_with_report, ExportPlan, PageSpec};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use varos_core::{model::Document, ExportNote, ExportReport};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfPreset {
    Print,
    Press,
    Smallest,
    #[default]
    Custom,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfMarks {
    pub crop: bool,
    pub registration: bool,
    pub colour_bars: bool,
    pub page_info: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PdfBoxes {
    pub bleed: bool,
    pub bleed_override: Option<f32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PdfOptions {
    pub preset: PdfPreset,
    pub image_ppi: u32,
    pub compress_streams: bool,
    pub boxes: PdfBoxes,
    pub marks: PdfMarks,
}
impl<'de> Deserialize<'de> for PdfOptions {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize, Default)]
        #[serde(default, deny_unknown_fields)]
        struct Input {
            preset: PdfPreset,
            image_ppi: Option<u32>,
            compress_streams: Option<bool>,
            boxes: Option<PdfBoxes>,
            marks: Option<PdfMarks>,
        }
        let input = Input::deserialize(deserializer)?;
        let mut options = Self::preset(input.preset);
        if let Some(ppi) = input.image_ppi {
            options.image_ppi = ppi;
        }
        if let Some(compress) = input.compress_streams {
            options.compress_streams = compress;
        }
        if let Some(boxes) = input.boxes {
            options.boxes = boxes;
        }
        if let Some(marks) = input.marks {
            options.marks = marks;
        }
        options.validate().map_err(serde::de::Error::custom)?;
        Ok(options)
    }
}
impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            preset: PdfPreset::Custom,
            image_ppi: 300,
            compress_streams: false,
            boxes: PdfBoxes::default(),
            marks: PdfMarks::default(),
        }
    }
}
impl PdfOptions {
    pub fn preset(preset: PdfPreset) -> Self {
        Self {
            preset,
            image_ppi: match preset {
                PdfPreset::Smallest => 72,
                PdfPreset::Print => 150,
                _ => 300,
            },
            compress_streams: preset != PdfPreset::Custom,
            boxes: PdfBoxes { bleed: preset == PdfPreset::Press, bleed_override: None },
            ..Self::default()
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=9600).contains(&self.image_ppi) {
            return Err("Image ppi must be between 1 and 9600.".into());
        }
        if self.boxes.bleed_override.is_some_and(|v| !v.is_finite() || !(0.0..=720.0).contains(&v)) {
            return Err("Bleed must be between 0 and 720 points.".into());
        }
        if self.marks.colour_bars {
            return Err("Colour bars require the later colour-management phase.".into());
        }
        Ok(())
    }
}
/// Default options use the historic writer verbatim. Options never affect native VRS saves.
pub fn export_pdf_with_options(
    doc: &Document,
    plan: &ExportPlan,
    options: &PdfOptions,
    cancel: &AtomicBool,
) -> Result<(Vec<u8>, ExportReport), String> {
    options.validate()?;
    let ppi_note = ExportNote {
        kind: "no_raster_content".into(),
        object_id: None,
        message: format!("Image ppi {}: no raster content.", options.image_ppi),
    };
    if *options == PdfOptions::default() {
        let (bytes, mut report) = export_pdf_bytes_with_report(doc, plan, cancel).map_err(|e| e.to_string())?;
        report.notes.push(ppi_note);
        return Ok((bytes, report));
    }
    let margin = if options.marks.crop || options.marks.registration || options.marks.page_info { 24.0 } else { 0.0 };
    let bleeds: Vec<[f32; 4]> = plan
        .pages
        .iter()
        .map(|p| {
            if !options.boxes.bleed {
                return [0.0; 4];
            }
            options.boxes.bleed_override.map_or(p.bleed_edges, |b| [b; 4])
        })
        .collect();
    let expanded = ExportPlan {
        scope: plan.scope,
        pages: plan
            .pages
            .iter()
            .zip(&bleeds)
            .map(|(p, b)| {
                let [x, y, w, h] = p.rect;
                let [top, right, bottom, left] = *b;
                PageSpec {
                    rect: [
                        x - left - margin,
                        y - top - margin,
                        w + left + right + 2.0 * margin,
                        h + top + bottom + 2.0 * margin,
                    ],
                    background: p.background,
                    bleed: p.bleed,
                    bleed_edges: p.bleed_edges,
                }
            })
            .collect(),
    };
    let (bytes, mut report) = export_pdf_bytes_with_report(doc, &expanded, cancel).map_err(|e| e.to_string())?;
    report.notes.push(ppi_note);
    let mut pdf = lopdf::Document::load_mem(&bytes).map_err(|e| e.to_string())?;
    for ((number, id), (page, bleed)) in pdf.get_pages().into_iter().zip(plan.pages.iter().zip(bleeds)) {
        if cancel.load(Ordering::Relaxed) {
            return Err("The export was cancelled.".into());
        }
        let [_, _, w, h] = page.rect;
        let [top, right, bottom, left] = bleed;
        let rect = |x: f32, y: f32, w: f32, h: f32| {
            lopdf::Object::Array(vec![x.into(), y.into(), (x + w).into(), (y + h).into()])
        };
        let font =
            if options.marks.page_info {
                Some(pdf.add_object(
                    lopdf::dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" },
                ))
            } else {
                None
            };
        let dict = pdf.get_object_mut(id).and_then(lopdf::Object::as_dict_mut).map_err(|e| e.to_string())?;
        dict.set("TrimBox", rect(margin + left, margin + bottom, w, h));
        dict.set("BleedBox", rect(margin, margin, w + left + right, h + top + bottom));
        if let Some(font) = font {
            let resources =
                dict.get_mut(b"Resources").and_then(lopdf::Object::as_dict_mut).map_err(|e| e.to_string())?;
            resources.set("Font", lopdf::dictionary! { "VarosPageInfo" => font });
        }
        let marks = marks_content(w, h, bleed, margin, options.marks, number);
        // Clip artwork to BleedBox, keeping marks margin clean even for off-page objects.
        let clip = pdf.add_object(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            format!("q {margin} {margin} {} {} re W n\n", w + left + right, h + top + bottom).into_bytes(),
        ));
        {
            let stream =
                pdf.add_object(lopdf::Stream::new(lopdf::Dictionary::new(), format!("Q\n{marks}").into_bytes()));
            let dict = pdf.get_object_mut(id).and_then(lopdf::Object::as_dict_mut).map_err(|e| e.to_string())?;
            let old = dict.get(b"Contents").map_err(|e| e.to_string())?.clone();
            dict.set("Contents", vec![lopdf::Object::Reference(clip), old, lopdf::Object::Reference(stream)]);
        }
    }
    if options.compress_streams {
        pdf.compress();
    }
    let mut output = Vec::new();
    pdf.save_to(&mut output).map_err(|e| e.to_string())?;
    if cancel.load(Ordering::Relaxed) {
        return Err("The export was cancelled.".into());
    }
    Ok((output, report))
}
fn marks_content(w: f32, h: f32, b: [f32; 4], m: f32, marks: PdfMarks, page: u32) -> String {
    if m == 0.0 {
        return String::new();
    }
    let [top, right, bottom, left] = b;
    let mut s = "q 0 0 0 1 K 0.25 w\n".to_owned();
    let mut line = |x: f32, y: f32, a: f32, c: f32| s.push_str(&format!("{x} {y} m {a} {c} l S\n"));
    if marks.crop {
        for x in [m + left, m + left + w] {
            line(x, 3.0, x, m - 3.0);
            line(x, m + top + bottom + h + 3.0, x, 2.0 * m + top + bottom + h - 3.0);
        }
        for y in [m + bottom, m + bottom + h] {
            line(3.0, y, m - 3.0, y);
            line(m + left + right + w + 3.0, y, 2.0 * m + left + right + w - 3.0, y);
        }
    }
    if marks.registration {
        for (x, y) in [(m + left + w / 2.0, m / 2.0), (m / 2.0, m + bottom + h / 2.0)] {
            line(x - 5.0, y, x + 5.0, y);
            line(x, y - 5.0, x, y + 5.0);
        }
    }
    if marks.page_info {
        s.push_str(&format!("0 0 0 1 k BT /VarosPageInfo 7 Tf {m} 4 Td (Varos page {page} - {w} x {h} pt) Tj ET\n"));
    }
    s.push_str("Q\n");
    s
}
