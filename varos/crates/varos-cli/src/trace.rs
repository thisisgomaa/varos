use super::{json, required, same_file, write_output, OsString, PathBuf, Value};
use varos_core::{
    command::EditCommand,
    editor::Editor,
    model::{Artboard, Document},
    trace::{TraceMode, TraceOptions},
};
pub(super) fn run(args: Vec<OsString>) -> Result<Value, String> {
    let mut input = None;
    let mut out = None;
    let mut options = TraceOptions::default();
    let mut color_count = None;
    let mut seen = std::collections::BTreeSet::new();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let key = arg.to_string_lossy();
        if !key.starts_with("--") {
            if input.replace(PathBuf::from(&arg)).is_some() {
                return Err("trace expects one PNG".into());
            }
            continue;
        }
        if !seen.insert(key.to_string()) {
            return Err(format!("duplicate option {key}"));
        }
        if key == "--ignore-white" {
            options.ignore_white = true;
            continue;
        }
        if key == "--include-white" {
            options.ignore_white = false;
            continue;
        }
        let value = it.next().ok_or_else(|| format!("missing value for {key}"))?;
        let text = value.to_string_lossy();
        match key.as_ref() {
            "--out" => out = Some(PathBuf::from(&value)),
            "--mode" => {
                options.mode = match text.as_ref() {
                    "bw" | "black-white" => TraceMode::BlackWhite,
                    "grayscale" => TraceMode::Grayscale,
                    "color" => TraceMode::Color { colors: 8 },
                    _ => return Err("mode must be black-white, grayscale or color".into()),
                }
            }
            "--colors" => {
                let colors = text.parse().map_err(|_| "invalid colors")?;
                color_count = Some(colors);
            }
            "--threshold" => options.threshold = text.parse().map_err(|_| "invalid threshold")?,
            "--paths-fidelity" => options.paths_fidelity = text.parse().map_err(|_| "invalid fidelity")?,
            "--corners" => options.corners = text.parse().map_err(|_| "invalid corners")?,
            "--noise-px" => options.noise_px = text.parse().map_err(|_| "invalid noise area")?,
            _ => return Err(format!("unknown option {key}")),
        }
    }
    if let Some(colors) = color_count {
        if !matches!(options.mode, TraceMode::Color { .. }) {
            return Err("--colors requires --mode color".into());
        }
        options.mode = TraceMode::Color { colors };
    }
    let input = required(input, "input PNG")?;
    let out = required(out, "--out")?;
    if same_file(&input, &out)? {
        return Err("trace output must differ from input PNG".into());
    }
    let reader = image::ImageReader::with_format(
        std::io::BufReader::new(std::fs::File::open(&input).map_err(|e| e.to_string())?),
        image::ImageFormat::Png,
    );
    let (w, h) = reader.into_dimensions().map_err(|e| e.to_string())?;
    if u64::from(w) * u64::from(h) > varos_core::trace::MAX_PIXELS {
        return Err("PNG exceeds trace pixel limit".into());
    }
    let img = image::ImageReader::with_format(
        std::io::BufReader::new(std::fs::File::open(&input).map_err(|e| e.to_string())?),
        image::ImageFormat::Png,
    )
    .decode()
    .map_err(|e| e.to_string())?
    .to_rgba8();
    let (paths, report) = varos_core::trace::trace(img.as_raw(), w, h, &options)?;
    let mut doc = Document::default();
    doc.artboards.push(Artboard { x: 0., y: 0., w: w as f32, h: h as f32, ..Artboard::default() });
    let mut editor = Editor::new();
    editor.replace_doc(doc);
    editor.try_execute(EditCommand::InsertTracedPaths { paths })?;
    let bytes = varos_pdf::write_pdf_checked(&editor.doc, &varos_core::format::Limits::DEFAULT)?;
    write_output(&out, &bytes)?;
    Ok(json!({"out":out.to_string_lossy(),"report":report}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_to_vrs_roundtrip_and_input_guard() {
        let dir = std::env::temp_dir().join(format!("varos-trace-cli-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("generated.png");
        let out = dir.join("generated.vrs");
        let img = image::RgbaImage::from_fn(20, 20, |x, y| {
            image::Rgba(if (4..16).contains(&x) && (4..16).contains(&y) { [255; 4] } else { [0, 0, 0, 255] })
        });
        img.save(&input).unwrap();
        let args = vec![
            input.clone().into_os_string(),
            "--mode".into(),
            "black-white".into(),
            "--out".into(),
            out.clone().into_os_string(),
        ];
        let reply = run(args).unwrap();
        assert_eq!(reply["report"]["holes"], 1);
        let doc = varos_pdf::load_vrs(&out).unwrap();
        assert_eq!(doc.paths.len(), 1);
        assert_eq!(doc.paths[0].holes.len(), 1);
        assert_eq!(doc.artboards[0].w, 20.);
        let before = std::fs::read(&input).unwrap();
        assert!(run(vec![input.clone().into_os_string(), "--out".into(), input.clone().into_os_string()]).is_err());
        assert_eq!(std::fs::read(input).unwrap(), before);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn malformed_options_are_refused() {
        for args in [vec!["--mode", "bad"], vec!["--colors", "2"], vec!["--threshold", "256"], vec!["--noise-px", "-1"]]
        {
            assert!(run(args.into_iter().map(Into::into).collect()).is_err());
        }
    }
}
