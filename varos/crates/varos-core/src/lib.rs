//! varos-core — the pure Rust core (data model + modeless interaction + render-agnostic scene).
//! NO gpu/window/tauri deps. Everything below the "hard seam".

pub mod board;
pub mod boolean;
pub mod clipboard;
mod clipping;
pub mod command;
mod construction;
pub mod guard;
pub use guard::EngineError;
pub mod editor;
pub mod flatten;
pub mod format;
pub mod geom;
pub mod model;
pub mod planar;
pub mod scene;
pub mod stroke;
pub mod svg;
pub mod tools;
pub mod units;

pub use boolean::BoolOp;
pub use command::EditCommand;
pub use editor::{AlignMode, DistAxis, Editor, Mods, ToolKind, ZOrder};
pub use geom::{Pt, Rgba, View};
pub use scene::{build_scene, build_scene_in_view, Group, Prim, Scene};
pub mod file;
pub use units::{DocUnits, Unit};

pub mod bridge;

pub mod export;
pub use export::{ExportNote, ExportReport};

pub mod document_setup;
pub mod placement;
pub mod select_transform;
pub mod trace;

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
