# ADR-0001: Native GPU UI stack

- **Date:** 2026-07-11
- **Decision owner:** Product owner
- **Supersedes:** The Tauri, web-panel, and promised CPU-fallback architecture in `docs/VAROS_CONSTITUTION.md:17-20`
- **Superseded by:** None

## Context

The running application is a native Windows stack: `varos-app` directly depends on Winit, Egui, and Egui-Winit (`varos/crates/varos-app/Cargo.toml:15-23`), while `varos-render-wgpu` owns WGPU rendering (`varos/crates/varos-render-wgpu/Cargo.toml:8-13`). The app creates a native window and paints Egui on the renderer's WGPU surface (`varos/crates/varos-app/src/main.rs:3-5`, `varos/crates/varos-render-wgpu/src/lib.rs:925-934`). This replaced the constitution's Tauri/web-panel design and CPU-fallback promise. The foundation charter records the pivot and its limits (`docs/foundation/FOUNDATION_CHARTER.md:86`, `docs/foundation/FOUNDATION_CHARTER.md:121`).

GPU initialization already returns readable errors from the renderer (`varos/crates/varos-render-wgpu/src/lib.rs:204-234`) and presents them through the application's fatal dialog (`varos/crates/varos-app/src/main.rs:557-560`).

## Decision

Varos V1 uses the native Winit + Egui + WGPU stack. Tauri and web panels are not part of the product architecture. Historical web prototypes may remain as references, but production UI work targets the native stack.

GPU rendering is the V1 requirement. A CPU renderer is a possible future option, not a compatibility promise. When no compatible GPU can start, Varos must fail with a readable user-facing error rather than panic or imply that a CPU fallback exists.

## Consequences

- Current application and renderer boundaries remain the implementation direction; no web shell is maintained in parallel.
- Documentation that presents Tauri, web panels, or a CPU fallback as current law must be corrected or marked historical under F2a.2/F2a.3.
- Adding a CPU renderer later requires a separately scoped decision, implementation, and test strategy.
- GPU-startup error presentation remains part of the supported startup behavior.

## Status

Accepted — product owner, 2026-07-11.
