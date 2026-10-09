//! varos-core — the pure Rust core (data model + modeless interaction + render-agnostic scene).
//! NO gpu/window/tauri deps. Everything below the "hard seam".

pub mod board;
pub mod boolean;
pub mod clipboard;
mod clipping;
pub mod command;
mod construction;
pub mod guard;
pub mod images;
pub use guard::EngineError;
pub mod editor;
pub mod flatten;
pub mod format;
pub mod geom;
pub mod model;
pub mod planar;
pub mod scene;
// ---- Lane E ----
pub mod stroke;
pub mod svg;
pub mod tools;
// ---- Lane D: drawing tools ----
pub mod drawing;
pub mod units;
pub mod view_depth;
pub mod view_depth_scene;

pub use boolean::BoolOp;
pub use command::EditCommand;
pub use editor::{AlignMode, DistAxis, Editor, Mods, ToolKind, ZOrder};
pub use geom::{Pt, Rgba, View};
pub use scene::{build_scene, build_scene_for_export, build_scene_in_view, Group, Prim, Scene};
pub mod file;
pub use units::{DocUnits, Unit};

pub mod bridge;

pub mod export;
pub use export::{ExportNote, ExportReport};

pub mod document_setup;
pub mod placement;
pub mod select_transform;
pub mod trace;

// ---- Lane G ----
pub mod text;
pub mod text_format;
