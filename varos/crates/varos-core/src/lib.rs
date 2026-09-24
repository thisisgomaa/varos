//! varos-core — the pure Rust core (data model + modeless interaction + render-agnostic scene).
//! NO gpu/window/tauri deps. Everything below the "hard seam".

pub mod boolean;
pub mod clipboard;
pub mod command;
pub mod editor;
pub mod flatten;
pub mod format;
pub mod geom;
pub mod model;
pub mod scene;
pub mod tools;
pub mod units;

pub use boolean::BoolOp;
pub use command::EditCommand;
pub use editor::{AlignMode, DistAxis, Editor, Mods, ToolKind, ZOrder};
pub use geom::{Pt, Rgba, View};
pub use scene::{build_scene, build_scene_in_view, Group, Prim, Scene};
pub mod file;
pub use units::{DocUnits, Unit};
