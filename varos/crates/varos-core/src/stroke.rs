//! Authored stroke style, independent of UI and rendering. Wire spellings are format-5 law.
use serde::{Deserialize, Serialize};

macro_rules! wire_enum {
    ($name:ident, $default:ident, $($other:ident),* $(,)?) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { #[default] $default, $($other),* }
        impl $name { fn is_default(&self) -> bool { *self == Self::default() } }
    };
}
wire_enum!(StrokeCap, Round, Butt, Square);
wire_enum!(StrokeJoin, Round, Miter, Bevel);
wire_enum!(StrokeAlign, Center, Inside, Outside);
wire_enum!(ArrowAlign, Tip, Extend);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrowHead {
    Triangle,
    TriangleOpen,
    Circle,
    CircleOpen,
    Square,
    SquareOpen,
    Bar,
    Diamond,
    Arrow,
    ArrowOpen,
    Barbed,
    HalfArrowLeft,
    HalfArrowRight,
    Concave,
    DoubleBar,
    Feather,
    DotOnBar,
    Chevron,
    DoubleArrow,
    Target,
    Star,
    Cross,
    Plus,
    Hexagon,
    HexagonOpen,
    Tag,
    TagOpen,
    HalfCircle,
    Drop,
}
impl ArrowHead {
    pub const ALL: [Self; 29] = [
        Self::Triangle,
        Self::TriangleOpen,
        Self::Circle,
        Self::CircleOpen,
        Self::Square,
        Self::SquareOpen,
        Self::Bar,
        Self::Diamond,
        Self::Arrow,
        Self::ArrowOpen,
        Self::Barbed,
        Self::HalfArrowLeft,
        Self::HalfArrowRight,
        Self::Concave,
        Self::DoubleBar,
        Self::Feather,
        Self::DotOnBar,
        Self::Chevron,
        Self::DoubleArrow,
        Self::Target,
        Self::Star,
        Self::Cross,
        Self::Plus,
        Self::Hexagon,
        Self::HexagonOpen,
        Self::Tag,
        Self::TagOpen,
        Self::HalfCircle,
        Self::Drop,
    ];
}
fn ten() -> f32 {
    10.0
}
fn one() -> f32 {
    1.0
}
fn is_ten(v: &f32) -> bool {
    *v == 10.0
}
fn is_one(v: &f32) -> bool {
    *v == 1.0
}
fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
fn is_false(v: &bool) -> bool {
    !v
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StrokeArrows {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<ArrowHead>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<ArrowHead>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale_start: f32,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale_end: f32,
    #[serde(skip_serializing_if = "ArrowAlign::is_default")]
    pub align: ArrowAlign,
}
impl Default for StrokeArrows {
    fn default() -> Self {
        Self { start: None, end: None, scale_start: 1.0, scale_end: 1.0, align: ArrowAlign::Tip }
    }
}
impl StrokeArrows {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StrokeStyle {
    #[serde(skip_serializing_if = "StrokeCap::is_default")]
    pub cap: StrokeCap,
    #[serde(skip_serializing_if = "StrokeJoin::is_default")]
    pub join: StrokeJoin,
    #[serde(default = "ten", skip_serializing_if = "is_ten")]
    pub miter_limit: f32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dash: Vec<f32>,
    #[serde(skip_serializing_if = "is_zero")]
    pub dash_phase: f32,
    #[serde(skip_serializing_if = "is_false")]
    pub align_dashes_to_corners: bool,
    #[serde(skip_serializing_if = "StrokeAlign::is_default")]
    pub align: StrokeAlign,
    #[serde(skip_serializing_if = "StrokeArrows::is_default")]
    pub arrows: StrokeArrows,
}
impl Default for StrokeStyle {
    fn default() -> Self {
        Self {
            cap: StrokeCap::Round,
            join: StrokeJoin::Round,
            miter_limit: 10.0,
            dash: vec![],
            dash_phase: 0.0,
            align_dashes_to_corners: false,
            align: StrokeAlign::Center,
            arrows: StrokeArrows::default(),
        }
    }
}
impl StrokeStyle {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Same checked bounds for edit, load and save; errors identify the object and authored field.
    pub fn validate(&self, id: u32) -> Result<(), crate::format::Invalid> {
        use crate::format::Invalid;
        let range = |field: &str, v: f32, lo: f32, hi: f32| {
            let what = format!("path {id} stroke_style.{field}");
            if !v.is_finite() {
                Err(Invalid::NonFinite { what })
            } else if !(lo..=hi).contains(&v) {
                Err(Invalid::OutOfRange { what, value: f64::from(v) })
            } else {
                Ok(())
            }
        };
        range("miter_limit", self.miter_limit, 1.0, 1000.0)?;
        range("dash_phase", self.dash_phase, -1_000_000.0, 1_000_000.0)?;
        if ![0, 2, 4, 6].contains(&self.dash.len()) {
            return Err(Invalid::OutOfRange {
                what: format!("path {id} stroke_style.dash pair count"),
                value: self.dash.len() as f64,
            });
        }
        for (i, &v) in self.dash.iter().enumerate() {
            range(&format!("dash[{i}]"), v, if v == 0.0 { 0.0 } else { 0.0001 }, 1_000_000.0)?;
        }
        for (i, pair) in self.dash.as_chunks::<2>().0.iter().enumerate() {
            if pair[0] + pair[1] == 0.0 {
                return Err(Invalid::OutOfRange {
                    what: format!("path {id} stroke_style.dash pair {i} sum"),
                    value: 0.0,
                });
            }
        }
        range("arrows.scale_start", self.arrows.scale_start, 0.01, 100.0)?;
        range("arrows.scale_end", self.arrows.scale_end, 0.01, 100.0)
    }
    /// Expanded projection for opt-in Bridge 1.2, separate from the compact persisted encoding.
    pub fn expanded(&self) -> serde_json::Value {
        serde_json::json!({"cap":self.cap,"join":self.join,"miter_limit":self.miter_limit,
            "dash":self.dash,"dash_phase":self.dash_phase,"align_dashes_to_corners":self.align_dashes_to_corners,
            "align":self.align,"arrows":{"start":self.arrows.start,"end":self.arrows.end,
            "scale_start":self.arrows.scale_start,"scale_end":self.arrows.scale_end,"align":self.arrows.align}})
    }
}
pub mod evaluate;
pub mod heads;
pub use evaluate::{evaluate, StrokeCoverage, StrokeError};

/// Apply just the fields changed by one inspector gesture to each selected object's own style.
pub fn apply_difference(base: &StrokeStyle, next: &StrokeStyle, target: &mut StrokeStyle) {
    macro_rules! update { ($($field:ident),*) => {$(if base.$field != next.$field {target.$field=next.$field;})*}; }
    update!(cap, join, miter_limit, dash_phase, align_dashes_to_corners, align);
    if base.dash.len() != next.dash.len() {
        // Enabling/disabling the pattern is a whole-vector operation.
        target.dash = next.dash.clone();
    } else {
        for (index, (before, after)) in base.dash.iter().zip(&next.dash).enumerate() {
            if before != after {
                while target.dash.len() <= index {
                    target.dash.extend([6.0, 3.0]);
                }
                target.dash[index] = *after;
            }
        }
    }
    macro_rules! arrow { ($($field:ident),*) => {$(if base.arrows.$field != next.arrows.$field {target.arrows.$field=next.arrows.$field;})*}; }
    arrow!(start, end, scale_start, scale_end, align);
}

pub mod inspection;

/// Canvas-only integration seam: integrator routes this to stroke/canvas.rs (main 9f14e1e).
/// Export evaluators keep their strict budgets. Gradient strokes must use this same seam.
pub fn canvas_seam(
    _editor: &crate::Editor,
    path: &crate::model::Path,
    ppu: f32,
) -> Result<evaluate::StrokeCoverage, evaluate::StrokeError> {
    // Main 9f14e1e: editor.canvas_stroke_cache.lookup(path, editor.doc.unit_xform(path.id), ppu).
    // Bring its cap/backoff/fallback branch with the cache; do not change strict export evaluation.
    evaluate(path, 0.025 / f64::from(ppu.max(0.0001)), &|| false)
}
