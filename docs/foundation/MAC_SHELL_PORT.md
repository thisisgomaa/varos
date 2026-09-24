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
| `main.rs:573-583` → `cursors.rs` `hcursor`, `hcursor_svg_file` | Win32 HCURSOR from our SVGs | **replaced 2026-09-23 (cursor set v1 wiring):** one `cursors::create_cursors` per platform builds all 28 cursors and returns the CK → handle/token table; `hcursor_svg_file` is gone (see "Tool cursors: Varos cursor set v1") |
| `main.rs:583, 1023` → `cursors.rs` `set` | set the HCURSOR on the window | token → cached Retina `NSCursor` via `NSCursor::set` (macOS); cached winit `CustomCursor` elsewhere or if AppKit refused one; winit built-in `CursorIcon` only if that failed too |
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

**Cursor ownership (review P2):** off Windows, egui-winit processes egui's cursor output every frame
(it deduplicates unchanged icons, so it does not write the OS cursor every frame, but it does write it
whenever egui's wanted icon changes — e.g. crossing a panel splitter), so `main.rs` re-asserts the
resolved tool cursor every frame *after* `gui.run` (pure `resolve_ck` + `cursor_apply_needed`,
unit-tested). Windows keeps "set only on change" (its subclass owns the cursor).

## Tool cursors: Varos cursor set v1, Retina representations on macOS — 2026-09-24

**Reason:** the Varos cursor set v1 (our own drawings, `docs/studies/2026-09-23-CURSOR_SET_V1.md`) was
approved and merged but not wired; the Mac showed 1× bitmaps that macOS scales up 2× (soft), and in a
fresh clone 17 states had no glyph of their own. This replaces the earlier "winit `CustomCursor`"
section, which described the placeholder / Illustrator-first behaviour.

**Status (honest):** implemented and test-verified; **startup verification is blocked** in this
execution environment. The release launch failed before cursor initialization with winit's
`invalid display ID` panic (details below). On-screen sharpness and cursor transitions have
**NOT yet been verified by a human**. No screenshots were taken during this verification.

**v1 is embedded and is the default everywhere.**
- `cursors.rs` compiles all 30 v1 SVGs (`include_str!`, `Move` reuses `select.svg`;
  zoom-in / zoom-out / artboard are embedded for proposed states with no CK yet) and
  `assets/cursors/v1/hotspots.json`. The JSON's `ck` map is parsed once (`v1_table`, `serde_json`)
  into one entry per `ALL_CURSORS` slot: CK, file, SVG text, hotspot in the 32-px space. A missing CK,
  an un-embedded file or a hotspot outside 0..32 is a parse error, and the tests pin all of it.
- The old placeholder glyphs (`svg()`, the Font Awesome nib, the Lucide pipette, the arrow placeholder)
  and `has_builtin` are deleted — every state has a real glyph. `THIRD_PARTY_NOTICES.md` records
  removal of the Font Awesome cursor and the macOS-only objc2 dependencies.
- One render path (`rasterize`: viewBox fitted to N px, premultiplied → straight alpha) serves every
  platform: 32 px = 1×, 64 px = 2×.
- **Windows:** `create_cursors()` builds the 28 HCURSORs from the 32-px RGBA with the unchanged
  `build_hcursor` (byte-identical Win32 code). A handle Win32 refuses stays 0 → WM_SETCURSOR keeps the
  OS arrow for that state (the only system fallback). Checked by the Windows-target clippy gate only —
  not run on a Windows machine in this piece.
- **Local Illustrator reference (dev A/B only):** off by default. `VAROS_CURSORS_AI=1` swaps in
  `assets/cursors-ai/svg/<stem>.svg` (gitignored, never committed or shipped) wherever that file exists;
  a missing file uses v1. Tests check opt-in selection and the absent-file fallback; any local
  reference files present must render successfully. Proprietary files are not copied into the repo.

**Verification, 2026-09-24:**
- `cargo test --workspace -j 4`: **276 passed, 0 failed, 0 ignored** (37 test-suite results).
- `cargo clippy --workspace --all-targets -j 4 -- -D warnings`: passed.
- `cargo fmt --all --check`: passed.
- `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc -j 4 -- -D warnings`: passed.
- `cargo build --release -p varos-app -j 4`: passed.
- `build_hcursor` and the `win` module match HEAD byte-for-byte. The proprietary
  `**/cursors-ai/` ignore rule still matches; `.gitignore` is unchanged.

**Startup attempt:** launched `target/release/varos` in the background with `VAROS_CURSORS_AI`
unset, captured stderr, and checked it after eight seconds. It had already exited with code 101.
macOS reported service connection errors; `target/panic.txt` records winit 0.30.13
`src/platform_impl/macos/monitor.rs:205:46: invalid display ID`. This happens before cursor creation,
so **no `[varos] cursors:` log line was emitted**. Evidence is local-only:
`/tmp/varos-cursor-startup.stderr.log` and `varos/target/panic.txt`.
The eight-second desktop launch and actual cursor log must still be verified in a desktop-capable
session. The unit tests do construct and inspect all 28 Retina NSCursors without an EventLoop or GPU.

**Retina approach (macOS).** winit 0.30.13's `CustomCursor` makes the `NSImage` `width × height`
POINTS from a bitmap of the same pixel count (`src/platform_impl/macos/cursor.rs:36-60`), so it can only
make 1× cursors. `cursors.rs` `mod mac` (`#[cfg(target_os = "macos")]`) builds the cursor itself with
`objc2-app-kit` 0.3.2 / `objc2-foundation` 0.3.2 / `objc2` 0.6.4 — the versions already in `Cargo.lock`
through `arboard`; added as direct macOS-only dependencies with only the features used:
- one `NSImage` of **32 × 32 points** carrying **two** `NSBitmapImageRep`s, 32 px (1×) and 64 px (2×),
  each rendered straight from the SVG and each given `setSize(32 pt)`; AppKit picks the rep that fits
  the screen (Retina → the 64-px one, a 1× external screen → the 32-px one);
- `NSCursor::initWithImage_hotSpot`, hotspot in points = the v1 hotspot;
- **no `unsafe`**: the bitmaps go in as PNG bytes → `NSBitmapImageRep::imageRepWithData`, all safe objc2
  calls. The NSCursors are `!Send`, so they live in a main-thread `thread_local!`.
- If AppKit refused one, that state falls back to the 32-px winit `CustomCursor`, then to a system
  `CursorIcon` (mapping below). The startup log reports which path was actually built.

**Cursor ownership on macOS (review fixes, 2026-09-24).** `main.rs` re-asserts the resolved cursor
following `gui.run` only when `native_cursor_apply_needed(pointer_inside, focused)` is true.
`CursorEntered` / `CursorLeft` track client-view containment; loss of focus relinquishes effective
ownership even without pointer movement. Physical containment is retained separately so focus gain
can restore the cursor immediately. The content view does not extend into the native title strip,
so winit's content-view tracking bounds exclude that strip. Startup does not set a macOS tool cursor
before entry. Outside or unfocused redraws make no cursor write; AppKit owns the cursor there.
On macOS `set` calls `NSCursor::set`.
winit keeps its own cursor in the view's cursor rect, and every `Window::set_cursor` invalidates that
rect, after which AppKit re-shows winit's cursor over ours. So on macOS `ui.rs` reads egui's wanted
cursor (for `chrome_ck`) and then hands egui-winit a constant `Default` icon — winit's cursor is written
once at startup by egui-winit; fallback `Window::set_cursor` writes can still invalidate the rect.
`ui.rs` honors egui-winit's `repaint` response on non-Windows platforms, and `main.rs` explicitly
requests redraws on `CursorEntered` and `Focused(true)` on macOS. The next eligible redraw restores
the tool cursor even if its CK has not changed. Pure tests cover all containment/focus combinations
and exit/re-entry/deactivation/reactivation with an unchanged tool. Windows event and cursor behavior
is unchanged under `#[cfg]`. Entry/focus transitions and on-screen sharpness remain unverified.

**Tests (no GPU, no EventLoop, no window):** the v1 table covers every `CK` exactly once, in
`ALL_CURSORS` order, hotspots inside 32 × 32, all 30 files embedded; hotspots equal the JSON `files`
map; bad JSON (missing CK / unknown file / hotspot 32) is rejected with a message; every v1 bitmap is
32 × 32 and 64 × 64, non-blank, hotspot inside and on/next to ink, accepted by winit's
`CustomCursor::from_rgba`; distinct files → distinct bitmaps at both sizes (all 30 files), `Move` == `Select`; v1 is the
default and the reference override is opt-in (present file → used, absent → v1); macOS: every CK's
NSCursor image is 32 × 32 pt with exactly a 32-px and a 64-px rep, both 32 pt, hotspot in points.

**System cursor mapping** (only for a state whose custom cursor the OS refused): Select/Direct →
`Default` · Pen family, Convert, Cross, Eye, Rotate* → `Crosshair` · ResizeH → `EwResize` · ResizeV →
`NsResize` · ResizeNE → `NeswResize` · ResizeNW → `NwseResize` · Move → `Move` · Hand → `Grab` · Grab →
`Grabbing` · Copy → `Copy` · NoDrop → `NotAllowed`.

## Three renderer bugs the macOS launch and review exposed (fixed in `varos-render-wgpu`, not Mac-only)

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
2. **Tool cursors:** Varos cursor set v1 and 1×/2× Retina representations implemented,
   2026-09-24 (see "Tool cursors: Varos cursor set v1"). On-screen sharpness and all cursor
   states still await Ahmed's hand test.
3. **Screen eyedropper disabled** (needs macOS Screen Recording permission + CoreGraphics capture).
4. **No single-instance / "open with" forwarding** and **no remembered window geometry**.

## Run on macOS (Varos.app bundle, added 2026-09-23)

**Reason:** Ahmed opens Varos like any Mac app (Spotlight / Launchpad / Finder), not from a terminal.

- **Build + install:** from the repo root run `tools/mac/bundle.sh`. It runs
  `cargo build --release -p varos-app` (a no-op when the build is fresh; `SKIP_BUILD=1` skips it),
  assembles `varos/target/mac/Varos.app` (ignored via `target/`), turns `icon.png` into `Varos.icns`,
  writes `Info.plist` (bundle id `com.varos.editor`, version from `varos-app`'s `Cargo.toml`,
  macOS 13+, Retina on, `.vrs` document type), ad-hoc signs it, and copies it to
  `/Applications/Varos.app` (or `~/Applications/Varos.app` when `/Applications` is not writable).
  No sudo. Re-run it after every pull to replace the installed copy.
- **Not notarized.** The bundle is **ad-hoc signed for local use on this Mac only** — not for
  sharing. A copy built here carries no quarantine flag, so Gatekeeper lets it open normally
  (`spctl --assess` still says "rejected", which is expected for ad-hoc). If a copy ever arrives
  from another Mac/download and macOS blocks it, right-click → **Open** once, or allow it in
  System Settings → Privacy & Security.
- **`.vrs` double-click:** Finder now lists Varos as the `.vrs` app and launches it, but the file is
  **not loaded yet** — macOS hands files over as an "open document" event, not a command-line
  argument, and the app does not handle that event (gap 4 above).
- **Icon:** `icon.png` is 279×279, so the 512/1024 icon sizes are upscaled and slightly soft; a
  1024×1024 master would fix that.
