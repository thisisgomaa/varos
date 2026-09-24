//! Declared limits for reading and writing `.vrs` (ADR-0008 rule 6, `docs/reference/VRS_FORMAT.md`).
//! Over-limit input is refused with the number, never truncated. The numbers may be LOWERED after
//! measurement; raising one needs the owner.

/// Every bound the load and save pipelines enforce. `Limits::DEFAULT` is what the app uses; tests pass
/// tiny limits to exercise refusals without big inputs. There is no JSON-depth field on purpose:
/// serde_json's built-in 128-level recursion limit already applies to every typed decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Whole file on disk (raw JSON or PDF container).
    pub max_file_bytes: u64,
    /// The editable model JSON (the `{"varos":…,"doc":…}` blob).
    pub max_model_bytes: usize,
    /// Indirect objects in a PDF container.
    pub max_pdf_objects: usize,
    /// Total bytes the PDF reader may decode from streams.
    pub max_decoded_stream_bytes: usize,
    /// Nesting depth of PDF structures the reader walks (name trees).
    pub max_pdf_depth: usize,
    /// Scene-tree nodes (legacy registry groups count here too: they become nodes).
    pub max_nodes: usize,
    /// Paths.
    pub max_paths: usize,
    /// Anchors, outer rings plus holes, summed over every path.
    pub max_anchors: usize,
    /// Artboards.
    pub max_artboards: usize,
    /// Scene-tree depth: a root Layer is level 1.
    pub max_tree_depth: usize,
}

impl Limits {
    /// The shipped numbers (mirrored in `docs/reference/VRS_FORMAT.md`). Nodes and paths were lowered
    /// from the planned 100,000 to 40,000 after measurement (work order R4: loading at the cap must stay
    /// near 2 s): `sync_tree` is roughly quadratic, and a release build here took ~1.5 s to load and
    /// ~1.3 s to save 40,000 paths, ~2.8 s / 2.4 s at 50,000 (`format_v2::decode_timing_at_caps`).
    pub const DEFAULT: Limits = Limits {
        max_file_bytes: 256 * 1024 * 1024,
        max_model_bytes: 32 * 1024 * 1024,
        max_pdf_objects: 100_000,
        max_decoded_stream_bytes: 64 * 1024 * 1024,
        max_pdf_depth: 64,
        max_nodes: 40_000,
        max_paths: 40_000,
        max_anchors: 1_000_000,
        max_artboards: 1_000,
        max_tree_depth: 64,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Limits::DEFAULT
    }
}

/// Which limit a `LoadError::TooLarge` refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LimitKind {
    FileBytes,
    ModelBytes,
    PdfObjects,
    DecodedStreams,
    PdfDepth,
    Nodes,
    Paths,
    Anchors,
    Artboards,
    TreeDepth,
}

impl LimitKind {
    /// What is being measured, for the user-readable refusal ("The {what} exceeds …").
    pub fn what(self) -> &'static str {
        match self {
            LimitKind::FileBytes => "file",
            LimitKind::ModelBytes => "editable model",
            LimitKind::PdfObjects => "number of PDF objects",
            LimitKind::DecodedStreams => "decoded PDF data",
            LimitKind::PdfDepth => "PDF structure depth",
            LimitKind::Nodes => "number of layers, groups and objects",
            LimitKind::Paths => "number of paths",
            LimitKind::Anchors => "number of anchor points",
            LimitKind::Artboards => "number of artboards",
            LimitKind::TreeDepth => "layer nesting depth",
        }
    }
    /// Is this limit a byte size (shown in MiB/KiB/bytes) rather than a count?
    pub fn is_bytes(self) -> bool {
        matches!(self, LimitKind::FileBytes | LimitKind::ModelBytes | LimitKind::DecodedStreams)
    }
    /// A limit value as the user reads it: "32 MiB", "100,000".
    pub fn amount(self, n: u64) -> String {
        if self.is_bytes() {
            human_bytes(n)
        } else {
            thousands(n)
        }
    }
}

/// 33554432 → "32 MiB"; 1536 → "1.5 KiB"; 12 → "12 bytes".
fn human_bytes(n: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    let (unit, div) = if n >= MIB {
        ("MiB", MIB)
    } else if n >= KIB {
        ("KiB", KIB)
    } else {
        return format!("{n} bytes");
    };
    if n.is_multiple_of(div) {
        format!("{} {unit}", n / div)
    } else {
        format!("{:.1} {unit}", n as f64 / div as f64)
    }
}

/// 100000 → "100,000".
fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn amounts_read_like_the_docs() {
        assert_eq!(LimitKind::ModelBytes.amount(32 * 1024 * 1024), "32 MiB");
        assert_eq!(LimitKind::FileBytes.amount(1536), "1.5 KiB");
        assert_eq!(LimitKind::FileBytes.amount(12), "12 bytes");
        assert_eq!(LimitKind::Nodes.amount(40_000), "40,000");
        assert_eq!(LimitKind::Anchors.amount(1_000_000), "1,000,000");
        assert_eq!(LimitKind::TreeDepth.amount(64), "64");
    }
}
