//! Lane F / ADR-0015: stable identity index, never arbitrary command execution.
use serde::Serialize;
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Availability {
    pub enabled: bool,
    pub disabled_reason: Option<String>,
}
impl Availability {
    pub fn enabled() -> Self {
        Self { enabled: true, disabled_reason: None }
    }
    pub fn disabled(reason: impl Into<String>) -> Self {
        Self { enabled: false, disabled_reason: Some(reason.into()) }
    }
}
pub fn tool_id(tool: &str) -> String {
    match tool {
        "preferences" => "app.preferences".into(),
        "shortcuts" => "app.shortcuts".into(),
        "command_index" => "app.command-index".into(),
        "history_list" => "history.list".into(),
        "history_jump" => "history.jump".into(),
        "actions" => "app.actions".into(),
        "history" => "history.step".into(),
        "select" => "selection.set".into(),
        "save" => "file.save".into(),
        "save_as" => "file.save-copy".into(),
        _ => format!("bridge.{tool}"),
    }
}
pub fn edit_id(verb: &str) -> String {
    match verb {
        "move" => "edit.nudge".into(),
        "set_opacity" => "edit.opacity".into(),
        _ => format!("edit.{verb}"),
    }
}
/// Preserve historical menu IDs as aliases. Existing table remains migration authority.
pub fn menu_id(alias: &str) -> String {
    match alias {
        "edit.undo" => "history.undo".into(),
        "obj.group"=>"edit.group".into(),"obj.ungroup"=>"edit.ungroup".into(),"obj.clip"=>"edit.clip".into(),"obj.release_clip"=>"edit.release_clip".into(),"file.savecopy"=>"file.save-copy".into(),"file.print"=>"bridge.print".into(),"file.export.pdf"=>"bridge.export_pdf".into(),"file.save-template"=>"bridge.save_template".into(),"file.new-template"=>"bridge.new_from_template".into(),"file.place.svg"=>"bridge.import_svg".into(),
        "edit.redo" => "history.redo".into(),
        "file.save-copy" => "file.save-copy".into(),
        _ => alias
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_aliases() {
        assert_eq!(edit_id("move"), "edit.nudge");
        assert_eq!(menu_id("edit.undo"), "history.undo");
        assert!(!Availability::disabled("No document").enabled);
    }
}
