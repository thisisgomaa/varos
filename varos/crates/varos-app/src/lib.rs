//! `varos-app` library crate: the reusable UI shell.
//!
//! The `shell` module holds the box-system (split tree of panels) and is shared by the `varos`
//! application binary and the `shell-sandbox` binary. Ruling 9: `egui_tiles` is confined to
//! `shell::boxtree`; the rest of the app talks to our own small API, so a dead/lagging crate is a
//! one-module swap. These modules are context-agnostic (they take `&mut egui::Ui` / `&Context`).
/// The editor's recovery card and its Review panel (owner decision 2026-10-06, direction B).
pub mod recovery_card;
pub mod shell;
/// Start page pure view model (DFS S2 piece E1) — no `egui`, no I/O.
pub mod start;
/// Start v2 — Boards: THE Start page, drawn from `start::StartModel`.
pub mod start_page;
/// App-owned storage: data-root resolver, durable writer, checksums, time text (DFS S2/S3).
pub mod storage;

// ---- Lane F ----
pub mod i18n;
// ---- end Lane F ----
