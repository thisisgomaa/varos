//! Lane A headless typed appearance/mask edits; creates a new destination exclusively.
use std::{ffi::OsString, io::Read, path::PathBuf};
use varos_core::{EditCommand, Editor};
pub fn run(verb: &str, args: Vec<OsString>) -> Result<serde_json::Value, String> {
    if args.len() != 3 {
        return Err("appearance|mask INPUT.vrs EDIT.json OUTPUT.vrs".into());
    }
    let input = PathBuf::from(&args[0]);
    let edit = PathBuf::from(&args[1]);
    let output = PathBuf::from(&args[2]);
    let loaded = varos_pdf::load_vrs_checked(&input, &Default::default()).map_err(|e| e.to_string())?;
    let mut ed = Editor::new();
    ed.replace_doc(loaded.doc);
    ed.blobs = loaded.blobs;
    let mut data = vec![];
    std::fs::File::open(edit)
        .map_err(|e| e.to_string())?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() > 1024 * 1024 {
        return Err("limit_exceeded: appearance command bytes".into());
    }
    let command = if verb == "appearance" {
        EditCommand::Appearance(serde_json::from_slice(&data).map_err(|e| e.to_string())?)
    } else {
        EditCommand::Mask(serde_json::from_slice(&data).map_err(|e| e.to_string())?)
    };
    ed.try_execute(command)?;
    let bytes = varos_pdf::images::write_vrs(&ed.doc, &ed.blobs, &Default::default())?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&output).map_err(|e| e.to_string())?;
    file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"out":output,"rev":ed.rev}))
}
