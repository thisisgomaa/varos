//! Typed load/save refusals. `Display` is the plain-English reason the app shows the user; every
//! refusal leaves the file on disk and the open document untouched.

use super::limits::LimitKind;
use std::fmt;

/// Why a `.vrs` could not be opened.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    /// The file could not be read from disk.
    Io(String),
    /// Over a declared limit (`found` and `max` are in the limit's own unit: bytes or a count).
    TooLarge { limit: LimitKind, found: u64, max: u64 },
    /// Neither a PDF container nor a JSON model.
    NotAVarosFile,
    /// A PDF without an embedded Varos model.
    NoEmbeddedModel,
    /// A PDF shape Varos cannot read safely (object/xref streams, incremental updates, encryption…).
    UnsupportedPdf(String),
    /// A PDF that is damaged.
    MalformedPdf(String),
    /// The model JSON is damaged or has fields/values this format does not allow.
    Malformed(String),
    /// The model has no `varos` format number.
    MissingVersion,
    /// The format number is zero, negative, fractional, too large or not a number.
    InvalidVersion(String),
    /// Written by a newer Varos.
    NewerVersion { found: u32, supported: u32 },
    /// The PDF catalog and the embedded model disagree about the format number.
    VersionMismatch { container: u32, model: u32 },
    /// Structurally or semantically invalid content.
    Invalid(Invalid),
    /// An older format could not be brought up to date.
    MigrationFailed { from: u32, reason: String },
}

/// What is wrong inside a model that decoded fine.
#[derive(Clone, Debug, PartialEq)]
pub enum Invalid {
    /// Two objects of the same kind share an id (ids are unique per kind; cross-kind reuse is legal).
    DuplicateId { kind: &'static str, id: u32 },
    /// `from` (object `id`) refers to an id `missing` that does not exist.
    Dangling { from: &'static str, id: u32, missing: u32 },
    /// The layer tree loops back on itself at `node`.
    Cycle { node: u32 },
    /// `node`'s parent link and its parent's children list disagree, or it has no single owner.
    BadParentage { node: u32, reason: &'static str },
    /// Clipping group `group` is broken (e.g. its mask shape is no longer inside it).
    BadMask { group: u32, reason: &'static str },
    /// A number is not finite; `what` names the object.
    NonFinite { what: String },
    /// A number is outside its allowed range; `what` names the object and field.
    OutOfRange { what: String, value: f64 },
    /// The id counter has no room left.
    IdExhausted,
    /// A retired v1-only structure in a v2 file.
    LegacyInV2 { what: &'static str },
    /// A v2 file that this build's own writer could not have produced (it would change on load).
    NotCanonical { what: &'static str },
}

/// A save refused before anything was written. The editor keeps the document open and dirty.
#[derive(Clone, Debug, PartialEq)]
pub struct SaveRefused(pub LoadError);

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "The file could not be read ({e})."),
            LoadError::TooLarge { limit, found, max } => write!(
                f,
                "The {} exceeds the {} limit (found: {}).",
                limit.what(),
                limit.amount(*max),
                limit.amount(*found)
            ),
            LoadError::NotAVarosFile => f.write_str("This file is not a Varos document."),
            LoadError::NoEmbeddedModel => f.write_str(
                "This PDF has no editable Varos document. Open the original .vrs file. \
                 Importing other PDFs is not available yet.",
            ),
            LoadError::UnsupportedPdf(what) => write!(
                f,
                "This file was re-saved by another app in a form Varos can't read safely yet ({what}). \
                 Open the original .vrs."
            ),
            LoadError::MalformedPdf(e) => write!(f, "This PDF is damaged and can't be opened ({e})."),
            LoadError::Malformed(e) => write!(f, "This file is damaged or not a valid Varos document ({e})."),
            LoadError::MissingVersion => {
                f.write_str("This file has no Varos format number, so Varos can't tell how to read it.")
            }
            LoadError::InvalidVersion(v) => {
                write!(f, "This file's Varos format number ({v}) is not valid, so Varos can't tell how to read it.")
            }
            LoadError::NewerVersion { found, supported } => write!(
                f,
                "This file needs a newer Varos. It uses file format {found}; this build supports up to \
                 {supported}. Update Varos to open it. The file has not been changed."
            ),
            LoadError::VersionMismatch { container, model } => write!(
                f,
                "This file is inconsistent: the PDF says format {container} but the Varos document inside \
                 says format {model}. It may have been edited by another app."
            ),
            LoadError::Invalid(inv) => write!(f, "This document is damaged: {inv}."),
            LoadError::MigrationFailed { from, reason } => {
                write!(f, "This older file (format {from}) could not be updated: {reason}.")
            }
        }
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::DuplicateId { kind, id } => write!(f, "two {kind}s share the id {id}"),
            Invalid::Dangling { from: "root list", missing, .. } => {
                write!(f, "the root layer list refers to node {missing}, which does not exist")
            }
            Invalid::Dangling { from, id, missing } => {
                write!(f, "{from} {id} refers to {missing}, which does not exist")
            }
            Invalid::Cycle { node } => write!(f, "the layer tree loops back on itself at node {node}"),
            Invalid::BadParentage { node, reason } => {
                write!(f, "node {node} is misplaced in the layer tree ({reason})")
            }
            Invalid::BadMask { group, reason } => write!(f, "clipping group {group} is broken ({reason})"),
            Invalid::NonFinite { what } => write!(f, "{what} has a value that is not a finite number"),
            Invalid::OutOfRange { what, value } => write!(f, "{what} is out of range ({value})"),
            Invalid::IdExhausted => f.write_str("its object id counter has run out of room"),
            Invalid::LegacyInV2 { what } => write!(f, "it contains an old-format {what} that format 2 does not allow"),
            Invalid::NotCanonical { what } => {
                write!(f, "its {what} is not in the form Varos writes, so it may have been edited by another app")
            }
        }
    }
}

impl fmt::Display for SaveRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // an `Invalid` reads as the reason itself ("path 3 has a value that is not a finite number"),
        // without the load-side "This document is damaged:" preamble
        let reason = match &self.0 {
            LoadError::Invalid(inv) => inv.to_string(),
            other => other.to_string(),
        };
        write!(f, "This document can't be saved: {}. It is still open.", reason.trim_end_matches('.'))
    }
}

impl std::error::Error for LoadError {}
impl std::error::Error for Invalid {}
impl std::error::Error for SaveRefused {}

impl From<Invalid> for LoadError {
    fn from(i: Invalid) -> Self {
        LoadError::Invalid(i)
    }
}
