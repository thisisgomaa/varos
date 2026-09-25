//! Native tool cursors: the **Varos cursor set v1.1** (our own drawings, `assets/cursors/v1/`, see
//! docs/studies/2026-09-23-CURSOR_SET_V1.md §9), compiled into the binary. Every `CK` state has a real
//! glyph. Each SVG is rendered to straight-alpha RGBA (resvg) and turned into the platform cursor:
//! a Win32 HCURSOR on Windows, a Retina NSCursor (1× + 2× bitmaps in one 32-pt image) on macOS, a
//! winit `CustomCursor` elsewhere. A system cursor is used only if the OS refuses one of ours.
//!
//! Dev-only A/B switch: `VAROS_CURSORS_AI=1` swaps in the LOCAL Illustrator reference set
//! (`assets/cursors-ai/svg/`, gitignored, never shipped) wherever those files are present.

use resvg::{tiny_skia, usvg};
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CK {
    // ---- tool cursors ----
    Select,
    Direct,
    Pen,
    PenNew,
    PenAdd,
    PenDel,
    PenClose,
    PenConnect,
    Convert,
    Eye,
    // ---- interaction states ----
    ResizeH,
    ResizeV,
    ResizeNE,
    ResizeNW,
    Move,
    Hand,
    Grab,
    Copy,
    // ---- rotate cursors, 8 compass directions (corner of the transform frame) ----
    RotateE,
    RotateSE,
    RotateS,
    RotateSW,
    RotateW,
    RotateNW,
    RotateN,
    RotateNE,
    // ---- v1.1 (2026-09-25): Artboard tool + Illustrator-style hover badges ----
    /// Artboard tool over empty board / creating a page: crosshair + frame badge.
    Artboard,
    /// Selection tool over a selectable object: arrow + filled square.
    SelectObject,
    /// Direct Selection over an anchor: hollow arrow + hollow square.
    DirectAnchor,
    /// Direct Selection over a path (segment or fill): hollow arrow + filled square.
    DirectPath,
    // ---- crosshair + per-tool badge (owner decision 2026-09-25: "one icon per tool") ----
    CrossRect,
    CrossEllipse,
    CrossTriangle,
    CrossPolygon,
    CrossRotate,
    CrossScale,
}

/// Every cursor state, in slot order (the non-Windows cursor token is `slot + 1`).
pub const ALL_CURSORS: [CK; 36] = [
    CK::Select,
    CK::Direct,
    CK::Pen,
    CK::PenNew,
    CK::PenAdd,
    CK::PenDel,
    CK::PenClose,
    CK::PenConnect,
    CK::Convert,
    CK::Eye,
    CK::ResizeH,
    CK::ResizeV,
    CK::ResizeNE,
    CK::ResizeNW,
    CK::Move,
    CK::Hand,
    CK::Grab,
    CK::Copy,
    CK::RotateE,
    CK::RotateSE,
    CK::RotateS,
    CK::RotateSW,
    CK::RotateW,
    CK::RotateNW,
    CK::RotateN,
    CK::RotateNE,
    CK::Artboard,
    CK::SelectObject,
    CK::DirectAnchor,
    CK::DirectPath,
    CK::CrossRect,
    CK::CrossEllipse,
    CK::CrossTriangle,
    CK::CrossPolygon,
    CK::CrossRotate,
    CK::CrossScale,
];

/// A CK's slot in `ALL_CURSORS`. Total: every CK is in the table (unit-tested).
fn slot(ck: CK) -> usize {
    ALL_CURSORS.iter().position(|c| *c == ck).expect("every CK has a cursor slot")
}

// ───────────────────────────── Varos cursor set v1 (embedded) ─────────────────────────────

/// Logical cursor size: the v1 SVGs are drawn on a 32×32 grid and hotspots are in that space.
/// On macOS this is the NSImage size in POINTS; on Windows / winit it is the bitmap size in pixels.
pub const CURSOR_PT: u32 = 32;
/// The 2× (Retina) render size used for the macOS NSCursor's high-resolution representation.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))] // macOS runtime + tests
pub const CURSOR_PX_2X: u32 = CURSOR_PT * 2;

/// `hotspots.json` from the v1 set: the `ck` map (every CK name → file + hotspot in 32-px space).
const V1_HOTSPOTS_JSON: &str = include_str!("../assets/cursors/v1/hotspots.json");

macro_rules! v1_files {
    ($($f:literal),* $(,)?) => { [$(($f, include_str!(concat!("../assets/cursors/v1/", $f)))),*] };
}
/// All 38 v1.1 files, including the three proposed cursor states (zoom-in, zoom-out, no-drop: drawn,
/// no CK yet). The 36 current CK states use 35 files (`Move` reuses `select.svg`).
const V1_FILES: [(&str, &str); 38] = v1_files!(
    "select.svg",
    "direct.svg",
    "pen.svg",
    "pen-new.svg",
    "pen-add.svg",
    "pen-delete.svg",
    "pen-close.svg",
    "pen-connect.svg",
    "convert.svg",
    "eyedropper.svg",
    "resize-h.svg",
    "resize-v.svg",
    "resize-ne.svg",
    "resize-nw.svg",
    "hand.svg",
    "grab.svg",
    "copy.svg",
    "no-drop.svg",
    "rotate-e.svg",
    "rotate-se.svg",
    "rotate-s.svg",
    "rotate-sw.svg",
    "rotate-w.svg",
    "rotate-nw.svg",
    "rotate-n.svg",
    "rotate-ne.svg",
    "zoom-in.svg",
    "zoom-out.svg",
    "artboard.svg",
    "select-object.svg",
    "direct-anchor.svg",
    "direct-path.svg",
    "cross-rect.svg",
    "cross-ellipse.svg",
    "cross-triangle.svg",
    "cross-polygon.svg",
    "cross-rotate.svg",
    "cross-scale.svg",
);

/// One v1 cursor: the state, its file, the embedded SVG text, and the hotspot in 32-px space.
#[derive(Debug)]
pub struct V1Cursor {
    pub ck: CK,
    pub file: &'static str,
    pub svg: &'static str,
    pub hx: u16,
    pub hy: u16,
}

/// Parse `hotspots.json` → one entry per `ALL_CURSORS` slot (same order). Pure; fails with a message
/// if a CK is missing, names a file that is not embedded, or has a hotspot outside the 32×32 grid.
fn parse_v1(json: &str) -> Result<Vec<V1Cursor>, String> {
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("hotspots.json: {e}"))?;
    let map = v.get("ck").and_then(|m| m.as_object()).ok_or("hotspots.json: no \"ck\" map")?;
    ALL_CURSORS
        .iter()
        .map(|&ck| {
            let name = format!("{ck:?}");
            let e = map.get(&name).ok_or_else(|| format!("hotspots.json: CK {name} missing"))?;
            let file = e.get("file").and_then(|f| f.as_str()).ok_or_else(|| format!("{name}: no file"))?;
            let &(file, svg) =
                V1_FILES.iter().find(|(f, _)| *f == file).ok_or_else(|| format!("{name}: {file} is not embedded"))?;
            let hs = e.get("hotspot").and_then(|h| h.as_array()).ok_or_else(|| format!("{name}: no hotspot"))?;
            let coord = |i: usize| {
                hs.get(i)
                    .and_then(|n| n.as_u64())
                    .filter(|&n| n < CURSOR_PT as u64)
                    .map(|n| n as u16)
                    .ok_or_else(|| format!("{name}: hotspot[{i}] missing or outside 0..{CURSOR_PT}"))
            };
            Ok(V1Cursor { ck, file, svg, hx: coord(0)?, hy: coord(1)? })
        })
        .collect()
}

/// The v1 table, parsed once from the embedded `hotspots.json` (36 entries, `ALL_CURSORS` order).
/// The data is compiled in and fully checked by `v1_table_covers_every_ck_once_with_hotspots_inside`,
/// so the `expect` is an invariant of the build, not a runtime condition.
pub fn v1_table() -> &'static [V1Cursor] {
    static T: OnceLock<Vec<V1Cursor>> = OnceLock::new();
    T.get_or_init(|| parse_v1(V1_HOTSPOTS_JSON).expect("embedded cursors/v1/hotspots.json is valid"))
}

/// The v1 entry for a CK.
pub fn v1(ck: CK) -> &'static V1Cursor {
    &v1_table()[slot(ck)]
}

/// A cursor bitmap: (straight-alpha RGBA, width, height, hotspot_x, hotspot_y) in bitmap pixels.
pub type CursorBitmap = (Vec<u8>, u16, u16, u16, u16);

/// tiny-skia renders premultiplied RGBA; every cursor consumer (the Win32 DIB builder, winit
/// `CustomCursor`, the macOS PNG → NSBitmapImageRep path) takes straight alpha. In place.
fn unpremultiply(d: &mut [u8]) {
    for px in d.chunks_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            px[0] = ((px[0] as u32 * 255) / a) as u8;
            px[1] = ((px[1] as u32 * 255) / a) as u8;
            px[2] = ((px[2] as u32 * 255) / a) as u8;
        }
    }
}

/// Render a cursor SVG into a `px`×`px` straight-alpha RGBA bitmap, its viewBox fitted to the square
/// (32 for v1, 64 for the @2x Illustrator reference files). None if the SVG does not parse.
fn rasterize(svg: &str, px: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let vb = tree.size().width().max(tree.size().height()).max(1.0);
    let scale = px as f32 / vb;
    let mut pm = tiny_skia::Pixmap::new(px, px)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    let mut d = pm.data().to_vec();
    unpremultiply(&mut d);
    Some(d)
}

/// Scale a 32-px-space hotspot coordinate to a `px` bitmap.
fn hotspot_px(h: f32, px: u32) -> u16 {
    (h * px as f32 / CURSOR_PT as f32).round() as u16
}

/// A CK's v1 bitmap rendered at `px` (32 = 1×, 64 = 2×), hotspot scaled to that bitmap.
pub fn v1_rgba(ck: CK, px: u32) -> Option<CursorBitmap> {
    let e = v1(ck);
    let d = rasterize(e.svg, px)?;
    let p = px as u16;
    Some((d, p, p, hotspot_px(e.hx as f32, px), hotspot_px(e.hy as f32, px)))
}

// ───────────────────── local Illustrator reference set (dev A/B only, never shipped) ─────────────────────

/// Env var that turns the local Illustrator reference override on (`=1`). Off by default.
pub const AI_ENV: &str = "VAROS_CURSORS_AI";
/// Where the local reference SVGs live (gitignored — never committed or shipped, see .gitignore).
const AI_SVG_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/cursors-ai/svg/");

/// Is the dev-only reference override switched on (`VAROS_CURSORS_AI=1`)?
pub fn ai_override_enabled() -> bool {
    std::env::var(AI_ENV).is_ok_and(|v| v == "1")
}

/// The Illustrator reference cursor for a CK: (SVG filename stem, hotspot_x, hotspot_y). Hotspots are
/// in the 32-px-logical space (the files are @2x, viewBox 64). Local A/B reference only.
pub fn ai_svg(ck: CK) -> (&'static str, f32, f32) {
    match ck {
        CK::Select => ("CUR_SELECT", 1.0, 1.0),
        CK::Direct => ("CUR_DIRECTSELECT", 1.0, 1.0),
        CK::Pen => ("CUR_PEN", 1.0, 1.0),
        CK::PenNew => ("CUR_PENNEW", 1.0, 1.0),
        CK::PenAdd => ("CUR_PENADD", 1.0, 1.0),
        CK::PenDel => ("CUR_PENSUBSTRACT", 1.0, 1.0),
        CK::PenClose => ("CUR_PENCLOSE", 1.0, 1.0),
        CK::PenConnect => ("CUR_PENCONTINUE", 1.0, 1.0),
        CK::Convert => ("CUR_PENCORNER", 1.0, 1.0),
        CK::Eye => ("CUR_EYEDROPPER", 2.0, 17.0),
        CK::ResizeH => ("CUR_SCALEHORIZONTAL", 9.0, 9.0),
        CK::ResizeV => ("CUR_SCALEVERTICAL", 9.0, 9.0),
        CK::ResizeNW => ("CUR_SCALETLBR", 9.0, 9.0), // ↖↘
        CK::ResizeNE => ("CUR_SCALETRBL", 9.0, 9.0), // ↗↙
        CK::Move => ("CUR_MOVE", 1.0, 1.0),
        CK::Hand => ("CUR_HAND", 11.0, 11.0),
        CK::Grab => ("CUR_FIST", 11.0, 11.0),
        CK::Copy => ("CUR_MOVECOPY", 1.0, 1.0),
        CK::RotateE => ("CUR_ROTATEFROMRIGHT", 7.0, 7.0),
        CK::RotateSE => ("CUR_ROTATEBOTTOMRIGHTCORNER", 7.0, 7.0),
        CK::RotateS => ("CUR_ROTATEFROMBOTTOM", 7.0, 7.0),
        CK::RotateSW => ("CUR_ROTATEBOTTOMLEFTCORNER", 7.0, 7.0),
        CK::RotateW => ("CUR_ROTATEFROMLEFT", 7.0, 7.0),
        CK::RotateNW => ("CUR_ROTATETOPLEFTCORNER", 7.0, 7.0),
        CK::RotateN => ("CUR_ROTATEFROMTOP", 7.0, 7.0),
        CK::RotateNE => ("CUR_ROTATETOPRIGHTCORNER", 7.0, 7.0),
        CK::Artboard => ("CUR_ARTBOARD", 8.0, 8.0),
        CK::SelectObject => ("CUR_RESELECT", 1.0, 1.0),
        CK::DirectAnchor => ("CUR_DIRECTSELECTANCHOR", 1.0, 1.0),
        CK::DirectPath => ("CUR_DIRECTSELECTBBOX", 1.0, 1.0),
        // dev A/B only: the reference set's plain crosshair for every badge variant
        CK::CrossRect | CK::CrossEllipse | CK::CrossTriangle | CK::CrossPolygon | CK::CrossRotate | CK::CrossScale => {
            ("CUR_CROSSHAIRS", 12.0, 12.0)
        }
    }
}

/// The local reference bitmap for a CK at `px`, or None when that file is absent / unreadable.
fn ai_rgba(ck: CK, px: u32) -> Option<CursorBitmap> {
    let (stem, hx, hy) = ai_svg(ck);
    let svg = std::fs::read_to_string(format!("{AI_SVG_DIR}{stem}.svg")).ok()?;
    let d = rasterize(&svg, px)?;
    let p = px as u16;
    Some((d, p, p, hotspot_px(hx, px), hotspot_px(hy, px)))
}

/// The bitmap a CK's cursor is built from at `px`: the local reference file when `use_ai` and it is
/// present (`.1 == true`), else the v1 glyph. None only if even the embedded v1 SVG failed to render
/// (never, per the tests) — the caller then keeps the system cursor.
pub fn cursor_rgba(ck: CK, use_ai: bool, px: u32) -> Option<(CursorBitmap, bool)> {
    if use_ai {
        if let Some(b) = ai_rgba(ck, px) {
            return Some((b, true));
        }
    }
    v1_rgba(ck, px).map(|b| (b, false))
}

/// The startup log line every platform prints (a platform may append its own detail after it).
pub fn summary_line(overrides: usize) -> String {
    format!("[varos] cursors: {} v1 (+ {overrides} reference overrides)", v1_table().len())
}

/// Render an arbitrary SVG string to straight-alpha RGBA, fit into a `size`×`size` box.
/// Used for the UI icons and the dev `--preview` mode. `force_black` recolors
/// everything to solid black (many icon sets use currentColor / theme fills).
pub fn render_svg(svg: &str, size: u32, force_black: bool) -> Option<(Vec<u8>, u32, u32)> {
    let svg = if force_black {
        // strip explicit fills/strokes so our wrapper color wins; cheap textual nudge
        svg.replace("currentColor", "#000").replace("fill=\"none\"", "")
    } else {
        svg.to_string()
    };
    let wrapped = if force_black {
        format!("<g fill=\"#000\" stroke=\"none\">{}</g>", inner_of(&svg)).replacen(
            "<g",
            &format!("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{}\"><g", viewbox_of(&svg)),
            1,
        ) + "</svg>"
    } else {
        svg.clone()
    };
    let tree = usvg::Tree::from_str(&wrapped, &usvg::Options::default())
        .or_else(|_| usvg::Tree::from_str(&svg, &usvg::Options::default()))
        .ok()?;
    let ts = tree.size();
    let sc = (size as f32 / ts.width().max(ts.height())).max(0.01);
    let (w, h) = (((ts.width() * sc).ceil() as u32).max(1), ((ts.height() * sc).ceil() as u32).max(1));
    let mut pm = tiny_skia::Pixmap::new(w, h)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(sc, sc), &mut pm.as_mut());
    let mut d = pm.data().to_vec();
    unpremultiply(&mut d);
    Some((d, w, h))
}
fn viewbox_of(svg: &str) -> String {
    if let Some(i) = svg.find("viewBox=\"") {
        let r = &svg[i + 9..];
        if let Some(j) = r.find('"') {
            return r[..j].to_string();
        }
    }
    "0 0 24 24".into()
}
fn inner_of(svg: &str) -> String {
    if let (Some(a), Some(b)) = (svg.find('>'), svg.rfind("</svg>")) {
        svg[a + 1..b].to_string()
    } else {
        svg.to_string()
    }
}

/// Build every Windows HCURSOR once (32-px bitmaps — the standard Windows cursor size) and log the
/// summary. A reference bitmap Win32 refuses retries with v1; a handle of 0 (Win32 refused both)
/// makes WM_SETCURSOR keep the OS arrow for that state.
#[cfg(windows)]
pub fn create_cursors() -> std::collections::HashMap<CK, isize> {
    let use_ai = ai_override_enabled();
    let (mut overrides, mut refused) = (0, 0);
    let map = ALL_CURSORS
        .iter()
        .map(|&ck| {
            let make = |ai: bool| {
                let ((d, w, h, hx, hy), from_ai) = cursor_rgba(ck, ai, CURSOR_PT)?;
                Some((build_hcursor(&d, w as u32, h as u32, hx, hy), from_ai)).filter(|(hc, _)| *hc != 0)
            };
            let hc = match make(use_ai).or_else(|| make(false)) {
                Some((hc, from_ai)) => {
                    overrides += from_ai as usize;
                    hc
                }
                None => {
                    refused += 1;
                    0
                }
            };
            (ck, hc)
        })
        .collect();
    eprintln!("{}; {refused} refused by Win32 (OS arrow)", summary_line(overrides));
    map
}

/// Build a Windows HCURSOR from straight-alpha RGBA. Returns the handle as isize, or 0 if Windows
/// refuses (GDI handle exhaustion) — callers and the WM_SETCURSOR path treat 0 as "keep the OS arrow",
/// so a failed cursor degrades instead of crashing (ENGINEERING_REVIEW §3.3).
#[cfg(windows)]
fn build_hcursor(rgba: &[u8], w: u32, h: u32, hx: u16, hy: u16) -> isize {
    use windows::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
        DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};
    unsafe {
        let mut bits: *mut core::ffi::c_void = core::ptr::null_mut();
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: core::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32), // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let hdc = GetDC(None);
        let hbm = CreateDIBSection(Some(hdc), &bi, DIB_RGB_COLORS, &mut bits, None, 0);
        ReleaseDC(None, hdc);
        let Ok(hbm) = hbm else { return 0 };
        if bits.is_null() {
            let _ = DeleteObject(HGDIOBJ(hbm.0));
            return 0;
        }
        // RGBA(straight) -> BGRA(premultiplied) for the alpha cursor
        let n = (w as usize) * (h as usize);
        let dst = core::slice::from_raw_parts_mut(bits as *mut u8, n * 4);
        for i in 0..n {
            let (r, g, b, a) = (rgba[i * 4], rgba[i * 4 + 1], rgba[i * 4 + 2], rgba[i * 4 + 3]);
            let pm = |c: u8| ((c as u16 * a as u16) / 255) as u8;
            dst[i * 4] = pm(b);
            dst[i * 4 + 1] = pm(g);
            dst[i * 4 + 2] = pm(r);
            dst[i * 4 + 3] = a;
        }
        let mask_bytes = vec![0u8; ((w as usize).div_ceil(16) * 2) * h as usize];
        let mask = CreateBitmap(w as i32, h as i32, 1, 1, Some(mask_bytes.as_ptr() as *const _));
        let ii =
            ICONINFO { fIcon: false.into(), xHotspot: hx as u32, yHotspot: hy as u32, hbmMask: mask, hbmColor: hbm };
        let hicon = CreateIconIndirect(&ii);
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        match hicon {
            Ok(hc) => hc.0 as isize,
            Err(_) => 0,
        }
    }
}

// ───────────────────────────── system-wide eyedropper (A5) ─────────────────────────────
// Sampling a pixel from ANYWHERE on the desktop (not just our canvas) is a plain Win32 screen-DC
// read: GetCursorPos → GetDC(NULL) → GetPixel. Committing the pick with a click over any window is
// a hook-less global read of the left button via GetAsyncKeyState (polled each frame while the
// eyedropper is armed). Nothing here installs a hook or captures input from other apps.

/// COLORREF (`0x00BBGGRR`, the Win32 GDI packing) → straight RGBA in 0..1. Pure; unit-tested.
#[cfg_attr(not(windows), allow(dead_code))] // used only by the Win32 paths
pub fn colorref_to_rgba(c: u32) -> [f32; 4] {
    let r = (c & 0xFF) as f32 / 255.0;
    let g = ((c >> 8) & 0xFF) as f32 / 255.0;
    let b = ((c >> 16) & 0xFF) as f32 / 255.0;
    [r, g, b, 1.0]
}

/// Sample the screen pixel currently under the OS cursor — works over ANY window on the desktop
/// (system-wide eyedropper). Returns straight RGBA in 0..1, or `None` if the read failed (GetPixel
/// yields `CLR_INVALID` over some hardware-accelerated / protected surfaces).
#[cfg(windows)]
pub fn screen_color_at_cursor() -> Option<[f32; 4]> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    // SAFETY: GetCursorPos writes into a POINT we own. GetDC(None) returns the screen DC, which we
    // ReleaseDC before returning (no leak, no handle outlives this call). GetPixel only reads.
    unsafe {
        let mut pt = POINT::default();
        if GetCursorPos(&mut pt).is_err() {
            return None;
        }
        let hdc = GetDC(None);
        if hdc.0.is_null() {
            return None;
        }
        let px = GetPixel(hdc, pt.x, pt.y);
        ReleaseDC(None, hdc);
        // CLR_INVALID == 0xFFFF_FFFF (GetPixel's failure sentinel).
        if px.0 == 0xFFFF_FFFF {
            return None;
        }
        Some(colorref_to_rgba(px.0))
    }
}
#[cfg(not(windows))]
pub fn screen_color_at_cursor() -> Option<[f32; 4]> {
    None
}

/// True while the physical left mouse button is down — polled globally (no hook), so the eyedropper
/// can commit a pick with a click over ANY window, not just ours.
#[cfg(windows)]
pub fn left_button_down() -> bool {
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
    // SAFETY: GetAsyncKeyState is a pure read of global key state; the high bit = currently down.
    unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}
#[cfg(not(windows))]
pub fn left_button_down() -> bool {
    false
}

// winit owns WM_SETCURSOR (it resets the cursor on every move), so we subclass the canvas window
// to intercept WM_SETCURSOR over the client area and set OUR current cursor instead. We ALSO set
// the window-class cursor + call SetCursor immediately as belt-and-suspenders.
#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};
    use std::sync::Mutex;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, GetWindowRect, SetClassLongPtrW, SetCursor, SetWindowPos, GCLP_HCURSOR, HCURSOR, HTBOTTOM,
        HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT,
        NCCALCSIZE_PARAMS, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME, SWP_FRAMECHANGED, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWMAXIMIZED, WINDOWPLACEMENT, WM_ACTIVATE, WM_DPICHANGED,
        WM_NCCALCSIZE, WM_NCHITTEST, WM_SETCURSOR, WM_SYSCOMMAND,
    };

    static CUR: AtomicIsize = AtomicIsize::new(0);
    static HWND_: AtomicIsize = AtomicIsize::new(0);
    static HITS: AtomicUsize = AtomicUsize::new(0); // WM_SETCURSOR HTCLIENT intercepts (debug)
    static INSTALLED: AtomicIsize = AtomicIsize::new(-1); // -1 unknown, 0 fail, 1 ok

    // The custom title-bar geometry, published each frame from the egui layout (physical px). The
    // wndproc reads it to decide where the window drags (HTCAPTION) vs where our controls live (HTCLIENT).
    #[derive(Clone, Copy, Default)]
    struct PxRect {
        l: i32,
        t: i32,
        r: i32,
        b: i32,
    }
    impl PxRect {
        fn has(&self, x: i32, y: i32) -> bool {
            x >= self.l && x < self.r && y >= self.t && y < self.b
        }
    }
    struct Caption {
        h: i32,
        excl: Vec<PxRect>,
    }
    static CAPTION: Mutex<Caption> = Mutex::new(Caption { h: 0, excl: Vec::new() });

    /// Publish the caption height + the interactive (non-drag) rects, in PHYSICAL px, for hit-testing.
    pub fn set_caption(h: i32, excl: &[[i32; 4]]) {
        if let Ok(mut g) = CAPTION.lock() {
            g.h = h;
            g.excl.clear();
            g.excl.extend(excl.iter().map(|r| PxRect { l: r[0], t: r[1], r: r[2], b: r[3] }));
        }
    }

    unsafe fn maximized(h: HWND) -> bool {
        let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
        GetWindowPlacement(h, &mut wp).is_ok() && wp.showCmd == SW_SHOWMAXIMIZED.0 as u32
    }

    // Strip the top caption (so the client area reaches y=0) while keeping the side/bottom resize frame,
    // shadow and rounded corners. Re-inset all sides when maximized so content doesn't overshoot the monitor.
    unsafe fn nccalcsize(h: HWND, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if wp.0 == 0 {
            return DefSubclassProc(h, WM_NCCALCSIZE, wp, lp);
        }
        let params = &mut *(lp.0 as *mut NCCALCSIZE_PARAMS);
        let requested = params.rgrc[0];
        let _ = DefSubclassProc(h, WM_NCCALCSIZE, wp, lp); // normal side/bottom frame
        params.rgrc[0].top = requested.top; // remove the top caption inset
        if maximized(h) {
            let dpi = GetDpiForWindow(h);
            let pad = GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
            let fx = GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) + pad;
            let fy = GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) + pad;
            params.rgrc[0].top = requested.top + fy;
            params.rgrc[0].left = requested.left + fx;
            params.rgrc[0].right = requested.right - fx;
            params.rgrc[0].bottom = requested.bottom - fy;
        }
        LRESULT(0)
    }

    // Resize borders + caption drag band (minus our interactive rects). Buttons/tabs are HTCLIENT so egui
    // handles them; the empty caption band is HTCAPTION so Windows drags/snaps the window natively.
    unsafe fn nchittest(h: HWND, lp: LPARAM) -> LRESULT {
        let dpi = GetDpiForWindow(h);
        let border = GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi);
        let sx = (lp.0 & 0xFFFF) as i16 as i32;
        let sy = ((lp.0 >> 16) & 0xFFFF) as i16 as i32;
        let mut rc = RECT::default();
        let _ = GetWindowRect(h, &mut rc);
        if !maximized(h) {
            let (l, r) = (sx < rc.left + border, sx >= rc.right - border);
            let (t, b) = (sy < rc.top + border, sy >= rc.bottom - border);
            let code: u32 = if t && l {
                HTTOPLEFT
            } else if t && r {
                HTTOPRIGHT
            } else if b && l {
                HTBOTTOMLEFT
            } else if b && r {
                HTBOTTOMRIGHT
            } else if t {
                HTTOP
            } else if b {
                HTBOTTOM
            } else if l {
                HTLEFT
            } else if r {
                HTRIGHT
            } else {
                0
            };
            if code != 0 {
                return LRESULT(code as isize);
            }
        }
        let mut p = POINT { x: sx, y: sy };
        let _ = ScreenToClient(h, &mut p);
        if let Ok(g) = CAPTION.lock() {
            if p.y >= 0 && p.y < g.h && !g.excl.iter().any(|e| e.has(p.x, p.y)) {
                return LRESULT(HTCAPTION as isize);
            }
        }
        LRESULT(HTCLIENT as isize)
    }

    /// Keep the DWM drop shadow + rounded corners (extend the frame 1px) and force the recalc.
    pub fn custom_frame(hwnd: isize) {
        use core::ffi::c_void;
        use windows::Win32::Graphics::Dwm::{
            DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        };
        use windows::Win32::UI::Controls::MARGINS;
        let h = HWND(hwnd as *mut _);
        unsafe {
            let m = MARGINS { cxLeftWidth: 0, cxRightWidth: 0, cyTopHeight: 1, cyBottomHeight: 0 };
            let _ = DwmExtendFrameIntoClientArea(h, &m);
            let pref = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                h,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &pref as *const _ as *const c_void,
                std::mem::size_of::<i32>() as u32,
            );
            // force a frame recalc now so the caption is stripped immediately
            let _ = SetWindowPos(
                h,
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

    /// True maximized state straight from Win32 (GetWindowPlacement) — winit's `is_maximized()` reports
    /// wrong with our custom (NCCALCSIZE-stripped) frame, so the saved geometry never recorded "maximized".
    pub fn is_maximized(hwnd: isize) -> bool {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{GetWindowPlacement, SW_SHOWMAXIMIZED, WINDOWPLACEMENT};
        let h = HWND(hwnd as *mut _);
        unsafe {
            let mut wp =
                WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            GetWindowPlacement(h, &mut wp).is_ok() && wp.showCmd == SW_SHOWMAXIMIZED.0 as u32
        }
    }
    /// Maximize via Win32 directly (SW_MAXIMIZE) — reliable through the custom frame; used to re-open maximized.
    pub fn maximize(hwnd: isize) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MAXIMIZE};
        let h = HWND(hwnd as *mut _);
        unsafe {
            let _ = ShowWindow(h, SW_MAXIMIZE);
        }
    }

    /// Hide/show the window via the DWM compositor (keeps focus/taskbar; lets us render frame 0 before
    /// the window is ever composited → no white/caption flash at startup).
    pub fn set_cloaked(hwnd: isize, on: bool) {
        use core::ffi::c_void;
        use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_CLOAK};
        let h = HWND(hwnd as *mut _);
        let v: i32 = on as i32; // BOOL = 4-byte int
        unsafe {
            let _ = DwmSetWindowAttribute(h, DWMWA_CLOAK, &v as *const _ as *const c_void, 4);
        }
    }

    /// Paint any OS-driven background fill (startup, resize gutter) in #141313 instead of white.
    pub fn set_dark_class_brush(hwnd: isize) {
        use windows::Win32::Foundation::COLORREF;
        use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, HGDIOBJ};
        use windows::Win32::UI::WindowsAndMessaging::GCLP_HBRBACKGROUND;
        let h = HWND(hwnd as *mut _);
        unsafe {
            let brush = CreateSolidBrush(COLORREF(0x0013_1314)); // 0x00BBGGRR for #141313
            let old = SetClassLongPtrW(h, GCLP_HBRBACKGROUND, brush.0 as isize);
            if old != 0 {
                let _ = DeleteObject(HGDIOBJ(old as *mut _));
            }
        }
    }

    unsafe extern "system" fn subclass(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
        match msg {
            WM_SETCURSOR if (lp.0 as u32 & 0xFFFF) == 1 => {
                HITS.fetch_add(1, Ordering::Relaxed);
                let c = CUR.load(Ordering::Relaxed);
                if c != 0 {
                    SetCursor(Some(HCURSOR(c as *mut _)));
                    return LRESULT(1);
                }
                DefSubclassProc(h, msg, wp, lp)
            }
            // swallow the Alt/F10 window system menu (SC_KEYMENU) — Alt is an editor modifier here
            WM_SYSCOMMAND if (wp.0 & 0xFFF0) == 0xF100 => LRESULT(0),
            WM_NCCALCSIZE => nccalcsize(h, wp, lp),
            WM_NCHITTEST => nchittest(h, lp),
            WM_ACTIVATE | WM_DPICHANGED => {
                custom_frame(h.0 as isize);
                DefSubclassProc(h, msg, wp, lp)
            }
            _ => DefSubclassProc(h, msg, wp, lp),
        }
    }

    pub fn install(hwnd: isize) -> bool {
        HWND_.store(hwnd, Ordering::Relaxed);
        let ok = unsafe { SetWindowSubclass(HWND(hwnd as *mut _), Some(subclass), 1, 0).as_bool() };
        INSTALLED.store(ok as isize, Ordering::Relaxed);
        ok
    }
    pub fn set(hcursor: isize) {
        CUR.store(hcursor, Ordering::Relaxed);
        let hwnd = HWND_.load(Ordering::Relaxed);
        if hwnd != 0 && hcursor != 0 {
            unsafe {
                SetClassLongPtrW(HWND(hwnd as *mut _), GCLP_HCURSOR, hcursor); // class default
                SetCursor(Some(HCURSOR(hcursor as *mut _))); // apply now
            }
        }
    }
    pub fn dbg() -> (isize, isize, usize, isize) {
        (
            HWND_.load(Ordering::Relaxed),
            INSTALLED.load(Ordering::Relaxed),
            HITS.load(Ordering::Relaxed),
            CUR.load(Ordering::Relaxed),
        )
    }
}
#[cfg(windows)]
pub use win::{
    custom_frame, dbg, install, is_maximized, maximize, set, set_caption, set_cloaked, set_dark_class_brush,
};

// Non-Windows twins of the Win32 shell functions above — SAME signatures, so main.rs / ui.rs call
// sites are identical on every platform (docs/foundation/MAC_SHELL_PORT.md). Nothing here pretends:
// features with no native equivalent yet are plain no-ops. Cursors are built once by
// `create_cursors` from the SAME v1 bitmaps + hotspots as the Windows HCURSORs (`cursor_rgba`):
// on macOS a Retina `NSCursor` (see `mac`), elsewhere a winit `CustomCursor`. A winit system
// `CursorIcon` is used only for a state whose custom cursor the OS refused. The winit window is
// handed over once via `bind_window`.
#[cfg(not(windows))]
mod portable {
    use super::{ai_override_enabled, cursor_rgba, summary_line, CursorBitmap, ALL_CURSORS, CK, CURSOR_PT};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{Arc, OnceLock};
    use winit::event_loop::EventLoop;
    use winit::window::{CursorIcon, CustomCursor, Window};

    static WINDOW: OnceLock<Arc<Window>> = OnceLock::new();
    static CUR: AtomicIsize = AtomicIsize::new(0);

    /// One winit cursor per `ALL_CURSORS` slot (slot = token − 1). On macOS a slot is `None` when its
    /// Retina NSCursor was built (that one is used instead) or when winit refused the bitmap; elsewhere
    /// `None` only when winit refused it. `set` then falls back as documented there.
    static WINIT: OnceLock<Vec<Option<CustomCursor>>> = OnceLock::new();

    /// Give the cursor/window helpers the live winit window (call once, right after creation).
    pub fn bind_window(w: Arc<Window>) {
        let _ = WINDOW.set(w);
    }

    /// Stand-in "cursor handle": a non-zero token (slot in `ALL_CURSORS` + 1), decoded by `set`.
    pub fn hcursor(ck: CK) -> isize {
        ALL_CURSORS.iter().position(|c| *c == ck).map_or(0, |i| i as isize + 1)
    }

    /// Build every cursor once (creating an NSCursor is not free), cache it per slot, log one line,
    /// and return the CK → token table `main.rs` passes to `set`. macOS: a Retina NSCursor (32-pt
    /// image with 32-px + 64-px bitmaps); if AppKit refuses it, a 32-px winit `CustomCursor`.
    /// Elsewhere: the 32-px winit `CustomCursor`. A reference bitmap that is refused retries with v1.
    pub fn create_cursors<T: 'static>(el: &EventLoop<T>) -> HashMap<CK, isize> {
        /// What one slot ended up as (`bool` = built from the local reference set).
        enum Made {
            #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
            Native(bool),
            Winit(CustomCursor, bool),
            System,
        }
        let use_ai = ai_override_enabled();
        let made: Vec<Made> = ALL_CURSORS
            .iter()
            .enumerate()
            .map(|(slot, &ck)| {
                #[cfg(target_os = "macos")]
                if let Some(from_ai) = super::mac::build(slot, ck, use_ai) {
                    return Made::Native(from_ai);
                }
                let _ = slot; // only the macOS native path caches per slot
                let make = |ai: bool| {
                    let ((d, w, h, hx, hy), from_ai): (CursorBitmap, bool) = cursor_rgba(ck, ai, CURSOR_PT)?;
                    let src = CustomCursor::from_rgba(d, w, h, hx, hy).ok()?;
                    Some(Made::Winit(el.create_custom_cursor(src), from_ai))
                };
                make(use_ai).or_else(|| make(false)).unwrap_or(Made::System)
            })
            .collect();
        let count = |f: fn(&Made) -> bool| made.iter().filter(|m| f(m)).count();
        let native = count(|m| matches!(m, Made::Native(_)));
        let winit_1x = count(|m| matches!(m, Made::Winit(..)));
        let system = count(|m| matches!(m, Made::System));
        let overrides = count(|m| matches!(m, Made::Native(true) | Made::Winit(_, true)));
        let _ = WINIT.set(
            made.into_iter()
                .map(|m| match m {
                    Made::Winit(c, _) => Some(c),
                    _ => None,
                })
                .collect(),
        );
        let how = if cfg!(target_os = "macos") { "Retina NSCursor (32 pt, 1x + 2x)" } else { "native" };
        eprintln!("{}; {native} {how}, {winit_1x} winit 1x, {system} system fallbacks", summary_line(overrides));
        ALL_CURSORS.iter().map(|&ck| (ck, hcursor(ck))).collect()
    }

    /// System cursor for a state whose custom cursor the OS refused: the closest winit built-in.
    pub fn icon(ck: CK) -> CursorIcon {
        match ck {
            CK::Select | CK::Direct | CK::SelectObject | CK::DirectAnchor | CK::DirectPath => CursorIcon::Default,
            CK::Pen
            | CK::PenNew
            | CK::PenAdd
            | CK::PenDel
            | CK::PenClose
            | CK::PenConnect
            | CK::Convert
            | CK::CrossRect
            | CK::CrossEllipse
            | CK::CrossTriangle
            | CK::CrossPolygon
            | CK::CrossRotate
            | CK::CrossScale
            | CK::Artboard
            | CK::Eye => CursorIcon::Crosshair,
            CK::ResizeH => CursorIcon::EwResize,
            CK::ResizeV => CursorIcon::NsResize,
            CK::ResizeNE => CursorIcon::NeswResize,
            CK::ResizeNW => CursorIcon::NwseResize,
            CK::Move => CursorIcon::Move,
            CK::Hand => CursorIcon::Grab,
            CK::Grab => CursorIcon::Grabbing,
            CK::Copy => CursorIcon::Copy,
            CK::RotateE
            | CK::RotateSE
            | CK::RotateS
            | CK::RotateSW
            | CK::RotateW
            | CK::RotateNW
            | CK::RotateN
            | CK::RotateNE => CursorIcon::Crosshair,
        }
    }

    /// Apply a cursor token after egui's platform output. On macOS main.rs only calls this while
    /// the pointer is inside the focused client view. macOS Retina NSCursor first (`NSCursor::set`), else the cached winit cursor, else the system
    /// icon. Cheap: `NSCursor::set` of the current cursor is a no-op for AppKit, a winit clone is a
    /// refcount bump and winit returns early when the view already shows that cursor.
    pub fn set(hcursor: isize) {
        CUR.store(hcursor, Ordering::Relaxed);
        let Some(i) = usize::try_from(hcursor - 1).ok().filter(|&i| i < ALL_CURSORS.len()) else {
            return; // 0 / unknown token → keep the OS arrow
        };
        #[cfg(target_os = "macos")]
        if super::mac::set(i) {
            return;
        }
        if let Some(w) = WINDOW.get() {
            match WINIT.get().and_then(|v| v.get(i)).and_then(|c| c.as_ref()) {
                Some(c) => w.set_cursor(c.clone()),
                None => w.set_cursor(icon(ALL_CURSORS[i])),
            }
        }
    }
    /// No window subclass on this platform — reports "not installed".
    pub fn install(_hwnd: isize) -> bool {
        false
    }
    pub fn dbg() -> (isize, isize, usize, isize) {
        (0, 0, 0, CUR.load(Ordering::Relaxed))
    }
    pub fn is_maximized(_hwnd: isize) -> bool {
        WINDOW.get().is_some_and(|w| w.is_maximized())
    }
    pub fn maximize(_hwnd: isize) {
        if let Some(w) = WINDOW.get() {
            w.set_maximized(true);
        }
    }
    /// The native title bar stays on this platform (no caption stripping) — nothing to do.
    pub fn custom_frame(_hwnd: isize) {}
    /// The bar's caption band (physical px height) + its interactive rects, published each frame by
    /// the top bar exactly as on Windows. macOS reads it back through `caption_drag_hit` to drag the
    /// window from an empty bar spot (docs/foundation/MAC_CHROME.md §A); elsewhere it is unused.
    static CAPTION: std::sync::Mutex<(i32, Vec<[i32; 4]>)> = std::sync::Mutex::new((0, Vec::new()));
    pub fn set_caption(h: i32, excl: &[[i32; 4]]) {
        if let Ok(mut g) = CAPTION.lock() {
            g.0 = h;
            g.1.clear();
            g.1.extend_from_slice(excl);
        }
    }
    /// Is this physical-px window point on the EMPTY part of the top bar (a drag spot)?
    #[cfg(target_os = "macos")]
    pub fn caption_drag_hit(x: f32, y: f32) -> bool {
        CAPTION.lock().is_ok_and(|g| crate::chrome::caption_hit(g.0, &g.1, x as i32, y as i32))
    }
    /// No DWM cloak equivalent wired yet — the window is simply shown.
    pub fn set_cloaked(_hwnd: isize, _on: bool) {}
    /// No OS class brush to recolour here.
    pub fn set_dark_class_brush(_hwnd: isize) {}
}
#[cfg(target_os = "macos")]
pub use portable::caption_drag_hit;
#[cfg(not(windows))]
pub use portable::{
    bind_window, create_cursors, custom_frame, dbg, install, is_maximized, maximize, set, set_caption, set_cloaked,
    set_dark_class_brush,
};

// macOS Retina cursors. winit 0.30's `CustomCursor` makes the NSImage `width × height` POINTS from a
// bitmap of the same pixel count (winit-0.30.13 src/platform_impl/macos/cursor.rs), so a 32-px bitmap
// is a 32-pt cursor that AppKit upscales 2× on Retina — soft. Here each cursor is one NSImage of
// 32×32 POINTS carrying TWO bitmap representations, 32 px (1×) and 64 px (2×), each rendered straight
// from the SVG; AppKit picks the one matching the screen. The hotspot is in points (the 32-px space).
//
// Ownership: main.rs calls `set` after egui's platform output only while the pointer is inside the
// focused client view. Entry and focus gain request redraws; exit/focus loss relinquish ownership.
// ui.rs suppresses egui's changing cursor icons to avoid routine cursor-rect invalidations; fallback
// Window::set_cursor writes can still invalidate the rect. AppKit owns the native title bar/outside.
//
// No `unsafe`: the bitmaps travel as PNG → `NSBitmapImageRep::imageRepWithData`, all safe objc2 APIs.
// The NSCursors are !Send, so they live in a main-thread `thread_local!` (all cursor calls run there).
#[cfg(target_os = "macos")]
mod mac {
    use super::{cursor_rgba, CursorBitmap, CK, CURSOR_PT, CURSOR_PX_2X};
    use objc2::rc::Retained;
    use objc2::AllocAnyThread;
    use objc2_app_kit::{NSBitmapImageRep, NSCursor, NSImage};
    use objc2_foundation::{NSData, NSPoint, NSSize};
    use std::cell::RefCell;

    thread_local! {
        static NATIVE: RefCell<Vec<Option<Retained<NSCursor>>>> = const { RefCell::new(Vec::new()) };
    }

    /// Straight-alpha RGBA → PNG bytes (the image crate is already a dependency).
    fn png(b: &CursorBitmap) -> Option<Vec<u8>> {
        let img = image::RgbaImage::from_raw(b.1 as u32, b.2 as u32, b.0.clone())?;
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(out.into_inner())
    }

    /// One 32×32-POINT NSCursor from a 1× (32 px) and a 2× (64 px) bitmap of the same glyph;
    /// `hotspot_pt` is in points. None if AppKit cannot decode a bitmap.
    pub fn nscursor(one_x: &CursorBitmap, two_x: &CursorBitmap, hotspot_pt: (f64, f64)) -> Option<Retained<NSCursor>> {
        let pt = NSSize::new(CURSOR_PT as f64, CURSOR_PT as f64);
        let image = NSImage::initWithSize(NSImage::alloc(), pt);
        for b in [one_x, two_x] {
            let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(&png(b)?))?;
            rep.setSize(pt); // 32 pt: the 64-px rep is therefore the 2× (Retina) one
            image.addRepresentation(&rep);
        }
        Some(NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, NSPoint::new(hotspot_pt.0, hotspot_pt.1)))
    }

    /// The 1× + 2× bitmaps for a CK from ONE source (reference override or v1), with that flag.
    pub fn bitmaps(ck: CK, use_ai: bool) -> Option<(CursorBitmap, CursorBitmap, bool)> {
        let pair = |ai: bool| {
            let (a, a_ai) = cursor_rgba(ck, ai, CURSOR_PT)?;
            let (b, b_ai) = cursor_rgba(ck, ai, CURSOR_PX_2X)?;
            (a_ai == b_ai).then_some((a, b, a_ai))
        };
        pair(use_ai).or_else(|| pair(false))
    }

    /// Build + cache the Retina NSCursor for `slot`. Some(from reference set) on success.
    pub fn build(slot: usize, ck: CK, use_ai: bool) -> Option<bool> {
        let (one_x, two_x, from_ai) = bitmaps(ck, use_ai)?;
        // hotspot in points = the 1× bitmap's pixel hotspot (1 px = 1 pt at 1×)
        let cursor = nscursor(&one_x, &two_x, (one_x.3 as f64, one_x.4 as f64))?;
        NATIVE.with(|n| {
            let mut n = n.borrow_mut();
            if n.len() <= slot {
                n.resize(slot + 1, None);
            }
            n[slot] = Some(cursor);
        });
        Some(from_ai)
    }

    /// `NSCursor::set` the cached Retina cursor for `slot`; false if there is none (caller falls back).
    pub fn set(slot: usize) -> bool {
        NATIVE.with(|n| match n.borrow().get(slot) {
            Some(Some(c)) => {
                c.set();
                true
            }
            _ => false,
        })
    }
}

/// True when the system-wide (outside-the-window) eyedropper can actually sample the screen. On other
/// platforms the picker shows the eyedropper disabled rather than arming a pick that can never land.
pub const SCREEN_EYEDROPPER: bool = cfg!(windows);

#[cfg(test)]
mod tests {
    use super::*;

    // Every CK variant is covered by the v1 table exactly once, in ALL_CURSORS order, with a hotspot
    // inside the 32×32 grid and an embedded SVG; ALL_CURSORS itself lists each variant once.
    #[test]
    fn v1_table_covers_every_ck_once_with_hotspots_inside() {
        let t = parse_v1(V1_HOTSPOTS_JSON).expect("hotspots.json parses");
        assert_eq!(t.len(), 36);
        for (i, e) in t.iter().enumerate() {
            assert!(e.ck == ALL_CURSORS[i], "slot {i} out of order");
            assert_eq!(ALL_CURSORS.iter().filter(|c| **c == e.ck).count(), 1, "{:?} listed twice", e.ck);
            assert!(e.hx < 32 && e.hy < 32, "{:?} hotspot outside 32x32", e.ck);
            assert!(e.svg.contains("<svg") && e.svg.contains("viewBox=\"0 0 32 32\""), "{}", e.file);
            assert_eq!(slot(e.ck), i);
        }
        // exhaustive match: adding a CK variant without a slot fails to compile here
        for ck in ALL_CURSORS {
            match ck {
                CK::Select
                | CK::Direct
                | CK::Pen
                | CK::PenNew
                | CK::PenAdd
                | CK::PenDel
                | CK::PenClose
                | CK::PenConnect
                | CK::Convert
                | CK::Eye
                | CK::ResizeH
                | CK::ResizeV
                | CK::ResizeNE
                | CK::ResizeNW
                | CK::Move
                | CK::Hand
                | CK::Grab
                | CK::Copy
                | CK::RotateE
                | CK::RotateSE
                | CK::RotateS
                | CK::RotateSW
                | CK::RotateW
                | CK::RotateNW
                | CK::RotateN
                | CK::RotateNE
                | CK::Artboard
                | CK::SelectObject
                | CK::DirectAnchor
                | CK::DirectPath
                | CK::CrossRect
                | CK::CrossEllipse
                | CK::CrossTriangle
                | CK::CrossPolygon
                | CK::CrossRotate
                | CK::CrossScale => {}
            }
        }
        // Embed the entire set, including the three proposed states without a CK yet.
        let json: serde_json::Value = serde_json::from_str(V1_HOTSPOTS_JSON).unwrap();
        let files = json["files"].as_object().unwrap();
        assert_eq!(V1_FILES.len(), 38);
        assert_eq!(files.len(), V1_FILES.len());
        let unique: std::collections::HashSet<_> = V1_FILES.iter().map(|(file, _)| *file).collect();
        assert_eq!(unique.len(), V1_FILES.len());
        for (file, _) in V1_FILES {
            assert!(files.contains_key(file), "{file} missing from hotspots.json");
        }
        // No orphans: every embedded file is used by a CK or named by a proposed state (v1.1 deleted
        // shape-rect.svg, then cross.svg once every crosshair tool got its own badge), and every proposed state's
        // file is embedded with the same hotspot as the files map.
        let proposed = json["proposed_ck"].as_object().unwrap();
        for (name, e) in proposed {
            let f = e["file"].as_str().unwrap();
            assert!(V1_FILES.iter().any(|(file, _)| *file == f), "proposed {name}: {f} not embedded");
            assert_eq!(&e["hotspot"], &files[f], "proposed {name}: hotspot differs from the files map");
            assert!(!t.iter().any(|c| format!("{:?}", c.ck) == *name), "{name} is both a CK and proposed");
        }
        for (file, _) in V1_FILES {
            let used = t.iter().any(|e| e.file == file) || proposed.values().any(|e| e["file"] == file);
            assert!(used, "{file} is embedded but no CK or proposed state uses it");
        }
    }

    // v1.1 family: every arrow-based cursor is built on the SAME arrow silhouette (one path, copied
    // verbatim) with its hotspot on the tip; filled arrow = Selection family, hollow = Direct family.
    #[test]
    fn arrow_family_shares_one_silhouette_and_the_tip_hotspot() {
        let first_path = |svg: &str| {
            let i = svg.find("<path d=\"").expect("a path") + 9;
            svg[i..i + svg[i..].find('"').unwrap()].to_string()
        };
        let base = first_path(v1(CK::Select).svg);
        let filled = [CK::Select, CK::Move, CK::Copy, CK::SelectObject];
        let hollow = [CK::Direct, CK::DirectAnchor, CK::DirectPath];
        for ck in filled.iter().chain(hollow.iter()) {
            let e = v1(*ck);
            assert_eq!(first_path(e.svg), base, "{ck:?} ({}) is not on the shared arrow", e.file);
            assert_eq!((e.hx, e.hy), (3, 3), "{ck:?} hotspot is not the arrow tip");
        }
        let no_drop = V1_FILES.iter().find(|(f, _)| *f == "no-drop.svg").unwrap().1;
        assert_eq!(first_path(no_drop), base, "no-drop.svg is not on the shared arrow");
        // the hollow family paints a stroke-less white interior over the ink; the filled family never does
        let hollow_interior = |svg: &str| svg.contains("fill=\"#ffffff\"/>");
        for ck in hollow {
            assert!(hollow_interior(v1(ck).svg), "{ck:?} is not hollow");
        }
        for ck in filled {
            assert!(!hollow_interior(v1(ck).svg), "{ck:?} is not filled");
        }
        // the two square badges differ only in fill: Direct-over-anchor hollow, the others filled
        assert!(v1(CK::DirectAnchor).svg.contains("fill=\"#ffffff\" stroke=\"#141313\""));
        assert!(v1(CK::DirectPath).svg.contains("fill=\"#141313\" stroke=\"#141313\" stroke-width=\"1.5\""));
        assert!(v1(CK::SelectObject).svg.contains("fill=\"#141313\" stroke=\"#141313\" stroke-width=\"1.5\""));
    }

    // v1.1 crosshair family (owner 2026-09-25: "one icon per tool"): every crosshair tool cursor carries the
    // SAME crosshair as artboard.svg (arms + centre dot, hotspot 11,11 at its centre) plus its own badge, and
    // no two badges are alike. No plain, badge-less crosshair remains.
    #[test]
    fn crosshair_family_shares_the_crosshair_and_each_tool_has_its_own_badge() {
        let arms = "<path d=\"M11 2.25V8.25\"/>";
        let family = [
            (CK::CrossRect, "cross-rect.svg"),
            (CK::CrossEllipse, "cross-ellipse.svg"),
            (CK::CrossTriangle, "cross-triangle.svg"),
            (CK::CrossPolygon, "cross-polygon.svg"),
            (CK::CrossRotate, "cross-rotate.svg"),
            (CK::CrossScale, "cross-scale.svg"),
            (CK::Artboard, "artboard.svg"),
        ];
        let mut badges = std::collections::HashSet::new();
        for (ck, file) in family {
            let e = v1(ck);
            assert_eq!(e.file, file, "{ck:?}");
            assert_eq!((e.hx, e.hy), (11, 11), "{ck:?} hotspot is not the crosshair centre");
            assert!(e.svg.contains(arms), "{file} does not carry the shared crosshair");
            let badge = &e.svg[e.svg.find("<rect x=\"10.25\"").expect("centre dot")..];
            assert!(badge.matches('<').count() > 3, "{file} has no badge after the crosshair");
            assert!(badges.insert(badge.to_string()), "{file} repeats another tool's badge");
        }
        assert!(!V1_FILES.iter().any(|(f, _)| *f == "cross.svg"), "a plain crosshair is back");
    }

    // The embedded table matches hotspots.json's own "files" map, and `Move` is the selection arrow.
    #[test]
    fn move_is_select_and_hotspots_match_the_files_map() {
        let v: serde_json::Value = serde_json::from_str(V1_HOTSPOTS_JSON).unwrap();
        for e in v1_table() {
            let h = &v["files"][e.file];
            assert_eq!((h[0].as_u64(), h[1].as_u64()), (Some(e.hx as u64), Some(e.hy as u64)), "{}", e.file);
        }
        assert_eq!(v1(CK::Move).file, "select.svg");
        assert_eq!((v1(CK::Move).hx, v1(CK::Move).hy), (v1(CK::Select).hx, v1(CK::Select).hy));
        assert!(v1_rgba(CK::Move, 32).unwrap().0 == v1_rgba(CK::Select, 32).unwrap().0);
    }

    // Broken data is rejected with a message (missing CK, unknown file, hotspot outside the grid).
    #[test]
    fn parse_v1_rejects_bad_data() {
        assert!(parse_v1("{}").is_err());
        let bad_hot = V1_HOTSPOTS_JSON.replacen("\"hotspot\": [2, 2]", "\"hotspot\": [32, 2]", 1);
        assert!(parse_v1(&bad_hot).unwrap_err().contains("outside"));
        let bad_file = V1_HOTSPOTS_JSON.replacen("\"file\": \"direct.svg\"", "\"file\": \"nope.svg\"", 1);
        assert!(parse_v1(&bad_file).unwrap_err().contains("not embedded"));
    }

    // Every v1 bitmap: 32×32 (1×) and 64×64 (2×) straight RGBA, not blank, hotspot inside and on or
    // next to ink, accepted by winit's `CustomCursor::from_rgba` (pure: no EventLoop, no GPU).
    #[test]
    fn every_v1_bitmap_is_non_blank_at_1x_and_2x_with_hotspot_inside() {
        for ck in ALL_CURSORS {
            for px in [CURSOR_PT, CURSOR_PX_2X] {
                let (d, w, h, hx, hy) = v1_rgba(ck, px).unwrap_or_else(|| panic!("{ck:?} failed to render"));
                assert_eq!((w as u32, h as u32), (px, px), "{ck:?}");
                assert_eq!(d.len(), px as usize * px as usize * 4);
                assert!(hx < w && hy < h, "{ck:?} hotspot ({hx},{hy}) outside {w}x{h}");
                let ink = d.chunks(4).filter(|p| p[3] > 0).count();
                assert!(ink > 20, "{ck:?} at {px}px is (nearly) blank");
                // the hotspot lies on ink or within 2 px (1×) of it — a misplaced hotspot would not
                let r = (2 * px / CURSOR_PT) as i32;
                let near = (-r..=r).any(|dy| {
                    (-r..=r).any(|dx| {
                        let (x, y) = (hx as i32 + dx, hy as i32 + dy);
                        x >= 0 && y >= 0 && x < w as i32 && y < h as i32 && d[((y * w as i32 + x) * 4 + 3) as usize] > 0
                    })
                });
                assert!(near, "{ck:?} hotspot not on/near ink at {px}px");
                assert!(winit::window::CustomCursor::from_rgba(d, w, h, hx, hy).is_ok(), "{ck:?}");
            }
        }
    }

    // Check all 38 embedded files (including proposed states), with no window or GPU.
    // Distinct files must have distinct bitmaps at both sizes.
    #[test]
    fn all_embedded_files_render_distinct_non_blank_bitmaps_at_both_sizes() {
        let json: serde_json::Value = serde_json::from_str(V1_HOTSPOTS_JSON).unwrap();
        for px in [CURSOR_PT, CURSOR_PX_2X] {
            let mut rendered = Vec::new();
            for (file, svg) in V1_FILES {
                let bitmap = rasterize(svg, px).unwrap_or_else(|| panic!("{file} at {px}px"));
                assert_eq!(bitmap.len(), (px * px * 4) as usize, "{file}");
                assert!(bitmap.chunks(4).any(|p| p[3] != 0), "{file} is blank");
                let hs = json["files"][file].as_array().unwrap();
                assert_eq!(hs.len(), 2);
                for h in hs {
                    let h = h.as_u64().unwrap();
                    assert!(h < CURSOR_PT as u64, "{file} hotspot outside logical grid");
                    assert!((hotspot_px(h as f32, px) as u32) < px, "{file} hotspot outside bitmap");
                }
                for (other_file, other_bitmap) in &rendered {
                    assert!(&bitmap != other_bitmap, "{file} == {other_file} at {px}px");
                }
                rendered.push((file, bitmap));
            }
        }
        let files: std::collections::HashSet<_> = v1_table().iter().map(|e| e.file).collect();
        assert_eq!(files.len(), 35);
    }

    // v1 is the default: without the env var the reference set is never consulted, and with it an
    // absent reference file falls back to v1 (the reference set is never in CI).
    #[test]
    fn v1_is_the_default_and_the_reference_override_is_opt_in() {
        for ck in ALL_CURSORS {
            let (b, from_ai) = cursor_rgba(ck, false, 32).unwrap();
            assert!(!from_ai);
            assert!(b.0 == v1_rgba(ck, 32).unwrap().0);
            let (stem, hx, hy) = ai_svg(ck);
            assert!(hx < 32.0 && hy < 32.0, "{stem}");
            let present = std::path::Path::new(&format!("{AI_SVG_DIR}{stem}.svg")).exists();
            let (b, from_ai) = cursor_rgba(ck, true, 32).unwrap();
            assert_eq!(from_ai, present, "{stem}");
            assert_eq!((b.1, b.2), (32, 32));
        }
        assert_eq!(AI_ENV, "VAROS_CURSORS_AI");
        assert_eq!(summary_line(0), "[varos] cursors: 36 v1 (+ 0 reference overrides)");
    }

    // macOS port — every cursor gets a distinct non-zero token that decodes back to itself, so
    // `set` never falls through to "keep the OS arrow" for a real cursor.
    #[cfg(not(windows))]
    #[test]
    fn portable_cursor_tokens_round_trip() {
        for (i, ck) in ALL_CURSORS.iter().enumerate() {
            let t = portable::hcursor(*ck);
            assert_eq!(t, i as isize + 1);
            assert!(ALL_CURSORS[(t - 1) as usize] == *ck);
        }
    }

    // macOS Retina: every CK builds an NSCursor whose image is 32×32 POINTS and carries a 32-px and a
    // 64-px representation (the 2× one is what a Retina screen shows), hotspot in points. AppKit
    // objects only — no window, no EventLoop, no GPU.
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_retina_nscursor_is_32pt_with_1x_and_2x_reps() {
        for ck in ALL_CURSORS {
            let (one_x, two_x, from_ai) = mac::bitmaps(ck, false).expect("bitmaps");
            assert!(!from_ai);
            assert_eq!((one_x.1, two_x.1), (32, 64));
            let c = mac::nscursor(&one_x, &two_x, (one_x.3 as f64, one_x.4 as f64)).expect("NSCursor");
            let img = c.image();
            let size = img.size();
            assert_eq!((size.width, size.height), (32.0, 32.0), "{ck:?} image not 32 pt");
            let mut px: Vec<isize> = img.representations().iter().map(|r| r.pixelsWide()).collect();
            px.sort();
            assert_eq!(px, vec![32, 64], "{ck:?} reps");
            for r in img.representations().iter() {
                let s = r.size();
                assert_eq!((s.width, s.height), (32.0, 32.0), "{ck:?} rep not 32 pt");
            }
            let hs = c.hotSpot();
            assert_eq!((hs.x, hs.y), (v1(ck).hx as f64, v1(ck).hy as f64), "{ck:?} hotspot");
            // with the dev override on, a PRESENT reference file gives both sizes from that file
            // (absent files — always, in CI — fall back to v1 for both)
            let (a, b, from_ai) = mac::bitmaps(ck, true).expect("bitmaps (override)");
            let (stem, _, _) = ai_svg(ck);
            assert_eq!(from_ai, std::path::Path::new(&format!("{AI_SVG_DIR}{stem}.svg")).exists(), "{stem}");
            assert_eq!((a.1, b.1), (32, 64), "{stem}");
            assert!(mac::nscursor(&a, &b, (a.3 as f64, a.4 as f64)).is_some(), "{stem}");
        }
    }

    // A5 — GDI COLORREF is 0x00BBGGRR; decode must swap B and R back into straight RGBA.
    #[test]
    fn colorref_decodes_bgr_order() {
        // pure red pixel: COLORREF 0x000000FF → R=1, G=0, B=0
        assert_eq!(colorref_to_rgba(0x0000_00FF), [1.0, 0.0, 0.0, 1.0]);
        // pure blue pixel: COLORREF 0x00FF0000 → R=0, G=0, B=1
        assert_eq!(colorref_to_rgba(0x00FF_0000), [0.0, 0.0, 1.0, 1.0]);
        // white and black
        assert_eq!(colorref_to_rgba(0x00FF_FFFF), [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(colorref_to_rgba(0x0000_0000), [0.0, 0.0, 0.0, 1.0]);
        // a mixed value 0x00204060 → B=0x20, G=0x40, R=0x60
        let c = colorref_to_rgba(0x0020_4060);
        assert!((c[0] - 0x60 as f32 / 255.0).abs() < 1e-6);
        assert!((c[1] - 0x40 as f32 / 255.0).abs() < 1e-6);
        assert!((c[2] - 0x20 as f32 / 255.0).abs() < 1e-6);
        assert_eq!(c[3], 1.0);
    }
}
