//! Arabic PDF fixtures: verify authored text with our reader and, when installed, Poppler.
//! Set ARABIC_PDF_FIXTURES to retain the generated PDFs for inspection in other viewers.
//! Set REQUIRE_PDFTOTEXT=1 to fail instead of skipping the external check without Poppler.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Once;
use vectorcraft_doc::text::{KashidaMode, KashidaOptions};
use vectorcraft_doc::{CharStyle, Digits, Document, Node, NodeId, NodeKind, TextObject};
use vectorcraft_geom::Point;
use vectorcraft_pdf::{ImportOptions, PdfOptions, export, import_with_report};
use vectorcraft_text::FontDb;

const NOTO: &[u8] = include_bytes!("../../text-arabic/fixtures/NotoSansArabic.ttf");
const LIGATURE: &str = "بسم الله الرحمن الرحيم";
const KASHIDA: &str = "بسم الرحمن الرحيم";

fn fonts() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        FontDb::global().add_font(NOTO.to_vec());
    });
}

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
    fonts();
    let st = CharStyle { font_family: "Noto Sans Arabic".into(), size: 24.0, digits, ..Default::default() };
    // Point type's origin is its left edge, including RTL: keep the entire fixture on the page.
    TextObject::point(Point::new(50.0, y), text, st)
}

fn stroked(text: &str) -> TextObject {
    let mut t = arabic(text, 100.0, Digits::Default);
    t.para.kashida = KashidaOptions { mode: KashidaMode::Force, strength: 100 };
    t.runs[0].style.stroke = t.runs[0].style.fill.clone();
    t.runs[0].style.stroke_width = 0.25;
    t
}

/// Feed the PDF over stdin; no shared temporary filenames when tests run in parallel.
fn poppler(program: &str, args: &[&str], pdf: &[u8]) -> Option<Vec<u8>> {
    let child = Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && std::env::var_os("REQUIRE_PDFTOTEXT").is_none() => {
            eprintln!("SKIP {program}: install poppler-utils to check the external reader/rendering");
            return None;
        }
        Err(e) => panic!("cannot run {program}: {e}"),
    };
    child.stdin.take().unwrap().write_all(pdf).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{program}: {}", String::from_utf8_lossy(&output.stderr));
    Some(output.stdout)
}

fn round_trip(name: &str, texts_in: &[TextObject]) -> Vec<String> {
    let pdf = export(&doc_with(texts_in), &PdfOptions::default()).unwrap();
    if let Some(dir) = std::env::var_os("ARABIC_PDF_FIXTURES") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join(format!("{name}.pdf")), &pdf).unwrap();
    }
    let expected: Vec<_> = texts_in.iter().map(TextObject::plain_text).collect();
    // Poppler adds bidi embedding controls and page/line terminators. Strip only those;
    // never remove internal whitespace, tatweels, or reorder the extracted characters.
    for args in [&["-", "-"][..], &["-layout", "-", "-"][..]] {
        if let Some(bytes) = poppler("pdftotext", args, &pdf) {
            let raw = String::from_utf8(bytes).unwrap();
            let clean: String = raw.chars().filter(|c| !matches!(c, '\u{202a}'..='\u{202e}' | '\u{000c}')).collect();
            let got: Vec<_> = clean.lines().filter(|line| !line.is_empty()).collect();
            assert_eq!(got, expected, "{name}: pdftotext {args:?}: {raw:?}");
        }
    }
    let back = import_with_report(&pdf, &ImportOptions::default()).unwrap();
    let got = texts(&back.document);
    assert_eq!(got, expected, "{name}: our reader");
    got
}

#[test]
fn arabic_and_its_digits_read_back_from_pdf_as_typed() {
    round_trip("digits", &[arabic(KASHIDA, 80.0, Digits::Default), arabic("سنة 2026", 160.0, Digits::Hindi)]);
}

/// The multi-letter Allah glyph used to supply logical characters inside a visual glyph run.
#[test]
fn a_word_with_a_ligature_reads_back_in_order() {
    let t = arabic(LIGATURE, 80.0, Digits::Default);
    assert!(
        vectorcraft_text::layout(FontDb::global(), &t)
            .glyphs
            .iter()
            .any(|g| { g.rtl && LIGATURE.get(g.byte..g.byte + g.len).is_some_and(|s| s.chars().count() > 1) }),
        "fixture must exercise a multi-codepoint glyph"
    );
    round_trip("ligature", &[t]);
}

#[test]
fn stroked_force_kashida_keeps_authored_searchable_text() {
    let t = stroked(KASHIDA);
    assert!(vectorcraft_text::layout(FontDb::global(), &t).glyphs.iter().any(|g| g.len == 0));
    let got = round_trip("force-stroked", &[t]);
    assert!(got.iter().all(|s| !s.contains('\u{0640}')), "virtual tatweels never enter copied text");
}

#[test]
fn ligatures_and_authored_tatweels_survive_kashida_modes_and_strengths() {
    for mode in [KashidaMode::Off, KashidaMode::Auto, KashidaMode::Force] {
        for strength in [0, 50, 100] {
            let mut t = stroked("بسم الله بسـم الرحمن الرحيم");
            t.para.kashida = KashidaOptions { mode, strength };
            round_trip(&format!("modes-{mode:?}-{strength}"), &[t]);
        }
    }
}

#[test]
fn repeated_ligatures_and_latin_digits_keep_their_authored_order() {
    round_trip("repeated", &[arabic("الله الله 2026", 80.0, Digits::Hindi)]);
    round_trip("latin", &[arabic("office 2026", 80.0, Digits::Default)]);
}

#[test]
fn force_kashida_search_layer_does_not_change_the_painted_outline() {
    let d = doc_with(&[stroked(LIGATURE)]);
    let searchable = export(&d, &PdfOptions::default()).unwrap();
    let mut options = PdfOptions::default();
    options.settings.advanced.outline_text = true;
    let outlines = export(&d, &options).unwrap();
    let args = ["-scale-to", "1200", "-singlefile", "-"];
    if let (Some(a), Some(b)) = (poppler("pdftoppm", &args, &searchable), poppler("pdftoppm", &args, &outlines)) {
        assert_eq!(a, b, "the searchable PDF paints exactly the same pixels as outline-only export");
    }
}

#[test]
fn installed_aat_fonts_keep_ligatures_and_stroked_force_words() {
    fonts();
    let db = FontDb::global();
    let mut checked = 0;
    for family in ["Geeza Pro", "Al Nile", "Baghdad", "Damascus", "KufiStandardGK", "Diwan Thuluth"] {
        let Some(face) = db.face(family, "Regular") else { continue };
        if !vectorcraft_text::arabic_fonts::face_arabic_support(&face).morx_shaped() {
            continue;
        }
        let mut t = arabic(LIGATURE, 80.0, Digits::Default);
        t.runs[0].style.font_family = family.into();
        round_trip(&format!("aat-{family}-ligature"), &[t]);
        let mut t = stroked(KASHIDA);
        t.runs[0].style.font_family = family.into();
        let layout = vectorcraft_text::layout(db, &t);
        assert!(layout.glyphs.iter().all(|g| g.font_id == face.id()), "{family}: no fallback");
        assert!(layout.glyphs.iter().any(|g| g.len == 0), "{family}: exercise real elongation");
        round_trip(&format!("aat-{family}-force-stroked"), &[t]);
        checked += 1;
    }
    eprintln!("AAT fixtures checked: {checked} (installed morx fonts only; zero means unavailable)");
}

#[test]
fn unstroked_force_words_keep_the_same_searchable_text() {
    let mut t = stroked(LIGATURE);
    t.runs[0].style.stroke = vectorcraft_color::Paint::None;
    t.runs[0].style.stroke_width = 0.0;
    round_trip("force-unstroked", &[t]);
}
