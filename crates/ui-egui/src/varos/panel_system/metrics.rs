//! Shared logical sizes for panel chrome and its hosted tool controls.
//! Existing application zoom scales these points; no separate density setting is introduced.

pub(crate) const TITLE: f32 = 20.0;
// Match tab height to the application's standard 24-point controls.
pub(crate) const TABS: f32 = 24.0;
pub(crate) const ICON: f32 = TOOL_ICON;
pub(crate) const CONTROL: f32 = 14.0;
pub(crate) const FOOT: f32 = 6.0;
pub(crate) const ROW: f32 = TOOL_BUTTON;
pub(crate) const GRIP: f32 = 4.0;
pub(crate) const COMPACT_PAD: f32 = 0.0;
pub(crate) const COMPACT: f32 = 44.0;
/// A compact column at least this wide shows panel names beside their icons.
pub(crate) const NAMES_MIN_WIDTH: f32 = 72.0;

pub(crate) const FONT: f32 = 12.0;
pub(crate) const TAB_PAD: f32 = 10.0;
pub(crate) const ROW_PAD: f32 = 8.0;
pub(crate) const ICON_LABEL_GAP: f32 = 10.0;
pub(crate) const FRAME_RADIUS: u8 = 6;
pub(crate) const BORDER: f32 = 1.0;
/// One header control slot (panel menu, tab overflow, flyout close) at the right of a group header.
pub(crate) const HEADER_CONTROL: f32 = 24.0;
/// The accent line that marks a drop target.
pub(crate) const DROP_LINE: f32 = 2.0;

// One glyph and row scale for both tool and panel identities.
pub(crate) const TOOL_BUTTON: f32 = 28.0;
pub(crate) const TOOL_ICON: f32 = 16.0;
pub(crate) const TOOL_GAP: f32 = 2.0;
pub(crate) const TOOL_GROUP_GAP: f32 = 12.0;
pub(crate) const TOOL_PAINT_PROXY: f32 = 60.0;
pub(crate) const TOOL_PAINT_INSET: f32 = 6.0;
pub(crate) const TOOL_PAINT_CHIP: f32 = 16.0;
pub(crate) const TOOL_PAINT_HIT: f32 = 20.0;
pub(crate) const TOOL_PAINT_GAP: f32 = 2.0;

/// Full Tools group widths, including host borders, in logical points.
pub(crate) const TOOLS_MIN_WIDTH: f32 = 44.0;
pub(crate) const TOOLS_MAX_WIDTH: f32 = 80.0;

/// Inline drag strip for the horizontal Control bar; no separate header or tab row.
pub(crate) const CONTROL_GRIP: f32 = 12.0;
pub(crate) const CONTROL_INITIAL_HEIGHT: f32 = 34.0;
