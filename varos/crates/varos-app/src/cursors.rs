//! Real native tool cursors, modeled on Adobe Illustrator's: a small fountain-pen NIB (not a
//! generic icon), solid/hollow arrows, a precise crosshair, eyedropper. SVGs are rendered to
//! bitmaps (resvg) and turned into Windows HCURSORs, set on the canvas window so they show across
//! the whole client area (the web panels use a matching CSS cursor). Glyphs are kept SMALL inside
//! the 32px frame and use a thin white halo so they read on any background without looking bloated.

use resvg::{tiny_skia, usvg};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum CK {
    // ---- our own SVG-rendered cursors (shippable: Font Awesome / Lucide / drawn) ----
    Select,
    Direct,
    Pen,
    PenNew,
    PenAdd,
    PenDel,
    PenClose,
    PenConnect,
    Convert,
    Cross,
    Eye,
    // ---- interaction-state cursors backed by the Illustrator set (TEMP local, see ai_svg) ----
    ResizeH,
    ResizeV,
    ResizeNE,
    ResizeNW,
    Move,
    Hand,
    Grab,
    Copy,
    NoDrop,
    // ---- rotate cursors, 8 compass directions (corner of the transform frame) ----
    RotateE,
    RotateSE,
    RotateS,
    RotateSW,
    RotateW,
    RotateNW,
    RotateN,
    RotateNE,
}

// SVG-backed cursors only — these are what the dev `--dump-cursors` previews.
pub const ALL: [CK; 11] = [
    CK::Select,
    CK::Direct,
    CK::Pen,
    CK::PenNew,
    CK::PenAdd,
    CK::PenDel,
    CK::PenClose,
    CK::PenConnect,
    CK::Convert,
    CK::Cross,
    CK::Eye,
];

// Every cursor we build an HCURSOR for (SVG ones + the interaction/rotate states).
pub const ALL_CURSORS: [CK; 28] = [
    CK::Select,
    CK::Direct,
    CK::Pen,
    CK::PenNew,
    CK::PenAdd,
    CK::PenDel,
    CK::PenClose,
    CK::PenConnect,
    CK::Convert,
    CK::Cross,
    CK::Eye,
    CK::ResizeH,
    CK::ResizeV,
    CK::ResizeNE,
    CK::ResizeNW,
    CK::Move,
    CK::Hand,
    CK::Grab,
    CK::Copy,
    CK::NoDrop,
    CK::RotateE,
    CK::RotateSE,
    CK::RotateS,
    CK::RotateSW,
    CK::RotateW,
    CK::RotateNW,
    CK::RotateN,
    CK::RotateNE,
];

/// The real Adobe Illustrator vector cursor for a CK: (SVG filename stem, hotspot_x, hotspot_y).
/// Hotspots are in the cursor's 1× (32px-logical) space (the SVGs are @2x / viewBox 64). TEMP local
/// set in assets/cursors-ai/svg/ — proprietary, never shipped; we fall back to our own SVG if absent.
pub fn ai_svg(ck: CK) -> Option<(&'static str, f32, f32)> {
    Some(match ck {
        CK::Select => ("CUR_SELECT", 1.0, 1.0),
        CK::Direct => ("CUR_DIRECTSELECT", 1.0, 1.0),
        CK::Pen => ("CUR_PEN", 1.0, 1.0),
        CK::PenNew => ("CUR_PENNEW", 1.0, 1.0),
        CK::PenAdd => ("CUR_PENADD", 1.0, 1.0),
        CK::PenDel => ("CUR_PENSUBSTRACT", 1.0, 1.0),
        CK::PenClose => ("CUR_PENCLOSE", 1.0, 1.0),
        CK::PenConnect => ("CUR_PENCONTINUE", 1.0, 1.0),
        CK::Convert => ("CUR_PENCORNER", 1.0, 1.0),
        CK::Cross => ("CUR_CROSSHAIRS", 12.0, 12.0),
        CK::Eye => ("CUR_EYEDROPPER", 2.0, 17.0),
        CK::ResizeH => ("CUR_SCALEHORIZONTAL", 9.0, 9.0),
        CK::ResizeV => ("CUR_SCALEVERTICAL", 9.0, 9.0),
        CK::ResizeNW => ("CUR_SCALETLBR", 9.0, 9.0), // ↖↘
        CK::ResizeNE => ("CUR_SCALETRBL", 9.0, 9.0), // ↗↙
        CK::Move => ("CUR_MOVE", 1.0, 1.0),
        CK::Hand => ("CUR_HAND", 11.0, 11.0),
        CK::Grab => ("CUR_FIST", 11.0, 11.0),
        CK::Copy => ("CUR_MOVECOPY", 1.0, 1.0),
        CK::NoDrop => ("CUR_NOMOVE", 1.0, 1.0),
        CK::RotateE => ("CUR_ROTATEFROMRIGHT", 7.0, 7.0),
        CK::RotateSE => ("CUR_ROTATEBOTTOMRIGHTCORNER", 7.0, 7.0),
        CK::RotateS => ("CUR_ROTATEFROMBOTTOM", 7.0, 7.0),
        CK::RotateSW => ("CUR_ROTATEBOTTOMLEFTCORNER", 7.0, 7.0),
        CK::RotateW => ("CUR_ROTATEFROMLEFT", 7.0, 7.0),
        CK::RotateNW => ("CUR_ROTATETOPLEFTCORNER", 7.0, 7.0),
        CK::RotateN => ("CUR_ROTATEFROMTOP", 7.0, 7.0),
        CK::RotateNE => ("CUR_ROTATETOPRIGHTCORNER", 7.0, 7.0),
    })
}

const SZ: u32 = 32; // cursor bitmap size (standard Windows cursor) for our built-in SVG cursors

/// Rendered size for the Illustrator vector cursors. On macOS winit makes the NSCursor image
/// `width × height` POINTS (not pixels), so this is also the cursor's logical size there — keep 32.
const CURSOR_PX: u32 = 32;
/// Where the TEMP local Illustrator cursor SVGs live (gitignored — never shipped, see .gitignore).
const AI_SVG_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/cursors-ai/svg/");

/// tiny-skia renders premultiplied RGBA; every cursor consumer (the Win32 DIB builder, winit
/// `CustomCursor`) takes straight alpha. In place.
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

// ---- glyph geometry (24×24 viewBox; kept compact, upper-left, like Illustrator) ----
// Selection arrow: tip (hotspot) at (3,3), short tail. ~10×15 in viewBox.
const ARROW: &str = "M3 3 L3 17 L6.8 13.2 L9.2 18.5 L11.2 17.6 L8.8 12.5 L13.5 12.5 Z";
// Pen nib: the real fountain-pen nib from Font Awesome Free 6 (icon `pen-nib`, CC-BY-4.0) — a
// professional silhouette with a sharp lower-left tip (the active point), center slit, and round
// vent hole. Native 512×512 viewBox; we scale it into the 24-space and add a white halo. See NOTICE.
const FA_NIB: &str = "M368.4 18.3L312.7 74.1 437.9 199.3l55.7-55.7c21.9-21.9 21.9-57.3 0-79.2L447.6 18.3c-21.9-21.9-57.3-21.9-79.2 0zM288 94.6l-9.2 2.8L134.7 140.6c-19.9 6-35.7 21.2-42.3 41L3.8 445.8c-3.8 11.3-1 23.9 7.3 32.4L164.7 324.7c-3-6.3-4.7-13.3-4.7-20.7c0-26.5 21.5-48 48-48s48 21.5 48 48s-21.5 48-48 48c-7.4 0-14.4-1.7-20.7-4.7L33.7 500.9c8.6 8.3 21.1 11.2 32.4 7.3l264.3-88.6c19.7-6.6 35-22.4 41-42.3l43.2-144.1 2.7-9.2L288 94.6z";
const PIPETTE: &str = r##"<path d="m12 9-8.414 8.414A2 2 0 0 0 3 18.828v1.344a2 2 0 0 1-.586 1.414A2 2 0 0 1 3.828 21h1.344a2 2 0 0 0 1.414-.586L15 12"/><path d="m18 9 .4.4a1 1 0 1 1-3 3l-3.8-3.8a1 1 0 1 1 3-3l.4.4 3.4-3.4a1 1 0 1 1 3 3z"/><path d="m2 22 .414-.414"/>"##;
const CARET: &str = r##"<path d="M7 13 L11 9 L15 13"/>"##; // convert-anchor "^"
const CROSS: &str = r##"<path d="M12 4V10"/><path d="M12 14V20"/><path d="M4 12H10"/><path d="M14 12H20"/>"##; // crosshair with center gap

/// stroked line-glyph (caret / crosshair / pipette): thin white halo + dark glyph.
fn stroked(paths: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round"><g stroke="#f4f4f7" stroke-width="2.2">{paths}</g><g stroke="#161619" stroke-width="1.4">{paths}</g></svg>"##
    )
}

/// solid (Select) or hollow (Direct) arrow with a thin contrasting edge.
fn arrow(fill: &str, edge: &str) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" stroke-linejoin="round"><path d="{ARROW}" fill="{fill}" stroke="{edge}" stroke-width="1.3"/></svg>"##
    )
}

/// the pen nib, optionally with a small state badge (✱/+/−/○/⁄) at its lower-right.
fn pen(glyph: &str) -> String {
    let badge = if glyph.is_empty() {
        String::new()
    } else {
        format!(
            r##"<g transform="translate(14.5,15)" stroke-linecap="round" stroke-linejoin="round"><g stroke="#f4f4f7" stroke-width="2.6" fill="none">{glyph}</g><g stroke="#161619" stroke-width="1.5" fill="none">{glyph}</g></g>"##
        )
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g transform="translate(1.4 1.4) scale(0.0415)" stroke-linejoin="round"><path d="{FA_NIB}" fill="none" stroke="#f4f4f7" stroke-width="58"/><path d="{FA_NIB}" fill="#161619"/></g>{badge}</svg>"##
    )
}

/// (svg, hotspot_x, hotspot_y) in the 24×24 viewBox space
fn svg(ck: CK) -> (String, f32, f32) {
    match ck {
        CK::Select => (arrow("#161619", "#f4f4f7"), 3.0, 3.0),
        CK::Direct => (arrow("#f4f4f7", "#161619"), 3.0, 3.0),
        CK::Pen => (pen(""), 2.7, 21.8),
        CK::PenNew => {
            (pen(r##"<path d="M3 0V6"/><path d="M0.6 1.5 5.4 4.5"/><path d="M5.4 1.5 0.6 4.5"/>"##), 2.7, 21.8)
        }
        CK::PenAdd => (pen(r##"<path d="M3 0V6"/><path d="M0 3H6"/>"##), 2.7, 21.8),
        CK::PenDel => (pen(r##"<path d="M0 3H6"/>"##), 2.7, 21.8),
        CK::PenClose => (pen(r##"<circle cx="3" cy="3" r="2.8"/>"##), 2.7, 21.8),
        CK::PenConnect => (pen(r##"<path d="M0 6 6 0"/>"##), 2.7, 21.8),
        CK::Convert => (stroked(CARET), 11.0, 9.0),
        CK::Cross => (stroked(CROSS), 12.0, 12.0),
        CK::Eye => (stroked(PIPETTE), 2.6, 21.4),
        // PNG-backed interaction cursors: only reached as a fallback when the AI PNGs are absent.
        _ => (arrow("#161619", "#f4f4f7"), 3.0, 3.0),
    }
}

/// Render an arbitrary SVG string to straight-alpha RGBA, fit into a `size`×`size` box.
/// Used by the dev `--preview` mode to eyeball candidate nib assets. `force_black` recolors
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

/// straight-alpha RGBA + hotspot in bitmap pixels
pub fn rgba(ck: CK) -> (Vec<u8>, u16, u16, u16, u16) {
    let (s, hx, hy) = svg(ck);
    let tree = usvg::Tree::from_str(&s, &usvg::Options::default()).unwrap();
    let mut pm = tiny_skia::Pixmap::new(SZ, SZ).unwrap();
    let sc = SZ as f32 / 24.0;
    resvg::render(&tree, tiny_skia::Transform::from_scale(sc, sc), &mut pm.as_mut());
    let mut d = pm.data().to_vec(); // premultiplied RGBA
    unpremultiply(&mut d);
    (d, SZ as u16, SZ as u16, (hx * sc).round() as u16, (hy * sc).round() as u16)
}

/// Build a Windows HCURSOR for one of our SVG-rendered cursors.
#[cfg(windows)]
pub fn hcursor(ck: CK) -> isize {
    let (rgba, w, h, hx, hy) = rgba(ck);
    build_hcursor(&rgba, w as u32, h as u32, hx, hy)
}

/// Render an Illustrator vector cursor SVG (`AI_SVG_DIR/<stem>.svg`) to straight-alpha RGBA at
/// CURSOR_PX — same tuple shape as `rgba`. The hotspot is given in 1× (32px-logical) space and scaled
/// to the rendered bitmap. None if the file is missing or unreadable. Shared by Windows (HCURSOR) and
/// macOS (winit `CustomCursor`), so both platforms show the identical bitmap.
pub fn svg_file_rgba(stem: &str, hx: f32, hy: f32) -> Option<(Vec<u8>, u16, u16, u16, u16)> {
    let svg = std::fs::read_to_string(format!("{AI_SVG_DIR}{stem}.svg")).ok()?;
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).ok()?;
    let vb = tree.size().width().max(tree.size().height()).max(1.0); // 64 for these @2x assets
    let scale = CURSOR_PX as f32 / vb;
    let mut pm = tiny_skia::Pixmap::new(CURSOR_PX, CURSOR_PX)?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(scale, scale), &mut pm.as_mut());
    let mut d = pm.data().to_vec();
    unpremultiply(&mut d);
    let hs = CURSOR_PX as f32 / 32.0; // 1×-logical → bitmap pixels
    let px = CURSOR_PX as u16;
    Some((d, px, px, (hx * hs).round() as u16, (hy * hs).round() as u16))
}

/// Build a Windows HCURSOR from an Illustrator vector cursor SVG (see `svg_file_rgba`). None if the
/// file is missing or Win32 refuses the handle.
#[cfg(windows)]
pub fn hcursor_svg_file(stem: &str, hx: f32, hy: f32) -> Option<isize> {
    let (d, w, h, hx, hy) = svg_file_rgba(stem, hx, hy)?;
    match build_hcursor(&d, w as u32, h as u32, hx, hy) {
        0 => None, // Win32 refused → let the caller fall back to the built-in SVG cursor
        hc => Some(hc),
    }
}

/// A cursor bitmap: (straight-alpha RGBA, width, height, hotspot_x, hotspot_y) — the `rgba` shape.
pub type CursorBitmap = (Vec<u8>, u16, u16, u16, u16);

/// True when `svg(ck)` draws a REAL, distinct built-in cursor for this state. False = `svg` only
/// returns the generic selection-arrow placeholder (the interaction / rotate states have no shippable
/// glyph of their own yet). Explicit and exhaustive on purpose — a new CK must choose.
#[cfg_attr(windows, allow(dead_code))] // only the non-Windows CustomCursor path (and tests) ask
pub const fn has_builtin(ck: CK) -> bool {
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
        | CK::Cross
        | CK::Eye => true,
        CK::ResizeH
        | CK::ResizeV
        | CK::ResizeNE
        | CK::ResizeNW
        | CK::Move
        | CK::Hand
        | CK::Grab
        | CK::Copy
        | CK::NoDrop
        | CK::RotateE
        | CK::RotateSE
        | CK::RotateS
        | CK::RotateSW
        | CK::RotateW
        | CK::RotateNW
        | CK::RotateN
        | CK::RotateNE => false,
    }
}

/// The bitmap a CK's non-Windows cursor is built from: the local Illustrator SVG when present
/// (`.1 == true`), else our own built-in SVG — but ONLY if that built-in is a real distinct glyph
/// (`has_builtin`). `None` = no distinct bitmap exists → the caller keeps the system `CursorIcon`, so
/// e.g. the resize / hand / no-drop cues never collapse into the plain arrow. `use_ai = false` forces
/// the built-in (shippable) set.
#[cfg_attr(windows, allow(dead_code))] // Windows picks per CK in main.rs (hcursor_svg_file → hcursor)
pub fn cursor_rgba(ck: CK, use_ai: bool) -> Option<(CursorBitmap, bool)> {
    let ai = if use_ai { ai_svg(ck).and_then(|(stem, hx, hy)| svg_file_rgba(stem, hx, hy)) } else { None };
    match ai {
        Some(b) => Some((b, true)),
        None if has_builtin(ck) => Some((rgba(ck), false)),
        None => None,
    }
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
// features with no native equivalent yet are plain no-ops. Cursors are real winit `CustomCursor`s
// built from the SAME bitmaps + hotspots as the Windows HCURSORs (`cursor_rgba`), created once by
// `create_custom_cursors` and cached per CK — but only where a DISTINCT bitmap exists (a cursors-ai
// file, or a real built-in glyph per `has_builtin`). Every other state (e.g. resize / hand / no-drop
// in a fresh clone) keeps winit's system `CursorIcon`, so no cue collapses into the plain arrow. The
// winit window is handed over once via `bind_window`.
#[cfg(not(windows))]
mod portable {
    use super::{ai_svg, cursor_rgba, ALL_CURSORS, CK};
    use std::sync::atomic::{AtomicIsize, Ordering};
    use std::sync::{Arc, OnceLock};
    use winit::event_loop::EventLoop;
    use winit::window::{CursorIcon, CustomCursor, Window};

    static WINDOW: OnceLock<Arc<Window>> = OnceLock::new();
    static CUR: AtomicIsize = AtomicIsize::new(0);

    /// One ready cursor per `ALL_CURSORS` slot (slot = token − 1) and whether it came from the local
    /// Illustrator set. `None` in a slot = no distinct bitmap for that state (or winit refused it) →
    /// `set` uses the system `icon` fallback.
    struct Built {
        cursor: CustomCursor,
        from_ai: bool,
    }
    static CUSTOM: OnceLock<Vec<Option<Built>>> = OnceLock::new();

    /// Give the cursor/window helpers the live winit window (call once, right after creation).
    pub fn bind_window(w: Arc<Window>) {
        let _ = WINDOW.set(w);
    }

    /// Build each tool cursor that has a distinct bitmap once (creating an NSCursor is not free) and
    /// cache it per CK; the rest stay on the system `CursorIcon` (counted as "system fallbacks"). Call once,
    /// after the event loop + window exist and BEFORE `hcursor` / `hcursor_svg_file` are queried.
    /// Bitmaps are CURSOR_PX/SZ = 32 px, which winit on macOS turns into a 32×32-POINT cursor — the
    /// right on-screen size on Retina (a 64 px bitmap would be a double-size cursor). Logs one line.
    /// Returns (custom cursors built, of which from the Illustrator set).
    pub fn create_custom_cursors<T: 'static>(el: &EventLoop<T>) -> (usize, usize) {
        let built: Vec<Option<Built>> = ALL_CURSORS
            .iter()
            .map(|&ck| {
                let make = |use_ai: bool| {
                    let ((d, w, h, hx, hy), from_ai) = cursor_rgba(ck, use_ai)?;
                    CustomCursor::from_rgba(d, w, h, hx, hy)
                        .ok()
                        .map(|src| Built { cursor: el.create_custom_cursor(src), from_ai })
                };
                // an Illustrator bitmap winit rejects falls back to the built-in one, like Windows does;
                // no distinct bitmap at all → None → `set` keeps the system CursorIcon for this state
                make(true).or_else(|| make(false))
            })
            .collect();
        let n = built.iter().flatten().count();
        let m = built.iter().flatten().filter(|b| b.from_ai).count();
        let s = built.iter().filter(|b| b.is_none()).count();
        let _ = CUSTOM.set(built);
        eprintln!(
            "[varos] cursors: {n} custom ({m} from cursors-ai, {} built-in) + {s} system fallbacks of {}",
            n - m,
            ALL_CURSORS.len()
        );
        (n, m)
    }

    fn built(slot: usize) -> Option<&'static Built> {
        CUSTOM.get().and_then(|v| v.get(slot)).and_then(|b| b.as_ref())
    }

    /// Stand-in "cursor handle": a non-zero token (index into `ALL_CURSORS` + 1), decoded by `set`.
    pub fn hcursor(ck: CK) -> isize {
        ALL_CURSORS.iter().position(|c| *c == ck).map_or(0, |i| i as isize + 1)
    }
    /// Some(token) when the cached cursor for the CK that uses this Illustrator `stem` really was built
    /// from the local cursors-ai SVG; None otherwise (the caller then falls back to `hcursor`, which
    /// resolves to the built-in bitmap for that CK). Hotspots come from `ai_svg`, as on Windows.
    pub fn hcursor_svg_file(stem: &str, _hx: f32, _hy: f32) -> Option<isize> {
        let i = ALL_CURSORS.iter().position(|&ck| ai_svg(ck).is_some_and(|(s, _, _)| s == stem))?;
        built(i).filter(|b| b.from_ai).map(|_| i as isize + 1)
    }

    /// System cursor for a state with no distinct custom bitmap (or one winit refused): the closest
    /// winit built-in cursor for each tool/interaction state.
    pub fn icon(ck: CK) -> CursorIcon {
        match ck {
            CK::Select | CK::Direct => CursorIcon::Default,
            CK::Pen
            | CK::PenNew
            | CK::PenAdd
            | CK::PenDel
            | CK::PenClose
            | CK::PenConnect
            | CK::Convert
            | CK::Cross
            | CK::Eye => CursorIcon::Crosshair,
            CK::ResizeH => CursorIcon::EwResize,
            CK::ResizeV => CursorIcon::NsResize,
            CK::ResizeNE => CursorIcon::NeswResize,
            CK::ResizeNW => CursorIcon::NwseResize,
            CK::Move => CursorIcon::Move,
            CK::Hand => CursorIcon::Grab,
            CK::Grab => CursorIcon::Grabbing,
            CK::Copy => CursorIcon::Copy,
            CK::NoDrop => CursorIcon::NotAllowed,
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

    pub fn set(hcursor: isize) {
        CUR.store(hcursor, Ordering::Relaxed);
        let Some(i) = usize::try_from(hcursor - 1).ok().filter(|&i| i < ALL_CURSORS.len()) else {
            return; // 0 / unknown token → keep the OS arrow
        };
        if let Some(w) = WINDOW.get() {
            // Re-asserted every frame by main.rs; cheap — the clone is a refcount bump and winit's
            // macOS `set_cursor` returns early when the view already shows this NSCursor.
            match built(i) {
                Some(b) => w.set_cursor(b.cursor.clone()),
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
    bind_window, create_custom_cursors, custom_frame, dbg, hcursor, hcursor_svg_file, install, is_maximized, maximize,
    set, set_caption, set_cloaked, set_dark_class_brush,
};

/// True when the system-wide (outside-the-window) eyedropper can actually sample the screen. On other
/// platforms the picker shows the eyedropper disabled rather than arming a pick that can never land.
pub const SCREEN_EYEDROPPER: bool = cfg!(windows);

#[cfg(test)]
mod tests {
    use super::colorref_to_rgba;

    // macOS port — every cursor gets a distinct non-zero token that decodes back to itself, so
    // `set` never falls through to "keep the OS arrow" for a real cursor.
    #[cfg(not(windows))]
    #[test]
    fn portable_cursor_tokens_round_trip() {
        use super::{hcursor, ALL_CURSORS};
        for (i, ck) in ALL_CURSORS.iter().enumerate() {
            let t = hcursor(*ck);
            assert_eq!(t, i as isize + 1);
            assert!(ALL_CURSORS[(t - 1) as usize] == *ck);
        }
    }

    // Every CK's built-in bitmap (`rgba` — what the Win32 HCURSOR fallback uses for all 28) is 32×32
    // straight RGBA with its hotspot inside, is not blank, and passes winit's own
    // `CustomCursor::from_rgba` validation (pure: no EventLoop, no GPU).
    #[test]
    fn every_cursor_bitmap_is_32px_with_hotspot_inside_and_winit_accepts_it() {
        use super::{rgba, ALL_CURSORS};
        for ck in ALL_CURSORS {
            let (d, w, h, hx, hy) = rgba(ck);
            assert_eq!((w, h), (32, 32));
            assert_eq!(d.len(), w as usize * h as usize * 4);
            assert!(hx < w && hy < h, "hotspot ({hx},{hy}) outside {w}x{h}");
            assert!(d.chunks(4).any(|p| p[3] > 0), "blank cursor bitmap");
            assert!(winit::window::CustomCursor::from_rgba(d, w, h, hx, hy).is_ok());
        }
    }

    // `has_builtin` is the explicit list of real glyphs: it matches `ALL` (the SVG-backed set), and each
    // of those glyphs is a different bitmap from the Select arrow placeholder (except Select itself).
    // With use_ai=false, `cursor_rgba` offers exactly those and nothing else.
    #[test]
    fn has_builtin_is_exactly_the_distinct_svg_glyphs() {
        use super::{cursor_rgba, has_builtin, rgba, ALL, ALL_CURSORS, CK};
        let arrow = rgba(CK::Select).0;
        for ck in ALL_CURSORS {
            assert_eq!(has_builtin(ck), ALL.contains(&ck));
            if has_builtin(ck) && ck != CK::Select {
                assert!(rgba(ck).0 != arrow, "a 'distinct' built-in is really the arrow placeholder");
            }
            match cursor_rgba(ck, false) {
                Some((_, from_ai)) => assert!(has_builtin(ck) && !from_ai),
                None => assert!(!has_builtin(ck)),
            }
        }
    }

    // Review P2 — in a fresh clone (no cursors-ai), no state may silently collapse into the plain arrow:
    // each CK has a distinct built-in bitmap OR keeps a non-default system CursorIcon on this platform.
    #[cfg(not(windows))]
    #[test]
    fn without_cursors_ai_no_state_collapses_to_the_arrow() {
        use super::portable::icon;
        use super::{cursor_rgba, ALL_CURSORS};
        use winit::window::CursorIcon;
        for (slot, ck) in ALL_CURSORS.into_iter().enumerate() {
            let distinct = cursor_rgba(ck, false).is_some();
            assert!(distinct || icon(ck) != CursorIcon::Default, "ALL_CURSORS[{slot}] collapses to the arrow");
        }
    }

    // The Illustrator (local, gitignored) set: hotspots scale inside the CURSOR_PX bitmap. A file that
    // is ABSENT is skipped (that set is never shipped; the built-in set above is what CI always checks),
    // but a file that is PRESENT must render at CURSOR_PX, be preferred, and pass winit — a present but
    // broken file fails the test with its name instead of being silently skipped.
    #[test]
    fn illustrator_cursor_hotspots_fit_and_present_files_render() {
        use super::{ai_svg, cursor_rgba, svg_file_rgba, AI_SVG_DIR, ALL_CURSORS, CURSOR_PX};
        for ck in ALL_CURSORS {
            let (stem, hx, hy) = ai_svg(ck).expect("every CK maps to an Illustrator stem");
            let s = CURSOR_PX as f32 / 32.0;
            assert!((hx * s).round() < CURSOR_PX as f32 && (hy * s).round() < CURSOR_PX as f32, "{stem}");
            if !std::path::Path::new(&format!("{AI_SVG_DIR}{stem}.svg")).exists() {
                continue;
            }
            let (d, w, h, bx, by) =
                svg_file_rgba(stem, hx, hy).unwrap_or_else(|| panic!("{stem}.svg is present but failed to render"));
            assert_eq!((w as u32, h as u32), (CURSOR_PX, CURSOR_PX), "{stem}");
            assert_eq!(d.len(), w as usize * h as usize * 4, "{stem}");
            assert!(bx < w && by < h, "{stem}");
            assert!(cursor_rgba(ck, true).is_some_and(|(_, ai)| ai), "{stem} present but not preferred");
            assert!(winit::window::CustomCursor::from_rgba(d, w, h, bx, by).is_ok(), "{stem}");
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
