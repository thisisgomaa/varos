use serde::{Deserialize, Serialize};
use serde_json::Value;

fn api() -> String {
    crate::API.into()
}
fn page() -> usize {
    20
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "tool", content = "arguments", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Capabilities(Capabilities),
    ListBoards(ListBoards),
    Describe(Describe),
    Select(Select),
    Edit(Edit),
    History(History),
    RequestStatus(Status),
    Snapshot(Snapshot),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    #[serde(default = "api")]
    pub api: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ListBoards {
    #[serde(default = "api")]
    pub api: String,
    #[serde(default = "page")]
    pub limit: usize,
    #[serde(default)]
    pub cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Describe {
    #[serde(default = "api")]
    pub api: String,
    pub board: String,
    #[serde(default)]
    pub rev: Option<u64>,
    #[serde(default)]
    pub ids: Option<Vec<String>>,
    #[serde(default)]
    pub fields: Option<Vec<String>>,
    #[serde(default)]
    pub since: Option<u64>,
    #[serde(default = "page")]
    pub limit: usize,
    #[serde(default)]
    pub cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Select {
    pub api: String,
    pub request_id: String,
    pub board: String,
    pub expected_rev: u64,
    pub ids: Vec<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub api: String,
    pub request_id: String,
    pub board: String,
    pub expected_rev: u64,
    pub ops: Vec<Operation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}
/// Distinguish absent paint (leave it alone) from null (remove it).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Paint {
    #[default]
    Unchanged,
    None,
    Solid(String),
}
impl Serialize for Paint {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Solid(v) => s.serialize_str(v),
            _ => s.serialize_none(),
        }
    }
}
impl<'de> Deserialize<'de> for Paint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match Option::<String>::deserialize(d)? {
            Some(v) => Self::Solid(v),
            None => Self::None,
        })
    }
}
impl Paint {
    pub fn unchanged(&self) -> bool {
        *self == Self::Unchanged
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "verb", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    AddShape {
        kind: ShapeKind,
        bounds: [f32; 4],
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        insert: Option<InsertPosition>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        local: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Paint::unchanged")]
        fill: Paint,
        #[serde(default, skip_serializing_if = "Paint::unchanged")]
        stroke: Paint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_width: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opacity: Option<f32>,
    },
    Resize {
        ids: Vec<String>,
        bounds: [f32; 4],
    },
    Rotate {
        ids: Vec<String>,
        degrees: f32,
    },
    Rename {
        ids: Vec<String>,
        name: String,
    },
    Delete {
        ids: Vec<String>,
    },
    Align {
        ids: Vec<String>,
        mode: Alignment,
        target: String,
    },
    Distribute {
        ids: Vec<String>,
        axis: Axis,
    },
    Group {
        ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        local: Option<String>,
    },
    Ungroup {
        ids: Vec<String>,
    },
    Order {
        ids: Vec<String>,
        order: Order,
    },
    Move {
        ids: Vec<String>,
        delta: [f32; 2],
    },
    SetPaint {
        ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Paint::unchanged")]
        fill: Paint,
        #[serde(default, skip_serializing_if = "Paint::unchanged")]
        stroke: Paint,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        stroke_width: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        opacity: Option<f32>,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rect,
    Ellipse,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InsertPosition {
    Top,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Alignment {
    Left,
    Center,
    Right,
    Top,
    Middle,
    Bottom,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    H,
    V,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    Front,
    Forward,
    Backward,
    Back,
}
impl Operation {
    pub fn ids(&self) -> &[String] {
        match self {
            Self::AddShape { .. } => &[],
            Self::Move { ids, .. }
            | Self::SetPaint { ids, .. }
            | Self::Resize { ids, .. }
            | Self::Rotate { ids, .. }
            | Self::Rename { ids, .. }
            | Self::Delete { ids }
            | Self::Align { ids, .. }
            | Self::Distribute { ids, .. }
            | Self::Group { ids, .. }
            | Self::Ungroup { ids }
            | Self::Order { ids, .. } => ids,
        }
    }
    pub fn destructive(&self) -> bool {
        matches!(self, Self::Delete { .. } | Self::Ungroup { .. })
    }
}
fn snapshot_width() -> u32 {
    544
}
fn snapshot_height() -> u32 {
    246
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    #[serde(default = "api")]
    pub api: String,
    pub board: String,
    pub rev: u64,
    #[serde(default = "snapshot_width")]
    pub width: u32,
    #[serde(default = "snapshot_height")]
    pub height: u32,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HistoryAction {
    Undo,
    Redo,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    pub api: String,
    pub request_id: String,
    pub board: String,
    pub expected_rev: u64,
    pub action: HistoryAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    #[serde(default = "api")]
    pub api: String,
    pub request_id: String,
}
impl Request {
    pub fn api(&self) -> &str {
        match self {
            Self::Capabilities(v) => &v.api,
            Self::ListBoards(v) => &v.api,
            Self::Describe(v) => &v.api,
            Self::Select(v) => &v.api,
            Self::Edit(v) => &v.api,
            Self::History(v) => &v.api,
            Self::RequestStatus(v) => &v.api,
            Self::Snapshot(v) => &v.api,
        }
    }
    pub fn board(&self) -> Option<&str> {
        match self {
            Self::Describe(v) => Some(&v.board),
            Self::Snapshot(v) => Some(&v.board),
            Self::Select(v) => Some(&v.board),
            Self::Edit(v) => Some(&v.board),
            Self::History(v) => Some(&v.board),
            _ => None,
        }
    }
    pub fn mutation(&self) -> Option<(&str, u64)> {
        match self {
            Self::Select(v) => Some((&v.request_id, v.expected_rev)),
            Self::Edit(v) => Some((&v.request_id, v.expected_rev)),
            Self::History(v) => Some((&v.request_id, v.expected_rev)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Error {
    pub code: String,
    pub reason: String,
    pub retryable: bool,
    #[serde(flatten)]
    details: Box<ErrorDetails>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct ErrorDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_index: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_rev: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_rev: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}
impl std::ops::Deref for Error {
    type Target = ErrorDetails;
    fn deref(&self) -> &ErrorDetails {
        &self.details
    }
}
impl std::ops::DerefMut for Error {
    fn deref_mut(&mut self) -> &mut ErrorDetails {
        &mut self.details
    }
}
impl Error {
    pub fn new(code: &str, reason: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            reason: reason.into().chars().take(1024).collect(),
            retryable: matches!(code, "busy" | "revision_conflict"),
            details: Box::default(),
        }
    }
    pub fn at(mut self, index: usize) -> Self {
        self.op_index = Some(index);
        self
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Reply {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rev: Option<u64>,
    pub undo_steps: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}
impl Reply {
    pub fn success(value: Value) -> Self {
        Self { ok: true, result: Some(value), request_id: None, board: None, rev: None, undo_steps: 0, error: None }
    }
    pub fn failure(error: Error) -> Self {
        Self { ok: false, result: None, request_id: None, board: None, rev: None, undo_steps: 0, error: Some(error) }
    }
}
