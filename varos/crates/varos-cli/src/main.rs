//! Thin filesystem/argument host for the provisional core Bridge contracts.
mod colour;
// ---- Lane A ----
mod appearance;
mod document;
// ---- Lane E ----
mod images;
mod view_depth;
// ---- Lane C ----
mod lane_c;
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};
use varos_core::{
    board,
    bridge::{self, json, Value},
    editor::Editor,
    format::Limits,
};

// The only CLI verb table. No desktop binary names or UI routing are changed.
mod trace;
// ---- Lane G ----
mod text;
// ---- Lane H ----
mod import;
const VERBS: &[&str] = &[
    "appearance",
    "mask",
    "view-depth",
    "add-text",
    "set-text",
    "new-document",
    "export-screens",
    "trace",
    "import-svg",
    "import",
    "import-pdf",
    "import-ai",
    "import-dxf",
    "describe",
    "snapshot",
    "export-pdf",
    "export-svg",
    "export-raster",
    "print",
    "clipboard-out",
    "save-as",
    "apply",
    "palette-import",
    "palette-export",
    "new",
    "diff",
];
struct Failure {
    reason: String,
    index: Option<usize>,
}
impl From<String> for Failure {
    fn from(reason: String) -> Self {
        Self { reason, index: None }
    }
}
impl From<bridge::BatchError> for Failure {
    fn from(e: bridge::BatchError) -> Self {
        Self { reason: e.reason, index: Some(e.index) }
    }
}
fn response(action: impl FnOnce() -> Result<Value, Failure> + std::panic::UnwindSafe) -> (Value, i32) {
    let result = std::panic::catch_unwind(action).unwrap_or_else(|payload| {
        let message = payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| payload.downcast_ref::<&str>().copied())
            .unwrap_or("unknown panic");
        Err(format!("internal panic: {message}").into())
    });
    match result {
        Ok(v) => (json!({"ok":true,"result":v}), 0),
        Err(e) => (json!({"ok":false,"error":{"reason":e.reason,"index":e.index}}), 1),
    }
}
fn main() {
    // ---- Lane H ----
    if varos_import::worker::worker_main() {
        return;
    }
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("image") => {
            let (value, code) = response(|| images::run(args.collect()).map_err(Failure::from));
            println!("{value}");
            std::process::exit(code);
        }
        Some("bridge") => {
            let rest: Vec<String> = args.collect();
            // ADR-0011 owner commands (pair/agents/hosts/register) vs attached tool calls.
            let outcome = if varos_bridge::conn::manage::handles(&rest) {
                varos_bridge::conn::manage::run(rest)
            } else {
                varos_bridge::cli::run(rest)
            };
            match outcome {
                Ok(code) => std::process::exit(code),
                Err(e) => {
                    eprintln!("varos-cli bridge: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("bridge-endpoint") => {
            if args.next().is_some() {
                eprintln!("bridge-endpoint takes no arguments");
                std::process::exit(1);
            }
            match varos_bridge::ipc::endpoint_paths() {
                Ok(paths) => {
                    for p in paths {
                        println!("{}", p.display());
                    }
                }
                Err(e) => {
                    eprintln!("bridge-endpoint: {e}");
                    std::process::exit(1);
                }
            }
            return;
        }
        _ => {}
    }
    std::panic::set_hook(Box::new(|_| {}));
    let (value, code) = response(|| run(std::env::args_os().skip(1).collect()));
    if writeln!(std::io::stdout().lock(), "{value}").is_err() {
        std::process::exit(1);
    }
    std::process::exit(code);
}
struct Args {
    positional: Vec<PathBuf>,
    out: Option<PathBuf>,
    size: Option<u32>,
    detail: Option<String>,
    batch: Option<PathBuf>,
    preset: Option<String>,
    artboard: Option<String>,
    in_place: bool,
    format: Option<String>,
    scale: Option<f32>,
    ppi: Option<f32>,
    quality: Option<u8>,
    transparent: Option<bool>,
    ids: Option<String>,
    marks: Option<String>,
    bleed: Option<f32>,
}
fn parse(args: Vec<OsString>, allowed: &[&str], count: usize) -> Result<Args, String> {
    let mut result = Args {
        positional: vec![],
        out: None,
        size: None,
        detail: None,
        batch: None,
        preset: None,
        artboard: None,
        in_place: false,
        format: None,
        scale: None,
        ppi: None,
        quality: None,
        transparent: None,
        ids: None,
        marks: None,
        bleed: None,
    };
    let mut it = args.into_iter();
    let mut seen = std::collections::HashSet::new();
    let mut literal = false;
    while let Some(arg) = it.next() {
        let text = arg.to_string_lossy();
        if !literal && text == "--" {
            literal = true;
            continue;
        }
        if !literal && text.starts_with('-') {
            if !allowed.contains(&text.as_ref()) {
                return Err(format!("unknown option {text}"));
            }
            if !seen.insert(text.to_string()) {
                return Err(format!("duplicate option {text}"));
            }
            if text == "--in-place" {
                result.in_place = true;
                continue;
            }
            let value = it.next().ok_or_else(|| format!("missing value for {text}"))?;
            match text.as_ref() {
                "--format" => result.format = Some(value.into_string().map_err(|_| "format must be UTF-8")?),
                "--scale" => result.scale = Some(value.to_str().and_then(|v| v.parse().ok()).ok_or("invalid scale")?),
                "--ppi" => result.ppi = Some(value.to_str().and_then(|v| v.parse().ok()).ok_or("invalid ppi")?),
                "--quality" => {
                    result.quality = Some(
                        value
                            .to_str()
                            .and_then(|v| v.parse().ok())
                            .filter(|v| *v <= 100)
                            .ok_or("quality must be 0-100")?,
                    )
                }
                "--transparent" => {
                    result.transparent =
                        Some(value.to_str().and_then(|v| v.parse().ok()).ok_or("transparent must be true or false")?)
                }
                "--ids" => result.ids = Some(value.into_string().map_err(|_| "ids must be UTF-8")?),
                "--marks" => result.marks = Some(value.into_string().map_err(|_| "marks must be UTF-8")?),
                "--bleed" => result.bleed = Some(value.to_str().and_then(|v| v.parse().ok()).ok_or("invalid bleed")?),
                "--out" => result.out = Some(value.into()),
                "--batch" => result.batch = Some(value.into()),
                "--detail" => result.detail = Some(value.into_string().map_err(|_| "detail id must be UTF-8")?),
                "--preset" => result.preset = Some(value.into_string().map_err(|_| "preset must be UTF-8")?),
                "--artboard" => result.artboard = Some(value.into_string().map_err(|_| "artboard must be UTF-8")?),
                "--size" => {
                    result.size = Some(
                        value
                            .to_str()
                            .and_then(|v| v.parse().ok())
                            .filter(|v| (1..=2048).contains(v))
                            .ok_or("size must be an integer from 1 to 2048")?,
                    )
                }
                _ => unreachable!(),
            }
        } else {
            result.positional.push(arg.into());
        }
    }
    if result.positional.len() != count {
        return Err(format!("expected {count} file argument(s), got {}", result.positional.len()));
    }
    Ok(result)
}
fn required<T>(value: Option<T>, flag: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("missing required {flag}"))
}
fn run(mut args: Vec<OsString>) -> Result<Value, Failure> {
    if args.is_empty() {
        return Err(format!("expected a subcommand: {}", VERBS.join(", ")).into());
    }
    let verb = args.remove(0).into_string().map_err(|_| "subcommand must be UTF-8".to_owned())?;
    // ---- Lane A ----
    if ["appearance", "mask"].contains(&verb.as_str()) {
        return appearance::run(&verb, args).map_err(Failure::from);
    }
    if verb == "view-depth" {
        return view_depth::run(args).map_err(Failure::from);
    }
    if ["new-document", "export-screens"].contains(&verb.as_str()) {
        return lane_c::run(&verb, args).map_err(Into::into);
    }
    if ["document-info", "document-setup", "save-template", "new-from-template"].contains(&verb.as_str()) {
        return document::run(&verb, args).map_err(Into::into);
    }
    if !VERBS.contains(&verb.as_str()) {
        return Err(format!("unknown subcommand {verb}; expected {}", VERBS.join(", ")).into());
    }
    match verb.as_str() {
        "add-text" | "set-text" => text::run(&verb, args).map_err(Into::into),
        "palette-import" | "palette-export" => colour::run(&verb, args).map_err(Into::into),
        "trace" => trace::run(args).map_err(Into::into),
        "import" | "import-pdf" | "import-ai" | "import-dxf" => import::run(&verb, args).map_err(Into::into),
        "import-svg" => {
            let a = parse(args, &["--out"], 1)?;
            let out = required(a.out, "--out")?;
            if same_file(&a.positional[0], &out)? {
                return Err("import output must differ from source".to_owned().into());
            }
            let mut bytes = Vec::new();
            std::fs::File::open(&a.positional[0])
                .map_err(|e| e.to_string())?
                .take((varos_import::MAX_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            let (doc, report) = varos_import::import_svg(&bytes)?;
            let bytes = varos_pdf::write_pdf_checked(&doc, &Limits::DEFAULT)?;
            write_output(&out, &bytes)?;
            Ok(json!({"out":out.to_string_lossy(), "report":report}))
        }
        "describe" => {
            let a = parse(args, &["--detail"], 1)?;
            let doc = varos_pdf::load_vrs(&a.positional[0])?;
            Ok(bridge::describe(&doc, a.detail.as_deref())?)
        }
        "snapshot" => {
            let a = parse(args, &["--out", "--size"], 1)?;
            let out = required(a.out, "--out")?;
            let size = a.size.unwrap_or(400);
            let loaded = varos_pdf::load_vrs_checked(&a.positional[0], &Limits::DEFAULT).map_err(|e| e.to_string())?;
            let png = if loaded.doc.images.is_empty() {
                varos_raster::rasterize(Arc::new(loaded.doc), [size, size]).encode_png()?
            } else {
                varos_raster::images::fitted(&loaded.doc, &loaded.blobs, [size, size], None)?.encode_png()?
            };
            write_output(&out, &png)?;
            Ok(json!({"out":out.to_string_lossy(),"width":size,"height":size,"bytes":png.len()}))
        }
        "export-svg" | "export-raster" => {
            use varos_raster::export::{self, Format, Options, Scope};
            let a = parse(
                args,
                if verb == "export-svg" {
                    &["--out", "--artboard"]
                } else {
                    &["--out", "--artboard", "--format", "--scale", "--ppi", "--transparent", "--quality"]
                },
                1,
            )?;
            let out = required(a.out, "--out")?;
            if same_file(&a.positional[0], &out)? {
                return Err("Export cannot replace the editable input.".to_owned().into());
            }
            if a.scale.is_some() && a.ppi.is_some() {
                return Err("Choose scale or ppi, not both.".to_owned().into());
            }
            let loaded = varos_pdf::load_vrs_checked(&a.positional[0], &Limits::DEFAULT).map_err(|e| e.to_string())?;
            let doc = loaded.doc;
            let blobs = loaded.blobs;
            let scope = match a.artboard.as_deref() {
                Some("all") => Scope::AllArtboards,
                Some("whole") => Scope::WholeBoard,
                Some(id) => Scope::Artboard(
                    id.strip_prefix("artboard:")
                        .unwrap_or(id)
                        .parse()
                        .map_err(|_| "Invalid artboard id.".to_owned())?,
                ),
                None if doc.artboards.is_empty() => Scope::WholeBoard,
                None => Scope::AllArtboards,
            };
            let format =
                if verb == "export-svg" { Format::Svg } else { Format::parse(a.format.as_deref().unwrap_or("png"))? };
            if verb == "export-raster" && format.vector() {
                return Err("export-raster requires a raster format.".to_owned().into());
            }
            let options = Options {
                format,
                scale: a.ppi.map(|p| p / 72.0).or(a.scale).unwrap_or(1.0),
                quality: a.quality.unwrap_or(90),
                transparent: a.transparent.unwrap_or(true),
            };
            options.validate()?;
            let assets = export::plan(&doc, &scope)?;
            if assets.len() > 1 && !out.is_dir() {
                return Err("Multiple artboards require --out to be an existing folder.".to_owned().into());
            }
            let mut files = vec![];
            for asset in assets {
                let output = export::encode_with_images(&asset, &options, &blobs, &AtomicBool::new(false))?;
                let path = if out.is_dir() { out.join(&output.name) } else { out.clone() };
                // Deliverables never silently overwrite a prior export.
                if path.exists() {
                    return Err("Output exists; choose a fresh filename.".to_owned().into());
                }
                write_fresh_output(&path, &output.bytes)?;
                for note in &output.report.notes {
                    eprintln!("{}: {}", note.kind, note.message);
                }
                files.push(json!({"out":path.to_string_lossy(), "bytes":output.bytes.len(), "report":output.report}));
            }
            Ok(json!({"files":files}))
        }
        "clipboard-out" => {
            let a = parse(args, &["--out", "--ids"], 1)?;
            let out = required(a.out, "--out")?;
            if out.exists() {
                return Err("clipboard output directory already exists".to_owned().into());
            }
            let ids = required(a.ids, "--ids")?
                .split(',')
                .map(|v| {
                    v.strip_prefix("path:")
                        .or_else(|| v.strip_prefix("image:"))
                        .unwrap_or(v)
                        .parse::<u32>()
                        .map_err(|_| "ids must be comma-separated path:N".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            let loaded = varos_pdf::load_vrs_checked(&a.positional[0], &Limits::DEFAULT).map_err(|e| e.to_string())?;
            let doc = loaded.doc;
            let mut clipboard = varos_core::clipboard::Clipboard::capture(&doc, &ids);
            clipboard.pin_images(&loaded.blobs);
            let vectors = varos_pdf::clipboard_vectors(&doc, &clipboard)?;
            let [x, y, w, h] = vectors.rect;
            let png = if vectors.document.images.is_empty() {
                varos_raster::clipboard_png(vectors.document.clone(), vectors.rect)?
            } else {
                varos_raster::images::rasterize_with_images(
                    &vectors.document,
                    &clipboard.resources,
                    [(w * 2.).ceil() as u32, (h * 2.).ceil() as u32],
                    [-x * 2., -y * 2.],
                    2.,
                    None,
                )?
                .encode_png()?
            };
            std::fs::create_dir(&out).map_err(|e| e.to_string())?;
            for (name, bytes) in [
                ("selection.varos.json", vectors.internal),
                ("selection.pdf", vectors.pdf),
                ("selection.svg", vectors.svg),
                ("selection.png", png),
            ] {
                write_output(&out.join(name), &bytes)?;
            }
            Ok(
                json!({"out":out.to_string_lossy(),"flavours":["org.varos.clipboard","com.adobe.pdf","public.svg-image","public.png"]}),
            )
        }
        "export-pdf" | "print" | "save-as" => {
            let a = parse(
                args,
                if verb != "save-as" {
                    &["--out", "--artboard", "--preset", "--ppi", "--marks", "--bleed"]
                } else {
                    &["--out"]
                },
                1,
            )?;
            let out = required(a.out, "--out")?;
            // An export is a model-free PDF: writing it over the input would destroy the editable
            // document. Refused outright (there is no `--in-place` for export), checked before loading.
            if verb != "save-as" && same_file(&a.positional[0], &out)? {
                return Err("--out resolves to the input; an export would replace the editable document with a \
                     model-free PDF. Choose another --out"
                    .to_owned()
                    .into());
            }
            let loaded = varos_pdf::load_vrs_checked(&a.positional[0], &Limits::DEFAULT).map_err(|e| e.to_string())?;
            let blobs = loaded.blobs;
            let mut doc = loaded.doc;
            let mut export_report = None;
            let show_report =
                verb == "print" || a.preset.is_some() || a.ppi.is_some() || a.marks.is_some() || a.bleed.is_some();
            let pdf = if verb != "save-as" {
                // `--artboard artboard:N` (or N): export that one page — the stable id (format 4) mapped
                // to the ActiveArtboard scope on this in-memory copy; the file itself is not changed
                let scope = match &a.artboard {
                    Some(id) => {
                        let n = id
                            .strip_prefix("artboard:")
                            .unwrap_or(id)
                            .parse::<u32>()
                            .ok()
                            .filter(|n| *n > 0)
                            .ok_or_else(|| "--artboard must be artboard:N or N".to_owned())?;
                        doc.active = doc.artboard_index(n).ok_or_else(|| format!("unknown artboard:{n}"))?;
                        varos_pdf::ExportScope::ActiveArtboard
                    }
                    None => varos_pdf::default_scope(&doc),
                };
                let plan = varos_pdf::plan_pdf_export(&doc, scope).map_err(|e| e.to_string())?;
                {
                    let preset = match a.preset.as_deref() {
                        None | Some("custom") => varos_pdf::PdfPreset::Custom,
                        Some("print") => varos_pdf::PdfPreset::Print,
                        Some("press") => varos_pdf::PdfPreset::Press,
                        Some("smallest") => varos_pdf::PdfPreset::Smallest,
                        _ => return Err("preset must be print, press, smallest or custom".to_owned().into()),
                    };
                    let mut options = varos_pdf::PdfOptions::preset(preset);
                    if let Some(ppi) = a.ppi {
                        if !ppi.is_finite() || ppi < 1.0 || ppi.fract() != 0.0 || ppi > u32::MAX as f32 {
                            return Err("PDF ppi must be a positive integer".to_owned().into());
                        }
                        options.image_ppi = ppi as u32;
                    }
                    if let Some(bleed) = a.bleed {
                        options.boxes.bleed = true;
                        options.boxes.bleed_override = Some(bleed);
                    }
                    if let Some(marks) = a.marks {
                        for mark in marks.split(',') {
                            match mark {
                                "crop" => options.marks.crop = true,
                                "registration" => options.marks.registration = true,
                                "page_info" => options.marks.page_info = true,
                                "none" => {}
                                _ => return Err("unknown mark".to_owned().into()),
                            }
                        }
                    }
                    let (bytes, report) =
                        varos_pdf::images::export_with_options(&doc, &blobs, &plan, &options, &AtomicBool::new(false))
                            .map_err(|e| e.to_string())?;
                    if show_report {
                        export_report = Some(report);
                    }
                    bytes
                }
            } else {
                varos_pdf::images::write_vrs(&doc, &blobs, &Limits::DEFAULT)?
            };
            write_output(&out, &pdf)?;
            let mut result = json!({"out":out.to_string_lossy(),"bytes":pdf.len()});
            if let Some(report) = export_report {
                result["report"] = json!(report);
            }
            Ok(result)
        }
        "apply" => {
            let a = parse(args, &["--batch", "--out", "--in-place", "--ids"], 1)?;
            let out = match a.out {
                Some(out) => out,
                None if a.in_place => a.positional[0].clone(),
                None => return Err("missing required --out (or --in-place)".to_owned().into()),
            };
            let input = std::fs::canonicalize(&a.positional[0]).map_err(|e| e.to_string())?;
            let same = match std::fs::canonicalize(&out) {
                Ok(output) => output == input,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => return Err(e.to_string().into()),
            };
            if same && !a.in_place {
                return Err("--out resolves to the input; use --in-place to authorize replacing it".to_owned().into());
            }
            if a.in_place && !same {
                return Err("--in-place requires --out to resolve to the input, or omit --out".to_owned().into());
            }
            let out = if a.in_place { input } else { out };
            let batch = required(a.batch, "--batch")?;
            let mut bytes = vec![];
            std::fs::File::open(batch)
                .map_err(|e| e.to_string())?
                .take(1_048_577)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 1_048_576 {
                return Err("batch exceeds 1 MiB".to_owned().into());
            }
            let action = serde_json::from_slice::<Value>(&bytes).ok().is_some_and(|v| v.get("version").is_some());
            let count;
            let mut editor = Editor::new();
            let loaded = varos_pdf::load_vrs_checked(&a.positional[0], &Limits::DEFAULT).map_err(|e| e.to_string())?;
            editor.replace_doc(loaded.doc);
            editor.blobs = loaded.blobs;
            let before_rev = editor.rev;
            if action {
                if let Some(ids) = a.ids.as_deref() {
                    let ids = ids
                        .split(',')
                        .map(|id| id.parse::<u32>().map_err(|_| "--ids must be comma-separated path IDs".to_string()))
                        .collect::<Result<Vec<_>, _>>()?;
                    editor.try_execute(varos_core::EditCommand::SelectPaths(ids))?;
                }
                let actions = varos_core::actions::Actions::decode(&bytes)?;
                count = actions.steps.len();
                actions.replay(&mut editor)?;
            } else {
                let commands = bridge::parse_batch(&bytes)?;
                count = commands.len();
                editor.execute_batch(commands)?;
            }
            // w2-images: the native writer carries placed-image resources
            let pdf = varos_pdf::images::write_vrs(&editor.doc, &editor.blobs, &Limits::DEFAULT)?;
            write_output(&out, &pdf)?;
            Ok(json!({"out":out.to_string_lossy(),"commands":count,"changed":editor.rev>before_rev}))
        }
        "new" => {
            let a = parse(args, &["--preset", "--out"], 0)?;
            let out = required(a.out, "--out")?;
            let name = required(a.preset, "--preset")?;
            let doc = if name.eq_ignore_ascii_case("free") {
                board::new_board()
            } else {
                let p = board::PRESETS
                    .iter()
                    .find(|p| p.label.eq_ignore_ascii_case(&name))
                    .ok_or("unknown preset; expected free, square, portrait, story, a4".to_owned())?;
                board::new_board_with_preset(p.id)
            };
            let pdf = varos_pdf::write_pdf_checked(&doc, &Limits::DEFAULT)?;
            write_output(&out, &pdf)?;
            Ok(json!({"out":out.to_string_lossy(),"preset":name.to_lowercase(),"artboards":doc.artboards}))
        }
        "diff" => {
            let a = parse(args, &[], 2)?;
            let left = varos_pdf::load_vrs(&a.positional[0])?;
            let right = varos_pdf::load_vrs(&a.positional[1])?;
            Ok(bridge::diff(&left, &right))
        }
        _ => unreachable!(),
    }
}
/// Unique sibling temp file: refusal never truncates the destination or a user's fixed-name temp.
/// Does `out` name the same file as `input`? The canonical paths (symlinks, `..`) and, on Unix, the
/// device/inode pair (hard links). A missing `out` is never the input.
fn same_file(input: &Path, out: &Path) -> Result<bool, String> {
    let input_path = std::fs::canonicalize(input).map_err(|e| e.to_string())?;
    let out_path = match std::fs::canonicalize(out) {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.to_string()),
    };
    if input_path == out_path {
        return Ok(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let (a, b) = (
            std::fs::metadata(&input_path).map_err(|e| e.to_string())?,
            std::fs::metadata(&out_path).map_err(|e| e.to_string())?,
        );
        if (a.dev(), a.ino()) == (b.dev(), b.ino()) {
            return Ok(true);
        }
    }
    Ok(false)
}
fn write_output(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    for serial in 0..100 {
        let temp = parent.join(format!(".varos-cli-{}-{serial}.tmp", std::process::id()));
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        let result = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        let result = result.and_then(|()| std::fs::rename(&temp, path));
        if let Err(e) = result {
            let _ = std::fs::remove_file(&temp);
            return Err(e.to_string());
        }
        return Ok(());
    }
    Err("could not reserve an output temp file".into())
}

/// Exclusive destination creation for new deliverables; a raced destination remains intact.
fn write_fresh_output(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let temp = parent.join(format!(".varos-export-{}-{}.tmp", std::process::id(), bytes.len()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp).map_err(|e| e.to_string())?;
    let result = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let result = result.and_then(|()| std::fs::hard_link(&temp, path));
    let _ = std::fs::remove_file(&temp);
    result.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn panic_is_a_json_error_with_failure_status() {
        let (value, code) = super::response(|| panic!("test panic"));
        assert_eq!(code, 1);
        assert_eq!(value, super::json!({"ok":false,"error":{"reason":"internal panic: test panic","index":null}}));
    }
}
