//! S5-D public boundary tests. Hostile inputs are tiny; fixtures come from the native writer.
use lopdf::{dictionary, Document as Pdf, Object, Stream};
use varos_core::format::{Invalid, LimitKind, Limits, LoadError};
use varos_core::model::Document;
use varos_pdf::{load_vrs_bytes, load_vrs_checked, write_pdf};

fn native() -> Vec<u8> {
    write_pdf(&Document::default()).unwrap()
}
fn parsed() -> Pdf {
    Pdf::load_mem(&native()).unwrap()
}
fn save(pdf: &mut Pdf) -> Vec<u8> {
    let mut bytes = Vec::new();
    pdf.save_to(&mut bytes).unwrap();
    bytes
}
fn load(bytes: &[u8]) -> Result<varos_core::format::Loaded, LoadError> {
    load_vrs_bytes(bytes, &Limits::DEFAULT)
}
fn model_id(pdf: &Pdf) -> lopdf::ObjectId {
    pdf.catalog().unwrap().get(b"VAROS_Model").unwrap().as_reference().unwrap()
}
fn catalog_mut(pdf: &mut Pdf) -> &mut lopdf::Dictionary {
    let root = pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();
    pdf.get_object_mut(root).unwrap().as_dict_mut().unwrap()
}
fn unsupported(bytes: &[u8], reason: &str) {
    assert!(
        matches!(load(bytes), Err(LoadError::UnsupportedPdf(s)) if s == reason),
        "expected {reason}, got {:?}",
        load(bytes)
    );
}
fn fallback(pdf: &mut Pdf, root: Object) {
    let catalog = catalog_mut(pdf);
    catalog.remove(b"VAROS_Model");
    catalog.set("Names", dictionary! { "EmbeddedFiles" => root });
}
fn spec(pdf: &Pdf) -> Object {
    Object::Dictionary(dictionary! { "EF" => dictionary!{ "F" => model_id(pdf) } })
}

#[test]
fn native_current_and_all_frozen_v1_containers_load_through_checked_bytes() {
    let bytes = native();
    let first = load(&bytes).unwrap();
    assert_eq!(first.source_version, varos_core::format::FORMAT_VERSION);
    assert!(!first.migrated);
    assert_eq!(write_pdf(&first.doc).unwrap(), bytes);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures/v1");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.file_name().unwrap().to_string_lossy().ends_with("_pdf.vrs") {
            let loaded = load_vrs_checked(&path, &Limits::DEFAULT).unwrap();
            assert_eq!(loaded.source_version, 1);
            assert!(loaded.notice().is_some());
            checked += 1;
        }
    }
    assert_eq!(checked, 8);
}
#[test]
fn catalog_version_is_checked_before_model_decoding() {
    let mut pdf = parsed();
    catalog_mut(&mut pdf).set("VAROS_SchemaVersion", 1);
    let id = model_id(&pdf);
    pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap().set_content(br#"{"varos":2,"doc":42}"#.to_vec());
    assert_eq!(load(&save(&mut pdf)).unwrap_err(), LoadError::VersionMismatch { container: 1, model: 2 });
    for value in [
        Object::Integer(0),
        Object::Integer(-1),
        Object::Integer(u32::MAX as i64 + 1),
        Object::Real(2.5),
        Object::Null,
        Object::Name(b"two".to_vec()),
    ] {
        catalog_mut(&mut pdf).set("VAROS_SchemaVersion", value);
        assert!(matches!(load(&save(&mut pdf)), Err(LoadError::InvalidVersion(_))));
    }
    catalog_mut(&mut pdf).remove(b"VAROS_SchemaVersion");
    pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap().set_content(br#"{"varos":6,"doc":42}"#.to_vec());
    assert!(matches!(load(&save(&mut pdf)), Err(LoadError::NewerVersion { found: 6, supported: 5 })));
}
#[test]
fn missing_catalog_version_is_legacy_compatible() {
    let mut pdf = parsed();
    catalog_mut(&mut pdf).remove(b"VAROS_SchemaVersion");
    assert_eq!(load(&save(&mut pdf)).unwrap().source_version, varos_core::format::FORMAT_VERSION);
}
#[test]
fn plain_pdf_has_no_editable_model() {
    let mut pdf = parsed();
    let c = catalog_mut(&mut pdf);
    c.remove(b"VAROS_Model");
    c.remove(b"Names");
    assert_eq!(load(&save(&mut pdf)).unwrap_err(), LoadError::NoEmbeddedModel);
}
#[test]
fn trailer_revision_hybrid_and_encryption_are_refused_before_lopdf() {
    for (key, reason) in [("Prev", "incremental update"), ("XRefStm", "hybrid xref"), ("Encrypt", "encrypted")] {
        let mut pdf = parsed();
        pdf.trailer.set(key, 1);
        unsupported(&save(&mut pdf), reason);
    }
}
#[test]
fn escaped_trailer_names_cannot_bypass_preflight() {
    for (name, reason) in [("Pr#65v", "incremental update"), ("XRef#53tm", "hybrid xref"), ("Encr#79pt", "encrypted")] {
        let bytes = native();
        let s = String::from_utf8_lossy(&bytes);
        let at = s.rfind("trailer").unwrap();
        let open = bytes[at..].windows(2).position(|w| w == b"<<").unwrap() + at + 2;
        let mut changed = bytes[..open].to_vec();
        changed.extend_from_slice(format!(" /{name} 1 ").as_bytes());
        changed.extend_from_slice(&bytes[open..]);
        unsupported(&changed, reason);
    }
}
#[test]
fn harmless_trailer_strings_comments_and_similar_names_are_not_keys() {
    let mut pdf = parsed();
    pdf.trailer.set("Previous", 1);
    pdf.trailer.set("InfoText", Object::string_literal("/Encrypt /Prev /XRefStm (escaped)"));
    assert!(load(&save(&mut pdf)).is_ok());
    let bytes = save(&mut pdf);
    let at = bytes.windows(7).rposition(|w| w == b"trailer").unwrap() + 7;
    let mut changed = bytes[..at].to_vec();
    changed.extend_from_slice(b"\n% /Encrypt 4 0 R\n");
    changed.extend_from_slice(&bytes[at..]);
    assert!(load(&changed).is_ok());
}
#[test]
fn compressed_xref_and_modern_object_stream_profiles_are_refused() {
    let mut pdf = parsed();
    pdf.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceStream;
    unsupported(&save(&mut pdf), "compressed cross-reference");
    let mut modern = Vec::new();
    parsed().save_modern(&mut modern).unwrap();
    unsupported(&modern, "compressed cross-reference");
}
#[test]
fn model_filters_never_inflate_or_fall_back_to_raw_bytes() {
    for filter in [Object::Name(b"FlateDecode".to_vec()), Object::Array(vec![]), Object::Null] {
        let mut pdf = parsed();
        let id = model_id(&pdf);
        pdf.get_object_mut(id).unwrap().as_stream_mut().unwrap().dict.set("Filter", filter);
        unsupported(&save(&mut pdf), "model encoding");
    }
}
#[test]
fn normal_filtered_streams_are_not_inflated() {
    let mut pdf = parsed();
    pdf.add_object(Stream::new(dictionary! { "Filter" => "FlateDecode" }, b"deliberately not deflate".to_vec()));
    assert!(load(&save(&mut pdf)).is_ok());
}
#[test]
fn only_exact_embedded_model_name_is_accepted() {
    for name in ["other.json", "model.varos.json.bak", "model.varos.json"] {
        let mut pdf = parsed();
        let fs = spec(&pdf);
        fallback(&mut pdf, Object::Dictionary(dictionary! { "Names" => vec![Object::string_literal(name), fs] }));
        let result = load(&save(&mut pdf));
        if name == "model.varos.json" {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err(), LoadError::NoEmbeddedModel);
        }
    }
}
#[test]
fn invalid_private_model_does_not_silently_fall_back() {
    let mut pdf = parsed();
    catalog_mut(&mut pdf).set("VAROS_Model", Object::Null);
    assert!(matches!(load(&save(&mut pdf)), Err(LoadError::MalformedPdf(_))));
}
#[test]
fn name_tree_cycles_depth_and_duplicate_models_are_bounded() {
    let mut pdf = parsed();
    let id = pdf.new_object_id();
    pdf.objects.insert(id, Object::Dictionary(dictionary! { "Kids" => vec![Object::Reference(id)] }));
    fallback(&mut pdf, Object::Reference(id));
    assert!(matches!(load(&save(&mut pdf)), Err(LoadError::MalformedPdf(_))));
    let mut pdf = parsed();
    let fs = spec(&pdf);
    let mut id = pdf.add_object(dictionary! { "Names" => vec![Object::string_literal("model.varos.json"),fs.clone()] });
    for _ in 0..70 {
        id = pdf.add_object(dictionary! { "Kids" => vec![Object::Reference(id)] });
    }
    fallback(&mut pdf, Object::Reference(id));
    assert!(matches!(load(&save(&mut pdf)), Err(LoadError::TooLarge { limit: LimitKind::PdfDepth, .. })));
    let mut pdf = parsed();
    let fs = spec(&pdf);
    fallback(
        &mut pdf,
        Object::Dictionary(
            dictionary! { "Names" => vec![Object::string_literal("model.varos.json"),fs.clone(),Object::string_literal("model.varos.json"),fs] },
        ),
    );
    assert!(matches!(load(&save(&mut pdf)), Err(LoadError::MalformedPdf(_))));
}
#[test]
fn tiny_limits_exercise_file_object_model_and_stream_budgets() {
    let bytes = native();
    for (limits, kind) in [
        (Limits { max_file_bytes: 1, ..Limits::DEFAULT }, LimitKind::FileBytes),
        (Limits { max_pdf_objects: 1, ..Limits::DEFAULT }, LimitKind::PdfObjects),
        (Limits { max_model_bytes: 1, ..Limits::DEFAULT }, LimitKind::ModelBytes),
        (Limits { max_decoded_stream_bytes: 1, ..Limits::DEFAULT }, LimitKind::DecodedStreams),
    ] {
        assert!(matches!(load_vrs_bytes(&bytes,&limits), Err(LoadError::TooLarge { limit, .. }) if limit==kind));
    }
    let path = std::env::temp_dir().join(format!("varos-bounds-cap-{}.vrs", std::process::id()));
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(1024).unwrap();
    drop(file);
    assert!(matches!(
        load_vrs_checked(&path, &Limits { max_file_bytes: 10, ..Limits::DEFAULT }),
        Err(LoadError::TooLarge { limit: LimitKind::FileBytes, .. })
    ));
    std::fs::remove_file(path).unwrap();
}
#[test]
fn malformed_and_truncated_inputs_never_panic() {
    let bytes = native();
    for len in 0..bytes.len() {
        let _ = load(&bytes[..len]);
    }
    for offset in (0..bytes.len()).step_by(13) {
        let mut changed = bytes.clone();
        changed[offset] = 0;
        let _ = load(&changed);
    }
    assert!(matches!(
        load(br#"{"varos":2,"doc":{}}"#),
        Err(LoadError::Malformed { .. }) | Err(LoadError::Invalid(Invalid::NotCanonical { .. }))
    ));
}
#[test]
fn footer_ambiguity_and_wrong_xref_count_are_refused() {
    let bytes = native();
    let mut bad = bytes.clone();
    bad.extend_from_slice(b"\nstartxref\n1\n");
    assert!(matches!(load(&bad), Err(LoadError::MalformedPdf(_))));
    let xref = bytes.windows(5).rposition(|w| w == b"\nxref").unwrap() + 1;
    let header_start = xref + 5;
    let header_end = bytes[header_start..].iter().position(|b| *b == b'\n').unwrap() + header_start;
    let mut bad = bytes[..header_start].to_vec();
    bad.extend_from_slice(b"0 1");
    bad.extend_from_slice(&bytes[header_end..]);
    assert!(matches!(load(&bad), Err(LoadError::MalformedPdf(_))));
}

#[test]
fn large_direct_arrays_and_deep_objects_are_refused_before_parser_allocation() {
    let mut pdf = parsed();
    pdf.add_object(Object::Array(vec![Object::Integer(0); 1000]));
    let bytes = save(&mut pdf);
    assert!(
        matches!(load_vrs_bytes(&bytes,&Limits { max_pdf_objects:32, ..Limits::DEFAULT }),Err(LoadError::UnsupportedPdf(s)) if s=="direct object complexity limit")
    );
    let mut pdf = parsed();
    let mut object = Object::Null;
    for _ in 0..70 {
        object = Object::Array(vec![object]);
    }
    pdf.add_object(object);
    assert!(matches!(load(&save(&mut pdf)), Err(LoadError::TooLarge { limit: LimitKind::PdfDepth, .. })));
}

#[test]
fn oversized_xref_lines_and_indirect_stream_lengths_are_refused() {
    let bytes = native();
    let xref = bytes.windows(5).rposition(|w| w == b"\nxref").unwrap() + 1;
    let mut bad = bytes[..xref].to_vec();
    bad.extend_from_slice(b"xref ");
    bad.extend_from_slice(&[b' '; 100]);
    bad.extend_from_slice(&bytes[xref + 4..]);
    assert!(matches!(load(&bad), Err(LoadError::MalformedPdf(_))));
    let pdf = parsed();
    let id = model_id(&pdf);
    // lopdf overwrites /Length on write; replace the direct integer in-place afterwards, before xref,
    // rebuilding offsets in a tiny standalone fixture below instead of trusting stale offsets.
    let content = pdf.get_object(id).unwrap().as_stream().unwrap().content.clone();
    let object = format!("<< /Length 2 0 R >>\nstream\n{}\nendstream", String::from_utf8(content).unwrap());
    let bytes = raw_pdf(&[object.as_bytes(), b"1"], b"<< /Size 3 /Root 1 0 R >>");
    unsupported(&bytes, "indirect stream length");
}

fn raw_pdf(objects: &[&[u8]], trailer: &[u8]) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        bytes.extend_from_slice(object);
        bytes.extend_from_slice(b"\nendobj\n");
    }
    let xref = bytes.len();
    bytes.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(b"trailer\n");
    bytes.extend_from_slice(trailer);
    bytes.extend_from_slice(format!("\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    bytes
}

#[test]
#[ignore = "manual release /usr/bin/time -l measurement; rejects before lopdf allocation"]
fn lopdf_peak_memory_on_large_array() {
    let count = Limits::DEFAULT.max_pdf_objects * 16 + 1;
    let mut array = Vec::with_capacity(count * 2 + 2);
    array.push(b'[');
    for _ in 0..count {
        array.extend_from_slice(b"0 ");
    }
    array.push(b']');
    let bytes = raw_pdf(&[&array], b"<< /Size 2 /Root 1 0 R >>");
    let start = std::time::Instant::now();
    unsupported(&bytes, "direct object complexity limit");
    println!("{} bytes / {count} array entries rejected in {:?} before lopdf", bytes.len(), start.elapsed());
}
