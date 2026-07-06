//! The Varos UI shell — a box-system (a split tree of panels) rendered with egui.
//! egui_tiles is wrapped entirely inside `boxtree` (ruling 9); everything else is plain egui,
//! so these modules are context-agnostic: the `shell-sandbox` bin (eframe) and later the real
//! app (custom wgpu boot) both just call [`ShellState::ui`].
pub mod boxtree;
pub mod registry;
pub mod tokens;

pub use boxtree::ShellState;
pub use registry::PanelId;
