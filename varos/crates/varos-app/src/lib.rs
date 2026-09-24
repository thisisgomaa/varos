//! `varos-app` library crate: the reusable UI shell.
//!
//! The `shell` module holds the box-system (split tree of panels) and is shared by the `varos`
//! application binary and the `shell-sandbox` binary. Ruling 9: `egui_tiles` is confined to
//! `shell::boxtree`; the rest of the app talks to our own small API, so a dead/lagging crate is a
//! one-module swap. These modules are context-agnostic (they take `&mut egui::Ui` / `&Context`).
pub mod shell;
/// Start page pure view model (DFS S2 piece E1) — no `egui`, no I/O.
pub mod start;
/// App-owned storage: data-root resolver, durable writer, checksums, time text (DFS S2/S3).
pub mod storage;
