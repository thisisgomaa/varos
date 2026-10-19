//! Varos: Arabic exported to PDF as real text reads back as the source text: the letters in their
//! logical order (not the shaped glyphs' visual order) and the digits as typed, whatever numeral
//! system the Digits attribute draws them in. One test file, one process: the fixture font is
//! added to the process-wide font database, as an installed font would be found.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use vectorcraft_doc::{CharStyle, Digits, Document, Node, NodeId, NodeKind, TextObject};
use vectorcraft_geom::Point;
use vectorcraft_pdf::{ImportOptions, PdfOptions, export, import_with_report};
use vectorcraft_text::FontDb;

const NOTO: &[u8] = include_bytes!("../../text-arabic/fixtures/NotoSansArabic.ttf");

fn doc_with(texts: &[TextObject]) -> Document {
    let mut d = Document::new(600.0, 300.0);
    for t in texts {
        let mut n = Node::new(NodeId(0), NodeKind::Text(Box::new(t.clone())));
        n.id = d.alloc_id();
        let layer = d.default_layer().unwrap();
        let len = d.children(Some(layer)).map_or(0, |c| c.len());
        d.insert(Some(layer), len, n).unwrap();
    }
    d
}

/// The plain text of every type object in `d`, in document order.
fn texts(d: &Document) -> Vec<String> {
    let mut v = vec![];
    d.walk(|n| {
        if let NodeKind::Text(t) = &n.kind {
            v.push(t.plain_text());
        }
    });
    v
}

fn arabic(text: &str, y: f64, digits: Digits) -> TextObject {
    let st = CharStyle { font_family: "Noto Sans Arabic".into(), size: 24.0, digits, ..Default::default() };
    TextObject::point(Point::new(500.0, y), text, st)
}

/// Export `texts` (Noto Sans Arabic) to PDF and import it back: the type objects' plain texts.
fn round_trip(texts_in: &[TextObject]) -> Vec<String> {
    FontDb::global().add_font(NOTO.to_vec());
    let pdf = export(&doc_with(texts_in), &PdfOptions::default()).unwrap();
    let back = import_with_report(&pdf, &ImportOptions::default()).unwrap();
    texts(&back.document)
}

#[test]
fn arabic_and_its_digits_read_back_from_pdf_as_typed() {
    // Words set with one glyph per letter (joined forms, no multi-letter ligature).
    let sentence = "بسم الرحمن الرحيم";
    let year = "سنة 2026";
    let got = round_trip(&[arabic(sentence, 80.0, Digits::Default), arabic(year, 160.0, Digits::Hindi)]);
    eprintln!("read back: {got:?}");
    assert!(!got.is_empty(), "the PDF's Arabic comes back as text, not outlines");
    assert!(got.iter().any(|t| t == sentence), "letters in logical order: {got:?}");
    assert!(got.iter().any(|t| t == year), "digits as typed (drawn as Hindi digits): {got:?}");
}

/// Known (2026-10-19): a word set with a multi-letter ligature glyph (the Allah ligature, لله in
/// one glyph) reads back reversed, "هللا" for "الله"; the other words of the line are in order.
/// Where it goes wrong (the exported text map of a right-to-left ligature, or the importer's
/// reordering of it) is not established yet.
#[test]
#[ignore = "known: the Allah ligature reads back reversed"]
fn a_word_with_a_ligature_reads_back_in_order() {
    let got = round_trip(&[arabic("بسم الله الرحمن الرحيم", 80.0, Digits::Default)]);
    assert!(got.iter().any(|t| t == "بسم الله الرحمن الرحيم"), "{got:?}");
}
