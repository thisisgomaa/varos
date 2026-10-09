//! API 1.0/1.1 adapters. No window, GPU, provider or filesystem editing dependency.
pub mod cli;
pub mod conn;
mod design;
pub mod dto;
mod economy;
pub mod files;
pub mod ipc;
pub mod mcp;
pub mod service;
pub use dto::{Error, Reply, Request};
pub use service::{BoardAccess, BoardInfo, Context, Host, Service};
pub const API: &str = "1.0";
pub const MCP_VERSION: &str = "2025-06-18";
pub const MAX_FRAME: usize = 1_048_576;
pub const MAX_OPS: usize = 100;
pub const MAX_TARGETS: usize = 1000;
pub const MAX_PAGE: usize = 100;
pub const MAX_TEXT: usize = 16_384;
pub const TOOLS: &[&str] = &[
    "capabilities",
    "list_boards",
    "describe",
    "select",
    "edit",
    "history",
    "request_status",
    "snapshot",
    "save",
    "save_as",
    "export_pdf",
];

pub const EDIT_VERBS: &[&str] = &[
    "move",
    "set_paint",
    "add_shape",
    "add_path",
    "reorder_artboard",
    "duplicate_artboard",
    "set_artboard_color",
    "set_artboard_clip",
    "resize",
    "rotate",
    "rename",
    "delete",
    "align",
    "distribute",
    "group",
    "ungroup",
    "order",
    "add_artboard",
    "resize_artboard",
    "rename_artboard",
    "delete_artboard",
    "set_active_artboard",
];

/// Opt-in API 1.2 operations; the legacy edit verb catalogue above stays frozen.
pub const CONSTRUCTION_VERBS: &[&str] =
    &["pathfinder", "shape_builder", "scissors", "knife", "eraser", "divide_objects_below"];

/// Expanded, ordered leaf operations of an edit (API 1.1 defaults/tuples/repeat applied; API 1.0 as-is).
/// Used by the host's agent-presence overlay, which needs the creation order; never fails — an
/// invalid batch yields the 1.0 ops that parse, and an unparsable batch yields nothing.
pub fn expanded_ops(edit: &dto::Edit) -> Vec<dto::Operation> {
    match economy::expand(edit) {
        Ok(leaves) => leaves.into_iter().map(|l| l.op).collect(),
        Err(_) => edit.ops.iter().filter_map(|v| serde_json::from_value(v.clone()).ok()).collect(),
    }
}

pub mod images;
mod select_transform;
// ---- Lane D ----
mod drawing;
// ---- Lane C ----
mod path_advanced;
pub mod storage_paths;

pub mod templates;
/// New host effects are explicitly opt-in; legacy capabilities retain their tool list.
pub const TOOLS_12: &[&str] = &["print", "copy", "cut"];

// ---- Lane E ----
mod view_depth;
// ---- Lane G ----
mod text;
// ---- w2-gradients ----
mod colour;
// ---- Lane F ----
pub mod application;

// ---- Lane F ----
mod action_recording;
