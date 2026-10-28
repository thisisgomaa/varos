//! Arabic PDF fixtures: verify authored text with our reader and, when installed, Poppler.
//! Set ARABIC_PDF_FIXTURES to retain the generated PDFs for inspection in other viewers.
//! Set REQUIRE_PDFTOTEXT=1 to fail instead of skipping the external check without Poppler.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sha2::{Digest, Sha256};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Once;
use vectorcraft_doc::text::{KashidaMode, KashidaOptions};
use vectorcraft_doc::{CharStyle, Digits, Document, Justify, Node, NodeId, NodeKind, TextKind, TextObject, TextRun};
use vectorcraft_geom::{Affine, PathData, Point, Rect, Shape};
use vectorcraft_pdf::{ImportOptions, PdfOptions, export, import_with_report};
use vectorcraft_text::FontDb;

const NOTO: &[u8] = include_bytes!("../../text-arabic/fixtures/NotoSansArabic.ttf");
const PLEX: &[u8] = include_bytes!("../../ui-egui/assets/varos/fonts/IBMPlexSansArabic-Regular.ttf");
const LIGATURE: &str = "بسم الله الرحمن الرحيم";
const KASHIDA: &str = "بسم الرحمن الرحيم";
/// Families whose files are committed as test fixtures or bundled with the app. System fonts are
/// not consulted: a Mac with Arial must not turn the Hebrew case into a pass.
const FIXTURE_FAMILIES: &[&str] = &["Noto Sans Arabic", "IBM Plex Sans Arabic", "Source Sans 3", "Source Serif 4", "Inter", "JetBrains Mono"];
/// Latin-only export with the creation date pinned (2020-01-01 UTC). The writer has no switch
/// around the Arabic visual-order mapping; that loop only rewrites RTL clusters, and this
/// document has none. Regenerate by running the test (it prints the digest on failure):
///
/// ```sh
/// CARGO_TARGET_DIR="~/Documents/AI workspace/lanes/_target-shared" \
///   cargo test -p vectorcraft-pdf --test arabic_copy_back --release --offline -j 2 -- \
///   --test-threads=2 latin_only_pdf_bytes_stay_frozen --nocapture
/// ```
const LATIN_ONLY_SHA256: &str = "319af8ed0b457ed35c353c611e4b2de98c373c2aba4c3347ef3a97c1d6785cba";
const LATIN_ONLY: &str = "Hello, world. (2026)";

fn fonts() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        let db = FontDb::global();
        db.add_font(NOTO.to_vec());
        db.add_font(PLEX.to_vec());
    });
}

fn doc_sized(width: f64, height: f64, texts: &[TextObject]) -> Document {
    let mut d = Document::new(width, height);
    for t in texts {
        let mut n = Node::new(NodeId(0), NodeKind::Text(Box::new(t.clone())));
        n.id = d.alloc_id();
        let layer = d.default_layer().unwrap();
        let len = d.children(Some(layer)).map_or(0, |c| c.len());
        d.insert(Some(layer), len, n).unwrap();
    }
    d
}

fn doc_with(texts: &[TextObject]) -> Document {
    doc_sized(600.0, 300.0, texts)
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

/// `which program` — Poppler is optional. Absence skips the external check; it is not treated as a pass
/// of that check. `REQUIRE_PDFTOTEXT=1` makes absence a failure.
fn tool_on_path(program: &str) -> bool {
    let found = Command::new("which").arg(program).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    if !found {
        if std::env::var_os("REQUIRE_PDFTOTEXT").is_some() {
            panic!("REQUIRE_PDFTOTEXT=1 but `which {program}` found nothing");
        }
        eprintln!("SKIP {program}: `which {program}` found nothing; external copy-back check skipped");
    }
    found
}

/// Feed the PDF over stdin; no shared temporary filenames when tests run in parallel.
fn poppler(program: &str, args: &[&str], pdf: &[u8]) -> Option<Vec<u8>> {
    if !tool_on_path(program) {
        return None;
    }
    let mut child =
        Command::new(program).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap_or_else(|e| {
            panic!("cannot run {program}: {e}");
        });
    child.stdin.take().unwrap().write_all(pdf).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{program}: {}", String::from_utf8_lossy(&output.stderr));
    Some(output.stdout)
}

fn publish(name: &str, pdf: &[u8]) {
    if let Some(dir) = std::env::var_os("ARABIC_PDF_FIXTURES") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(std::path::Path::new(&dir).join(format!("{name}.pdf")), pdf).unwrap();
    }
}

/// Poppler adds bidi embedding controls and page/line terminators. Strip only those;
/// never remove internal whitespace, tatweels, or reorder the extracted characters.
fn check_copy(name: &str, pdf: &[u8], expected: &[String]) -> Vec<String> {
    for args in [&["-", "-"][..], &["-layout", "-", "-"][..]] {
        if let Some(bytes) = poppler("pdftotext", args, pdf) {
            let raw = String::from_utf8(bytes).unwrap();
            let clean: String = raw.chars().filter(|c| !matches!(c, '\u{202a}'..='\u{202e}' | '\u{000c}')).collect();
            let got: Vec<_> = clean.lines().filter(|line| !line.is_empty()).collect();
            assert_eq!(got, expected, "{name}: pdftotext {args:?}: {raw:?}");
        }
    }
    let back = import_with_report(pdf, &ImportOptions::default()).unwrap();
    let got = texts(&back.document);
    assert_eq!(got, expected, "{name}: our reader");
    got
}

fn round_trip_in(name: &str, width: f64, height: f64, texts_in: &[TextObject]) -> Vec<String> {
    let pdf = export(&doc_sized(width, height, texts_in), &PdfOptions::default()).unwrap();
    publish(name, &pdf);
    let expected: Vec<_> = texts_in.iter().map(TextObject::plain_text).collect();
    check_copy(name, &pdf, &expected)
}

fn round_trip(name: &str, texts_in: &[TextObject]) -> Vec<String> {
    round_trip_in(name, 600.0, 300.0, texts_in)
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().fold(String::with_capacity(64), |mut s, b| {
        s.push_str(&format!("{b:02x}"));
        s
    })
}

/// Justified area type. The frame is in document space; `xf` does not move it.
fn area_text(text: &str, style: CharStyle, frame: Rect, justify: Justify, kashida: KashidaOptions) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, text, style);
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&frame.to_path(0.1)) };
    t.xf = Affine::IDENTITY;
    t.para.justify = justify;
    t.para.kashida = kashida;
    t
}

fn virtual_kashida_advance(t: &TextObject) -> f64 {
    let face = FontDb::global().face("Noto Sans Arabic", "Regular").expect("Noto Sans Arabic");
    let tat = face.glyph_for('\u{0640}');
    let id = face.id();
    vectorcraft_text::layout(FontDb::global(), t).glyphs.iter().filter(|g| g.font_id == id && g.gid == tat && g.len == 0).map(|g| g.advance).sum()
}

/// Advance of glyphs that are not source spaces: letters, plus virtual tatweels (they cover no
/// source byte). Word spacing is left out, so kashida shows up as growth.
fn nonspace_advance(t: &TextObject) -> f64 {
    let text = t.plain_text();
    vectorcraft_text::layout(FontDb::global(), t)
        .glyphs
        .iter()
        .filter(|g| {
            let source = text.get(g.byte..g.byte.saturating_add(g.len)).unwrap_or("");
            g.len == 0 || !source.chars().next().is_some_and(|c| c.is_whitespace())
        })
        .map(|g| g.advance)
        .sum()
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
        // Shaped Arabic outlines are not the face's nominal cmap glyph. When they differ by
        // more than the reader's tolerance, it keeps the run as paths and there is no text to
        // compare. That is the reader's font check, not a missing text layer.
        let pdf = export(&doc_with(&[t.clone()]), &PdfOptions::default()).unwrap();
        let back = import_with_report(&pdf, &ImportOptions::default()).unwrap();
        if back.warnings.iter().any(|w| w == vectorcraft_pdf::UNMATCHED_GLYPHS_NOTE) {
            eprintln!(
                "SKIP aat-{family}: reader kept shaped glyphs as outlines ({}); copy-back is not asserted",
                vectorcraft_pdf::UNMATCHED_GLYPHS_NOTE
            );
            continue;
        }
        publish(&format!("aat-{family}-ligature"), &pdf);
        check_copy(&format!("aat-{family}-ligature"), &pdf, &[t.plain_text()]);
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

/// Auto on point type never elongates. A justified area wider than the line does: virtual
/// tatweels grow the non-space advances, and the searchable text stays the authored string.
#[test]
fn justified_area_auto_kashida_elongates_and_copies_back() {
    fonts();
    let text = "بسم الله الرحمن الرحيم الحمد لله رب العالمين";
    let st = CharStyle { font_family: "Noto Sans Arabic".into(), size: 24.0, ..Default::default() };
    let measure =
        area_text(text, st.clone(), Rect::new(40.0, 40.0, 4040.0, 280.0), Justify::Left, KashidaOptions { mode: KashidaMode::Off, strength: 100 });
    let measured = vectorcraft_text::layout(FontDb::global(), &measure);
    assert_eq!(measured.lines.len(), 1, "the natural-width fixture must not wrap");
    let natural = measured.lines[0].x1 - measured.lines[0].x0;
    assert!(natural > 80.0 && natural < 3000.0, "natural width {natural}");
    // Slack the strength-100 policy can spend on tatweels (four a word) before word spacing.
    let width = natural + 140.0;
    let frame = Rect::new(40.0, 40.0, 40.0 + width, 280.0);
    let off = area_text(text, st.clone(), frame, Justify::JustifyAll, KashidaOptions { mode: KashidaMode::Off, strength: 100 });
    let auto = area_text(text, st, frame, Justify::JustifyAll, KashidaOptions { mode: KashidaMode::Auto, strength: 100 });
    assert!(matches!(auto.kind, TextKind::Area { .. }), "the elongating fixture is area text, not point text");
    let grown = virtual_kashida_advance(&auto);
    assert!(grown > 1.0, "Auto inserted virtual kashida (advance {grown})");
    assert_eq!(virtual_kashida_advance(&off), 0.0, "Off inserts none");
    let (off_letters, auto_letters) = (nonspace_advance(&off), nonspace_advance(&auto));
    assert!(auto_letters > off_letters + 1.0, "glyph advances grew: Off {off_letters} Auto {auto_letters}");
    let laid = vectorcraft_text::layout(FontDb::global(), &auto);
    assert_eq!(laid.lines.len(), 1, "still one line, so copy-back is not a wrap");
    assert!((laid.lines[0].x1 - laid.lines[0].x0 - width).abs() < 1.0, "the line fills the area");
    let got = round_trip_in("auto-area", frame.x1 + 40.0, 360.0, &[auto]);
    assert_eq!(got, [text.to_string()]);
    assert!(got.iter().all(|s| !s.contains('\u{0640}')), "virtual tatweels never enter copied text");
}

/// One paragraph: Arabic, then Latin with ASCII punctuation and digits, then Arabic-Indic digits,
/// with the Latin run in another face.
#[test]
fn mixed_script_paragraph_copies_back_with_punctuation_digits_and_a_font_change() {
    fonts();
    let arabic = CharStyle { font_family: "Noto Sans Arabic".into(), size: 24.0, ..Default::default() };
    let latin = CharStyle { font_family: "Source Sans 3".into(), size: 24.0, ..Default::default() };
    let mut t = TextObject::point(Point::new(40.0, 90.0), "", arabic.clone());
    t.runs = vec![
        TextRun::new("مرحبا ", arabic.clone()),
        TextRun::new("(Office), 2026. ", latin),
        TextRun::new("سنة \u{0660}\u{0661}\u{0662}\u{0663}", arabic),
    ];
    let text = t.plain_text();
    for c in [',', '.', '(', ')'] {
        assert!(text.contains(c), "missing {c:?} in {text}");
    }
    assert!(text.contains("2026"), "{text}");
    assert!(text.contains('\u{0660}') && text.contains('\u{0663}'), "Arabic-Indic digits: {text}");
    let layout = vectorcraft_text::layout(FontDb::global(), &t);
    assert_eq!(layout.lines.len(), 1, "one line, so the font change is mid-line");
    let db = FontDb::global();
    let noto = db.face("Noto Sans Arabic", "Regular").expect("Noto Sans Arabic").id();
    let sans = db.face("Source Sans 3", "Regular").expect("Source Sans 3");
    assert_eq!(sans.family, "Source Sans 3");
    let sans = sans.id();
    assert!(layout.glyphs.iter().any(|g| g.font_id == noto), "Arabic run");
    assert!(layout.glyphs.iter().any(|g| g.font_id == sans), "Latin run");
    round_trip_in("mixed", 900.0, 300.0, &[t]);
}

/// The visual-order cluster mapping is not Arabic-specific. Hebrew needs a fixture font with
/// niqqud; Persian (Arabic script plus ZWNJ, and a lam-alef ligature) uses Noto Sans Arabic.
#[test]
fn hebrew_niqqud_and_persian_zwnj_copy_back() {
    fonts();
    let db = FontDb::global();
    let hebrew = FIXTURE_FAMILIES.iter().find_map(|family| {
        let face = db.face(family, "Regular")?;
        (face.family == *family && face.covers('ש') && face.covers('\u{05B8}')).then_some(face.family.clone())
    });
    match hebrew {
        Some(family) => {
            // Shin + qamats + shin-dot, then the rest of shalom: one cluster spans base and niqqud.
            let word = "\u{05E9}\u{05B8}\u{05C1}\u{05DC}\u{05D5}\u{05B9}\u{05DD}";
            let st = CharStyle { font_family: family, size: 24.0, ..Default::default() };
            let t = TextObject::point(Point::new(50.0, 80.0), word, st);
            let layout = vectorcraft_text::layout(db, &t);
            assert!(
                layout.glyphs.iter().any(|g| g.rtl && word.get(g.byte..g.byte + g.len).is_some_and(|s| s.chars().count() > 1)),
                "Hebrew fixture must exercise a multi-codepoint RTL cluster"
            );
            round_trip("hebrew-niqqud", &[t]);
        }
        None => {
            eprintln!("SKIP hebrew niqqud: no Hebrew-capable font in the repo fixtures ({}); system fonts are not used", FIXTURE_FAMILIES.join(", "));
        }
    }

    // کتاب + ZWNJ + ها, then بالا so Noto's lam-alef ligature is a multi-codepoint RTL cluster.
    let persian = "کتاب\u{200C}ها بالا";
    assert!(persian.contains('\u{200C}'));
    let t = arabic(persian, 80.0, Digits::Default);
    let face = db.face("Noto Sans Arabic", "Regular").expect("Noto Sans Arabic");
    assert!(face.covers('\u{200C}') && face.covers('\u{06A9}'), "Noto Sans Arabic covers the Persian word");
    let layout = vectorcraft_text::layout(db, &t);
    assert!(layout.glyphs.iter().all(|g| g.font_id == face.id()), "no fallback");
    let zwnj = persian.find('\u{200C}').unwrap();
    assert!(layout.glyphs.iter().any(|g| (g.byte..g.byte + g.len).contains(&zwnj)), "a glyph cluster covers the ZWNJ");
    assert!(
        layout.glyphs.iter().any(|g| g.rtl && persian.get(g.byte..g.byte + g.len).is_some_and(|s| s.chars().count() > 1)),
        "Persian fixture must exercise the RTL cluster mapping"
    );
    round_trip("persian-zwnj", &[t]);
}

/// Latin bytes stay put: the mapping rewrites only RTL clusters, and this document has none.
/// There is no writer switch to compile that path out, so the file is locked by hash.
#[test]
fn latin_only_pdf_bytes_stay_frozen() {
    let st = CharStyle { font_family: "Source Sans 3".into(), size: 24.0, ..Default::default() };
    let t = TextObject::point(Point::new(50.0, 80.0), LATIN_ONLY, st);
    let sans = FontDb::global().face("Source Sans 3", "Regular").expect("bundled Source Sans 3");
    assert_eq!(sans.family, "Source Sans 3");
    let layout = vectorcraft_text::layout(FontDb::global(), &t);
    assert!(layout.glyphs.iter().all(|g| !g.rtl && g.font_id == sans.id()), "Latin only, in the bundled face: the visual-order rewrite does not run");
    let d = doc_with(&[t]);
    // 2020-01-01 00:00:00 UTC. Unpinned, the file's dates would move and the hash with them.
    let opts = PdfOptions { created: Some(1_577_836_800), ..Default::default() };
    let pdf = export(&d, &opts).unwrap();
    let again = export(&d, &opts).unwrap();
    assert_eq!(pdf, again, "two Latin exports with a pinned date are the same bytes");
    publish("latin-frozen", &pdf);
    check_copy("latin-frozen", &pdf, &[LATIN_ONLY.to_string()]);
    let hash = sha256_hex(&pdf);
    assert_eq!(
        hash, LATIN_ONLY_SHA256,
        "Latin PDF sha256 changed (the Arabic mapping has no switch; this locks the bytes without it).\n\
         Paste the digest into LATIN_ONLY_SHA256. Regenerate:\n\
         CARGO_TARGET_DIR=\"~/Documents/AI workspace/lanes/_target-shared\" \\\n\
           cargo test -p vectorcraft-pdf --test arabic_copy_back --release --offline -j 2 -- \\\n\
           --test-threads=2 latin_only_pdf_bytes_stay_frozen --nocapture\n\
         actual sha256: {hash}"
    );
}
