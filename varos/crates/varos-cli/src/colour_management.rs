//! Lane C: checked headless document colour settings and targeted paint.
use std::{ffi::OsString, io::Read, path::PathBuf};
use varos_core::{colour_management_commands::Command, EditCommand};
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    ids: Vec<u32>,
    command: Command,
}
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Input {
    Targeted(Envelope),
    Bare(Command),
}
pub fn run(args: Vec<OsString>) -> Result<serde_json::Value, String> {
    if args.len() != 3 {
        return Err("colour-management FILE COMMAND.json OUT.vrs".into());
    }
    let doc = varos_pdf::load_vrs(&PathBuf::from(&args[0]))?;
    let mut bytes = Vec::new();
    std::fs::File::open(PathBuf::from(&args[1]))
        .map_err(|e| e.to_string())?
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("colour command exceeds 8 MiB".into());
    }
    let input: Input = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    let (ids, command) = match input {
        Input::Targeted(e) => (e.ids, e.command),
        Input::Bare(c) => (vec![], c),
    };
    if matches!(command, Command::Paint { .. }) && ids.is_empty() {
        return Err("paint requires {ids:[path_id],command:{...}}".into());
    }
    if !matches!(command, Command::Paint { .. }) && !ids.is_empty() {
        return Err("document settings omit targets".into());
    }
    if matches!(command, Command::Proof { .. } | Command::Overprint { .. }) {
        return Err("view-only previews use Bridge or the desktop; they are not saved as artwork".into());
    }
    let mut ed = varos_core::Editor::new();
    ed.replace_doc(doc);
    if !ids.is_empty() {
        ed.try_execute(EditCommand::SelectPaths(ids))?;
    }
    ed.try_execute(EditCommand::ColourManagement(command))?;
    let out = PathBuf::from(&args[2]);
    varos_bridge::templates::save_new(&ed.doc, &out).map_err(|e| e.reason)?;
    Ok(serde_json::json!({"out":out,"colour_mode":ed.doc.colour_mode}))
}
