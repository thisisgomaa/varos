//! Real native tool cursors: Lucide SVGs rendered to bitmaps (resvg) and turned into Windows
//! HCURSORs. The app sets them on the canvas window class so they show across the whole client
//! area (the web panels use a matching CSS cursor). See DESIGN_BRIEF / memory varos-ui-shell.

use resvg::{tiny_skia, usvg};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum CK { Select, Direct, Pen, PenNew, PenAdd, PenDel, PenClose, PenConnect, Convert, Cross, Eye }

pub const ALL: [CK; 11] = [
    CK::Select, CK::Direct, CK::Pen, CK::PenNew, CK::PenAdd, CK::PenDel,
    CK::PenClose, CK::PenConnect, CK::Convert, CK::Cross, CK::Eye,
];

const SZ: u32 = 32; // cursor bitmap size

// ---- Lucide (MIT) path data ----
const ARROW: &str = r##"<path d="M4.037 4.688a.495.495 0 0 1 .651-.651l16 6.5a.5.5 0 0 1-.063.947l-6.124 1.58a2 2 0 0 0-1.438 1.435l-1.579 6.126a.5.5 0 0 1-.947.063z"/>"##;
const PEN: &str = r##"<path d="M15.707 21.293a1 1 0 0 1-1.414 0l-1.586-1.586a1 1 0 0 1 0-1.414l5.586-5.586a1 1 0 0 1 1.414 0l1.586 1.586a1 1 0 0 1 0 1.414z"/><path d="m18 13-1.375-6.874a1 1 0 0 0-.746-.776L3.235 2.028a1 1 0 0 0-1.207 1.207L5.35 15.879a1 1 0 0 0 .776.746L13 18"/><path d="m2.3 2.3 7.286 7.286"/><circle cx="11" cy="11" r="2"/>"##;
const PIPETTE: &str = r##"<path d="m12 9-8.414 8.414A2 2 0 0 0 3 18.828v1.344a2 2 0 0 1-.586 1.414A2 2 0 0 1 3.828 21h1.344a2 2 0 0 0 1.414-.586L15 12"/><path d="m18 9 .4.4a1 1 0 1 1-3 3l-3.8-3.8a1 1 0 1 1 3-3l.4.4 3.4-3.4a1 1 0 1 1 3 3z"/><path d="m2 22 .414-.414"/>"##;
const CARET: &str = r##"<path d="m6 15 6-6 6 6"/>"##;
const CROSS: &str = r##"<path d="M12 3v18"/><path d="M3 12h18"/>"##;

// small badge glyphs (drawn into the pen's lower-right via a transform)
fn badge(glyph: &str) -> String {
    format!(r##"<g transform="translate(12.5,12.5) scale(0.46)">{glyph}</g>"##)
}
fn pen_with(glyph: &str) -> String { format!("{PEN}{}", badge(glyph)) }

/// wrap stroked paths with a white halo + dark glyph so the cursor reads on any background
fn stroked(paths: &str) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke-linecap="round" stroke-linejoin="round"><g stroke="#f4f4f7" stroke-width="3.1">{paths}</g><g stroke="#161619" stroke-width="1.7">{paths}</g></svg>"##)
}
/// filled arrow (black or white)
fn filled(fill: &str, edge: &str) -> String {
    format!(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" stroke-linejoin="round"><g fill="{fill}" stroke="{edge}" stroke-width="1.4">{ARROW}</g></svg>"##)
}

/// (svg, hotspot_x, hotspot_y) in the 24×24 viewBox space
fn svg(ck: CK) -> (String, f32, f32) {
    match ck {
        CK::Select => (filled("#161619", "#f4f4f7"), 4.0, 4.0),
        CK::Direct => (filled("#f4f4f7", "#161619"), 4.0, 4.0),
        CK::Pen => (stroked(PEN), 2.3, 2.3),
        CK::PenNew => (stroked(&pen_with(r##"<path d="M18 6 6 18"/><path d="m6 6 12 12"/>"##)), 2.3, 2.3),
        CK::PenAdd => (stroked(&pen_with(r##"<path d="M5 12h14"/><path d="M12 5v14"/>"##)), 2.3, 2.3),
        CK::PenDel => (stroked(&pen_with(r##"<path d="M5 12h14"/>"##)), 2.3, 2.3),
        CK::PenClose => (stroked(&pen_with(r##"<circle cx="12" cy="12" r="9"/>"##)), 2.3, 2.3),
        CK::PenConnect => (stroked(&pen_with(r##"<path d="M6 18 18 6"/>"##)), 2.3, 2.3),
        CK::Convert => (stroked(CARET), 12.0, 9.0),
        CK::Cross => (stroked(CROSS), 12.0, 12.0),
        CK::Eye => (stroked(PIPETTE), 2.6, 21.4),
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
    for px in d.chunks_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            px[0] = ((px[0] as u32 * 255) / a) as u8;
            px[1] = ((px[1] as u32 * 255) / a) as u8;
            px[2] = ((px[2] as u32 * 255) / a) as u8;
        }
    }
    (d, SZ as u16, SZ as u16, (hx * sc).round() as u16, (hy * sc).round() as u16)
}

/// Build a Windows HCURSOR from straight-alpha RGBA. Returns the handle as isize (for SetClassLongPtr).
#[cfg(windows)]
pub fn hcursor(ck: CK) -> isize {
    use windows::Win32::Graphics::Gdi::{
        CreateBitmap, CreateDIBSection, DeleteObject, GetDC, ReleaseDC,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, ICONINFO};
    let (rgba, w, h, hx, hy) = rgba(ck);
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
        let hbm = CreateDIBSection(Some(hdc), &bi, DIB_RGB_COLORS, &mut bits, None, 0).unwrap();
        ReleaseDC(None, hdc);
        // RGBA(straight) -> BGRA(premultiplied) for the alpha cursor
        let n = (w as usize) * (h as usize);
        let dst = core::slice::from_raw_parts_mut(bits as *mut u8, n * 4);
        for i in 0..n {
            let (r, g, b, a) = (rgba[i*4], rgba[i*4+1], rgba[i*4+2], rgba[i*4+3]);
            let pm = |c: u8| ((c as u16 * a as u16) / 255) as u8;
            dst[i*4] = pm(b); dst[i*4+1] = pm(g); dst[i*4+2] = pm(r); dst[i*4+3] = a;
        }
        let mask_bytes = vec![0u8; (((w as usize) + 15) / 16 * 2) * h as usize];
        let mask = CreateBitmap(w as i32, h as i32, 1, 1, Some(mask_bytes.as_ptr() as *const _));
        let ii = ICONINFO { fIcon: false.into(), xHotspot: hx as u32, yHotspot: hy as u32, hbmMask: mask, hbmColor: hbm };
        let hicon = CreateIconIndirect(&ii).unwrap();
        let _ = DeleteObject(HGDIOBJ(hbm.0));
        let _ = DeleteObject(HGDIOBJ(mask.0));
        hicon.0 as isize
    }
}

// winit owns WM_SETCURSOR (it resets the cursor on every move), so we subclass the canvas window
// to intercept WM_SETCURSOR over the client area and set OUR current cursor instead.
#[cfg(windows)]
mod win {
    use std::sync::atomic::{AtomicIsize, Ordering};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{SetCursor, HCURSOR, WM_SETCURSOR};

    static CUR: AtomicIsize = AtomicIsize::new(0);

    unsafe extern "system" fn subclass(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM, _id: usize, _data: usize) -> LRESULT {
        if msg == WM_SETCURSOR && (lp.0 as u32 & 0xFFFF) == 1 {
            let c = CUR.load(Ordering::Relaxed);
            if c != 0 { SetCursor(Some(HCURSOR(c as *mut _))); return LRESULT(1); }
        }
        DefSubclassProc(h, msg, wp, lp)
    }
    pub fn install(hwnd: isize) { unsafe { let _ = SetWindowSubclass(HWND(hwnd as *mut _), Some(subclass), 1, 0); } }
    pub fn set(hcursor: isize) { CUR.store(hcursor, Ordering::Relaxed); }
}
#[cfg(windows)]
pub use win::{install, set};

/// The select-arrow as a URL-encoded SVG data URI, for the web panels' CSS `cursor`.
pub fn panel_arrow_css() -> String {
    let svg = filled("#161619", "#f4f4f7");
    let enc = svg.replace('#', "%23").replace('<', "%3C").replace('>', "%3E").replace('"', "%22").replace('\n', "");
    format!("url('data:image/svg+xml,{enc}') 4 4, auto")
}
