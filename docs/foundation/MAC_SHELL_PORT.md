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
| `main.rs:573-583` → `cursors.rs` `hcursor`, `hcursor_svg_file` | Win32 HCURSOR from our SVGs | `hcursor` returns a non-zero **token** (index into `ALL_CURSORS` + 1); `hcursor_svg_file` returns that token only when the cached cursor really came from the local Illustrator SVG, else `None` (updated 2026-09-23, see "Tool cursors on macOS") |
| `main.rs:583, 1023` → `cursors.rs` `set` | set the HCURSOR on the window | token → cached winit `CustomCursor` via `window.set_cursor`; winit built-in `CursorIcon` only if that custom cursor could not be built |
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

## Tool cursors on macOS (winit `CustomCursor`) — 2026-09-23

**Reason:** Ahmed on his Mac: "the cursors are all missing" — the first port used generic winit
`CursorIcon`s (arrow / crosshair), so the pen nib and its + / − / ○ states never showed.

**Status (honest):** implemented and **startup-verified only** — the launch log shows the custom
cursors being built (below), and GPU-free tests pass. It has **NOT been hand-tested on screen yet**
(hovering can't be checked headlessly); that is pending Ahmed's batch-1 hand test on his Mac.

**How:** macOS now shows the **same bitmaps and hotspots as Windows** wherever a distinct bitmap exists.
- `cursors::cursor_rgba(ck, use_ai)` picks the bitmap for a `CK`: the local Illustrator SVG
  (`assets/cursors-ai/svg/<stem>.svg`, rendered by `svg_file_rgba`) when that file is present, else our
  own built-in SVG (`rgba(ck)`) — **but only if `has_builtin(ck)`**, an explicit exhaustive table of
  the 11 real glyphs (Select, Direct, the 6 Pen states, Convert, Cross, Eye). For the other 17 states
  (4 resize, Move, Hand, Grab, Copy, NoDrop, 8 rotate) the built-in `svg()` is only the arrow
  placeholder, so `cursor_rgba` returns `None` and macOS keeps the **system** `CursorIcon` for them —
  in a fresh clone (no cursors-ai) the resize / hand / no-drop cues never collapse into the plain
  arrow (review P2). `svg_file_rgba` is the rendering that used to live inside the Win32
  `hcursor_svg_file`; Windows now calls the same function (refactor by extraction — same pixels, same
  hotspots, same HCURSOR). The premultiplied → straight alpha loop is one helper.
- `cursors::create_custom_cursors(&event_loop)` (`#[cfg(not(windows))]`, called once in `main.rs` right
  after `bind_window`, before the `hcur` table) builds one `CustomCursor::from_rgba` →
  `EventLoop::create_custom_cursor` per `CK` that has a distinct bitmap, caches them, and logs one line:
  `[varos] cursors: N custom (M from cursors-ai, K built-in) + S system fallbacks of 28`
  (measured: with cursors-ai `28 custom (28 from cursors-ai, 0 built-in) + 0 system fallbacks`;
  without it `11 custom (0 from cursors-ai, 11 built-in) + 17 system fallbacks`). If winit rejects an
  Illustrator bitmap it retries with the built-in one; if there is none, `set` uses the system icon.
- The per-frame re-assert (`resolve_ck` / `cursor_apply_needed`) is unchanged. It is cheap: `set`
  clones a cached cursor (a reference-count bump) and winit's macOS `set_cursor` returns early when the
  view already shows that cursor (`winit-0.30.13/src/platform_impl/macos/window_delegate.rs:1106-1116`).

**Retina decision: 32 px bitmaps, hotspots in the 32-px-logical space (unchanged from Windows).**
Evidence: winit 0.30.13 builds the macOS cursor as `NSImage::initWithSize(NSSize::new(width, height))`
with a bitmap of the same pixel size (`src/platform_impl/macos/cursor.rs:36-60`) — the image size is
in **points**, and the hotspot is in points too. So a 32×32 bitmap is a 32×32-point cursor: the right
on-screen size on both Retina and non-Retina screens. Rendering at 64 px would make a double-size
cursor. Expected limit, **unverified** (not yet looked at on a Retina screen): macOS should scale the
32-px bitmap up 2×, so edges may look slightly softer than the native system cursors. Getting crisp @2x cursors needs an NSImage with a 64-px
representation at 32-pt size, which winit 0.30 does not expose (would need direct AppKit calls) —
a later piece if Ahmed finds it too soft.

**Illustrator set (local only):** `assets/cursors-ai/` is gitignored (`.gitignore` `assets/cursors-ai/`
and `**/cursors-ai/`) — proprietary, never committed, never shipped. Copy
`Cursors/tool-cursors-svg/*.svg` into `varos/crates/varos-app/assets/cursors-ai/svg/` on each
machine that should use it; without it the built-in (shippable) set is used. The 28 hotspots in
`ai_svg` match `Cursors/_hotspot_map.txt` (checked 2026-09-23).

**Tests (no GPU, no EventLoop):** every `CK`'s built-in bitmap is 32×32 RGBA, not blank, hotspot
inside, and accepted by `CustomCursor::from_rgba`; `has_builtin` equals the SVG-backed set and each of
those glyphs differs from the arrow placeholder; with `use_ai = false` every `CK` has a distinct
built-in bitmap OR a non-default system `CursorIcon` (none collapses to the arrow); every Illustrator
hotspot fits in `CURSOR_PX`; an Illustrator SVG that is absent is skipped, but one that is present
MUST render at 32×32 and be preferred, else the test fails naming the file.

**System cursor mapping** (states with no distinct bitmap, or one winit refused): Select/Direct → `Default` · Pen family,
Convert, Cross, Eye, Rotate* → `Crosshair` · ResizeH → `EwResize` · ResizeV → `NsResize` · ResizeNE →
`NeswResize` · ResizeNW → `NwseResize` · Move → `Move` · Hand → `Grab` · Grab → `Grabbing` · Copy →
`Copy` · NoDrop → `NotAllowed`.

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
2. ~~Tool cursors are generic~~ — **implemented 2026-09-23 and startup-verified, NOT yet hand-tested
   on screen** (pending Ahmed's batch-1; see "Tool cursors on macOS"). Expected but **unverified**:
   the cursors are 1× bitmaps that macOS scales up on Retina, so they may look slightly soft — nobody
   has looked at them on a Retina screen yet. In a fresh clone the 17 interaction / rotate states use
   system cursors, not Illustrator-style ones.
3. **Screen eyedropper disabled** (needs macOS Screen Recording permission + CoreGraphics capture).
4. **No single-instance / "open with" forwarding** and **no remembered window geometry**.
