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
    WindowMemory(Capabilities),
    ListBoards(ListBoards),
    Describe(Describe),
    Select(Select),
    Edit(Edit),
    History(History),
    RequestStatus(Status),
    Snapshot(Snapshot),
    Save(FileEffect),
    SaveAs(FileEffect),
    ExportPdf(FileEffect),
    SaveTemplate(FileEffect),
    NewFromTemplate(FileEffect),
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
    /// Board sections and object fields compose; ids scopes only object detail.
    pub fields: Option<Vec<String>>,
    #[serde(default)]
    pub since: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present_usize")]
    pub summary_budget: Option<usize>,
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
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Edit {
    pub api: String,
    pub request_id: String,
    pub board: String,
    pub expected_rev: u64,
    pub ops: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present_value")]
    pub defaults: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present_string")]
    pub receipt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}
impl Serialize for Edit {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // 1.0 retains the exact historical Operation DTO projection (including float spelling,
        // anchor defaults and omitted Option fields). 1.1 hashes/transports the original forms.
        let ops: Vec<Value> = self
            .ops
            .iter()
            .map(|raw| {
                if self.api == "1.0" {
                    serde_json::from_value::<Operation>(raw.clone())
                        .ok()
                        .and_then(|op| serde_json::to_value(op).ok())
                        .unwrap_or_else(|| raw.clone())
                } else {
                    raw.clone()
                }
            })
            .collect();
        let mut value = serde_json::json!({"api":self.api,"request_id":self.request_id,"board":self.board,"expected_rev":self.expected_rev,"ops":ops});
        if let Some(v) = &self.digest {
            value["digest"] = serde_json::json!(v);
        }
        if let Some(v) = &self.defaults {
            value["defaults"] = v.clone();
        }
        if let Some(v) = &self.receipt {
            value["receipt"] = serde_json::json!(v);
        }
        value.serialize(serializer)
    }
}
fn present_usize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<usize>, D::Error> {
    usize::deserialize(d).map(Some)
}
fn present_value<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
fn present_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
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
    /// API 1.2 only; one field per operation makes history intent explicit.
    DocumentSetup {
        field: String,
        value: Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artboard: Option<String>,
    },
    AddShape {
        kind: ShapeKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        radius: Option<f32>,
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
    AddPath {
        anchors: Vec<PathAnchor>,
        closed: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
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
    /// Slice 3: a new page, from explicit `bounds` OR a `preset` (optionally placed at `origin`, else
    /// to the right of the right-most page). Returns `artboard:<id>` through `locals`/`artboards_created`.
    AddArtboard {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bounds: Option<[f32; 4]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        preset: Option<ArtboardPreset>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        origin: Option<[f32; 2]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        local: Option<String>,
    },
    ResizeArtboard {
        id: String,
        bounds: [f32; 4],
    },
    RenameArtboard {
        id: String,
        name: String,
    },
    DeleteArtboard {
        id: String,
    },
    ReorderArtboard {
        id: String,
        position: usize,
    },
    DuplicateArtboard {
        id: String,
        with_art: bool,
        #[serde(default)]
        offset: Option<[f32; 2]>,
        #[serde(default)]
        local: Option<String>,
    },
    SetArtboardColor {
        id: String,
        #[serde(deserialize_with = "nullable_color")]
        color: Option<String>,
    },
    SetArtboardClip {
        id: String,
        clip: bool,
    },
    SetActiveArtboard {
        id: String,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PathAnchor {
    pub p: [f32; 2],
    #[serde(default)]
    pub hin: Option<[f32; 2]>,
    #[serde(default)]
    pub hout: Option<[f32; 2]>,
    #[serde(default)]
    pub smooth: bool,
}
fn nullable_color<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(d)
}
/// The board presets (`varos_core::board::PRESETS`), in points.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtboardPreset {
    Square,
    Portrait,
    Story,
    A4,
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
            Self::DocumentSetup { .. }
            | Self::AddShape { .. }
            | Self::AddPath { .. }
            | Self::AddArtboard { .. }
            | Self::ResizeArtboard { .. }
            | Self::RenameArtboard { .. }
            | Self::DeleteArtboard { .. }
            | Self::ReorderArtboard { .. }
            | Self::DuplicateArtboard { .. }
            | Self::SetArtboardColor { .. }
            | Self::SetArtboardClip { .. }
            | Self::SetActiveArtboard { .. } => &[],
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
    /// The artboard this operation addresses (`artboard:N` or a request-local), if it is a page verb.
    pub fn artboard(&self) -> Option<&str> {
        match self {
            Self::ResizeArtboard { id, .. }
            | Self::RenameArtboard { id, .. }
            | Self::DeleteArtboard { id }
            | Self::ReorderArtboard { id, .. }
            | Self::DuplicateArtboard { id, .. }
            | Self::SetArtboardColor { id, .. }
            | Self::SetArtboardClip { id, .. }
            | Self::SetActiveArtboard { id } => Some(id),
            _ => None,
        }
    }
    /// A slice-3 page verb (these can add, remove or re-index artboards).
    pub fn is_page_verb(&self) -> bool {
        matches!(
            self,
            Self::AddArtboard { .. }
                | Self::ResizeArtboard { .. }
                | Self::RenameArtboard { .. }
                | Self::DeleteArtboard { .. }
                | Self::ReorderArtboard { .. }
                | Self::DuplicateArtboard { .. }
                | Self::SetArtboardColor { .. }
                | Self::SetArtboardClip { .. }
                | Self::SetActiveArtboard { .. }
        )
    }
    /// An `align` whose target is the deprecated revision-bound `aN@rev` page reference.
    pub fn uses_legacy_artboard_alias(&self) -> bool {
        matches!(self, Self::Align { target, .. }
            if target.starts_with('a') && target.contains('@') && !target.starts_with("artboard:"))
    }
    pub fn destructive(&self) -> bool {
        matches!(self, Self::Delete { .. } | Self::Ungroup { .. } | Self::DeleteArtboard { .. })
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
    /// Slice 3: render only this page (`artboard:N`): its own bounds and background, at its aspect
    /// ratio inside `width`×`height` (each default 1024 for a page). Absent: the fitted board preview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artboard: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present_string")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}
impl Snapshot {
    /// The requested image box: the board preview defaults to 544×246, a page snapshot to 1024×1024.
    pub fn size(&self) -> (u32, u32) {
        let (w, h) = if self.profile.as_deref() == Some("economy") {
            if self.artboard.is_some() {
                (512, 512)
            } else {
                (512, 232)
            }
        } else if self.artboard.is_some() {
            (1024, 1024)
        } else {
            (snapshot_width(), snapshot_height())
        };
        (self.width.unwrap_or(w), self.height.unwrap_or(h))
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present_string")]
    pub cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileEffect {
    #[serde(default = "api")]
    pub api: String,
    pub request_id: String,
    pub board: String,
    pub expected_rev: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}
impl Request {
    /// Wire tool name (for audit records; never carries arguments).
    pub fn tool(&self) -> &'static str {
        match self {
            Self::Capabilities(_) => "capabilities",
            Self::WindowMemory(_) => "window_memory",
            Self::ListBoards(_) => "list_boards",
            Self::Describe(_) => "describe",
            Self::Select(_) => "select",
            Self::Edit(_) => "edit",
            Self::History(_) => "history",
            Self::RequestStatus(_) => "request_status",
            Self::Snapshot(_) => "snapshot",
            Self::Save(_) => "save",
            Self::SaveAs(_) => "save_as",
            Self::ExportPdf(_) => "export_pdf",
            Self::SaveTemplate(_) => "save_template",
            Self::NewFromTemplate(_) => "new_from_template",
        }
    }
    pub fn api(&self) -> &str {
        match self {
            Self::Capabilities(v) | Self::WindowMemory(v) => &v.api,
            Self::ListBoards(v) => &v.api,
            Self::Describe(v) => &v.api,
            Self::Select(v) => &v.api,
            Self::Edit(v) => &v.api,
            Self::History(v) => &v.api,
            Self::RequestStatus(v) => &v.api,
            Self::Snapshot(v) => &v.api,
            Self::Save(v) | Self::SaveAs(v) | Self::ExportPdf(v) | Self::SaveTemplate(v) | Self::NewFromTemplate(v) => {
                &v.api
            }
        }
    }
    pub fn board(&self) -> Option<&str> {
        match self {
            Self::Describe(v) => Some(&v.board),
            Self::Snapshot(v) => Some(&v.board),
            Self::Save(v) | Self::SaveAs(v) | Self::ExportPdf(v) | Self::SaveTemplate(v) | Self::NewFromTemplate(v) => {
                Some(&v.board)
            }
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
            Self::Save(v) | Self::SaveAs(v) | Self::ExportPdf(v) | Self::SaveTemplate(v) | Self::NewFromTemplate(v) => {
                Some((&v.request_id, v.expected_rev))
            }
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
    pub location: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ids: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_rev: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_rev: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// ADR-0011 `ambiguous_target`: bounded, non-secret host candidates (no board names or paths).
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub candidates: Vec<Value>,
    /// Historical API 1.0 compatibility field; open local trust never creates pairing requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pairing: Option<Value>,
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
            retryable: matches!(code, "busy" | "revision_conflict" | "host_not_running" | "session_reset"),
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
