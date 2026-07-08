//! Units + the world↔screen coordinate contract (Foundations Stage-0, Decision 3).
//!
//! THE COORDINATE CONTRACT — every system reads this; none reinvents it:
//!  - Geometry in the model is stored in **world units = POINTS (pt)**, 1 pt = 1/72 inch (the SVG /
//!    PDF / PostScript user unit, matching Illustrator's internal unit). Every `Pt`/anchor coord is pt.
//!  - The camera `geom::View { pan, zoom }` maps world→screen: `screen_px = world_pt * zoom + pan`.
//!    At `zoom = 1.0` and 72 ppi, **1 world point == 1 logical screen pixel**. `Editor.ppu` carries
//!    the live pixels-per-unit (= zoom) so grab tolerances stay constant on screen.
//!  - Physical units (mm/cm/in) and device pixels (px) are DERIVED from points via the document's
//!    `ppi` (pixels-per-inch). px is only "real" once a ppi is known; default ppi = 72 (1 pt = 1 px).
//!  - Origin = the document/artboard top-left; +x right, +y down (screen-consistent).
//!
//! This module is pure math + parsing — NO View/Editor coupling — so Transform, Snapping, Rulers,
//! Grid, and the number fields all convert through ONE place. The document's `DocUnits` lives here
//! too (next to the math) and is embedded into the document model when the Artboard system is built.

use serde::{Deserialize, Serialize};

/// A measurement unit a user can type or pick. The internal/world unit is always [`Unit::Pt`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unit {
    Px,
    Pt,
    Pica,
    Mm,
    Cm,
    In,
}

impl Unit {
    /// How many POINTS one of this unit equals, given the document `ppi` (pixels-per-inch).
    /// 1 in = 72 pt; 1 pica = 12 pt; 1 in = 25.4 mm; 1 px = 72/ppi pt.
    pub fn pt_per(self, ppi: f32) -> f32 {
        match self {
            Unit::Pt => 1.0,
            Unit::In => 72.0,
            Unit::Pica => 12.0,
            Unit::Mm => 72.0 / 25.4,
            Unit::Cm => 720.0 / 25.4,
            Unit::Px => 72.0 / ppi,
        }
    }

    /// Short suffix as typed / shown.
    pub fn suffix(self) -> &'static str {
        match self {
            Unit::Px => "px",
            Unit::Pt => "pt",
            Unit::Pica => "pc",
            Unit::Mm => "mm",
            Unit::Cm => "cm",
            Unit::In => "in",
        }
    }

    /// Human-readable name for the Document settings panel (Pain A15).
    pub fn label(self) -> &'static str {
        match self {
            Unit::Px => "Pixels",
            Unit::Pt => "Points",
            Unit::Pica => "Picas",
            Unit::Mm => "Millimeters",
            Unit::Cm => "Centimeters",
            Unit::In => "Inches",
        }
    }

    /// Next display unit in a fixed ring (px → pt → pica → mm → cm → in → px). Drives the
    /// click-to-cycle Units row in the Document settings panel.
    pub fn cycle(self) -> Unit {
        match self {
            Unit::Px => Unit::Pt,
            Unit::Pt => Unit::Pica,
            Unit::Pica => Unit::Mm,
            Unit::Mm => Unit::Cm,
            Unit::Cm => Unit::In,
            Unit::In => Unit::Px,
        }
    }

    /// Parse a unit suffix (case-insensitive); accepts common long forms and `"` for inches.
    pub fn parse_suffix(s: &str) -> Option<Unit> {
        match s.trim().to_ascii_lowercase().as_str() {
            "px" => Some(Unit::Px),
            "pt" | "point" | "points" => Some(Unit::Pt),
            "pc" | "pica" | "picas" => Some(Unit::Pica),
            "mm" => Some(Unit::Mm),
            "cm" => Some(Unit::Cm),
            "in" | "inch" | "inches" | "\"" => Some(Unit::In),
            _ => None,
        }
    }
}

/// Convert a value from one unit to another, given the document `ppi`.
pub fn convert(value: f32, from: Unit, to: Unit, ppi: f32) -> f32 {
    value * from.pt_per(ppi) / to.pt_per(ppi)
}
/// A value expressed in `unit` → world points.
pub fn to_pt(value: f32, unit: Unit, ppi: f32) -> f32 {
    value * unit.pt_per(ppi)
}
/// World points → a value expressed in `unit`.
pub fn from_pt(pt: f32, unit: Unit, ppi: f32) -> f32 {
    pt / unit.pt_per(ppi)
}

/// The document's measurement settings — what the Artboard/Document system (slot 1) owns and
/// serializes. Defined here so the unit math and the document property stay a single source.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocUnits {
    /// Pixels-per-inch: the px↔physical bridge. 72 ⇒ 1 pt == 1 px (the screen default).
    pub ppi: f32,
    /// The unit shown in fields / rulers by default. Geometry stays in pt regardless.
    pub display: Unit,
}
impl Default for DocUnits {
    fn default() -> Self {
        DocUnits { ppi: 72.0, display: Unit::Px }
    }
}

/// Parse a typed field value into WORLD POINTS, honoring an optional explicit unit suffix and
/// otherwise falling back to the document's `display` unit. `None` if there is no number.
///
/// Examples (display = Px, ppi = 72): `"100"` → 100pt · `"10mm"` → 28.346pt · `"2in"` → 144pt ·
/// `"  72 px "` → 72pt · `"-3cm"` → −85.04pt.
pub fn parse_to_pt(input: &str, units: DocUnits) -> Option<f32> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    // Split the leading numeric part (sign / decimal / exponent) from a trailing unit suffix.
    let split = s.find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E'))).unwrap_or(s.len());
    let (num, rest) = s.split_at(split);
    let value: f32 = num.trim().parse().ok()?;
    let unit = if rest.trim().is_empty() { units.display } else { Unit::parse_suffix(rest)? };
    Some(to_pt(value, unit, units.ppi))
}

/// Format world points for display in the document's unit, to `decimals` places, with the suffix.
/// Example: 72pt with `DocUnits{ppi:72, display:Px}` → `"72px"`; with `display:Mm` → `"25.4mm"`.
pub fn format_pt(pt: f32, units: DocUnits, decimals: usize) -> String {
    let v = from_pt(pt, units.display, units.ppi);
    format!("{:.*}{}", decimals, v, units.display.suffix())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-2, "expected {b}, got {a}");
    }

    #[test]
    fn conversions_through_points() {
        close(Unit::In.pt_per(72.0), 72.0);
        close(Unit::Pica.pt_per(72.0), 12.0);
        close(Unit::Mm.pt_per(72.0) * 25.4, 72.0); // 25.4 mm == 1 inch == 72 pt
                                                   // px depends on ppi
        close(to_pt(72.0, Unit::Px, 72.0), 72.0); // 72px @72ppi = 72pt
        close(to_pt(72.0, Unit::Px, 144.0), 36.0); // 72px @144ppi = 36pt
                                                   // cross-unit + round-trip
        close(convert(1.0, Unit::In, Unit::Mm, 72.0), 25.4);
        close(convert(convert(123.0, Unit::Pt, Unit::Cm, 96.0), Unit::Cm, Unit::Pt, 96.0), 123.0);
    }

    #[test]
    fn parse_handles_suffixes_and_fallback() {
        let u = DocUnits::default(); // px display, 72 ppi
        close(parse_to_pt("100", u).unwrap(), 100.0); // bare → display (px@72 = pt)
        close(parse_to_pt("2in", u).unwrap(), 144.0);
        close(parse_to_pt("10mm", u).unwrap(), 28.3465);
        close(parse_to_pt("  72 px ", u).unwrap(), 72.0);
        close(parse_to_pt("-3cm", u).unwrap(), -85.0394);
        close(parse_to_pt("1e2", u).unwrap(), 100.0);
        assert_eq!(parse_to_pt("", u), None);
        assert_eq!(parse_to_pt("abc", u), None);
        assert_eq!(parse_to_pt("5zz", u), None); // unknown unit → None
    }

    #[test]
    fn format_uses_display_unit() {
        assert_eq!(format_pt(72.0, DocUnits { ppi: 72.0, display: Unit::Px }, 0), "72px");
        assert_eq!(format_pt(72.0, DocUnits { ppi: 72.0, display: Unit::Mm }, 1), "25.4mm");
        assert_eq!(format_pt(144.0, DocUnits { ppi: 72.0, display: Unit::In }, 2), "2.00in");
    }

    #[test]
    fn suffix_parse_is_lenient() {
        assert_eq!(Unit::parse_suffix("IN"), Some(Unit::In));
        assert_eq!(Unit::parse_suffix(" inch "), Some(Unit::In));
        assert_eq!(Unit::parse_suffix("pc"), Some(Unit::Pica));
        assert_eq!(Unit::parse_suffix("px"), Some(Unit::Px));
        assert_eq!(Unit::parse_suffix("furlong"), None);
    }

    #[test]
    fn cycle_visits_every_unit_and_returns_home() {
        // Six steps from Px cycle back to Px, touching each unit exactly once.
        let order = [Unit::Px, Unit::Pt, Unit::Pica, Unit::Mm, Unit::Cm, Unit::In];
        let mut u = Unit::Px;
        for expected in order.iter().skip(1).chain(std::iter::once(&Unit::Px)) {
            u = u.cycle();
            assert_eq!(u, *expected);
        }
        // Every unit has a non-empty display label.
        for unit in order {
            assert!(!unit.label().is_empty());
        }
    }

    #[test]
    fn docunits_round_trips_via_serde() {
        let u = DocUnits { ppi: 96.0, display: Unit::Mm };
        let back: DocUnits = serde_json::from_str(&serde_json::to_string(&u).unwrap()).unwrap();
        assert_eq!(u, back);
    }
}
