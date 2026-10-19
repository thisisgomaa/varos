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
        assert!(s.gsub_arabic && !s.morx && !s.morx_shaped(), "{family}: OpenType Arabic, {s:?}");
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
    assert!(!is_arabic_text("١٢٣ ٤٥"), "Arabic-Indic digits alone are not Arabic letters");
    assert!(is_arabic_text("١٢٣ بسم"));
}

/// `font` with its name table replaced (family `family`, style `style`), without the tables in
/// `drop`, with the raw tables `add`.
fn rebuilt(font: &[u8], family: &str, style: &str, drop: &[&[u8; 4]], add: &[(&[u8; 4], Vec<u8>)]) -> Vec<u8> {
    use write_fonts::types::Tag as WTag;
    let font = skrifa::FontRef::new(font).unwrap();
    let full = format!("{family} {style}");
    let ps = format!("{}-{}", family.replace(' ', ""), style.replace(' ', ""));
    let name = crate::test_fonts::name_table(&[(1, family), (2, style), (4, &full), (6, &ps)]).unwrap();
    let mut builder = write_fonts::FontBuilder::new();
    builder.add_raw(WTag::new(b"name"), name);
    for (tag, data) in add {
        builder.add_raw(WTag::new(tag), data.clone());
    }
    for r in font.table_directory.table_records() {
        let tag = WTag::new(&r.tag().to_be_bytes());
        if !drop.iter().any(|d| WTag::new(d) == tag) && !builder.contains(tag) {
            builder.add_raw(tag, font.table_data(r.tag()).unwrap().as_bytes());
        }
    }
    builder.build()
}

/// An empty `morx` table: version 2, no chains.
fn empty_morx() -> (&'static [u8; 4], Vec<u8>) {
    (b"morx", vec![0, 2, 0, 0, 0, 0, 0, 0])
}

/// HarfRust shapes any face with a `morx` table with it, whatever its `GSUB` says (Damascus has
/// both): such a face is morx-shaped, with or without `GSUB`. With an empty `morx` its letters
/// don't join: it has the Arabic letters (a character map hint) but can't set Arabic.
#[test]
fn faces_with_morx_are_morx_shaped_and_one_that_doesnt_join_is_left_out() {
    let only = rebuilt(NOTO, "Morx Only Arabic", "Regular", &[b"GSUB"], &[empty_morx()]);
    let both = rebuilt(NOTO, "Morx Gsub Arabic", "Regular", &[], &[empty_morx()]);
    let db = db_with(&[&only, &both, NOTO]);
    for family in ["Morx Only Arabic", "Morx Gsub Arabic"] {
        let face = db.face(family, "Regular").unwrap();
        assert_eq!(face.family, family);
        let s = face_arabic_support(&face);
        assert!(s.covers && s.morx && s.morx_shaped(), "{family}: {s:?}");
        // The cmap says Arabic, shaping says the letters don't join.
        assert!(!s.joins && !s.capable(), "{family}: {s:?}");
        assert!(face.traits.arabic, "{family}: the character map hint");
        assert!(!db.may_set_arabic(family), "{family}: loaded, its joining is known: left out of the filter");
    }
    assert!(face_arabic_support(&db.face("Morx Gsub Arabic", "Regular").unwrap()).gsub_arabic, "GSUB kept, still morx-shaped");
    assert!(!face_arabic_support(&db.face("Noto Sans Arabic", "Regular").unwrap()).morx_shaped());
    assert_eq!(db.arabic_families(None), ["Noto Sans Arabic"]);
}

/// A family's support is "any face": an italic without Arabic, loaded first, neither hides the
/// family from the Arabic filter nor overwrites what its other faces can do.
#[test]
fn a_family_sets_arabic_when_any_face_does_whatever_loads_first() {
    let serif = include_bytes!("../../../assets/fonts/SourceSerif4-Regular.ttf");
    let italic = rebuilt(serif, "Mixed Arabic", "Italic", &[], &[]);
    let regular = rebuilt(NOTO, "Mixed Arabic", "Regular", &[], &[]);
    // Fonts added as bytes, the italic first.
    let db = db_with(&[&italic, &regular]);
    let faces = db.loaded_faces("Mixed Arabic");
    assert_eq!(faces.len(), 2);
    assert!(!faces[0].traits.arabic && faces[1].traits.arabic, "the italic is the first loaded face");
    assert!(db.family_traits("Mixed Arabic").arabic, "family traits: any face");
    let s = db.known_arabic_support("Mixed Arabic").unwrap();
    assert!(s.capable(), "{s:?}");
    assert!(db.may_set_arabic("Mixed Arabic"));
    assert_eq!(db.arabic_families(None), ["Mixed Arabic"]);
    // The italic alone can't.
    let alone = db_with(&[&italic]);
    assert!(!alone.family_traits("Mixed Arabic").arabic && !alone.may_set_arabic("Mixed Arabic"));
    // Aggregation: the faces that can set Arabic decide, in any order.
    let latin = face_arabic_support(&faces[0]);
    let arabic = face_arabic_support(&faces[1]);
    assert_eq!(aggregate([latin, arabic]), aggregate([arabic, latin]));
    assert_eq!(aggregate([latin, arabic]), arabic);
    assert_eq!(aggregate([latin]), latin);
    assert_eq!(aggregate([]), ArabicSupport::default());
}

/// A scratch font folder, removed when dropped.
struct Folder(std::path::PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("varos-arabic-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Folder(dir)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Installed fonts: the catalog's family hint is any face's, and the first loaded face (an italic
/// without Arabic, its file read first) doesn't change it.
#[test]
fn an_installed_family_whose_italic_loads_first_still_sets_arabic() {
    let serif = include_bytes!("../../../assets/fonts/SourceSerif4-Regular.ttf");
    let dir = Folder::new("italic-first");
    std::fs::write(dir.0.join("a-italic.ttf"), rebuilt(serif, "Installed Arabic", "Italic", &[], &[])).unwrap();
    std::fs::write(dir.0.join("b-regular.ttf"), rebuilt(NOTO, "Installed Arabic", "Regular", &[], &[])).unwrap();
    let db = FontDb::with_font_dirs(vec![dir.0.clone()]);
    db.set_system_fallback(false);
    assert!(db.family_traits("Installed Arabic").arabic, "cataloged: any face");
    let italic = db.face("Installed Arabic", "Italic").unwrap();
    assert_eq!((italic.family.as_str(), italic.style.as_str()), ("Installed Arabic", "Italic"));
    let faces = db.loaded_faces("Installed Arabic");
    assert_eq!(faces.first().map(|f| f.style.as_str()), Some("Italic"), "the italic loaded first");
    assert!(db.family_traits("Installed Arabic").arabic, "loaded: still any face");
    assert!(db.may_set_arabic("Installed Arabic"));
    assert!(db.known_arabic_support("Installed Arabic").is_some_and(ArabicSupport::capable));
}

/// No support outlives a rescan: it lives with the loaded faces, which a rescan drops when their
/// files are gone; the family's new files are what count after it.
#[test]
fn a_rescan_leaves_no_stale_arabic_support() {
    let serif = include_bytes!("../../../assets/fonts/SourceSerif4-Regular.ttf");
    let dir = Folder::new("rescan");
    let arabic = dir.0.join("arabic.ttf");
    std::fs::write(&arabic, rebuilt(NOTO, "Rescan Arabic", "Regular", &[], &[])).unwrap();
    let db = FontDb::with_font_dirs(vec![dir.0.clone()]);
    db.set_system_fallback(false);
    assert!(db.arabic_support("Rescan Arabic").is_some_and(ArabicSupport::capable));
    assert!(db.may_set_arabic("Rescan Arabic"));
    // The family is replaced by a Latin font of the same name, in another file.
    std::fs::remove_file(&arabic).unwrap();
    std::fs::write(dir.0.join("latin.ttf"), rebuilt(serif, "Rescan Arabic", "Regular", &[], &[])).unwrap();
    db.load_system_fonts();
    assert_eq!(db.known_arabic_support("Rescan Arabic"), None, "the old faces went with the rescan");
    assert!(!db.may_set_arabic("Rescan Arabic"), "the catalog's new hint: no Arabic");
    assert!(db.arabic_support("Rescan Arabic").is_some_and(|s| !s.capable()), "loaded again: the Latin font");
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
            "{f:40} capable={} lam_alef={} gsub={} morx={} morx_shaped={} style={:?} arabic_name={:?}",
            s.capable(),
            s.lam_alef,
            s.gsub_arabic,
            s.morx,
            s.morx_shaped(),
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
