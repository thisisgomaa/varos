//! Varos: Arabic font facts ([`crate::arabic_fonts`]) on the Noto Sans Arabic fixture
//! (`crates/text-arabic/fixtures`, OFL), the bundled IBM Plex Sans Arabic of the Varos theme and
//! the bundled Latin-only fonts; a `morx`-only face is generated from Noto.

use super::*;
use crate::FontDb;

const NOTO: &[u8] = include_bytes!("../../text-arabic/fixtures/NotoSansArabic.ttf");
const PLEX: &[u8] = include_bytes!("../../ui-egui/assets/varos/fonts/IBMPlexSansArabic-Regular.ttf");

fn db_with(fonts: &[&[u8]]) -> FontDb {
    let db = FontDb::with_font_dirs(vec![]);
    db.set_system_fallback(false);
    for f in fonts {
        db.add_font(f.to_vec());
    }
    db
}

#[test]
fn arabic_coverage_is_detected_on_arabic_faces_only() {
    let db = db_with(&[NOTO, PLEX]);
    for family in ["Noto Sans Arabic", "IBM Plex Sans Arabic"] {
        let face = db.face(family, "Regular").expect(family);
        assert_eq!(face.family, family);
        let s = face_arabic_support(&face);
        assert!(s.covers && s.joins && s.capable(), "{family}: {s:?}");
        assert!(s.gsub_arabic && !s.morx && !s.aat_only(), "{family}: OpenType Arabic, {s:?}");
        assert!(face.traits.arabic, "{family}: the loaded face's hint is its character map");
        assert!(db.may_set_arabic(family));
    }
    assert!(face_arabic_support(&db.face("Noto Sans Arabic", "Regular").unwrap()).lam_alef, "Noto ligates لا");
    // The bundled Latin fonts: no Arabic.
    for family in ["Source Serif 4", "Source Sans 3", "Inter"] {
        let face = db.face(family, "Regular").expect(family);
        let s = face_arabic_support(&face);
        assert!(!s.covers && !s.capable(), "{family}: {s:?}");
        assert!(!face.traits.arabic, "{family}");
        assert_eq!(db.arabic_support(family).map(ArabicSupport::capable), Some(false));
    }
    assert_eq!(db.arabic_support("No Such Family"), None);
}

/// A face whose character map the scan couldn't read (past its size cap), built from just its
/// `name` and `OS/2` tables, is hinted by the OS/2 Unicode ranges.
#[test]
fn the_scan_hint_reads_the_os2_ranges_and_names() {
    use skrifa::raw::TableProvider;
    let scanned = |bytes: &[u8]| {
        let font = skrifa::FontRef::new(bytes).unwrap();
        let name = font.table_data(Tag::new(b"name")).unwrap();
        let os2 = font.table_data(Tag::new(b"OS/2")).unwrap();
        crate::fontdb::sfnt_of(&[(b"name", name.as_bytes()), (b"OS/2", os2.as_bytes())]).unwrap()
    };
    let noto = scanned(NOTO);
    let noto = skrifa::FontRef::new(&noto).unwrap();
    assert!(noto.cmap().is_err(), "no character map");
    assert!(arabic_hint(&noto, &[]), "OS/2 says Arabic");
    let serif = scanned(include_bytes!("../../../assets/fonts/SourceSerif4-Regular.ttf"));
    let serif = skrifa::FontRef::new(&serif).unwrap();
    assert!(!arabic_hint(&serif, &["sourceserif4"]));
    // Names: an Arabic name, or an Arabic keyword.
    assert!(arabic_hint(&serif, &["خطعربي"]));
    assert!(arabic_hint(&serif, &["arefruqaa"]));
}

#[test]
fn the_arabic_filter_lists_only_faces_that_set_arabic() {
    let db = db_with(&[NOTO, PLEX]);
    let all = db.arabic_families(None);
    assert_eq!(all, ["IBM Plex Sans Arabic", "Noto Sans Arabic"]);
    for f in &all {
        assert!(db.arabic_support(f).unwrap().capable());
    }
    // Every other family of the database is left out, and none of them sets Arabic.
    for f in db.menu_family_list().iter().filter(|f| !all.contains(f)) {
        assert!(!db.arabic_support(f).unwrap().capable(), "{f} sets Arabic but is not listed");
    }
    assert_eq!(db.arabic_families(Some(ArabicStyle::ModernSans)), all);
    assert!(db.arabic_families(Some(ArabicStyle::Naskh)).is_empty());
}

#[test]
fn the_arabic_sample_is_laid_out_right_to_left_in_the_face_itself() {
    let db = db_with(&[NOTO, PLEX]);
    for family in ["Noto Sans Arabic", "IBM Plex Sans Arabic"] {
        let face = db.face(family, "Regular").unwrap();
        let text = arabic_sample_for(&face).expect("a sample");
        assert_eq!(text, ARABIC_SAMPLE, "{family} has the harakat");
        let l = arabic_sample_layout(&db, &face, text, 24.0).expect("laid out in the face");
        assert_eq!(l.lines.len(), 1);
        assert!(l.lines[0].rtl, "{family}: a right-to-left line");
        assert!(l.glyphs.iter().all(|g| g.font_id == face.id()), "{family}: no fallback");
        assert!(l.glyphs.iter().all(|g| g.gid != 0), "{family}: no .notdef");
        // The first character's glyph is the rightmost.
        let first = l.glyphs.iter().filter(|g| g.byte == 0).map(|g| g.origin.x).fold(f64::NEG_INFINITY, f64::max);
        let rightmost = l.glyphs.iter().map(|g| g.origin.x).fold(f64::NEG_INFINITY, f64::max);
        assert!((first - rightmost).abs() < 1e-6, "{family}: first glyph at {first}, rightmost at {rightmost}");
        // سلام: lam-alef is one glyph covering both letters.
        let lam = "سَ".len();
        assert!(l.glyphs.iter().any(|g| g.byte == lam && g.len >= "لا".len()), "{family}: the لا ligature");
        // The harakat sit on their letters.
        let check = check_phrase(&db, &face, ARABIC_SAMPLE);
        assert!(check.harakat == 8 && check.marks >= 6, "{family}: {check:?}");
        assert_eq!((check.foreign, check.notdef, check.misplaced_marks), (0, 0, 0), "{family}: {check:?}");
    }
    // A Latin face has no Arabic sample.
    let serif = db.face("Source Serif 4", "Regular").unwrap();
    assert_eq!(arabic_sample_for(&serif), None);
    assert!(arabic_sample_layout(&db, &serif, ARABIC_SAMPLE, 24.0).is_none());
}

#[test]
fn style_groups_are_guessed_from_family_names() {
    for (name, style) in [
        ("Noto Naskh Arabic", ArabicStyle::Naskh),
        ("Noto Kufi Arabic", ArabicStyle::Kufi),
        ("Aref Ruqaa", ArabicStyle::Ruqaa),
        ("IBM Plex Sans Arabic", ArabicStyle::ModernSans),
        ("Noto Sans Arabic", ArabicStyle::ModernSans),
        ("Geeza Pro", ArabicStyle::Unknown),
        ("Diwan Thuluth", ArabicStyle::Thuluth),
        ("Diwan Kufi", ArabicStyle::Kufi),
        ("DecoType Naskh", ArabicStyle::Naskh),
        ("Amiri", ArabicStyle::Naskh),
        ("KufiStandardGK", ArabicStyle::Kufi),
        ("Al Nile", ArabicStyle::Unknown),
        ("SF Arabic", ArabicStyle::Unknown),
    ] {
        assert_eq!(guess_arabic_style(&[name]), style, "{name}");
    }
    // An Arabic name counts too.
    assert_eq!(guess_arabic_style(&["Some Font", "خط رقعة"]), ArabicStyle::Ruqaa);
}

#[test]
fn arabic_text_is_told_by_its_first_letter() {
    assert!(is_arabic_text("بسم الله"));
    assert!(is_arabic_text("123 بسم abc"));
    assert!(!is_arabic_text("abc بسم"));
    assert!(!is_arabic_text("123"));
    assert!(!is_arabic_text(""));
}

/// A face that shapes Arabic through `morx` alone (Noto without its `GSUB`, with an empty `morx`
/// table) is marked: kashida can't stretch it.
#[test]
fn a_morx_only_face_is_marked() {
    use write_fonts::types::Tag as WTag;
    let font = skrifa::FontRef::new(NOTO).unwrap();
    let name = crate::test_fonts::name_table(&[(1, "Morx Arabic"), (2, "Regular"), (4, "Morx Arabic"), (6, "MorxArabic")]).unwrap();
    let mut builder = write_fonts::FontBuilder::new();
    builder.add_raw(WTag::new(b"name"), name);
    // morx version 2, no chains.
    builder.add_raw(WTag::new(b"morx"), vec![0, 2, 0, 0, 0, 0, 0, 0]);
    for r in font.table_directory.table_records() {
        let tag = WTag::new(&r.tag().to_be_bytes());
        if tag != WTag::new(b"GSUB") && !builder.contains(tag) {
            builder.add_raw(tag, font.table_data(r.tag()).unwrap().as_bytes());
        }
    }
    let db = db_with(&[&builder.build(), NOTO]);
    let face = db.face("Morx Arabic", "Regular").unwrap();
    assert_eq!(face.family, "Morx Arabic");
    let s = face_arabic_support(&face);
    assert!(s.covers && s.morx && !s.gsub_arabic && s.aat_only(), "{s:?}");
    assert!(!face_arabic_support(&db.face("Noto Sans Arabic", "Regular").unwrap()).aat_only());
}

/// Diagnostic, not a gate: every installed family that may set Arabic, with what the checks say,
/// and the installed faces whose character map has Arabic but the scan's hint missed.
/// `cargo test -p vectorcraft-text --offline -- --ignored --nocapture installed_arabic_fonts`
#[test]
#[ignore = "reads every installed font"]
fn installed_arabic_fonts() {
    let db = FontDb::with_font_dirs(crate::system_font_dirs());
    let families = db.menu_family_list();
    let candidates: Vec<&String> = families.iter().filter(|f| db.may_set_arabic(f)).collect();
    eprintln!("{} families, {} hinted Arabic", families.len(), candidates.len());
    for f in candidates {
        let s = db.arabic_support(f).unwrap_or_default();
        eprintln!(
            "{f:40} capable={} lam_alef={} gsub={} morx={} aat_only={} style={:?} arabic_name={:?}",
            s.capable(),
            s.lam_alef,
            s.gsub_arabic,
            s.morx,
            s.aat_only(),
            db.arabic_style(f),
            db.arabic_family_name(f)
        );
    }
    // Misses: walk the font files directly (not through the database, which keeps what it loads).
    let mut stack = crate::system_font_dirs();
    let mut misses = vec![];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            stack.extend(std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()));
            continue;
        }
        if !crate::is_font_file(&p) {
            continue;
        }
        let Ok(data) = std::fs::read(&p) else { continue };
        let count = match skrifa::raw::FileRef::new(&data) {
            Ok(skrifa::raw::FileRef::Font(_)) => 1,
            Ok(skrifa::raw::FileRef::Collection(c)) => c.len(),
            Err(_) => 0,
        };
        for i in 0..count {
            let Ok(font) = skrifa::FontRef::from_index(&data, i) else { continue };
            let family = [skrifa::string::StringId::TYPOGRAPHIC_FAMILY_NAME, skrifa::string::StringId::FAMILY_NAME]
                .iter()
                .find_map(|id| font.localized_strings(*id).english_or_first().map(|s| s.to_string()))
                .unwrap_or_default();
            let map = font.charmap();
            if map.map('\u{0627}').is_some() && map.map('\u{0628}').is_some() && !db.may_set_arabic(&family) {
                misses.push(format!("{family} ({})", p.display()));
            }
        }
    }
    misses.sort();
    misses.dedup();
    eprintln!("{} faces with Arabic the hint missed:", misses.len());
    for m in misses {
        eprintln!("  {m}");
    }
}
