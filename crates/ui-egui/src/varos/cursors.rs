//! Original Varos v1.1 SVG cursors, with a true Retina AppKit path on macOS.
//!
//! Only explicitly mapped states replace upstream glyphs. In particular, a precise crosshair
//! must never acquire a shape badge just because a shape tool is active.
use egui::CustomCursorImage;
use std::sync::OnceLock;
use vectorcraft_tools::Cursor;

const POINTS: u16 = 32;
const HOTSPOTS: &str = include_str!("assets/cursors/v1/hotspots.json");

macro_rules! files {
    ($($name:literal),* $(,)?) => { &[$(($name, include_str!(concat!("assets/cursors/v1/", $name)))),*] };
}
const FILES: &[(&str, &str)] = files![
    "select.svg",
    "direct.svg",
    "select-object.svg",
    "direct-anchor.svg",
    "direct-path.svg",
    "copy.svg",
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
    "no-drop.svg",
    "zoom-in.svg",
    "zoom-out.svg",
    "artboard.svg",
    "cross-rect.svg",
    "cross-ellipse.svg",
    "cross-triangle.svg",
    "cross-polygon.svg",
    "cross-rotate.svg",
    "cross-scale.svg",
    "rotate-n.svg",
    "rotate-ne.svg",
    "rotate-e.svg",
    "rotate-se.svg",
    "rotate-s.svg",
    "rotate-sw.svg",
    "rotate-w.svg",
    "rotate-nw.svg",
];

fn file(c: Cursor, tool: &str) -> Option<&'static str> {
    Some(match c {
        Cursor::Arrow => "select.svg",
        Cursor::ArrowHollow => "direct.svg",
        Cursor::DirectAnchor => "direct-anchor.svg",
        Cursor::DirectAnchorSelected | Cursor::ReshapeSegment => "direct-path.svg",
        Cursor::SelectionCopy => "copy.svg",
        Cursor::Move if tool == "artboard" => "artboard.svg",
        // Upstream Selection emits Move only over a selected object (or with Alt), not during
        // its move drag: that drag emits Arrow/ArrowHollow. Keep the hover badge off the drag.
        Cursor::Move if tool == "selection" => "select-object.svg",
        Cursor::Move => "select.svg",
        Cursor::Pen => "pen-new.svg",
        Cursor::PenDrawing => "pen.svg",
        Cursor::PenAdd => "pen-add.svg",
        Cursor::PenDelete => "pen-delete.svg",
        Cursor::PenClose => "pen-close.svg",
        Cursor::PenContinue | Cursor::PenJoin => "pen-connect.svg",
        Cursor::PenConvert | Cursor::AnchorPoint => "convert.svg",
        Cursor::Eyedropper => "eyedropper.svg",
        Cursor::ResizeH => "resize-h.svg",
        Cursor::ResizeV => "resize-v.svg",
        Cursor::ResizeNeSw => "resize-ne.svg",
        Cursor::ResizeNwSe => "resize-nw.svg",
        Cursor::Hand => "hand.svg",
        Cursor::HandGrab => "grab.svg",
        Cursor::ZoomIn => "zoom-in.svg",
        Cursor::ZoomOut => "zoom-out.svg",
        Cursor::NotAllowed => "no-drop.svg",
        Cursor::Rotate => "rotate-n.svg",
        Cursor::RotateToward(o) => match o % 8 {
            0 => "rotate-n.svg",
            1 => "rotate-ne.svg",
            2 => "rotate-e.svg",
            3 => "rotate-se.svg",
            4 => "rotate-s.svg",
            5 => "rotate-sw.svg",
            6 => "rotate-w.svg",
            _ => "rotate-nw.svg",
        },
        Cursor::Crosshair => match tool {
            "rectangle" | "roundedRectangle" => "cross-rect.svg",
            "ellipse" => "cross-ellipse.svg",
            "polygon" => "cross-polygon.svg",
            "rotate" => "cross-rotate.svg",
            "scale" => "cross-scale.svg",
            "artboard" => "artboard.svg",
            _ => return None,
        },
        _ => return None,
    })
}

fn hotspot(name: &str) -> Option<[u16; 2]> {
    static JSON: OnceLock<Option<serde_json::Value>> = OnceLock::new();
    let values = JSON.get_or_init(|| serde_json::from_str(HOTSPOTS).ok()).as_ref()?.get("files")?.get(name)?.as_array()?;
    let coord = |i: usize| values.get(i)?.as_u64().filter(|&n| n < u64::from(POINTS)).map(|n| n as u16);
    Some([coord(0)?, coord(1)?])
}

/// Bounded, fallible SVG rasterization. resvg's premultiplied output becomes straight RGBA for
/// both egui/winit and AppKit's PNG decoder. External files and fonts are never loaded.
fn rasterize(name: &str, px: u16) -> Option<CustomCursorImage> {
    if px == 0 || px > 256 {
        return None;
    }
    let svg = FILES.iter().find(|(n, _)| *n == name)?.1;
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
    let scale = f32::from(px) / tree.size().width().max(tree.size().height());
    let mut pixmap = resvg::tiny_skia::Pixmap::new(u32::from(px), u32::from(px))?;
    resvg::render(&tree, resvg::tiny_skia::Transform::from_scale(scale, scale), &mut pixmap.as_mut());
    let mut rgba = pixmap.take();
    for [r, g, b, a] in rgba.as_chunks_mut::<4>().0 {
        let alpha = u32::from(*a);
        for c in [r, g, b] {
            // Transparent pixels have zero colour; checked division also handles alpha = 0.
            *c = (u32::from(*c) * 255 + alpha / 2).checked_div(alpha).unwrap_or(0).min(255) as u8;
        }
    }
    let hotspot = hotspot(name)?.map(|v| (f32::from(v) * f32::from(px) / f32::from(POINTS)).round() as u16);
    if hotspot.iter().any(|&h| h >= px) {
        return None;
    }
    Some(CustomCursorImage { rgba: rgba.into(), size: [px, px], hotspot })
}

/// Bitmaps for the upstream compositor path, cached by asset and physical size. The cache owns
/// the same RGBA Arc between frames, so egui-winit doesn't rebuild the native cursor each frame.
pub(crate) fn image(c: Cursor, tool: &str, ppp: f32) -> Option<CustomCursorImage> {
    use std::cell::RefCell;
    type Entry = (&'static str, u16, Option<CustomCursorImage>);
    thread_local! { static CACHE: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) }; }
    let name = file(c, tool)?;
    let px = f32::from(POINTS) * ppp;
    if !px.is_finite() || !(1.0..=256.0).contains(&px) {
        return None;
    }
    let px = px.round() as u16;
    CACHE.with(|cache| {
        // Reentrant cursor queries can safely fall back to the upstream glyph.
        let mut cache = cache.try_borrow_mut().ok()?;
        if let Some((_, _, image)) = cache.iter().find(|(n, p, _)| *n == name && *p == px) {
            return image.clone();
        }
        let image = rasterize(name, px);
        if cache.len() >= 128 {
            cache.clear();
        }
        cache.push((name, px, image.clone()));
        image
    })
}

#[cfg(target_os = "macos")]
pub(crate) use mac::{begin_frame, end_frame, request};

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use objc2::{AllocAnyThread, MainThreadMarker, rc::Retained};
    use objc2_app_kit::{NSApplication, NSBitmapImageRep, NSCursor, NSImage, NSWindow};
    use objc2_foundation::{NSData, NSPoint, NSSize};
    use std::cell::RefCell;

    #[derive(Clone, Copy, PartialEq, Eq)]
    struct Key {
        name: &'static str,
        points: u16,
    }

    #[derive(Default)]
    struct State {
        cache: Vec<(Key, Option<Retained<NSCursor>>)>,
        pending: Option<Key>,
        // Keep the active cursor alive even if the bounded cache is evicted on a scale change.
        active: Option<(Key, Retained<NSWindow>, Retained<NSCursor>)>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    fn png(bitmap: &CustomCursorImage) -> Option<Vec<u8>> {
        let image = image::RgbaImage::from_raw(u32::from(bitmap.size[0]), u32::from(bitmap.size[1]), bitmap.rgba.to_vec())?;
        let mut out = std::io::Cursor::new(Vec::new());
        image.write_to(&mut out, image::ImageFormat::Png).ok()?;
        Some(out.into_inner())
    }

    fn build(key: Key) -> Option<Retained<NSCursor>> {
        let size = NSSize::new(f64::from(key.points), f64::from(key.points));
        let image = NSImage::initWithSize(NSImage::alloc(), size);
        let one = rasterize(key.name, key.points)?;
        let two = rasterize(key.name, key.points.checked_mul(2)?)?;
        for bitmap in [&one, &two] {
            let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(&png(bitmap)?))?;
            rep.setSize(size);
            image.addRepresentation(&rep);
        }
        Some(NSCursor::initWithImage_hotSpot(NSCursor::alloc(), &image, NSPoint::new(f64::from(one.hotspot[0]), f64::from(one.hotspot[1]))))
    }

    pub(crate) fn begin_frame() {
        STATE.with(|s| {
            if let Ok(mut state) = s.try_borrow_mut() {
                state.pending = None;
            }
        });
    }

    /// Returns true only if a native cursor can be used. A headless frame or an unfocused window
    /// keeps upstream behaviour. The final UI cursor decision is checked again at frame end.
    pub(crate) fn request(c: Cursor, tool: &str, scale: f32, focused: bool) -> bool {
        if !focused {
            return false;
        }
        let Some(mtm) = MainThreadMarker::new() else { return false };
        let Some(window) = NSApplication::sharedApplication(mtm).keyWindow() else { return false };
        let Some(name) = file(c, tool) else { return false };
        let points = f32::from(POINTS) * scale;
        if !points.is_finite() || !(8.0..=128.0).contains(&points) {
            return false;
        }
        let key = Key { name, points: points.round() as u16 };
        STATE.with(|state| {
            let Ok(mut state) = state.try_borrow_mut() else { return false };
            if !window.areCursorRectsEnabled() && !state.active.as_ref().is_some_and(|(_, w, _)| *w == window) {
                return false;
            }
            if !state.cache.iter().any(|(k, _)| *k == key) {
                let cursor = build(key);
                if state.cache.len() >= 128 {
                    state.cache.clear();
                }
                state.cache.push((key, cursor));
            }
            let ready = state.cache.iter().any(|(k, c)| *k == key && c.is_some());
            if ready {
                state.pending = Some(key);
            }
            ready
        })
    }

    /// winit stores a system cursor in a cursor rectangle, even when NSCursor::set was called.
    /// Suspend those rectangles for the canvas lease; restore on leave/focus loss/overlay. This
    /// avoids setting the cursor every frame and prevents the rectangle restoring the old arrow.
    pub(crate) fn end_frame(ctx: &egui::Context) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let window = NSApplication::sharedApplication(mtm).keyWindow();
        let allowed = ctx.input(|i| i.focused && i.pointer.hover_pos().is_some()) && ctx.output(|o| o.cursor_icon == egui::CursorIcon::Default);
        STATE.with(|state| {
            let Ok(mut state) = state.try_borrow_mut() else { return };
            let desired = state.pending.filter(|_| allowed && window.is_some());
            let same = state.active.as_ref().is_some_and(|(k, w, _)| Some(*k) == desired && Some(w) == window.as_ref());
            if same {
                // Native menus/resize tracking can replace the actual cursor independently.
                // Reassert only when AppKit's cursor changed, never on an unchanged frame.
                if let Some((_, _, cursor)) = state.active.as_ref()
                    && *cursor != NSCursor::currentCursor()
                {
                    cursor.set();
                }
                return;
            }
            if let Some((_, old, _cursor)) = state.active.take() {
                old.enableCursorRects();
                old.resetCursorRects();
            }
            if let (Some(key), Some(window)) = (desired, window) {
                let cursor = state.cache.iter().find(|(k, _)| *k == key).and_then(|(_, c)| c.clone());
                if let Some(cursor) = cursor {
                    // Don't take ownership of another component's suspension (e.g. a native sheet).
                    if window.areCursorRectsEnabled() {
                        window.disableCursorRects();
                        cursor.set();
                        state.active = Some((key, window, cursor));
                    }
                }
            }
        });
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn retina_cursor_has_one_and_two_x_representations_in_points() {
            // NSImage/NSCursor construction creates no window and does not launch the app.
            for &(name, _) in FILES {
                let cursor = build(Key { name, points: 32 }).expect("Retina cursor");
                let image = cursor.image();
                assert_eq!(image.size(), NSSize::new(32.0, 32.0));
                let representations = image.representations();
                assert_eq!(representations.len(), 2);
                let mut pixels: Vec<_> = representations
                    .iter()
                    .map(|rep| {
                        assert_eq!(rep.size(), NSSize::new(32.0, 32.0));
                        (rep.pixelsWide(), rep.pixelsHigh())
                    })
                    .collect();
                pixels.sort_unstable();
                assert_eq!(pixels, [(32, 32), (64, 64)]);
                let [x, y] = hotspot(name).unwrap();
                assert_eq!(cursor.hotSpot(), NSPoint::new(f64::from(x), f64::from(y)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAPPED: &[Cursor] = &[
        Cursor::Arrow,
        Cursor::ArrowHollow,
        Cursor::DirectAnchor,
        Cursor::DirectAnchorSelected,
        Cursor::ReshapeSegment,
        Cursor::SelectionCopy,
        Cursor::Move,
        Cursor::Pen,
        Cursor::PenDrawing,
        Cursor::PenAdd,
        Cursor::PenDelete,
        Cursor::PenClose,
        Cursor::PenContinue,
        Cursor::PenJoin,
        Cursor::PenConvert,
        Cursor::AnchorPoint,
        Cursor::Eyedropper,
        Cursor::ResizeH,
        Cursor::ResizeV,
        Cursor::ResizeNeSw,
        Cursor::ResizeNwSe,
        Cursor::Hand,
        Cursor::HandGrab,
        Cursor::ZoomIn,
        Cursor::ZoomOut,
        Cursor::NotAllowed,
        Cursor::Rotate,
    ];

    #[test]
    fn every_mapped_variant_rasterizes_at_one_and_two_x_with_hotspot_inside() {
        let mut cases: Vec<_> = MAPPED.iter().map(|&c| (c, "selection")).collect();
        cases.extend((0..8).map(|o| (Cursor::RotateToward(o), "selection")));
        cases.extend(["rectangle", "roundedRectangle", "ellipse", "polygon", "rotate", "scale", "artboard"].map(|tool| (Cursor::Crosshair, tool)));
        cases.push((Cursor::Move, "artboard"));
        for (cursor, tool) in cases {
            assert!(file(cursor, tool).is_some(), "{cursor:?}/{tool}");
            for ppp in [1.0, 2.0] {
                let bitmap = image(cursor, tool, ppp).expect("mapped cursor rasterizes");
                let px = (32.0 * ppp) as u16;
                assert_eq!(bitmap.size, [px, px]);
                assert_eq!(bitmap.rgba.len(), usize::from(px).pow(2) * 4);
                assert!(bitmap.hotspot.iter().all(|&h| h < px), "{cursor:?}/{tool}/{ppp}");
                assert!(bitmap.rgba.as_chunks::<4>().0.iter().any(|p| p[3] == 255), "visible ink");
                assert_eq!(bitmap.hotspot, hotspot(file(cursor, tool).unwrap()).unwrap().map(|h| (f32::from(h) * ppp) as u16));
            }
        }
    }

    #[test]
    fn all_embedded_assets_match_the_manifest_and_render() {
        let json: serde_json::Value = serde_json::from_str(HOTSPOTS).unwrap();
        assert_eq!(FILES.len(), json["files"].as_object().unwrap().len());
        for &(name, _) in FILES {
            for px in [32, 64] {
                assert!(rasterize(name, px).is_some(), "{name} at {px}");
            }
        }
    }

    #[test]
    fn cached_rgba_is_shared_and_sizes_are_bounded() {
        let a = image(Cursor::Pen, "pen", 1.0).unwrap();
        let b = image(Cursor::Pen, "pen", 1.0).unwrap();
        assert!(std::sync::Arc::ptr_eq(&a.rgba, &b.rgba));
        let retina = image(Cursor::Pen, "pen", 2.0).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&a.rgba, &retina.rgba));
        for ppp in [0.0, -1.0, f32::NAN, f32::INFINITY, 100.0] {
            assert!(image(Cursor::Pen, "pen", ppp).is_none());
        }
        assert!(rasterize("missing.svg", 32).is_none());
        assert!(rasterize("pen.svg", 0).is_none());
        assert!(rasterize("pen.svg", 257).is_none());
    }

    #[test]
    fn unmatched_and_precise_states_keep_the_upstream_glyph() {
        for c in [
            Cursor::Text,
            Cursor::TypeVertical,
            Cursor::Curvature,
            Cursor::GroupSelection,
            Cursor::ShearH,
            Cursor::Width,
            Cursor::Scissors,
            Cursor::ShapeBuilder,
        ] {
            assert!(file(c, "").is_none(), "{c:?}");
        }
        assert!(file(Cursor::Crosshair, "").is_none(), "precise mode strips the tool badge");
        assert!(file(Cursor::Crosshair, "star").is_none(), "don't show an incorrect polygon badge");
        assert_ne!(file(Cursor::Crosshair, "rectangle"), file(Cursor::Crosshair, "ellipse"));
        assert_eq!(file(Cursor::Move, "artboard"), Some("artboard.svg"));
        assert_eq!(file(Cursor::Move, ""), Some("select.svg"));
        assert_eq!(file(Cursor::Move, "selection"), Some("select-object.svg"));
        assert_eq!(file(Cursor::Arrow, "selection"), Some("select.svg"), "moving drops the hover badge");
        assert_ne!(
            file(Cursor::DirectAnchor, "direct"),
            file(Cursor::DirectAnchorSelected, "direct"),
            "anchor selection keeps the hollow/filled badge distinction"
        );
        assert_eq!(file(Cursor::RotateToward(255), ""), file(Cursor::RotateToward(7), ""));
    }
}
