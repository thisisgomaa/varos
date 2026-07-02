//! varos-core — the pure Rust core (data model + modeless interaction + render-agnostic scene).
//! NO gpu/window/tauri deps. Everything below the "hard seam".

pub mod geom;
pub mod model;
pub mod units;
pub mod editor;
pub mod tools;
pub mod scene;
pub mod boolean;

pub use boolean::BoolOp;
pub use editor::{AlignMode, DistAxis, Editor, Mods, ToolKind, ZOrder};
pub use geom::{Pt, Rgba, View};
pub use scene::{build_scene, Group, Prim, Scene};
pub mod file;
pub use units::{DocUnits, Unit};
