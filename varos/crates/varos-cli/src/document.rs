//! Headless Phase 1 document/template commands; immutable files never get a template flag.
use serde_json::{json, Value};
use std::{ffi::OsString, path::PathBuf};
pub fn run(verb: &str, args: Vec<OsString>) -> Result<Value, String> {
    match verb {
        "document-info" => {
            if args.len() != 1 {
                return Err("document-info FILE".into());
            }
            Ok(varos_core::document_setup::info(&varos_pdf::load_vrs(&PathBuf::from(&args[0]))?))
        }
        "document-setup" => {
            if args.len() != 4 {
                return Err("document-setup FILE --batch EDIT_COMMANDS_JSON OUT.vrs".into());
            }
            if args[1] != "--batch" {
                return Err("expected --batch".into());
            }
            let mut ed = varos_core::Editor::new();
            ed.replace_doc(varos_pdf::load_vrs(&PathBuf::from(&args[0]))?);
            let bytes = std::fs::read(PathBuf::from(&args[2])).map_err(|e| e.to_string())?;
            let commands: Vec<varos_core::EditCommand> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if commands.is_empty() || commands.len() > 100 {
                return Err("batch needs 1..100 fields".into());
            }
            for command in commands {
                if !matches!(
                    command,
                    varos_core::EditCommand::SetUnits(_)
                        | varos_core::EditCommand::SetPpi(_)
                        | varos_core::EditCommand::SetBleed { .. }
                        | varos_core::EditCommand::SetTransparencyGrid(_)
                ) {
                    return Err("only setup fields allowed".into());
                }
                ed.try_execute(command)?;
            }
            varos_bridge::templates::save_new(&ed.doc, &PathBuf::from(&args[3])).map_err(|e| e.reason)?;
            Ok(varos_core::document_setup::info(&ed.doc))
        }
        "save-template" => {
            if args.len() != 2 {
                return Err("save-template FILE NAME.vrs".into());
            }
            let doc = varos_pdf::load_vrs(&PathBuf::from(&args[0]))?;
            let name = PathBuf::from(&args[1]);
            if name.components().count() != 1 || name.extension().is_none_or(|e| e != "vrs") {
                return Err("template name must be a single NAME.vrs".into());
            }
            let folder = varos_bridge::storage_paths::data_root().ok_or("app data unavailable")?.join("Templates");
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            let out = folder.join(name);
            if out.exists() {
                return Err("template already exists".into());
            }
            varos_bridge::templates::save_new(&doc, &out).map_err(|e| e.reason)?;
            Ok(json!({"path":out}))
        }
        "new-from-template" => {
            if args.len() != 2 {
                return Err("new-from-template TEMPLATE.vrs OUT.vrs".into());
            }
            let mut doc = varos_pdf::load_vrs(&PathBuf::from(&args[0]))?;
            doc.name.clear();
            varos_bridge::templates::save_new(&doc, &PathBuf::from(&args[1])).map_err(|e| e.reason)?;
            Ok(json!({"name":"Untitled","template_source":PathBuf::from(&args[0]),"out":PathBuf::from(&args[1])}))
        }
        _ => Err("unknown document command".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn folder() -> PathBuf {
        std::env::temp_dir().join(format!("varos-cli-doc-{}", varos_bridge::conn::random_hex(8).unwrap()))
    }
    #[test]
    fn setup_batch_and_template_headless_outputs() {
        let root = folder();
        std::fs::create_dir_all(&root).unwrap();
        let input = root.join("source.vrs");
        let output = root.join("setup.vrs");
        let batch = root.join("fields.json");
        let template = root.join("untitled.vrs");
        let doc = varos_core::model::Document { name: "Source".into(), ..Default::default() };
        varos_bridge::templates::save_new(&doc, &input).unwrap();
        std::fs::write(&batch, br#"[{"SetPpi":300},{"SetTransparencyGrid":true}]"#).unwrap();
        run(
            "document-setup",
            vec![
                input.clone().into_os_string(),
                "--batch".into(),
                batch.into_os_string(),
                output.clone().into_os_string(),
            ],
        )
        .unwrap();
        let back = varos_pdf::load_vrs(&output).unwrap();
        assert_eq!(back.units.ppi, 300.0);
        assert!(back.transparency_grid);
        run("new-from-template", vec![output.into_os_string(), template.clone().into_os_string()]).unwrap();
        let back = varos_pdf::load_vrs(&template).unwrap();
        assert!(back.name.is_empty());
        assert_eq!(back.units.ppi, 300.0);
        let info = run("document-info", vec![template.into_os_string()]).unwrap();
        assert_eq!(info["counts"]["paths"], 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
