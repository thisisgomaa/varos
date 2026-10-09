//! Adapted from VectorCraft doc/src/appearance.rs@a469568 (MIT OR Apache-2.0).
//! Lane B / S1: a read-only appearance facade. No persisted keys or duplicated paint storage.
//! The implicit stack paints the existing base fill followed by the existing base stroke.
use crate::{
    model::{Paint, Path},
    stroke::StrokeStyle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseSlot {
    Fill,
    Stroke,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntryOpts {
    pub opacity: f32,
    pub visible: bool,
}
impl Default for EntryOpts {
    fn default() -> Self {
        Self { opacity: 1.0, visible: true }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub opacity: f32,
    pub isolate: bool,
}
impl Default for Look {
    fn default() -> Self {
        Self { opacity: 1.0, isolate: false }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum StackItem<'a> {
    Base(BaseSlot, EntryOpts),
    Fill { paint: &'a Paint, opts: EntryOpts },
    Stroke { paint: &'a Paint, style: &'a StrokeStyle, opts: EntryOpts },
}

#[derive(Clone, Copy)]
pub struct Appearance<'a> {
    path: &'a Path,
}
impl Path {
    pub fn appearance(&self) -> Appearance<'_> {
        Appearance { path: self }
    }
}
impl<'a> Appearance<'a> {
    pub fn fill(self) -> &'a Paint {
        &self.path.fill
    }
    pub fn stroke(self) -> &'a Paint {
        &self.path.stroke
    }
    pub fn paint(self, slot: BaseSlot) -> &'a Paint {
        match slot {
            BaseSlot::Fill => self.fill(),
            BaseSlot::Stroke => self.stroke(),
        }
    }
    pub fn stack(self) -> [StackItem<'a>; 2] {
        [StackItem::Base(BaseSlot::Fill, EntryOpts::default()), StackItem::Base(BaseSlot::Stroke, EntryOpts::default())]
    }
    pub fn look(self) -> Look {
        Look { opacity: self.path.opacity, isolate: false }
    }
    pub fn stroke_style(self) -> &'a StrokeStyle {
        &self.path.stroke_style
    }
}
