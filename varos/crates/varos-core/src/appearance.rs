//! Adapted from VectorCraft doc/src/appearance.rs@a469568 (MIT OR Apache-2.0).
//! Lane A / v10: owned stack entries plus the backwards-compatible base paint facade.
//! The implicit stack paints the existing base fill followed by the existing base stroke.
use crate::{
    model::{Paint, Path},
    stroke::StrokeStyle,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaseSlot {
    Fill,
    Stroke,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EntryOpts {
    pub opacity: f32,
    pub visible: bool,
    pub blend: Blend,
}
impl Default for EntryOpts {
    fn default() -> Self {
        Self { opacity: 1.0, visible: true, blend: Blend::Normal }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Look {
    pub opacity: f32,
    pub isolate: bool,
}
impl Default for Look {
    fn default() -> Self {
        Self { opacity: 1.0, isolate: false }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum StackItem {
    Base(BaseSlot, EntryOpts),
    Fill { paint: Paint, opts: EntryOpts },
    Stroke { paint: Paint, width: f32, style: StrokeStyle, opts: EntryOpts },
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
    pub fn stack(self) -> Vec<StackItem> {
        if self.path.stack.is_empty() {
            base_stack()
        } else {
            self.path.stack.clone()
        }
    }
    pub fn look(self) -> Look {
        Look { opacity: self.path.opacity, isolate: false }
    }
    pub fn stroke_style(self) -> &'a StrokeStyle {
        &self.path.stroke_style
    }
}

// ---- Lane A: persisted appearance, normal blend only ----
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blend {
    #[default]
    Normal,
}
pub fn base_stack() -> Vec<StackItem> {
    vec![StackItem::Base(BaseSlot::Fill, EntryOpts::default()), StackItem::Base(BaseSlot::Stroke, EntryOpts::default())]
}
impl StackItem {
    pub fn opts(&self) -> &EntryOpts {
        match self {
            Self::Base(_, opts) | Self::Fill { opts, .. } | Self::Stroke { opts, .. } => opts,
        }
    }
    pub fn opts_mut(&mut self) -> &mut EntryOpts {
        match self {
            Self::Base(_, opts) | Self::Fill { opts, .. } | Self::Stroke { opts, .. } => opts,
        }
    }
}
