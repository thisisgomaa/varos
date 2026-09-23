> **Status:** current — macOS build/launch port design for `varos-app`, governed by `docs/foundation/FOUNDATION_CHARTER.md` §3.
# macOS Shell Port (build + launch)

**Date:** 2026-09-23
**Reason:** Ahmed now works on a Mac and must hand-test every stage in the real window. `varos-core`,
`varos-pdf` and `varos-render-wgpu` already build and test green on macOS (Metal); only `varos-app`
is Windows-bound. This piece makes it build and launch on macOS with the **Win32 shell behaviour
unchanged** (every Win32 call byte-identical behind `#[cfg(windows)]`) — **except** the two shared
renderer fixes below (texture-size limit + egui texture upload/free ordering), which also run on Windows.

## Rule for this piece

- Every Win32 call stays exactly as it is, behind `#[cfg(windows)]`.
- Every Windows-only function gets a `#[cfg(not(windows))]` twin with the **same signature**, so the
  call sites in `main.rs` / `ui.rs` do not change shape.
- A fallback may be simpler, but it must never crash and never pretend a feature works.
- `varos-core` is not touched. `ui.rs` / `editor.rs` are not restructured.

## Windows-only sites found (file:line at commit 61786a0) and the macOS fallback

| Site | What it is on Windows | macOS fallback |
|---|---|---|
| `varos-app/Cargo.toml:20` | `windows` crate, unconditional | moved to `[target.'cfg(windows)'.dependencies]` |
| `main.rs:18-19` | `WindowAttributesExtWindows` import | already `#[cfg(windows)]` — unchanged |
| `main.rs:518-521` | `with_class_name` (single-instance lookup) | already `#[cfg(windows)]` — unchanged |
| `main.rs:549-555` | HWND from `RawWindowHandle::Win32` | already portable: non-Win32 handle yields `0` |
| `main.rs:557, 652` → `cursors.rs:596` `set_cloaked` | DWM cloak to hide the startup flash | no-op (window may show one plain frame at start) |
| `main.rs:571` → `cursors.rs:644` `install` | subclass wndproc (WM_SETCURSOR, custom caption, hit-test) | returns `false` (nothing installed) |
| `main.rs:572` → `cursors.rs:607` `set_dark_class_brush` | dark class background brush | no-op |
| `main.rs:573-583` → `cursors.rs:265, 273` `hcursor`, `hcursor_svg_file` | Win32 HCURSOR from our SVGs | `hcursor` returns a non-zero **token** (index into `ALL_CURSORS` + 1); `hcursor_svg_file` returns `None` |
| `main.rs:583, 1023` → `cursors.rs:650` `set` | set the HCURSOR on the window | token → `CK` → winit built-in `CursorIcon` via `window.set_cursor` |
| `main.rs:1025` → `cursors.rs:659` `dbg` | cursor debug counters | same tuple, HWND/hits = 0 |
| `main.rs:644, 687, 695, 712, 960, 996, 999` → `cursors.rs:574` `is_maximized` | `GetWindowPlacement` | winit `Window::is_maximized()` |
| `main.rs:1060` → `cursors.rs:542` `custom_frame` | strip caption, DWM round corners | no-op — the **native macOS title bar stays** (see gaps) |
| `main.rs:1063` → `cursors.rs:585` `maximize` | `ShowWindow(SW_MAXIMIZE)` | winit `Window::set_maximized(true)` |
| `ui.rs:3363` → `cursors.rs:460` `set_caption` | caption drag band for WM_NCHITTEST | no-op (the native title bar drags the window) |
| `ui.rs:2707-2710` → `cursors.rs:372, 404` | system-wide eyedropper (`GetPixel`, `GetAsyncKeyState`) | fallbacks already existed (`None` / `false`) but the button still armed a picker that could never pick. Now the button is **shown disabled** with the tooltip "Screen eyedropper is Windows-only for now" and cannot arm. |
| `single_instance.rs:39-249` | mutex + WM_COPYDATA file forwarding | fallbacks already existed: always "first instance", no forwarding (each launch is its own window) |
| `main.rs:1` | `#![windows_subsystem = "windows"]` (no console window) | attribute is ignored off Windows — nothing to do |
| `ui.rs:1392` (`install_fonts`) | Segoe UI / Cascadia from `C:/Windows/Fonts` | path absent → silently falls back to egui's bundled fonts. Mac font lookup deliberately NOT done now |
| `main.rs:359-365` | `%APPDATA%\Varos\window.txt` / `crash.txt` | unchanged: no `APPDATA` on macOS → window size/position not remembered, no crash file (no crash either) |

The winit window is handed to the macOS cursor module once, right after it is created
(`cursors::bind_window`, `#[cfg(not(windows))]` only), so `set` / `is_maximized` / `maximize` can
reach it without changing their Windows signatures.

**Cursor ownership (review P2):** off Windows, egui-winit also writes the OS cursor each frame, so
`main.rs` re-asserts the resolved tool cursor every frame *after* `gui.run` (pure `resolve_ck` +
`cursor_apply_needed`, unit-tested). Windows keeps "set only on change" (its subclass owns the cursor).

## Cursor mapping (macOS, winit `CursorIcon`)

Select/Direct → `Default` · Pen family, Convert, Cross → `Crosshair` · Eye → `Crosshair` ·
ResizeH → `EwResize` · ResizeV → `NsResize` · ResizeNE → `NeswResize` · ResizeNW → `NwseResize` ·
Move → `Move` · Hand → `Grab` · Grab → `Grabbing` · Copy → `Copy` · NoDrop → `NotAllowed` ·
Rotate* → `Crosshair`. Honest limit: macOS does not show our pen-nib / add / delete glyphs yet.

## Two renderer bugs the macOS launch exposed (fixed in `varos-render-wgpu`, not Mac-only)

1. `Renderer::new` requested `Limits::downlevel_defaults()` (2048px texture cap). A Retina window
   (2920×1720 physical) panicked in `Surface::configure`. Fix: `.using_resolution(adapter.limits())`
   — the adapter's real cap. Same latent crash on Windows for any window wider than 2048 physical px.
2. egui texture uploads ran *after* acquiring the frame; when the OS gave no frame (occluded /
   timeout) the early return dropped the font-atlas upload and the next frame panicked in egui-wgpu
   ("texture that has not been allocated yet"). Fix: upload textures before acquiring the frame; the
   skipped frame's egui *frees* are parked (`FreeQueue`) and released after the next real submit.
3. (review) No upper clamp: startup now returns a readable `Err` if the window exceeds the device's
   texture cap; `resize` clamps the surface to the cap (logged once) instead of panicking.

## Known gaps on macOS (deliberate, for later pieces)

1. **Two title bars:** the native macOS bar plus our own egui title bar under it. Merging them
   (transparent full-size content view + traffic lights) is a separate design piece.
2. **Tool cursors are generic** (table above). Upgrade path: winit `CustomCursor::from_rgba` from the
   same SVG bitmaps, with Retina scaling.
3. **Screen eyedropper disabled** (needs macOS Screen Recording permission + CoreGraphics capture).
4. **No single-instance / "open with" forwarding** and **no remembered window geometry**.
