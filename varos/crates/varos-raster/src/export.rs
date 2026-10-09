//! Shared, deterministic deliverable encoding for desktop jobs, CLI and Bridge. No I/O or UI.
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use varos_core::{
    model::{Artboard, Document},
    svg, ExportNote, ExportReport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Pdf,
    Svg,
    Png,
    Jpeg,
    WebP,
    Tiff,
}
impl Format {
    pub const ALL: [Self; 6] = [Self::Pdf, Self::Svg, Self::Png, Self::Jpeg, Self::WebP, Self::Tiff];
    pub fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
            Self::Tiff => "tiff",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Pdf => "PDF",
            Self::Svg => "SVG",
            Self::Png => "PNG",
            Self::Jpeg => "JPEG",
            Self::WebP => "WebP",
            Self::Tiff => "TIFF",
        }
    }
    pub fn vector(self) -> bool {
        matches!(self, Self::Pdf | Self::Svg)
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|f| f.label().eq_ignore_ascii_case(s) || f.extension().eq_ignore_ascii_case(s))
            .ok_or_else(|| "Format must be PDF, SVG, PNG, JPEG, WebP or TIFF.".into())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub format: Format,
    pub scale: f32,
    pub transparent: bool,
    pub quality: u8,
}
impl Default for Options {
    fn default() -> Self {
        Self { format: Format::Pdf, scale: 1.0, transparent: true, quality: 90 }
    }
}
impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if self.quality > 100 {
            return Err("JPEG quality must be from 0 to 100.".into());
        }
        if !self.format.vector() && (!self.scale.is_finite() || self.scale <= 0.0 || self.scale > 64.0) {
            return Err("Scale must be greater than zero and at most 64 (ppi / 72).".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Scope {
    AllArtboards,
    Artboard(u32),
    WholeBoard,
    Selection(HashSet<u32>),
}
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub name: String,
    pub doc: Arc<Document>,
    pub page: svg::PageSpec,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub name: String,
    pub bytes: Vec<u8>,
    pub report: ExportReport,
}

pub fn plan(doc: &Document, scope: &Scope) -> Result<Vec<Asset>, String> {
    // ---- Lane G ----
    let outlined = varos_text_layout::outline_document(doc)?;
    let appearance = &outlined;
    let (snapshot, plan) = match scope {
        Scope::Selection(ids) => {
            let (narrowed, mut plan) = svg::plan_selection_svg_export(appearance, ids).map_err(|e| e.to_string())?;
            for page in &mut plan.pages {
                page.name = "Selection".into();
            }
            (narrowed, plan)
        }
        _ => {
            let mut snapshot = appearance.clone();
            let scope = match scope {
                Scope::AllArtboards => svg::ExportScope::AllVisibleArtboards,
                Scope::WholeBoard => svg::ExportScope::WholeBoard,
                Scope::Artboard(id) => {
                    snapshot.active = snapshot.artboard_index(*id).ok_or("Unknown artboard.")?;
                    svg::ExportScope::ActiveArtboard
                }
                Scope::Selection(_) => unreachable!(),
            };
            let plan = svg::plan_svg_export(&snapshot, scope).map_err(|e| e.to_string())?;
            (snapshot, plan)
        }
    };
    let mut authored = doc.clone();
    authored.active = snapshot.active;
    let doc = Arc::new(if matches!(scope, Scope::Selection(_)) { snapshot } else { authored });
    Ok(plan.pages.into_iter().map(|page| Asset { name: page.name.clone(), doc: doc.clone(), page }).collect())
}

pub fn encode(asset: &Asset, options: &Options, cancel: &AtomicBool) -> Result<Output, String> {
    encode_inner(asset, options, &varos_core::images::BlobStore::default(), cancel, None)
}
pub fn encode_with_images(
    asset: &Asset,
    options: &Options,
    store: &varos_core::images::BlobStore,
    cancel: &AtomicBool,
) -> Result<Output, String> {
    encode_inner(asset, options, store, cancel, None)
}
// ---- Lane C: shared advanced SVG encoding for app, Bridge and CLI ----
pub fn encode_with_svg_options(
    asset: &Asset,
    options: &Options,
    cancel: &AtomicBool,
    svg_options: &svg::options::Options,
) -> Result<Output, String> {
    encode_with_images_and_svg_options(asset, options, &varos_core::images::BlobStore::default(), cancel, svg_options)
}
/// Integration w2: Lane C's advanced SVG options and w2-images' resources in one encoder (the app's
/// export worker has both).
pub fn encode_with_images_and_svg_options(
    asset: &Asset,
    options: &Options,
    store: &varos_core::images::BlobStore,
    cancel: &AtomicBool,
    svg_options: &svg::options::Options,
) -> Result<Output, String> {
    svg_options.validate()?;
    encode_inner(asset, options, store, cancel, Some(svg_options))
}
fn encode_inner(
    asset: &Asset,
    options: &Options,
    store: &varos_core::images::BlobStore,
    cancel: &AtomicBool,
    svg_options: Option<&svg::options::Options>,
) -> Result<Output, String> {
    options.validate()?;
    check_cancel(cancel)?;
    // Validate caller-supplied pages and documents before allocation or traversal.
    let plan = svg::ExportPlan { scope: svg::ExportScope::WholeBoard, pages: vec![asset.page.clone()] };
    // Lane G outlines text for every deliverable; w2-images routes documents with images through the
    // image-aware SVG writer; Lane C's advanced SVG options apply to vector documents (integration w2).
    let outlined = varos_text_layout::outline_document(&asset.doc)?;
    let (svg_files, mut report) = if outlined.images.is_empty() {
        match svg_options.filter(|_| options.format == Format::Svg) {
            Some(svg_options) => svg::export_svg_files_with_options(&outlined, &plan, cancel, svg_options),
            None => svg::export_svg_files_with_report(&outlined, &plan, cancel),
        }
        .map_err(|e| e.to_string())?
    } else {
        varos_core::images::svg::export(&outlined, store, &plan, false, cancel)?
    };
    if !asset.doc.text_boxes.is_empty() {
        report.notes.extend(varos_text_layout::export_notes(&asset.doc)?);
    }
    let bytes = match options.format {
        Format::Svg => svg_files.into_iter().next().ok_or("No export page.")?.bytes,
        Format::Pdf => return Err("PDF encoding belongs to varos-pdf; the host uses the same page plan.".into()),
        format => {
            let [x, y, w, h] = asset.page.rect;
            let dimensions = [w * options.scale, h * options.scale];
            if dimensions.iter().any(|v| !v.is_finite() || *v > 16384.0) || dimensions[0] * dimensions[1] > 64_000_000.0
            {
                return Err("Raster export exceeds 64 million pixels or 16384 pixels per side.".into());
            }
            let size = dimensions.map(|v| v.round().max(1.0) as u32);
            // integration w2: the outlined copy, so text also reaches image-document rasters
            let mut doc = outlined.clone();
            // Export has no canvas ghost paper. Suppress other artboards' paper; artwork stays.
            for ab in &mut doc.artboards {
                ab.page_color = Some([0.0; 4]);
            }
            let background = if format == Format::Jpeg {
                let c = asset.page.background.unwrap_or([1.0; 4]);
                report.notes.push(ExportNote {
                    kind: "transparency".into(),
                    object_id: None,
                    message:
                        "JPEG has no transparency: background filled with page colour (white for a transparent page)."
                            .into(),
                });
                Some([c[0] * c[3] + 1.0 - c[3], c[1] * c[3] + 1.0 - c[3], c[2] * c[3] + 1.0 - c[3], 1.0])
            } else if options.transparent {
                None
            } else {
                Some(asset.page.background.unwrap_or([1.0; 4]))
            };
            let index = doc.artboards.len();
            doc.artboards.push(Artboard { x, y, w, h, page_color: background, ..Artboard::default() });
            let raster = if doc.images.is_empty() {
                crate::rasterize_artboard(Arc::new(doc), index, size).ok_or("Invalid raster page.")?
            } else {
                crate::images::rasterize_with_images(
                    &doc,
                    store,
                    size,
                    [-x * options.scale, -y * options.scale],
                    options.scale,
                    background,
                )?
            };
            check_cancel(cancel)?;
            match format {
                Format::Png => raster.encode_png()?,
                _ => {
                    let mut pixels = raster.pixels;
                    // image encoders require straight alpha, tiny-skia supplies premultiplied alpha.
                    for pixel in pixels.as_chunks_mut::<4>().0 {
                        let a = pixel[3] as u32;
                        for c in &mut pixel[..3] {
                            *c = ((*c as u32 * 255 + a / 2).checked_div(a).unwrap_or(0)).min(255) as u8;
                        }
                    }
                    let mut out = std::io::Cursor::new(Vec::new());
                    if format == Format::Jpeg {
                        let rgb: Vec<u8> =
                            pixels.as_chunks::<4>().0.iter().flat_map(|p| p[..3].iter().copied()).collect();
                        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, options.quality)
                            .encode(&rgb, raster.width, raster.height, image::ExtendedColorType::Rgb8)
                            .map_err(|e| e.to_string())?;
                    } else {
                        let img = image::RgbaImage::from_raw(raster.width, raster.height, pixels)
                            .ok_or("Invalid pixel buffer.")?;
                        image::DynamicImage::ImageRgba8(img)
                            .write_to(
                                &mut out,
                                if format == Format::WebP {
                                    image::ImageFormat::WebP
                                } else {
                                    image::ImageFormat::Tiff
                                },
                            )
                            .map_err(|e| e.to_string())?;
                    }
                    out.into_inner()
                }
            }
        }
    };
    check_cancel(cancel)?;
    Ok(Output { name: format!("{}.{}", safe_name(&asset.name), options.format.extension()), bytes, report })
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        Err("The export was cancelled.".into())
    } else {
        Ok(())
    }
}
pub fn safe_name(name: &str) -> String {
    let s: String =
        name.chars()
            .map(|c| {
                if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                    '_'
                } else {
                    c
                }
            })
            .collect();
    let s = s.trim().trim_matches('.');
    if s.is_empty() {
        "Untitled".into()
    } else {
        s.into()
    }
}
pub fn file_name(name: &str, suffix: &str, format: Format, collision: usize) -> String {
    let stem = safe_name(&format!("{name}{suffix}"));
    if collision <= 1 {
        format!("{stem}.{}", format.extension())
    } else {
        format!("{stem} {collision}.{}", format.extension())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_quality() {
        assert_eq!(file_name("A/B", "@2x", Format::Png, 2), "A_B@2x 2.png");
        assert!(Options { quality: 101, ..Options::default() }.validate().is_err());
        assert!(Options { scale: f32::NAN, ..Options::default() }.validate().is_ok());
    }
    #[test]
    fn formats_cancel_scale_and_pixels() {
        let doc = Document {
            artboards: vec![Artboard { id: 1, w: 8.0, h: 6.0, name: "Page".into(), ..Artboard::default() }],
            ..Default::default()
        };
        let asset = plan(&doc, &Scope::Artboard(1)).unwrap().remove(0);
        for format in Format::ALL.into_iter().filter(|format| *format != Format::Pdf) {
            let opt = Options { format, ..Options::default() };
            let out = encode(&asset, &opt, &AtomicBool::new(false)).unwrap();
            assert!(!out.bytes.is_empty());
            assert!(encode(&asset, &opt, &AtomicBool::new(true)).is_err());
            if format.vector() {
                assert_eq!(
                    out.bytes,
                    encode(&asset, &Options { scale: 3.0, ..opt }, &AtomicBool::new(false)).unwrap().bytes
                );
            } else {
                let decoded = image::load_from_memory(&out.bytes).unwrap();
                assert_eq!((decoded.width(), decoded.height()), (8, 6));
            }
        }
        let out =
            encode(&asset, &Options { format: Format::Png, scale: 2.0, ..Options::default() }, &AtomicBool::new(false))
                .unwrap();
        let decoded = image::load_from_memory(&out.bytes).unwrap();
        assert_eq!(decoded.width(), 16);
        assert_eq!(decoded.to_rgba8().get_pixel(0, 0)[3], 0);
    }
}

#[cfg(test)]
mod rendering_correctness_tests {
    use super::*;
    #[test]
    fn known_rectangle_has_correct_fill_transparency_scale_and_selection_crop() {
        let mut editor = varos_core::editor::Editor::new();
        editor
            .try_execute_created(varos_core::EditCommand::AddShape {
                kind: varos_core::model::ShapeKind::Rect,
                bounds: [2.0, 2.0, 3.0, 2.0],
                parent: None,
                fill: Some([1.0, 0.0, 0.0, 1.0]),
                stroke: None,
                stroke_width: 0.0,
                opacity: 1.0,
                name: None,
            })
            .unwrap();
        editor.doc.artboards = vec![Artboard { id: 1, w: 8.0, h: 6.0, ..Default::default() }];
        let asset = plan(&editor.doc, &Scope::Artboard(1)).unwrap().remove(0);
        for format in [Format::Png, Format::WebP, Format::Tiff] {
            let output =
                encode(&asset, &Options { format, scale: 2.0, ..Default::default() }, &AtomicBool::new(false)).unwrap();
            let pixels = image::load_from_memory(&output.bytes).unwrap().to_rgba8();
            assert_eq!(pixels.dimensions(), (16, 12));
            assert_eq!(pixels.get_pixel(6, 6).0, [255, 0, 0, 255]);
            assert_eq!(pixels.get_pixel(0, 0).0[3], 0);
        }
        let svg =
            encode(&asset, &Options { format: Format::Svg, ..Default::default() }, &AtomicBool::new(false)).unwrap();
        let svg = String::from_utf8(svg.bytes).unwrap();
        assert!(svg.contains("viewBox=\"0.000 0.000 8.000 6.000\""));
        assert!(svg.contains("fill=\"#ff0000\""));
        let selected = editor.doc.paths.iter().map(|p| p.id).collect();
        let selection = plan(&editor.doc, &Scope::Selection(selected)).unwrap().remove(0);
        for (actual, expected) in selection.page.rect.into_iter().zip([2.0, 2.0, 3.0, 2.0]) {
            assert!((actual - expected).abs() < 0.00001, "{actual} != {expected}");
        }
        let output =
            encode(&selection, &Options { format: Format::Png, ..Default::default() }, &AtomicBool::new(false))
                .unwrap();
        let pixels = image::load_from_memory(&output.bytes).unwrap().to_rgba8();
        assert_eq!(pixels.dimensions(), (3, 2));
        assert!(pixels.pixels().all(|p| p.0 == [255, 0, 0, 255]));
    }
}
