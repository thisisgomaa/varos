//! Varos: the font menu's Arabic side (provisional UI, awaiting the owner's review).
//!
//! Concept A: script-first filtering and Auto / Latin / Arabic sample modes are presented by
//! [`super::font_filters`]. Arabic styles appear only with the Arabic script selected.
//! * Arabic samples and names are set by the real text pipeline (bidi, HarfRust shaping, marks)
//!   in the row's own face, drawn right-aligned and cut at their left end. egui's own text has no
//!   Arabic shaping, so Arabic is never drawn with it here.
//! * Beside a family's Latin name: its Arabic name from the font's name table, when it has one,
//!   and an **AAT** marker on families HarfRust shapes with their `morx` tables
//!   ([`af::ArabicSupport::morx_shaped`]: any font with `morx`, Damascus included), once the family
//!   is loaded (its sample was set, or the filter checked it).
//!
//! Samples are textures cached by the font menu by (family, style, text, size): the text tells
//! the script, so an Arabic and a Latin sample of one face are separate entries.

use egui::{Color32, Ui, vec2};
use vectorcraft_text::FontDb;
use vectorcraft_text::arabic_fonts::{self as af, ArabicStyle};

use crate::theme::Tokens;

/// The Arabic filter and sample choices of one font menu (kept from one opening to the next, as
/// upstream's filters are).
#[derive(Clone, Debug, Default)]
pub(crate) struct ArabicMenu {
    /// List only the families that set Arabic.
    pub only: bool,
    /// With [`Self::only`]: one best-effort style group (`None`: all Arabic families).
    pub style: Option<ArabicStyle>,
    /// Sample language, independent of the family filter.
    pub samples: SampleMode,
}

/// Sample language, independent of the family filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum SampleMode {
    #[default]
    Auto,
    Latin,
    Arabic,
}

/// What the rows depend on beyond upstream's key: the filter. What loading fonts tells (a family
/// whose letters don't join) arrives with the font list's generation, which the key holds.
pub(crate) type FilterKey = (bool, Option<ArabicStyle>);

/// The labels are English source strings translated through the upstream catalogs (rows in each
/// `i18n/*.tsv`, under "Varos: Arabic font menu").
pub(super) fn style_label(s: ArabicStyle) -> &'static str {
    match s {
        ArabicStyle::Naskh => tl!("Naskh"),
        ArabicStyle::Kufi => tl!("Kufi"),
        ArabicStyle::Ruqaa => tl!("Ruq’ah"),
        ArabicStyle::Diwani => tl!("Diwani"),
        ArabicStyle::Thuluth => tl!("Thuluth"),
        ArabicStyle::ModernSans => tl!("Modern Sans"),
        ArabicStyle::Unknown => tl!("All Arabic"),
    }
}

impl ArabicMenu {
    pub fn key(&self) -> FilterKey {
        (self.only, self.style)
    }

    /// Does `family` pass the Arabic filter? Cheap: no font is loaded.
    pub fn wanted(&self, db: &FontDb, family: &str) -> bool {
        !self.only || db.may_set_arabic(family) && self.style.is_none_or(|s| db.arabic_style(family) == s)
    }

    /// The sample a row of `family` shows, given the selected text `selected`: `None` for none,
    /// else the text (`None`: upstream's own choice, the selected text or its generic sample) and
    /// whether it is set right to left.
    pub fn row_sample(&self, db: &FontDb, family: &str, selected: Option<&str>) -> Option<(Option<String>, bool)> {
        if self.samples == SampleMode::Latin {
            // A pinned Latin sample must not inherit Japanese, Thai or any other selected script.
            return Some((None, false));
        }
        if self.samples == SampleMode::Auto
            && let Some(text) = selected.filter(|s| af::is_arabic_text(s))
        {
            // Set by `render` in the face itself, or no sample when the face can't.
            return Some((Some(text.to_string()), true));
        }
        if self.samples == SampleMode::Arabic && db.may_set_arabic(family) {
            return Some((Some(af::ARABIC_SAMPLE.to_string()), true));
        }
        Some((selected.filter(|s| !af::is_arabic_text(s)).map(str::to_string), false))
    }
}

/// Longest sample, in pixels per pixel of height (as the font menu's own samples).
const MAX_ASPECT: f32 = 10.0;

/// The font menu's sample renderer for Arabic `text`: set right to left in `family`'s `style`
/// face through the real layout, `px` pixels high, as white with coverage in alpha, its start
/// (right end) kept when it is too long. `None` when the face can't set it (no Arabic, letters
/// that don't join, a character it lacks): a row then shows no sample. [`af::ARABIC_SAMPLE`]
/// falls back to its plain letters in a face without harakat.
pub(crate) fn render(family: &str, style: &str, text: &str, px: u32) -> Option<egui::ColorImage> {
    render_in(FontDb::global(), family, style, text, px)
}

/// [`render`] with the faces of `db`.
pub(crate) fn render_in(db: &FontDb, family: &str, style: &str, text: &str, px: u32) -> Option<egui::ColorImage> {
    // The family itself, by any of its names (the menu lists SF Arabic, whose faces are named
    // .SF Arabic), never the fallback face.
    let (canonical, _) = db.canonical(family, "");
    let face = db.face(family, style).filter(|f| f.family.eq_ignore_ascii_case(family) || f.family.eq_ignore_ascii_case(&canonical))?;
    if !af::face_arabic_support(&face).capable() {
        return None;
    }
    let text = if text == af::ARABIC_SAMPLE { af::arabic_sample_for(&face)? } else { text };
    let (asc, desc) = face.vertical_metrics();
    let em = (asc + desc).max(1.0) / face.units_per_em();
    // The em box fills the height with a little air (as the Latin samples).
    let size = px as f64 * 0.8 / em;
    let layout = af::arabic_sample_layout(db, &face, text, size)?;
    let mut path = layout.to_bezpath();
    let b = layout.bounds;
    if !(b.width().is_finite() && size.is_finite()) {
        return None;
    }
    let w = (b.width().ceil() as u32 + 2).clamp(1, (px as f32 * MAX_ASPECT) as u32) as usize;
    let h = px as usize;
    let baseline = px as f64 * 0.1 + asc / face.units_per_em() * size;
    // The right end at the texture's right edge: a long sample loses its end, not its start.
    path.apply_affine(vectorcraft_geom::Affine::translate((w as f64 - 1.0 - b.x1, baseline)));
    let mask = crate::panels::glyphs::rasterize(&path, w, h);
    Some(egui::ColorImage::new([w, h], mask.iter().map(|&a| Color32::from_rgba_premultiplied(a, a, a, a)).collect()))
}

/// Draw right-to-left sample `tex` (tinted `tint`) at the right of `area`, centred vertically,
/// cut at its left.
pub(crate) fn paint_rtl(ui: &Ui, area: egui::Rect, tex: &egui::TextureHandle, tint: Color32) {
    let size = tex.size_vec2() / ui.ctx().pixels_per_point();
    let at = egui::Rect::from_min_size(area.right_center() - vec2(size.x, size.y * 0.5), size);
    let shown = at.intersect(area);
    if shown.width() <= 0.0 || size.x <= 0.0 {
        return;
    }
    let uv = egui::Rect::from_min_max(egui::pos2((1.0 - shown.width() / size.x).clamp(0.0, 1.0), 0.0), egui::pos2(1.0, 1.0));
    ui.painter_at(area).image(tex.id(), shown, uv, tint);
}

/// Height (points) of a family's Arabic name drawn beside its Latin one.
const NAME_HEIGHT: f32 = 16.0;

/// A row's name in `name_rect`: `name` (upstream's label), set by the text pipeline when it is
/// Arabic (Show Font Names in English off), and for a family row its Arabic name and AAT marker
/// at the right end of the name column. `sample(text, height)` gives the texture of `text` set in
/// the row's family (the font menu's sample cache). With `check` (the Arabic filter is on), a
/// family row whose joining isn't known yet is checked ([`check_joining`]).
pub(crate) fn paint_name(
    ui: &Ui,
    name_rect: egui::Rect,
    name: &str,
    family: Option<&str>,
    check: bool,
    sample: &dyn Fn(&str, f32) -> Option<egui::TextureHandle>,
) {
    let t = Tokens::get(ui.ctx());
    let painter = ui.painter_at(name_rect);
    let mut right = name_rect.right();
    let db = FontDb::global();
    if check && let Some(f) = family {
        check_joining(ui.ctx(), f);
    }
    // The AAT shaping marker, at the right end: HarfRust shapes the family with its morx tables.
    if let Some(f) = family
        && db.known_arabic_support(f).is_some_and(af::ArabicSupport::morx_shaped)
    {
        let r = painter.text(name_rect.right_center(), egui::Align2::RIGHT_CENTER, "AAT", egui::FontId::proportional(9.0), t.text_dim);
        let id = ui.id().with(("varos-aat", f));
        ui.interact(r, id, egui::Sense::hover())
            .on_hover_text(tl!("AAT shaping: this font is shaped with its own morx tables, not OpenType Arabic rules"));
        right = r.left() - 4.0;
    }
    let left_end = if af::is_arabic_text(name) {
        match sample(name, NAME_HEIGHT) {
            Some(tex) => {
                let size = tex.size_vec2() / ui.ctx().pixels_per_point();
                let area = egui::Rect::from_min_max(name_rect.min, egui::pos2(right, name_rect.max.y));
                let at = egui::Rect::from_min_size(area.left_center() - vec2(0.0, size.y * 0.5), size).intersect(area);
                let uv = egui::Rect::from_min_max(egui::pos2((1.0 - at.width() / size.x.max(1.0)).clamp(0.0, 1.0), 0.0), egui::pos2(1.0, 1.0));
                ui.painter_at(area).image(tex.id(), at, uv, t.text);
                return;
            }
            // Until its texture is ready (or when its own face can't set it): nothing rather than
            // unjoined letters.
            None => return,
        }
    } else {
        painter.text(name_rect.left_center(), egui::Align2::LEFT_CENTER, name, egui::FontId::proportional(12.0), t.text).right()
    };
    // The family's Arabic name, when it fits after the Latin one.
    if let Some(arabic) = family.and_then(|f| arabic_name(db, f))
        && let Some(tex) = sample(&arabic, NAME_HEIGHT)
    {
        let size = tex.size_vec2() / ui.ctx().pixels_per_point();
        if right - size.x >= left_end + 6.0 {
            let at = egui::Rect::from_min_size(egui::pos2(right - size.x, name_rect.center().y - size.y * 0.5), size);
            painter.image(tex.id(), at, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), t.text_dim);
        }
    }
}

/// Rows checked for joining in one frame, at most (each loads its family's font files).
const CHECKS_PER_FRAME: usize = 2;

/// With the Arabic filter on, check whether `family`'s letters join when that isn't known yet:
/// load the family (off the UI thread on native, a few per frame), so its support becomes known
/// ([`FontDb::known_arabic_support`]) and the font list's generation moves, which lists the rows
/// again: a family whose letters don't join leaves them. Cost: the rows in view only, each once.
/// The previews do the same loading when they are on; this keeps the filter honest when they are
/// off.
pub(crate) fn check_joining(ctx: &egui::Context, family: &str) {
    use std::sync::Mutex;
    static STARTED: Mutex<(u64, usize, Vec<String>)> = Mutex::new((0, 0, Vec::new()));
    let db = FontDb::global();
    if !db.family_traits(family).arabic || db.known_arabic_support(family).is_some() {
        return;
    }
    {
        let mut started = STARTED.lock().unwrap_or_else(|e| e.into_inner());
        let (pass, count, names) = &mut *started;
        if names.iter().any(|n| n.eq_ignore_ascii_case(family)) {
            return;
        }
        let now = ctx.cumulative_pass_nr();
        if *pass != now {
            (*pass, *count) = (now, 0);
        }
        if *count >= CHECKS_PER_FRAME {
            ctx.request_repaint();
            return;
        }
        *count += 1;
        if names.len() > 8192 {
            names.clear();
        }
        names.push(family.to_string());
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (f, repaint) = (family.to_string(), ctx.clone());
        // A failed spawn leaves the family unchecked (its hint stands), as a failed sample does.
        let _ = std::thread::Builder::new().name("arabic-check".into()).spawn(move || {
            db.arabic_support(&f);
            repaint.request_repaint();
        });
    }
    // The web has no threads and no installed fonts: the loaded ones are checked here.
    #[cfg(target_arch = "wasm32")]
    {
        db.arabic_support(family);
    }
}

/// [`FontDb::arabic_family_name`], kept per family until the fonts change: the rows ask every
/// frame, and the lookup walks the loaded faces.
fn arabic_name(db: &FontDb, family: &str) -> Option<String> {
    type Names = (u64, std::collections::HashMap<String, Option<String>>);
    thread_local! {
        static NAMES: std::cell::RefCell<Names> = std::cell::RefCell::default();
    }
    NAMES.with(|n| {
        let mut n = n.borrow_mut();
        let generation = db.generation();
        if n.0 != generation || n.1.len() > 8192 {
            *n = (generation, Default::default());
        }
        n.1.entry(family.to_string()).or_insert_with(|| db.arabic_family_name(family)).clone()
    })
}

#[cfg(test)]
#[path = "tests_arabic_fonts.rs"]
mod tests;
