//! Lane F: API 1.2 typed application and session-history adapters.
use crate::{Error, Host, Reply};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub api: String,
    pub action: PreferenceAction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PreferenceAction {
    Read {},
    Reconcile {},
    Apply { expected_generation: u64, values: PreferenceValues },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceValues {
    pub keyboard_increment_pt: f32,
    pub default_units: String,
    pub gpu_preference: String,
    pub history_depth: usize,
    pub recovery_enabled: bool,
    pub autosave_enabled: bool,
    pub autosave_interval_seconds: u64,
    pub language: String,
    pub canvas_colour: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryList {
    pub api: String,
    pub board: String,
    #[serde(default = "limit")]
    pub limit: usize,
    #[serde(default)]
    pub cursor: usize,
}
fn limit() -> usize {
    20
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryJump {
    pub api: String,
    pub board: String,
    pub request_id: String,
    pub expected_rev: u64,
    pub undo_depth: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionsRequest {
    pub api: String,
    pub board: String,
    pub request_id: String,
    pub expected_rev: u64,
    pub action: Action,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Start {},
    Cancel {},
    Stop { name: String },
    Replay { actions: varos_core::actions::Actions },
    UndoMine {},
}
pub fn history_list(host: &mut dyn Host, v: &HistoryList) -> Result<Reply, Error> {
    if v.limit == 0 || v.limit > 100 {
        return Err(Error::new("invalid_argument", "limit must be 1–100"));
    }
    let a = host.access(&v.board)?;
    let rows = a.editor.history_entries();
    if v.cursor > rows.len() {
        return Err(Error::new("invalid_argument", "history cursor out of range"));
    }
    let entries = rows.iter().rev().skip(v.cursor).take(v.limit).collect::<Vec<_>>();
    let next = v.cursor + entries.len();
    Ok(Reply::success(
        json!({"rev":a.editor.rev,"undo_depth":rows.len(),"entries":entries,"next_cursor":if next<rows.len(){Some(next)}else{None}}),
    ))
}
pub fn schemas() -> Vec<Value> {
    let action_document_schema: Value =
        serde_json::from_str(include_str!("actions_schema.json")).unwrap_or(Value::Null);
    let api = json!({"const":"1.2"});
    let board = json!({"type":"string","pattern":"^b[1-9][0-9]*$"});
    let mut out = vec![];
    let values = json!({"type":"object","additionalProperties":false,"required":["keyboard_increment_pt","default_units","gpu_preference","history_depth","recovery_enabled","autosave_enabled","autosave_interval_seconds","language","canvas_colour"],"properties":{"keyboard_increment_pt":{"type":"number","minimum":0.001,"maximum":1296},"default_units":{"enum":["px","pt","pc","mm","cm","in"]},"gpu_preference":{"enum":["auto","low_power","high_performance"]},"history_depth":{"type":"integer","minimum":5,"maximum":200},"recovery_enabled":{"type":"boolean"},"autosave_enabled":{"type":"boolean"},"autosave_interval_seconds":{"type":"integer","minimum":30,"maximum":1800},"language":{"type":"string","pattern":"^[a-z0-9_-]{1,64}$","description":"Only System and English are shipped; unavailable prior catalog IDs are retained with English fallback"},"canvas_colour":{"type":"string","pattern":"^(match_ui|white|#[0-9A-Fa-f]{6})$"}}});
    for(name,description,properties,required)in [
        ("preferences","Read or queue a generation-checked durable preferences Apply",json!({"api":api,"action":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"enum":["read","reconcile"]}}},{"type":"object","additionalProperties":false,"required":["kind","expected_generation","values"],"properties":{"kind":{"const":"apply"},"expected_generation":{"type":"integer","minimum":0},"values":values}}]}}),vec!["api","action"]),
        ("history_list","Review session history newest first",json!({"api":api,"board":board,"limit":{"type":"integer","minimum":1,"maximum":100,"default":20},"cursor":{"type":"integer","minimum":0}}),vec!["api","board"]),
        ("history_jump","Jump to a retained history position",json!({"api":api,"board":board,"request_id":{"type":"string"},"expected_rev":{"type":"integer","minimum":0},"undo_depth":{"type":"integer","minimum":0}}),vec!["api","board","request_id","expected_rev","undo_depth"]),
        ("actions","Record supported successful document commands, replay one atomic batch or undo the top step owned by this agent",json!({"api":api,"board":board,"request_id":{"type":"string"},"expected_rev":{"type":"integer","minimum":0},"action":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"enum":["start","cancel","undo_mine"]}}},{"type":"object","additionalProperties":false,"required":["kind","name"],"properties":{"kind":{"const":"stop"},"name":{"type":"string","maxLength":256}}},{"type":"object","additionalProperties":false,"required":["kind","actions"],"properties":{"kind":{"const":"replay"},"actions":action_document_schema}}]}}),vec!["api","board","request_id","expected_rev","action"])
    ]{out.push(json!({"name":name,"description":description,"inputSchema":{"type":"object","additionalProperties":false,"properties":properties,"required":required}}));}
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_tools_have_schema_and_old_api_refuses() {
        for row in schemas() {
            let name = row["name"].as_str().unwrap();
            assert!(crate::mcp::schema(name, None).is_ok());
        }
        assert!(
            serde_json::from_value::<Preferences>(json!({"api":"1.2","action":{"kind":"read","extra":true}})).is_err()
        );
        assert!(crate::mcp::tools_for("1.0")["tools"].as_array().unwrap().iter().all(|r| r["name"] != "preferences"));
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortcutsRequest {
    pub api: String,
    pub action: ShortcutAction,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ShortcutAction {
    Read {},
    Apply { expected_generation: u64, bindings: std::collections::BTreeMap<String, Option<ShortcutChord>> },
    Reset { expected_generation: u64 },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortcutChord {
    pub key: String,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandIndex {
    pub api: String,
    #[serde(default = "limit")]
    pub limit: usize,
    #[serde(default)]
    pub cursor: usize,
}
pub fn extra_schemas() -> Vec<Value> {
    vec![
        json!({"name":"help","description":"Open desktop documentation, shortcut list or crash-log folder","inputSchema":{"type":"object","additionalProperties":false,"required":["api","action"],"properties":{"api":{"const":"1.2"},"action":{"enum":["docs","shortcuts","report_problem"]}}}}),
        json!({"name":"shortcuts","description":"Read or queue additive validated shortcut overrides; reset to Illustrator defaults","inputSchema":{"type":"object","additionalProperties":false,"required":["api","action"],"properties":{"api":{"const":"1.2"},"action":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"read"}}},{"type":"object","additionalProperties":false,"required":["kind","expected_generation"],"properties":{"kind":{"const":"reset"},"expected_generation":{"type":"integer","minimum":0}}},{"type":"object","additionalProperties":false,"required":["kind","expected_generation","bindings"],"properties":{"kind":{"const":"apply"},"expected_generation":{"type":"integer","minimum":0},"bindings":{"type":"object","additionalProperties":{"oneOf":[{"type":"null"},{"type":"object","additionalProperties":false,"required":["key","primary","shift","alt"],"properties":{"key":{"type":"string"},"primary":{"type":"boolean"},"shift":{"type":"boolean"},"alt":{"type":"boolean"}}}]}}}}]}}}}),
        json!({"name":"command_index","description":"Read-only bounded desktop command index; identity is never arbitrary command execution","inputSchema":{"type":"object","additionalProperties":false,"required":["api"],"properties":{"api":{"const":"1.2"},"limit":{"type":"integer","minimum":1,"maximum":100,"default":20},"cursor":{"type":"integer","minimum":0}}}}),
    ]
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HelpRequest {
    pub api: String,
    pub action: HelpAction,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HelpAction {
    Docs,
    Shortcuts,
    ReportProblem,
}
