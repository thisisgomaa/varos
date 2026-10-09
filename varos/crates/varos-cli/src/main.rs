//! Thin filesystem/argument host for the provisional core Bridge contracts.
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
const VERBS: &[&str] = &["describe", "snapshot", "export-pdf", "save-as", "apply", "new", "diff"];
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
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
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
    if !VERBS.contains(&verb.as_str()) {
        return Err(format!("unknown subcommand {verb}; expected {}", VERBS.join(", ")).into());
    }
    match verb.as_str() {
        "describe" => {
            let a = parse(args, &["--detail"], 1)?;
            let doc = varos_pdf::load_vrs(&a.positional[0])?;
            Ok(bridge::describe(&doc, a.detail.as_deref())?)
        }
        "snapshot" => {
            let a = parse(args, &["--out", "--size"], 1)?;
            let out = required(a.out, "--out")?;
            let size = a.size.unwrap_or(400);
            let doc = varos_pdf::load_vrs(&a.positional[0])?;
            let png = varos_raster::rasterize(Arc::new(doc), [size, size]).encode_png()?;
            write_output(&out, &png)?;
            Ok(json!({"out":out.to_string_lossy(),"width":size,"height":size,"bytes":png.len()}))
        }
        "export-pdf" | "save-as" => {
            let a = parse(args, if verb == "export-pdf" { &["--out", "--artboard"] } else { &["--out"] }, 1)?;
            let out = required(a.out, "--out")?;
            // An export is a model-free PDF: writing it over the input would destroy the editable
            // document. Refused outright (there is no `--in-place` for export), checked before loading.
            if verb == "export-pdf" && same_file(&a.positional[0], &out)? {
                return Err("--out resolves to the input; an export would replace the editable document with a \
                     model-free PDF. Choose another --out"
                    .to_owned()
                    .into());
            }
            let mut doc = varos_pdf::load_vrs(&a.positional[0])?;
            let pdf = if verb == "export-pdf" {
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
                    let (bytes, report) = varos_pdf::export_pdf_bytes_with_report(&doc, &plan, &AtomicBool::new(false))
                        .map_err(|e| e.to_string())?;
                    for note in report.notes {
                        eprintln!("{}: {}", note.kind, note.message);
                    }
                    bytes
                }
            } else {
                varos_pdf::write_pdf_checked(&doc, &Limits::DEFAULT)?
            };
            write_output(&out, &pdf)?;
            Ok(json!({"out":out.to_string_lossy(),"bytes":pdf.len()}))
        }
        "apply" => {
            let a = parse(args, &["--batch", "--out", "--in-place"], 1)?;
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
            let commands = bridge::parse_batch(&bytes)?;
            let count = commands.len();
            let mut editor = Editor::new();
            editor.replace_doc(varos_pdf::load_vrs(&a.positional[0])?);
            let before_rev = editor.rev;
            editor.execute_batch(commands)?;
            let pdf = varos_pdf::write_pdf_checked(&editor.doc, &Limits::DEFAULT)?;
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

#[cfg(test)]
mod tests {
    #[test]
    fn panic_is_a_json_error_with_failure_status() {
        let (value, code) = super::response(|| panic!("test panic"));
        assert_eq!(code, 1);
        assert_eq!(value, super::json!({"ok":false,"error":{"reason":"internal panic: test panic","index":null}}));
    }
}
