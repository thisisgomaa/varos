/// A corpus row can contain several inputs. Cases without literal text in the
/// ADR have explicit representative strings here; invisible controls use Rust
/// escapes and are decoded in the resulting string, not passed as backslashes.
#[derive(Debug)]
pub struct CorpusRow {
    pub id: &'static str,
    pub group: &'static str,
    pub texts: &'static [&'static str],
    pub required_evidence: &'static str,
}

pub const CORPUS: [CorpusRow; 10] = [
    CorpusRow {
        id: "joins", group: "shaping",
        texts: &["السلام عليكم ورحمة الله"],
        required_evidence: "Contextual joins, spaces, unchanged logical source; glyph IDs, advances, offsets, clusters and CPU image.",
    },
    CorpusRow {
        id: "ligatures", group: "shaping",
        texts: &["لا لأ لإ لآ — الله"],
        required_evidence: "Font-supported lam-alef/required ligatures; internal grapheme caret stops; no universal glyph-count assertion.",
    },
    CorpusRow {
        id: "marks", group: "marks",
        texts: &["السَّلَامُ عَلَيْكُمْ", "قُرْآنٌ كَرِيمٌ"],
        required_evidence: "Stacked marks attach to bases, ink is not clipped; selection/delete round-trip.",
    },
    CorpusRow {
        id: "mixed_bidi", group: "bidi",
        texts: &["Logo شعار v2", "السعر ١٢٣٫٤٥ ج.م. (USD 12.50)"],
        required_evidence: "UBA levels, per-line visual order, neutrals/brackets/numbers, both affinities, logical copy.",
    },
    CorpusRow {
        id: "wrapping", group: "wrapping",
        texts: &["موعدنا يوم الثلاثاء الساعة 10:30 صباحًا."],
        required_evidence: "Legal narrow/wide breaks, line-edge reshaping, RTL Start/End alignment.",
    },
    CorpusRow {
        id: "joining_controls", group: "controls",
        texts: &["می\u{200c}روم", "ب\u{200d}ب", "ب\u{200c}ب", "سـلام"],
        required_evidence: "Controls preserved, segmentation correct, literal tatweel respected (not automatic kashida).",
    },
    CorpusRow {
        id: "style_context", group: "shaping",
        texts: &["شعار"],
        required_evidence: "Paint-only split keeps joins; deliberate font change has deterministic context/fallback behavior.",
    },
    CorpusRow {
        id: "explicit_direction", group: "bidi",
        texts: &["Logo شعار", "شعار Logo", ""],
        required_evidence: "ASCII-first RTL, Arabic-first LTR and empty RTL independent of first strong character; stored source unchanged.",
    },
    CorpusRow {
        id: "isolates_newlines", group: "controls",
        texts: &["A\u{2067}شعار 12\u{2069}Z", "A\u{a0}B", "A\nشعار", "A\r\nشعار", "شعار   "],
        required_evidence: "Isolates/EOL, stable byte maps; CRLF-to-LF only as an explicit paste edit; trailing spaces preserved.",
    },
    CorpusRow {
        id: "fallback", group: "fallback",
        texts: &["office café e\u{0301}", "شعار", "👩\u{200d}💻"],
        required_evidence: "Latin ligatures/marks; missing Arabic face; whole-cluster fallback or explicit unsupported, never silent loss.",
    },
];
