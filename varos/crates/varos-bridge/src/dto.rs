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
    // ---- Lane F ----
    Help(crate::application::HelpRequest),
    Preferences(crate::application::Preferences),
    Shortcuts(crate::application::ShortcutsRequest),
    CommandIndex(crate::application::CommandIndex),
    HistoryList(crate::application::HistoryList),
    HistoryJump(crate::application::HistoryJump),
    Actions(crate::application::ActionsRequest),
    Capabilities(Capabilities),
    Schema(Schema),
    ListVerbs(Capabilities),
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
    /// API 1.2 only; import source under the files scope, placed into board.
    ImportSvg(FileEffect),
    // ---- Lane H ----
    ImportFile(FileEffect),
    ImportClipboard(FileEffect),
    // ---- w2-images ----
    AddImage(FileEffect),
    ImageAction(FileEffect),
    ExportSvg(FileEffect),
    // ---- Lane C ----
    #[serde(alias = "export_screens")]
    ExportRaster(FileEffect),
    SaveTemplate(FileEffect),
    NewFromTemplate(FileEffect),
    Print(FileEffect),
    Copy(FileEffect),
    Cut(FileEffect),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Schema {
    pub api: String,
    pub tool: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verb: Option<String>,
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
pub struct Lasso {
    pub points: Vec<[f32; 2]>,
    pub objects: bool,
    pub additive: bool,
}
fn selection_mode<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<varos_core::editor::wave::Selection>, D::Error> {
    varos_core::editor::wave::Selection::deserialize(d).map(Some)
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Select {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paste_remembers_layers: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lasso: Option<Lasso>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "selection_mode")]
    pub mode: Option<varos_core::editor::wave::Selection>,
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
    // ---- Lane A ----
    Appearance {
        edit: varos_core::appearance_edits::AppearanceEdit,
    },
    Mask {
        edit: varos_core::appearance_edits::MaskEdit,
    },
    // ---- Lane D: API 1.2 drawing ----
    ShapeTool {
        spec: varos_core::drawing::ShapeSpec,
    },
    Pencil {
        points: Vec<[f32; 2]>,
        options: varos_core::drawing::Options,
    },
    SmoothPath {
        ids: Vec<String>,
        points: Vec<[f32; 2]>,
        options: varos_core::drawing::Options,
    },
    PathErase {
        ids: Vec<String>,
        points: Vec<[f32; 2]>,
        options: varos_core::drawing::Options,
    },
    JoinTool {
        ids: Vec<String>,
        points: Vec<[f32; 2]>,
        options: varos_core::drawing::Options,
    },
    Curvature {
        points: Vec<[f32; 2]>,
        closed: bool,
    },
    DrawingOptions {
        options: varos_core::drawing::Options,
    },
    // ---- Lane G ----
    AddText {
        text: varos_core::text::TextBox,
        #[serde(default)]
        parent: Option<String>,
        #[serde(default)]
        local: Option<String>,
    },
    SetText {
        node: String,
        text: varos_core::text::TextBox,
    },
    // ---- w2-gradients ----
    // ---- w3-cmyk ----
    ColourManagement {
        ids: Vec<String>,
        command: varos_core::colour_management_commands::Command,
    },
    Colour {
        ids: Vec<String>,
        command: varos_core::colour_commands::ColourCommand,
    },
    // ---- Lane C ----
    OutlineStroke {
        ids: Vec<String>,
    },
    OffsetPath {
        ids: Vec<String>,
        delta: f32,
        join: varos_core::stroke::StrokeJoin,
        miter: f32,
    },
    Expand {
        ids: Vec<String>,
    },
    // ---- Lane B w3-effects ----
    LiveEffects {
        ids: Vec<String>,
        effects: Vec<varos_core::effects::Effect>,
    },
    WidthProfile {
        ids: Vec<String>,
        profile: Option<varos_core::width_profile::WidthProfile>,
    },
    WidthTool {},
    ExpandLive {
        ids: Vec<String>,
    },
    // ---- end Lane B w3-effects ----
    LiveCorners {
        ids: Vec<String>,
        corners: Vec<varos_core::live_corners::CornerParam>,
    },
    ScaleStrokes {
        enabled: bool,
    },
    NewDocument {
        settings: varos_core::new_document::Settings,
    },
    Pathfinder {
        ids: Vec<String>,
        operation: String,
    },
    ShapeBuilder {
        ids: Vec<String>,
        points: Vec<[f32; 2]>,
        delete: bool,
    },
    Scissors {
        ids: Vec<String>,
        segment: usize,
        t: f32,
    },
    View {
        ids: Vec<String>,
        action: varos_core::editor::view_commands::ViewAction,
    },
    AnchorType {
        ids: Vec<String>,
        anchor: u32,
        smooth: bool,
    },
    InsertAnchor {
        ids: Vec<String>,
        segment: usize,
        t: f32,
    },
    Knife {
        ids: Vec<String>,
        points: Vec<[f32; 2]>,
    },
    Eraser {
        ids: Vec<String>,
        points: Vec<[f32; 2]>,
        radius: f32,
    },
    DivideObjectsBelow {
        ids: Vec<String>,
    },
    ToolOptions {
        #[serde(default)]
        wand: Option<varos_core::select_transform::WandOptions>,
        #[serde(default)]
        eyedropper: Option<varos_core::select_transform::PickOptions>,
    },
    Transform {
        ids: Vec<String>,
        spec: varos_core::select_transform::Transform,
    },
    MagicWand {
        ids: Vec<String>,
        options: varos_core::select_transform::WandOptions,
        mode: varos_core::select_transform::SelectMode,
    },
    Eyedropper {
        ids: Vec<String>,
        source: String,
        options: varos_core::select_transform::PickOptions,
        colour_only: bool,
    },
    Isolation {
        ids: Vec<String>,
        exit: bool,
    },
    Layers {
        ids: Vec<String>,
        action: varos_core::select_transform::LayerAction,
    },
    TraceRgba {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        #[serde(default)]
        options: varos_core::trace::TraceOptions,
    },
    DeleteAnchor {
        ids: Vec<String>,
        anchor: u32,
    },
    DistributeMode {
        ids: Vec<String>,
        mode: Alignment,
    },
    Object {
        ids: Vec<String>,
        action: varos_core::editor::wave::ObjectAction,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anchors: Option<Vec<u32>>,
    },
    DistributeSpacing {
        ids: Vec<String>,
        axis: Axis,
        gap: f32,
    },
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
    Clip {
        ids: Vec<String>,
    },
    ReleaseClip {
        ids: Vec<String>,
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
    SetStrokeStyle {
        ids: Vec<String>,
        stroke_style: varos_core::stroke::StrokeStyle,
    },
    SetPaint {
        ids: Vec<String>,
        #[serde(default, deserialize_with = "optional_stroke_style", skip_serializing_if = "Option::is_none")]
        stroke_style: Option<varos_core::stroke::StrokeStyle>,
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
    pub fn drawing(&self) -> bool {
        matches!(
            self,
            Self::ShapeTool { .. }
                | Self::Pencil { .. }
                | Self::SmoothPath { .. }
                | Self::PathErase { .. }
                | Self::JoinTool { .. }
                | Self::Curvature { .. }
                | Self::DrawingOptions { .. }
        )
    }
    pub fn lane_c(&self) -> bool {
        matches!(
            self,
            // ---- Lane B w3-effects ----
            Self::WidthTool { .. }
                | Self::LiveEffects { .. }
                | Self::WidthProfile { .. }
                | Self::ExpandLive { .. }
                // ---- end Lane B w3-effects ----
                | Self::OutlineStroke { .. }
                | Self::OffsetPath { .. }
                | Self::Expand { .. }
                | Self::LiveCorners { .. }
                | Self::ScaleStrokes { .. }
                | Self::NewDocument { .. }
        )
    }
    pub fn slice4a(&self) -> bool {
        matches!(
            self,
            // ---- w3-cmyk ----
            Self::ColourManagement { .. }
                | Self::Colour { .. }
                | Self::ScaleStrokes { .. }
                | Self::NewDocument { .. }
                | Self::ToolOptions { .. }
                | Self::Transform { .. }
                | Self::MagicWand { .. }
                | Self::Eyedropper { .. }
                | Self::Isolation { .. }
                | Self::Layers { .. }
        )
    }

    pub fn ids(&self) -> &[String] {
        match self {
            // ---- Lane A ----
            Self::Appearance { .. } | Self::Mask { .. } => &[],
            Self::AddText { .. }
            | Self::SetText { .. }
            | Self::ScaleStrokes { .. }
            | Self::NewDocument { .. }
            | Self::ToolOptions { .. }
            | Self::TraceRgba { .. }
            | Self::DocumentSetup { .. }
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
            | Self::SetActiveArtboard { .. }
            | Self::ShapeTool { .. }
            | Self::Pencil { .. }
            | Self::Curvature { .. }
            | Self::DrawingOptions { .. } => &[],
            // ---- Lane B w3-effects ----
            Self::WidthTool { .. } => &[],
            // ---- end Lane B w3-effects ----
            Self::SmoothPath { ids, .. }
            | Self::PathErase { ids, .. }
            | Self::JoinTool { ids, .. }
            // ---- w3-cmyk ----
            | Self::ColourManagement { ids, .. }
            | Self::Colour { ids, .. }
            // ---- Lane B w3-effects ----
            | Self::LiveEffects { ids, .. }
            | Self::WidthProfile { ids, .. }
            | Self::ExpandLive { ids }
            // ---- end Lane B w3-effects ----
            | Self::OutlineStroke { ids }
            | Self::OffsetPath { ids, .. }
            | Self::Expand { ids }
            | Self::LiveCorners { ids, .. }
            | Self::Pathfinder { ids, .. }
            | Self::ShapeBuilder { ids, .. }
            | Self::Scissors { ids, .. }
            | Self::Knife { ids, .. }
            | Self::Eraser { ids, .. }
            | Self::DivideObjectsBelow { ids }
            | Self::Transform { ids, .. }
            | Self::MagicWand { ids, .. }
            | Self::Eyedropper { ids, .. }
            | Self::Isolation { ids, .. }
            | Self::Layers { ids, .. }
            | Self::View { ids, .. }
            | Self::AnchorType { ids, .. }
            | Self::InsertAnchor { ids, .. }
            | Self::DeleteAnchor { ids, .. }
            | Self::DistributeMode { ids, .. }
            | Self::Object { ids, .. }
            | Self::DistributeSpacing { ids, .. }
            | Self::Move { ids, .. }
            | Self::SetPaint { ids, .. }
            | Self::SetStrokeStyle { ids, .. }
            | Self::Resize { ids, .. }
            | Self::Rotate { ids, .. }
            | Self::Rename { ids, .. }
            | Self::Delete { ids }
            | Self::Align { ids, .. }
            | Self::Distribute { ids, .. }
            | Self::Group { ids, .. }
            | Self::Clip { ids }
            | Self::ReleaseClip { ids }
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
        matches!(
            self,
            Self::PathErase { .. }
                | Self::JoinTool { .. }
                | Self::Delete { .. }
                | Self::Ungroup { .. }
                | Self::DeleteArtboard { .. }
                | Self::Pathfinder { .. }
                | Self::ShapeBuilder { .. }
                | Self::Scissors { .. }
                | Self::Knife { .. }
                | Self::Eraser { .. }
                | Self::DivideObjectsBelow { .. }
                | Self::View {
                    action: varos_core::editor::view_commands::ViewAction::ClearGuides
                        | varos_core::editor::view_commands::ViewAction::ConvertArtboards,
                    ..
                }
                | Self::Object {
                    action: varos_core::editor::wave::ObjectAction::Join
                        | varos_core::editor::wave::ObjectAction::CleanUp
                        | varos_core::editor::wave::ObjectAction::CompoundMake
                        | varos_core::editor::wave::ObjectAction::CompoundRelease,
                    ..
                }
        )
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
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileEffect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(deserialize_with = "present_value")]
    pub options: Option<serde_json::Value>,
    #[serde(default = "api")]
    pub api: String,
    pub request_id: String,
    pub board: String,
    pub expected_rev: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppi: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transparent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<u8>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExportFileWire {
    #[serde(default, deserialize_with = "present_value")]
    options: Option<serde_json::Value>,
    #[serde(default = "api")]
    pub api: String,
    pub request_id: String,
    pub board: String,
    pub expected_rev: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, deserialize_with = "present_export_option")]
    pub format: Option<Option<String>>,
    #[serde(default, deserialize_with = "present_export_option")]
    pub scale: Option<Option<f32>>,
    #[serde(default, deserialize_with = "present_export_option")]
    pub ppi: Option<Option<f32>>,
    #[serde(default, deserialize_with = "present_export_option")]
    pub transparent: Option<Option<bool>>,
    #[serde(default, deserialize_with = "present_export_option")]
    pub quality: Option<Option<u8>>,
}
// Preserve both null-field presence and Serde's duplicate-field rejection.
fn present_export_option<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(d).map(Some)
}
impl<'de> Deserialize<'de> for FileEffect {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = ExportFileWire::deserialize(deserializer)?;
        if wire.api != "1.2" {
            for (key, present) in [
                ("options", wire.options.is_some()),
                ("format", wire.format.is_some()),
                ("scale", wire.scale.is_some()),
                ("ppi", wire.ppi.is_some()),
                ("transparent", wire.transparent.is_some()),
                ("quality", wire.quality.is_some()),
            ] {
                if present {
                    return Err(serde::de::Error::custom(format!("unknown field `{key}` for legacy file request")));
                }
            }
        }
        Ok(Self {
            options: wire.options,
            api: wire.api,
            request_id: wire.request_id,
            board: wire.board,
            expected_rev: wire.expected_rev,
            path: wire.path,
            scope: wire.scope,
            format: wire.format.flatten(),
            scale: wire.scale.flatten(),
            ppi: wire.ppi.flatten(),
            transparent: wire.transparent.flatten(),
            quality: wire.quality.flatten(),
        })
    }
}

impl Request {
    /// Wire tool name (for audit records; never carries arguments).
    pub fn tool(&self) -> &'static str {
        match self {
            Self::Help(_) => "help",
            Self::Preferences(_) => "preferences",
            Self::Shortcuts(_) => "shortcuts",
            Self::CommandIndex(_) => "command_index",
            Self::HistoryList(_) => "history_list",
            Self::HistoryJump(_) => "history_jump",
            Self::Actions(_) => "actions",
            Self::Schema(_) => "schema",
            Self::ListVerbs(_) => "list_verbs",
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
            Self::ImportSvg(_) => "import_svg",
            Self::ImportFile(_) => "import_file",
            Self::ImportClipboard(_) => "import_clipboard",
            Self::AddImage(_) => "add_image",
            Self::ImageAction(_) => "image_action",
            Self::ExportSvg(_) => "export_svg",
            Self::ExportRaster(_) => "export_raster",
            Self::SaveTemplate(_) => "save_template",
            Self::NewFromTemplate(_) => "new_from_template",
            Self::Print(_) => "print",
            Self::Copy(_) => "copy",
            Self::Cut(_) => "cut",
        }
    }
    pub fn api(&self) -> &str {
        match self {
            Self::Capabilities(v) | Self::WindowMemory(v) | Self::ListVerbs(v) => &v.api,
            Self::Help(v) => &v.api,
            Self::Preferences(v) => &v.api,
            Self::Shortcuts(v) => &v.api,
            Self::CommandIndex(v) => &v.api,
            Self::HistoryList(v) => &v.api,
            Self::HistoryJump(v) => &v.api,
            Self::Actions(v) => &v.api,
            Self::Schema(v) => &v.api,
            Self::ListBoards(v) => &v.api,
            Self::Describe(v) => &v.api,
            Self::Select(v) => &v.api,
            Self::Edit(v) => &v.api,
            Self::History(v) => &v.api,
            Self::RequestStatus(v) => &v.api,
            Self::Snapshot(v) => &v.api,
            Self::Save(v)
            | Self::SaveAs(v)
            | Self::ExportPdf(v)
            | Self::ExportSvg(v)
            | Self::ExportRaster(v)
            | Self::SaveTemplate(v)
            | Self::NewFromTemplate(v)
            | Self::Print(v)
            | Self::Copy(v)
            | Self::ImportClipboard(v)
            | Self::ImportFile(v)
            | Self::AddImage(v)
            | Self::ImageAction(v)
            | Self::ImportSvg(v)
            | Self::Cut(v) => &v.api,
        }
    }
    pub fn board(&self) -> Option<&str> {
        match self {
            Self::HistoryList(v) => Some(&v.board),
            Self::HistoryJump(v) => Some(&v.board),
            Self::Actions(v) => Some(&v.board),
            Self::Describe(v) => Some(&v.board),
            Self::Snapshot(v) => Some(&v.board),
            Self::Save(v)
            | Self::SaveAs(v)
            | Self::ExportPdf(v)
            | Self::ExportSvg(v)
            | Self::ExportRaster(v)
            | Self::SaveTemplate(v)
            | Self::NewFromTemplate(v)
            | Self::Print(v)
            | Self::Copy(v)
            | Self::ImportClipboard(v)
            | Self::ImportFile(v)
            | Self::AddImage(v)
            | Self::ImageAction(v)
            | Self::ImportSvg(v)
            | Self::Cut(v) => Some(&v.board),
            Self::Select(v) => Some(&v.board),
            Self::Edit(v) => Some(&v.board),
            Self::History(v) => Some(&v.board),
            _ => None,
        }
    }
    pub fn mutation(&self) -> Option<(&str, u64)> {
        match self {
            Self::HistoryJump(v) => Some((&v.request_id, v.expected_rev)),
            Self::Actions(v) => Some((&v.request_id, v.expected_rev)),
            Self::Select(v) => Some((&v.request_id, v.expected_rev)),
            Self::Edit(v) => Some((&v.request_id, v.expected_rev)),
            Self::History(v) => Some((&v.request_id, v.expected_rev)),
            Self::Save(v)
            | Self::SaveAs(v)
            | Self::ExportPdf(v)
            | Self::ExportSvg(v)
            | Self::ExportRaster(v)
            | Self::SaveTemplate(v)
            | Self::NewFromTemplate(v)
            | Self::Print(v)
            | Self::Copy(v)
            | Self::ImportClipboard(v)
            | Self::ImportFile(v)
            | Self::AddImage(v)
            | Self::ImageAction(v)
            | Self::ImportSvg(v)
            | Self::Cut(v) => Some((&v.request_id, v.expected_rev)),
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

fn optional_stroke_style<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<varos_core::stroke::StrokeStyle>, D::Error> {
    varos_core::stroke::StrokeStyle::deserialize(d).map(Some)
}

#[cfg(test)]
mod export_file_compat_tests {
    use super::*;
    #[test]
    fn legacy_file_decode_still_rejects_duplicate_fields() {
        let request = r#"{"tool":"save","arguments":{"api":"1.0","board":"b1","request_id":"r1","expected_rev":0,"path":"first","path":"second"}}"#;
        assert!(serde_json::from_str::<Request>(request).is_err());
    }
    #[test]
    fn legacy_file_requests_reject_every_raster_field_including_null() {
        for tool in ["save", "save_as", "export_pdf"] {
            for api in [None, Some("1.0"), Some("1.1")] {
                for key in ["format", "scale", "ppi", "transparent", "quality"] {
                    for value in [Value::Null, serde_json::json!(90)] {
                        let mut args = serde_json::json!({"board":"b1","request_id":"r","expected_rev":0});
                        if let Some(api) = api {
                            args["api"] = api.into();
                        }
                        args[key] = value;
                        assert!(crate::mcp::decode_tool(tool, args.clone()).is_err(), "{tool}: {args}");
                        assert!(serde_json::from_value::<Request>(serde_json::json!({"tool":tool,"arguments":args}))
                            .is_err());
                    }
                }
            }
        }
        assert!(crate::mcp::decode_tool(
            "export_raster",
            serde_json::json!({"api":"1.2","board":"b1","request_id":"r","expected_rev":0,"quality":90,"format":"jpeg"})
        )
        .is_ok());
    }
}
