//! Arabic PDF fixtures: verify authored text with our reader and, when installed, Poppler.
//! Set ARABIC_PDF_FIXTURES to retain the generated PDFs for inspection in other viewers.
//! Set REQUIRE_PDFTOTEXT=1 to fail instead of skipping the external check without Poppler.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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
/// cargo test -p vectorcraft-pdf --test arabic_copy_back --release --offline -j 2 -- \
///   --test-threads=2 latin_only_pdf_bytes_stay_frozen --nocapture
/// ```
const LATIN_ONLY_SHA256: &str = "319af8ed0b457ed35c353c611e4b2de98c373c2aba4c3347ef3a97c1d6785cba";
const LATIN_ONLY: &str = "Hello, world. (2026)";
/// Multi-run, multi-line Latin with an underline, a character stroke and a bulleted list.
/// Same pinned date. Regenerate with `latin_rich_pdf_bytes_stay_frozen` (the failure prints the digest).
const LATIN_RICH_SHA256: &str = "1f4585dc17661887012f4ebbd60c4addcd854c18e73cd57fe6919f4401e8154b";

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
fn check_pdftotext(name: &str, pdf: &[u8], expected: &[String]) {
    for args in [&["-", "-"][..], &["-layout", "-", "-"][..]] {
        if let Some(bytes) = poppler("pdftotext", args, pdf) {
            let raw = String::from_utf8(bytes).unwrap();
            let clean: String = raw.chars().filter(|c| !matches!(c, '\u{202a}'..='\u{202e}' | '\u{000c}')).collect();
            let got: Vec<_> = clean.lines().filter(|line| !line.is_empty()).collect();
            assert_eq!(got, expected, "{name}: pdftotext {args:?}: {raw:?}");
        }
    }
}

fn check_reader(name: &str, pdf: &[u8], expected: &[String]) -> Vec<String> {
    let back = import_with_report(pdf, &ImportOptions::default()).unwrap();
    let got = texts(&back.document);
    assert_eq!(got, expected, "{name}: our reader");
    got
}

fn check_copy(name: &str, pdf: &[u8], expected: &[String]) -> Vec<String> {
    check_pdftotext(name, pdf, expected);
    check_reader(name, pdf, expected)
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
        publish(&format!("aat-{family}-ligature"), &pdf);
        check_pdftotext(&format!("aat-{family}-ligature"), &pdf, &[t.plain_text()]);
        let back = import_with_report(&pdf, &ImportOptions::default()).unwrap();
        let unmatched = back.warnings.iter().any(|w| w == vectorcraft_pdf::UNMATCHED_GLYPHS_NOTE);
        if unmatched {
            eprintln!(
                "SKIP aat-{family}: reader kept shaped glyphs as outlines ({}); pdftotext still runs, the crate reader is not asserted",
                vectorcraft_pdf::UNMATCHED_GLYPHS_NOTE
            );
        } else {
            assert_eq!(texts(&back.document), vec![t.plain_text()], "aat-{family}-ligature: our reader");
        }
        let mut t = stroked(KASHIDA);
        t.runs[0].style.font_family = family.into();
        let layout = vectorcraft_text::layout(db, &t);
        assert!(layout.glyphs.iter().all(|g| g.font_id == face.id()), "{family}: no fallback");
        assert!(layout.glyphs.iter().any(|g| g.len == 0), "{family}: exercise real elongation");
        let pdf = export(&doc_with(&[t.clone()]), &PdfOptions::default()).unwrap();
        publish(&format!("aat-{family}-force-stroked"), &pdf);
        check_pdftotext(&format!("aat-{family}-force-stroked"), &pdf, &[t.plain_text()]);
        let back = import_with_report(&pdf, &ImportOptions::default()).unwrap();
        if back.warnings.iter().any(|w| w == vectorcraft_pdf::UNMATCHED_GLYPHS_NOTE) {
            eprintln!(
                "SKIP aat-{family}-force-stroked: reader kept shaped glyphs as outlines ({}); pdftotext still runs, the crate reader is not asserted",
                vectorcraft_pdf::UNMATCHED_GLYPHS_NOTE
            );
        } else {
            assert_eq!(texts(&back.document), vec![t.plain_text()], "aat-{family}-force-stroked: our reader");
        }
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
    let hash = vectorcraft_pdf::sha256_hex(&pdf);
    assert_eq!(
        hash, LATIN_ONLY_SHA256,
        "Latin PDF sha256 changed (the Arabic mapping has no switch; this locks the bytes without it).\n\
         Paste the digest into LATIN_ONLY_SHA256. Regenerate:\n\
         cargo test -p vectorcraft-pdf --test arabic_copy_back --release --offline -j 2 -- \\\n\
           --test-threads=2 latin_only_pdf_bytes_stay_frozen --nocapture\n\
         actual sha256: {hash}"
    );
}

fn stroke_runs(mut t: TextObject) -> TextObject {
    for run in &mut t.runs {
        if run.style.fill.is_none() {
            run.style.fill = vectorcraft_color::Paint::solid(vectorcraft_color::Color::gray(0.0));
        }
        run.style.stroke = run.style.fill.clone();
        run.style.stroke_width = 0.6;
    }
    t
}

/// A run that appears in two visual segments: another run sits between its glyphs, which is
/// the `seen` piece (the second time that run is painted).
fn run_is_split(glyphs: &[vectorcraft_text::PositionedGlyph]) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut i = 0;
    while let Some(g) = glyphs.get(i) {
        let run = g.run;
        let again = !seen.insert(run);
        i += 1;
        while glyphs.get(i).is_some_and(|next| next.run == run) {
            i += 1;
        }
        if again {
            return true;
        }
    }
    false
}

/// Narrow the area until the paragraph wraps onto `lines` lines and one run is interrupted
/// by another (the `seen` piece of a split run).
fn wrap_split(make: impl Fn() -> TextObject, lines: usize) -> TextObject {
    let mut samples = Vec::new();
    for width in (40..720).step_by(4) {
        let mut t = make();
        let frame = Rect::new(36.0, 36.0, 36.0 + width as f64, 640.0);
        t.kind = TextKind::Area { frame: PathData::from_bezpath(&frame.to_path(0.1)) };
        t.xf = Affine::IDENTITY;
        let layout = vectorcraft_text::layout(FontDb::global(), &t);
        if layout.lines.len() != lines {
            continue;
        }
        if samples.len() < 8 {
            let mut desc = format!("width {width}:");
            for line in 0..lines {
                let mut runs = std::collections::BTreeSet::new();
                let (mut rtl, mut ltr) = (false, false);
                for g in layout.glyphs.iter().filter(|g| g.line == line) {
                    runs.insert(g.run);
                    if g.rtl {
                        rtl = true;
                    } else {
                        ltr = true;
                    }
                }
                desc.push_str(&format!(" L{line} runs {runs:?} rtl {rtl} ltr {ltr};"));
            }
            samples.push(desc);
        }
        if run_is_split(&layout.glyphs) {
            return t;
        }
    }
    panic!("no frame width wrapped the paragraph into {lines} lines with a run split by another run.\n{}", samples.join("\n"));
}

/// Path-painting operators outside text objects, from uncompressed ASCII content streams.
fn paint_ops(pdf: &[u8]) -> Vec<String> {
    let mut ops = Vec::new();
    let mut i = 0;
    while i < pdf.len() {
        let Some(rel) = pdf.get(i..).and_then(|rest| rest.windows(6).position(|w| w == b"stream")) else { break };
        let mut start = i + rel + 6;
        if pdf.get(start) == Some(&b'\r') {
            start += 1;
        }
        if pdf.get(start) == Some(&b'\n') {
            start += 1;
        }
        let Some(end_rel) = pdf.get(start..).and_then(|rest| rest.windows(9).position(|w| w == b"endstream")) else { break };
        let end = start + end_rel;
        if let Some(stream) = pdf.get(start..end)
            && stream.iter().all(|b| b.is_ascii())
        {
            scan_paint(stream, &mut ops);
        }
        i = end + 9;
    }
    ops
}

fn scan_paint(stream: &[u8], ops: &mut Vec<String>) {
    let mut text = 0i32;
    let mut token = Vec::new();
    let mut i = 0;
    let flush = |token: &mut Vec<u8>, text: i32, ops: &mut Vec<String>| -> String {
        let s = String::from_utf8_lossy(token).into_owned();
        token.clear();
        if text == 0 && matches!(s.as_str(), "f" | "f*" | "S" | "s" | "B" | "B*" | "b" | "b*") {
            ops.push(s.clone());
        }
        s
    };
    while let Some(&b) = stream.get(i) {
        match b {
            b'%' => {
                let _ = flush(&mut token, text, ops);
                i += 1;
                while stream.get(i).is_some_and(|c| *c != b'\n') {
                    i += 1;
                }
            }
            b'(' => {
                let _ = flush(&mut token, text, ops);
                i += 1;
                let mut depth = 1;
                while let Some(&c) = stream.get(i) {
                    i += 1;
                    match c {
                        b'\\' => i += 1,
                        b'(' => depth += 1,
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                }
            }
            b'<' => {
                let _ = flush(&mut token, text, ops);
                i += 1;
                if stream.get(i) == Some(&b'<') {
                    i += 1;
                } else {
                    while stream.get(i).is_some_and(|c| *c != b'>') {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'>' => {
                let name = flush(&mut token, text, ops);
                i += 1;
                if stream.get(i) == Some(&b'>') && name.is_empty() {
                    i += 1;
                }
            }
            b' ' | b'\n' | b'\r' | b'\t' | b'[' | b']' => {
                let name = flush(&mut token, text, ops);
                if name == "BT" {
                    text += 1;
                } else if name == "ET" {
                    text -= 1;
                }
                i += 1;
            }
            _ => {
                token.push(b);
                i += 1;
            }
        }
    }
    let name = flush(&mut token, text, ops);
    if name == "BT" {
        // not a paint operator
    }
}

fn ink_pieces(t: &TextObject) -> usize {
    let layout = vectorcraft_text::layout(FontDb::global(), t);
    let mut n = 0;
    let mut i = 0;
    while let Some(g) = layout.glyphs.get(i) {
        let (run, line, start) = (g.run, g.line, i);
        i += 1;
        while layout.glyphs.get(i).is_some_and(|next| next.run == run && next.line == line) {
            i += 1;
        }
        let ink = layout.glyphs.get(start..i).unwrap_or(&[]).iter().any(|g| !g.outline.elements().is_empty());
        if ink {
            n += 1;
        }
    }
    n
}

fn assert_piece_paint(name: &str, t: &TextObject, stroked: bool) {
    let pieces = ink_pieces(t);
    assert!(pieces >= 2, "{name}: the split-run branch needs at least two ink pieces, got {pieces}");
    let mut opts = PdfOptions::uncompressed();
    opts.created = Some(1_577_836_800);
    let pdf = export(&doc_sized(720.0, 520.0, std::slice::from_ref(t)), &opts).unwrap();
    let ops = paint_ops(&pdf);
    let fills = ops.iter().filter(|o| *o == "f" || *o == "f*").count();
    let strokes = ops.iter().filter(|o| *o == "S" || *o == "s").count();
    if stroked {
        assert!(strokes >= pieces, "{name}: each piece is stroked on its own, ops {ops:?}, pieces {pieces}");
        assert_eq!(fills, strokes, "{name}: fill then that piece's stroke, ops {ops:?}");
        let mut want_fill = true;
        for op in &ops {
            if op == "f" || op == "f*" {
                assert!(want_fill, "{name}: a fill landed on an earlier stroke, ops {ops:?}");
                want_fill = false;
            } else if op == "S" || op == "s" {
                assert!(!want_fill, "{name}: a stroke landed before its fill, ops {ops:?}");
                want_fill = true;
            }
        }
    } else {
        assert_eq!(strokes, 0, "{name}: no character stroke, ops {ops:?}");
        assert!(fills >= pieces, "{name}: each piece is filled, ops {ops:?}, pieces {pieces}");
    }
}

/// Words of a wrapped paragraph. A line whose visual left edge is Latin is read back on its
/// own, so the paragraph is not one string; every authored word is still there.
fn assert_words(name: &str, t: &TextObject, words: &[&str]) {
    let pdf = export(&doc_sized(720.0, 520.0, std::slice::from_ref(t)), &PdfOptions::default()).unwrap();
    publish(name, &pdf);
    check_pdftotext(name, &pdf, &[t.plain_text()]);
    let back = import_with_report(&pdf, &ImportOptions::default()).unwrap();
    let joined = texts(&back.document).join(" ");
    for word in words {
        assert!(joined.contains(word), "{name}: {joined:?} is missing {word}");
    }
}

/// Multi-line RTL with two runs, and the interleaved wrap "مرحبا ABC" / "DEF سلام".
/// With a stroke and without: each piece's stroke follows that piece's fill, and the
/// authored words copy back.
#[test]
fn split_runs_stroke_after_each_piece_and_copy_back() {
    fonts();
    let arabic = CharStyle { font_family: "Noto Sans Arabic".into(), size: 20.0, ..Default::default() };
    let latin = CharStyle { font_family: "Source Sans 3".into(), size: 20.0, ..Default::default() };
    let two = wrap_split(
        || {
            let mut t = TextObject::point(Point::ZERO, "", arabic.clone());
            t.runs = vec![TextRun::new("مرحبا بالعالم ", arabic.clone()), TextRun::new("Hello world today", latin.clone())];
            t
        },
        2,
    );
    let two_text = two.plain_text();
    assert_words("split-two-runs", &two, &["مرحبا", "بالعالم", "Hello", "world", "today"]);
    assert_piece_paint("split-two-runs", &two, false);
    let two_stroke = stroke_runs(two);
    assert_words("split-two-runs-stroke", &two_stroke, &["مرحبا", "بالعالم", "Hello", "world", "today"]);
    assert_piece_paint("split-two-runs-stroke", &two_stroke, true);
    assert_eq!(two_stroke.plain_text(), two_text);

    let interleaved = wrap_split(
        || {
            let mut t = TextObject::point(Point::ZERO, "", arabic.clone());
            t.runs = vec![TextRun::new("مرحبا ", arabic.clone()), TextRun::new("ABC DEF ", latin.clone()), TextRun::new("سلام", arabic.clone())];
            t
        },
        2,
    );
    assert_eq!(interleaved.plain_text(), "مرحبا ABC DEF سلام");
    assert_words("interleaved", &interleaved, &["مرحبا", "ABC", "DEF", "سلام"]);
    assert_piece_paint("interleaved", &interleaved, false);
    let interleaved_stroke = stroke_runs(interleaved);
    assert_words("interleaved-stroke", &interleaved_stroke, &["مرحبا", "ABC", "DEF", "سلام"]);
    assert_piece_paint("interleaved-stroke", &interleaved_stroke, true);
}

/// A damma on the letter after ZWNJ is one cluster of several glyphs. Folding the joiner onto
/// only one of them used to emit the cluster twice ("خُخُ‌").
#[test]
fn a_joiner_before_a_marked_cluster_is_not_duplicated() {
    fonts();
    let word = "می\u{200C}خ\u{064F}واهم";
    let t = arabic(word, 80.0, Digits::Default);
    let layout = vectorcraft_text::layout(FontDb::global(), &t);
    let damma = word.find('\u{064F}').expect("damma");
    let cluster = layout.glyphs.iter().filter(|g| (g.byte..g.byte + g.len).contains(&damma)).count();
    assert!(cluster >= 2, "خ+damma must be more than one glyph so the fold covers the whole cluster, got {cluster}");
    let got = round_trip("persian-damma", &[t]);
    assert_eq!(got, vec![word.to_string()]);
    assert_eq!(got[0].matches('\u{064F}').count(), 1, "the damma cluster was emitted twice: {}", got[0]);
    assert_eq!(got[0].matches('\u{200C}').count(), 1, "the joiner was dropped or doubled: {}", got[0]);
}

/// The same glyph is first used beside a ZWNJ and later on its own. ToUnicode is first-wins;
/// the joiner must stay on the `/ActualText` of the first use, not on every later one.
#[test]
fn a_folded_joiner_does_not_stick_to_later_uses_of_the_glyph() {
    fonts();
    let word = "Auf\u{200C}lage and a f";
    let st = CharStyle { font_family: "Source Sans 3".into(), size: 24.0, ..Default::default() };
    let t = TextObject::point(Point::new(40.0, 80.0), word, st);
    let sans = FontDb::global().face("Source Sans 3", "Regular").expect("Source Sans 3");
    let layout = vectorcraft_text::layout(FontDb::global(), &t);
    let zwnj = word.find('\u{200C}').unwrap();
    assert!(layout.glyphs.iter().any(|g| (g.byte..g.byte + g.len).contains(&zwnj)), "a glyph covers the ZWNJ");
    let f_gid = sans.glyph_for('f');
    let f_uses = layout.glyphs.iter().filter(|g| g.gid == f_gid && g.font_id == sans.id()).count();
    assert!(f_uses >= 2, "the same f glyph must occur folded and plain, got {f_uses}");
    let got = round_trip_in("latin-zwnj", 700.0, 240.0, &[t]);
    assert_eq!(got, vec![word.to_string()]);
    assert_eq!(got[0].matches('\u{200C}').count(), 1, "a later f picked up the joiner: {}", got[0]);
    let mut opts = PdfOptions::uncompressed();
    opts.created = Some(1_577_836_800);
    let pdf = export(
        &doc_sized(
            700.0,
            240.0,
            &[TextObject::point(Point::new(40.0, 80.0), word, CharStyle { font_family: "Source Sans 3".into(), size: 24.0, ..Default::default() })],
        ),
        &opts,
    )
    .unwrap();
    let raw = String::from_utf8_lossy(&pdf);
    assert!(raw.contains("/ActualText"), "the joiner is an ActualText span, not a ToUnicode rewrite");
}

/// Latin only, so this is the run-index writer: several runs, several lines, an underline,
/// a character stroke and a bulleted list. The bytes are frozen.
#[test]
fn latin_rich_pdf_bytes_stay_frozen() {
    fonts();
    let sans = CharStyle { font_family: "Source Sans 3".into(), size: 16.0, ..Default::default() };
    let mut serif = CharStyle { font_family: "Source Serif 4".into(), size: 16.0, underline: true, ..Default::default() };
    serif.stroke = serif.fill.clone();
    serif.stroke_width = 0.35;
    let mut t = TextObject::point(Point::new(48.0, 48.0), "", sans.clone());
    t.runs = vec![TextRun::new("Alpha item of the list wraps\n", sans.clone()), TextRun::new("Beta ", sans), TextRun::new("stroked words", serif)];
    t.para.list = Some(vectorcraft_doc::text::ParaList::bulleted(vectorcraft_doc::text::Bullet::Dot));
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(40.0, 30.0, 168.0, 240.0).to_path(0.1)) };
    t.xf = Affine::IDENTITY;
    let layout = vectorcraft_text::layout(FontDb::global(), &t);
    assert!(layout.lines.len() >= 2, "multi-line, got {} lines", layout.lines.len());
    assert!(layout.glyphs.iter().map(|g| g.run).collect::<std::collections::HashSet<_>>().len() >= 2, "multi-run");
    assert!(t.runs.iter().any(|r| r.style.underline), "an underlined run");
    assert!(t.runs.iter().any(|r| r.style.has_stroke()), "a stroked run");
    assert!(layout.glyphs.iter().enumerate().any(|(i, _)| layout.marker_of(i).is_some()), "a bulleted list");
    assert!(layout.glyphs.iter().all(|g| !g.rtl), "Latin only, so the run-index writer runs");
    let opts = PdfOptions { created: Some(1_577_836_800), ..Default::default() };
    let d = doc_sized(240.0, 280.0, &[t.clone()]);
    let pdf = export(&d, &opts).unwrap();
    let again = export(&d, &opts).unwrap();
    assert_eq!(pdf, again, "two exports with a pinned date are the same bytes");
    publish("latin-rich", &pdf);
    let hash = vectorcraft_pdf::sha256_hex(&pdf);
    eprintln!("LATIN_RICH_SHA256 {hash}");
    // Run-index emission writes each run across its lines before the next run, so the reader
    // may return one node per visual line. The frozen bytes are the contract; the words stay
    // in order either way.
    check_pdftotext("latin-rich", &pdf, &[t.plain_text()]);
    let back = import_with_report(&pdf, &ImportOptions::default()).unwrap();
    let got = texts(&back.document);
    let joined = got.join("\n");
    for word in ["Alpha", "item", "wraps", "Beta", "stroked", "words"] {
        assert!(joined.contains(word), "latin-rich: reader text {got:?} is missing {word}");
    }
    assert_eq!(
        hash, LATIN_RICH_SHA256,
        "Latin multi-run PDF sha256 changed.\n\
         Paste the digest into LATIN_RICH_SHA256. Regenerate:\n\
         cargo test -p vectorcraft-pdf --test arabic_copy_back --release --offline -j 2 -- \\\n\
           --test-threads=2 latin_rich_pdf_bytes_stay_frozen --nocapture\n\
         actual sha256: {hash}"
    );
}
