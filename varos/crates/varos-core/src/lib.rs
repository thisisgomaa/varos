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
// ---- Lane C ----
pub mod live_corners;
pub mod new_document;
pub mod path_advanced;
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
// ---- Lane B: appearance and gradient mathematics ----
pub mod appearance;
mod current_paint;
pub mod gradient;
mod gradient_canvas;

// ---- w2-gradients ----
pub mod colour_commands;
pub mod colour_guide;
pub mod palette_io;
pub mod recolor;
pub mod swatches;

pub mod colour_lab;

mod gradient_scene;
mod gradient_transform;
// ---- Lane F ----
pub mod actions;
mod command_labels;
pub mod registry;

// ---- Lane B w3-effects ----
pub mod effects;
mod effects_warp;
pub mod width_profile;
// ---- end Lane B w3-effects ----

// ---- Lane B w3-effects ----
pub mod width_geometry;
mod width_geometry_helpers;
// ---- end Lane B w3-effects ----

// ---- Lane B w3-effects ----
pub mod effects_preview;
pub mod width_tool;
// ---- end Lane B w3-effects ----

// ---- Lane B w3-effects ----
pub mod effects_document;
pub mod effects_hit;
// ---- end Lane B w3-effects ----
