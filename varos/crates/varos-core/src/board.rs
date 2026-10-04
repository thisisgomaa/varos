//! Board metadata (Start page v2, work order `START_V2_BOARDS.md` lane L2): a document IS a board — a
//! free canvas with a name, a short description and tags. This module holds the one set of rules for
//! that metadata (bounds, cleaning, validation), the one display-name rule every surface uses, and the
//! one table of new-board presets. Pure: no I/O, no UI.
//!
//! **Text rules (product behaviour, `docs/reference/VRS_FORMAT.md` §6b):**
//! - Bounds count Unicode scalar values (`char`s), NOT grapheme clusters: Arabic and Latin share one
//!   budget, and an emoji ZWJ sequence counts every scalar in it.
//! - Control characters (general category Cc: line breaks, tabs, NUL…) are refused.
//! - Format characters are ALLOWED inside text: ZWNJ/ZWJ (U+200C/U+200D — Arabic and Persian need
//!   them for correct joining) and the bidi marks/embeddings/isolates. They are trimmed only at the
//!   EDGES, together with whitespace ([`clean_text`]).
//! - Text is stored as typed (UTF-8); NFC normalization is not required for storage. Tags COMPARE
//!   through [`fold`] (NFC + full case fold) everywhere: dedupe, Start's tag filter, counts, search.

use crate::model::{Artboard, Document};
use crate::units::Unit;
use std::fmt;
use std::path::Path;

/// Longest board name, in characters.
pub const MAX_NAME_CHARS: usize = 120;
/// Longest board description, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 500;
/// Most tags on one board.
pub const MAX_TAGS: usize = 16;
/// Longest tag, in characters.
pub const MAX_TAG_CHARS: usize = 32;

/// Why board metadata was refused. `Display` is the plain-English reason (no internal names).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaError {
    /// The name is longer than [`MAX_NAME_CHARS`].
    NameTooLong { found: usize },
    /// The description is longer than [`MAX_DESCRIPTION_CHARS`].
    DescriptionTooLong { found: usize },
    /// More than [`MAX_TAGS`] tags.
    TooManyTags { found: usize },
    /// One tag is longer than [`MAX_TAG_CHARS`].
    TagTooLong { tag: String, found: usize },
    /// A control character (line break, tab, …) in the named field.
    ControlCharacter { field: &'static str },
    /// A stored tag that is empty or has spaces at either end (only possible in a hand-edited file:
    /// the editor cleans tags before storing them).
    UncleanTag { tag: String },
    /// Two stored tags that differ only by letter case (hand-edited file only, as above).
    DuplicateTag { tag: String },
}

impl fmt::Display for MetaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetaError::NameTooLong { found } => {
                write!(f, "the board name is {found} characters long; the limit is {MAX_NAME_CHARS}")
            }
            MetaError::DescriptionTooLong { found } => {
                write!(f, "the board description is {found} characters long; the limit is {MAX_DESCRIPTION_CHARS}")
            }
            MetaError::TooManyTags { found } => write!(f, "the board has {found} tags; the limit is {MAX_TAGS}"),
            MetaError::TagTooLong { tag, found } => {
                write!(f, "the tag “{tag}” is {found} characters long; the limit is {MAX_TAG_CHARS}")
            }
            MetaError::ControlCharacter { field } => {
                write!(f, "the board {field} contains a line break or another control character")
            }
            MetaError::UncleanTag { tag } => write!(f, "the tag “{tag}” is empty or has spaces at its ends"),
            MetaError::DuplicateTag { tag } => write!(f, "the tag “{tag}” appears more than once"),
        }
    }
}
impl std::error::Error for MetaError {}

fn chars(s: &str) -> usize {
    s.chars().count()
}
fn no_control(s: &str, field: &'static str) -> Result<(), MetaError> {
    if s.chars().any(char::is_control) {
        Err(MetaError::ControlCharacter { field })
    } else {
        Ok(())
    }
}

/// Edge-cleaning for typed text: whitespace and the invisible direction marks an Arabic keyboard or a
/// paste can carry (the same rule as object names, [`crate::command::clean_name`]).
pub fn clean_text(s: &str) -> String {
    crate::command::clean_name(s).to_string()
}

/// THE comparison key for tags and Start search: NFC, then a full case-fold approximation (upper then
/// lower, so ß/SS and σ/ς/Σ meet), NFC again — the same rule as the app's volume-name keys
/// (`file_ports::volume_name`). Over-matching is the safe side for "the same tag".
pub fn fold(s: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let nfc: String = s.nfc().collect();
    nfc.to_uppercase().to_lowercase().nfc().collect()
}

/// What a checked board edit refuses, with its plain-English reason (`Display`).
pub type Reject = MetaError;

/// Clean a typed tag list: each tag edge-cleaned, empty tags dropped, case-insensitive duplicates
/// dropped (the FIRST spelling is kept), order preserved. Bounds are NOT applied here — see
/// [`check_tags`].
pub fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen: Vec<String> = Vec::with_capacity(tags.len());
    let mut out = Vec::with_capacity(tags.len());
    for tag in tags {
        let tag = clean_text(&tag);
        if tag.is_empty() {
            continue;
        }
        let folded = fold(&tag);
        if seen.contains(&folded) {
            continue;
        }
        seen.push(folded);
        out.push(tag);
    }
    out
}

/// Name bounds: at most [`MAX_NAME_CHARS`] characters, no control characters.
pub fn check_name(name: &str) -> Result<(), MetaError> {
    no_control(name, "name")?;
    let found = chars(name);
    if found > MAX_NAME_CHARS {
        return Err(MetaError::NameTooLong { found });
    }
    Ok(())
}

/// Description bounds: at most [`MAX_DESCRIPTION_CHARS`] characters, no control characters.
pub fn check_description(description: &str) -> Result<(), MetaError> {
    no_control(description, "description")?;
    let found = chars(description);
    if found > MAX_DESCRIPTION_CHARS {
        return Err(MetaError::DescriptionTooLong { found });
    }
    Ok(())
}

/// A STORED tag list: at most [`MAX_TAGS`], each at most [`MAX_TAG_CHARS`] characters, no control
/// characters, and already clean (no empty/edge-spaced tag, no case-insensitive duplicate) — i.e.
/// exactly what [`normalize_tags`] produces. Used on load and after the edit-side normalization.
pub fn check_tags(tags: &[String]) -> Result<(), MetaError> {
    if tags.len() > MAX_TAGS {
        return Err(MetaError::TooManyTags { found: tags.len() });
    }
    let mut seen: Vec<String> = Vec::with_capacity(tags.len());
    for tag in tags {
        no_control(tag, "tags")?;
        if tag.is_empty() || clean_text(tag) != *tag {
            return Err(MetaError::UncleanTag { tag: tag.clone() });
        }
        let found = chars(tag);
        if found > MAX_TAG_CHARS {
            return Err(MetaError::TagTooLong { tag: tag.clone(), found });
        }
        let folded = fold(tag);
        if seen.contains(&folded) {
            return Err(MetaError::DuplicateTag { tag: tag.clone() });
        }
        seen.push(folded);
    }
    Ok(())
}

/// All three fields of a document (the load- and save-side check).
pub fn check_document(doc: &Document) -> Result<(), MetaError> {
    check_name(&doc.name)?;
    check_description(&doc.description)?;
    check_tags(&doc.tags)
}

/// THE display-name rule (tab titles, the window title, Recent and Start all use it): the board's own
/// name when it has one, else the file stem (`Logo` for `Logo.vrs`), else `Untitled-N`.
pub fn display_name(name: &str, path: Option<&Path>, untitled: Option<u32>) -> String {
    if !name.is_empty() {
        return name.to_string();
    }
    if let Some(stem) = path.and_then(file_stem) {
        return stem;
    }
    match untitled {
        Some(n) => format!("Untitled-{n}"),
        None => "Untitled".into(),
    }
}

/// `Logo` for `/a/Logo.vrs`; the whole file name when it has no stem; `None` for a path without one.
fn file_stem(path: &Path) -> Option<String> {
    path.file_stem().or_else(|| path.file_name()).map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty())
}

/// A new-board preset (Start's "…or start with an artboard").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PresetId {
    Square,
    Portrait,
    Story,
    A4,
    /// One artboard of the last-used custom size. The size dialog is a later piece; until then the
    /// caller supplies the remembered size (or the default square).
    Custom,
}

/// One row of the preset table. `w`/`h` are in `unit`; geometry is stored in points (px == pt at the
/// default 72 ppi, so the px presets are numerically equal in the model).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub id: PresetId,
    pub label: &'static str,
    pub w: f32,
    pub h: f32,
    pub unit: Unit,
}

/// THE preset table (the one home for preset sizes). `Custom`'s size here is only its fallback.
pub const PRESETS: &[Preset] = &[
    Preset { id: PresetId::Square, label: "Square", w: 1080.0, h: 1080.0, unit: Unit::Px },
    Preset { id: PresetId::Portrait, label: "Portrait", w: 1080.0, h: 1350.0, unit: Unit::Px },
    Preset { id: PresetId::Story, label: "Story", w: 1080.0, h: 1920.0, unit: Unit::Px },
    Preset { id: PresetId::A4, label: "A4", w: 595.0, h: 842.0, unit: Unit::Pt },
    Preset { id: PresetId::Custom, label: "Custom…", w: 1080.0, h: 1080.0, unit: Unit::Px },
];

/// The table row for `id`.
pub fn preset(id: PresetId) -> &'static Preset {
    PRESETS.iter().find(|p| p.id == id).expect("every PresetId has a row in PRESETS")
}

/// A fresh board: the default document with ZERO artboards (a free canvas).
pub fn new_board() -> Document {
    Document::default()
}

/// A fresh board with one artboard from the preset table, at the origin, active, and the document's
/// display unit set to the preset's unit. `custom` = the last-used custom size in points (only read
/// for [`PresetId::Custom`]; `None` → the table's fallback). A non-finite or non-positive custom size
/// falls back too, so this never builds a document the save gate would refuse.
pub fn new_board_with_preset(id: PresetId, custom: Option<(f32, f32)>) -> Document {
    let p = preset(id);
    let (w, h) = match (id, custom) {
        (PresetId::Custom, Some((w, h))) if w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0 => (w, h),
        _ => (p.w * p.unit.pt_per(72.0), p.h * p.unit.pt_per(72.0)),
    };
    let mut doc = new_board();
    doc.units.display = p.unit;
    doc.artboards = vec![Artboard { w, h, ..Artboard::default() }];
    doc.active = 0;
    doc
}
