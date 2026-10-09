//! Lane B palette file host. Pure codecs/edits live in core; destinations are created exclusively.
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::PathBuf,
};
use varos_core::{
    colour_commands::ColourCommand as C,
    palette_io::{self, PaletteFormat},
    EditCommand, Editor,
};
fn format(path: &std::path::Path) -> Result<PaletteFormat, String> {
    match path.extension().and_then(|s| s.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("gpl") => Ok(PaletteFormat::Gpl),
        Some("ase") => Ok(PaletteFormat::Ase),
        Some("json") => Ok(PaletteFormat::Native),
        _ => Err("use .gpl / .ase / .json".into()),
    }
}
pub fn run(verb: &str, args: Vec<OsString>) -> Result<serde_json::Value, String> {
    if args.len() != if verb == "palette-import" { 3 } else { 2 } {
        return Err("palette-import FILE PALETTE OUT.vrs / palette-export FILE OUT.gpl|ase|json".into());
    }
    let source = PathBuf::from(&args[0]);
    let palette = PathBuf::from(&args[1]);
    let doc = varos_pdf::load_vrs(&source)?;
    if verb == "palette-export" {
        let data = palette_io::encode(&doc.swatches, format(&palette)?)?;
        let mut f =
            std::fs::OpenOptions::new().write(true).create_new(true).open(&palette).map_err(|e| e.to_string())?;
        f.write_all(&data).map_err(|e| e.to_string())?;
        Ok(serde_json::json!({"swatches":doc.swatches.len(),"bytes":data.len(),"out":palette}))
    } else {
        let mut data = vec![];
        std::fs::File::open(&palette)
            .map_err(|e| e.to_string())?
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        let mut ed = Editor::new();
        ed.replace_doc(doc);
        ed.try_execute(EditCommand::Colour(C::ImportPalette { format: format(&palette)?, data }))?;
        let out = PathBuf::from(&args[2]);
        varos_bridge::templates::save_new(&ed.doc, &out).map_err(|e| e.reason)?;
        Ok(serde_json::json!({"swatches":ed.doc.swatches.len(),"out":out}))
    }
}
