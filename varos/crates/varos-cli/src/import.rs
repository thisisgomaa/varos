//! Headless foreign import; native input is never sniffed/reconstructed.
use std::{ffi::OsString, io::Read, path::PathBuf};
use varos_core::bridge::{json, Value};
pub fn run(verb: &str, args: Vec<OsString>) -> Result<Value, String> {
    let mut options = varos_import::ImportOptions::default();
    let mut source = None;
    let mut out = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.to_str() {
            Some("--allow-loss") => options.loss_policy = varos_import::LossPolicy::AllowReported,
            Some("--page") => {
                options.page = Some(
                    iter.next()
                        .and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
                        .ok_or("--page requires integer")?,
                )
            }
            Some("--points-per-unit") => {
                options.points_per_unit = Some(
                    iter.next()
                        .and_then(|v| v.to_str().and_then(|s| s.parse().ok()))
                        .ok_or("--points-per-unit requires number")?,
                )
            }
            Some("--out") => out = Some(PathBuf::from(iter.next().ok_or("--out requires destination")?)),
            Some(v) if v.starts_with('-') => return Err(format!("Unknown import option {v}")),
            _ if source.is_none() => source = Some(PathBuf::from(arg)),
            _ => return Err("Import takes one source".into()),
        }
    }
    let source = source.ok_or("Import requires source")?;
    let out = out.ok_or("Import requires --out")?;
    if !out.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrs")) {
        return Err("Import output must be a new .vrs file".into());
    }
    if super::same_file(&source, &out)? {
        return Err("Import source and output must differ".into());
    }
    let extension = source.extension().and_then(|e| e.to_str()).ok_or("Source extension required")?;
    let format = varos_import::Format::from_extension(extension)?;
    let expected = match verb {
        "import-pdf" => Some(varos_import::Format::Pdf),
        "import-ai" => Some(varos_import::Format::Ai),
        "import-dxf" => Some(varos_import::Format::Dxf),
        _ => None,
    };
    if expected.is_some_and(|f| f != format) {
        return Err("Import verb and source extension disagree".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&source)
        .map_err(|e| e.to_string())?
        .take((varos_import::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let (doc, report) =
        varos_import::worker::isolated_import(&bytes, format, options, &std::sync::atomic::AtomicBool::new(false))?;
    let bytes = varos_pdf::write_pdf_checked(&doc, &varos_core::format::Limits::DEFAULT)?;
    write_new_output(&out, &bytes)?;
    Ok(json!({"out":out.to_string_lossy(),"report":report}))
}

// Publish without replacing an existing file, including a destination created during parsing.
fn write_new_output(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(std::path::Path::new("."));
    for serial in 0..100 {
        let temp = parent.join(format!(".varos-import-{}-{serial}.tmp", std::process::id()));
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        let result = file.write_all(bytes).and_then(|()| file.sync_all());
        drop(file);
        // A hard link publishes the complete file atomically and refuses existing destinations.
        let result = result.and_then(|()| std::fs::hard_link(&temp, path));
        let _ = std::fs::remove_file(&temp);
        return result.map_err(|e| format!("Import requires a fresh output file: {e}"));
    }
    Err("Could not reserve an import output temporary file".into())
}
