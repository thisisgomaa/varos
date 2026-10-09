//! Engine-independent request, result and editing identity types.
use crate::{FaceId, Script};
use std::ops::Range;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Auto,
    Ltr,
    Rtl,
}
#[derive(Clone, Debug)]
pub struct Style {
    pub range: Range<usize>,
    pub face: FaceId,
    pub size: f32,
    pub baseline_shift: f32,
    pub paint: u32,
}
#[derive(Clone, Debug)]
pub struct Feature {
    pub tag: [u8; 4],
    pub value: u32,
}
#[derive(Clone, Debug)]
pub struct LanguageRun {
    pub range: Range<usize>,
    pub language: String,
    pub script: Option<Script>,
}
#[derive(Clone, Debug)]
pub struct Request<'a> {
    pub text: &'a str,
    pub direction: Direction,
    pub language: &'a str,
    pub script: Option<Script>,
    pub size: f32,
    pub width: Option<f32>,
    pub face: FaceId,
    pub end_align: bool,
    pub styles: Vec<Style>,
    pub features: Vec<Feature>,
    pub language_runs: Vec<LanguageRun>,
}
impl<'a> Request<'a> {
    pub fn new(text: &'a str, size: f32, width: Option<f32>) -> Self {
        Self {
            text,
            size,
            width,
            direction: Direction::Auto,
            language: "und",
            script: None,
            face: FaceId(0),
            end_align: false,
            styles: vec![],
            features: vec![],
            language_runs: vec![],
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// A virtual hyphen at a selected provider break; authored bytes are unchanged.
    DiscretionaryHyphen {
        byte: usize,
    },
    UnsupportedLanguage(String),
    Substituted {
        range: Range<usize>,
        requested: FaceId,
        actual: FaceId,
    },
    UnsupportedCluster(Range<usize>),
    Overflow(usize),
    /// Requested justification could not consume slack safely.
    JustificationResidual {
        line: usize,
        residual: f32,
    },
    /// Virtual tatweels, never authored bytes.
    KashidaInserted {
        byte: usize,
        count: usize,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub id: u16,
    pub face: FaceId,
    pub cluster: Range<usize>,
    pub level: u8,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
    pub offset: [f32; 2],
    pub size: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Affinity {
    Upstream,
    Downstream,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Caret {
    pub byte: usize,
    pub affinity: Affinity,
    pub line: usize,
    pub x: f32,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub range: Range<usize>,
    pub rtl: bool,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub width: f32,
    pub empty_caret_x: f32,
    pub glyphs: Vec<Glyph>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub source: String,
    pub lines: Vec<Line>,
    pub carets: Vec<Caret>,
    pub issues: Vec<Issue>,
    /// Resolved UBA embedding levels per source byte. Newline bytes retain base level.
    pub levels: Vec<u8>,
    pub ink_bounds: Option<[f32; 4]>,
}
impl Layout {
    pub fn copy(&self, range: Range<usize>) -> Option<&str> {
        self.source.get(range)
    }
    pub fn caret(&self, byte: usize, affinity: Affinity) -> Vec<&Caret> {
        self.carets.iter().filter(|c| c.byte == byte && c.affinity == affinity).collect()
    }
    /// Coincident bidi/control stops are inherently ambiguous. Return all ties;
    /// a host must retain affinity and traversal identity, not pretend x is bijective.
    pub fn hit(&self, line: usize, x: f32) -> Vec<&Caret> {
        let best = self.carets.iter().filter(|c| c.line == line).map(|c| (c.x - x).abs()).fold(f32::INFINITY, f32::min);
        self.carets.iter().filter(|c| c.line == line && ((c.x - x).abs() - best).abs() < 0.001).collect()
    }
}
