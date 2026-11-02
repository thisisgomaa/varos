//! The Align panel's button rows as Varos presents them: smaller buttons, wider even spacing and
//! no extra gap around Horizontal & Vertical Align Center. Upstream's panel (`panels/align.rs`)
//! reads [`LOOK`] through marked hooks; its icons stay upstream's names, which the Varos icon
//! overrides (`icons.rs`) draw.

/// The sizes the Align and Distribute Objects rows use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct AlignLook {
    /// Each button's side, in points.
    pub(crate) button: f32,
    /// The space between neighbouring buttons.
    pub(crate) spacing: f32,
    /// The extra space on each side of Horizontal & Vertical Align Center.
    pub(crate) center_gap: f32,
}

pub(crate) const LOOK: AlignLook = AlignLook { button: 26.0, spacing: 8.0, center_gap: 0.0 };
