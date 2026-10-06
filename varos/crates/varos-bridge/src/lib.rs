//! API 1.0 adapters. No window, GPU, provider or filesystem editing dependency.
pub mod cli;
mod design;
pub mod dto;
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
pub const TOOLS: &[&str] =
    &["capabilities", "list_boards", "describe", "select", "edit", "history", "request_status", "snapshot"];

pub const EDIT_VERBS: &[&str] = &[
    "move",
    "set_paint",
    "add_shape",
    "resize",
    "rotate",
    "rename",
    "delete",
    "align",
    "distribute",
    "group",
    "ungroup",
    "order",
];
