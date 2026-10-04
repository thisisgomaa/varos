//! Frozen S5-E bytes: never generate fixtures inside these tests. Provenance is in fixture READMEs.
use std::path::PathBuf;
use varos_core::format::{Invalid, LimitKind, Limits, LoadError};
use varos_core::model::{GroupRole, Paint, Xform};
use varos_pdf::{load_vrs_bytes, load_vrs_checked, write_pdf};

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../varos-core/tests/fixtures").join(relative)
}
fn bytes(relative: &str) -> Vec<u8> {
    std::fs::read(fixture(relative)).unwrap()
}

/// A2: the frozen corpus is enforced, not just documented. Every `.vrs` in each folder must be listed
/// in its `SHA256SUMS` with the exact hash, and every listed file must exist (no missing, no extra).
#[test]
fn frozen_fixture_bytes_match_their_sha256sums() {
    use sha2::{Digest, Sha256};
    for folder in ["v2", "refused"] {
        let dir = fixture(folder);
        let sums = std::fs::read_to_string(dir.join("SHA256SUMS")).unwrap();
        let mut listed = std::collections::BTreeMap::new();
        for line in sums.lines().filter(|l| !l.trim().is_empty()) {
            let (hash, name) = line.split_once("  ").unwrap_or_else(|| panic!("{folder}: bad line {line:?}"));
            assert!(listed.insert(name.to_string(), hash.to_string()).is_none(), "{folder}: {name} listed twice");
        }
        let mut present = std::collections::BTreeSet::new();
        for entry in std::fs::read_dir(&dir).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            if name.ends_with(".vrs") {
                present.insert(name);
            }
        }
        let names: std::collections::BTreeSet<_> = listed.keys().cloned().collect();
        assert_eq!(present, names, "{folder}: fixtures and SHA256SUMS must list the same files");
        for (name, want) in &listed {
            let got: String =
                Sha256::digest(std::fs::read(dir.join(name)).unwrap()).iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(&got, want, "{folder}/{name}: frozen bytes changed");
        }
    }
}

#[test]
fn frozen_v2_twins_are_exact_and_byte_stable() {
    for scenario in ["masked_rotated", "boardless"] {
        let raw = bytes(&format!("v2/v2_{scenario}.vrs"));
        let pdf = bytes(&format!("v2/v2_{scenario}_pdf.vrs"));
        let a = load_vrs_bytes(&raw, &Limits::DEFAULT).unwrap();
        let b = load_vrs_bytes(&pdf, &Limits::DEFAULT).unwrap();
        assert_eq!(a, b, "{scenario}: container must preserve the complete document and metadata");
        assert_eq!(a.source_version, 2);
        assert!(!a.migrated && !a.released_legacy_masks);
        assert_eq!(a.notice(), None);
        assert_eq!(varos_core::file::doc_to_blob(&a.doc).unwrap().as_bytes(), raw, "{scenario}: raw frozen bytes");
        let saved = write_pdf(&a.doc).unwrap();
        assert_eq!(saved, pdf, "{scenario}: PDF frozen bytes");
        let reloaded = load_vrs_bytes(&saved, &Limits::DEFAULT).unwrap();
        assert_eq!(a.doc, reloaded.doc);
        assert_eq!(write_pdf(&reloaded.doc).unwrap(), saved);
    }
}

#[test]
fn frozen_art_retains_clip_hole_rotation_boards_and_arabic_name() {
    let d = load_vrs_checked(&fixture("v2/v2_masked_rotated_pdf.vrs"), &Limits::DEFAULT).unwrap().doc;
    let clip = d.node(15).unwrap();
    assert_eq!(clip.role, GroupRole::Clip);
    assert_eq!(clip.mask_child, Some(14));
    assert_eq!(clip.children, [14, 13]);
    assert_eq!(clip.xform, Xform { rot: 0.5, piv: [30.0, 30.0] });
    let donut = d.paths.iter().find(|p| p.id == 10).unwrap();
    assert_eq!(donut.fill, Paint::Solid([0.1, 0.4, 0.9, 1.0]));
    assert_eq!(donut.holes.len(), 1);
    assert_eq!(donut.holes[0].len(), 4);
    assert_eq!(d.artboards.len(), 2);
    assert_eq!(d.artboards[1].name, "لوحة ثانية");
    let boardless = load_vrs_checked(&fixture("v2/v2_boardless_pdf.vrs"), &Limits::DEFAULT).unwrap();
    assert!(boardless.doc.artboards.is_empty());
}

#[test]
fn frozen_legacy_broken_mask_repairs_only_in_memory_with_notice() {
    let path = fixture("v2/v1_broken_mask.vrs");
    let original = std::fs::read(&path).unwrap();
    let loaded = load_vrs_checked(&path, &Limits::DEFAULT).unwrap();
    assert_eq!(loaded.source_version, 1);
    assert!(loaded.migrated && loaded.released_legacy_masks);
    assert!(loaded.notice().unwrap().contains("broken clipping masks released"));
    let mut expected = load_vrs_checked(&fixture("v2/v2_masked_rotated.vrs"), &Limits::DEFAULT).unwrap().doc;
    expected.release_clip(15);
    assert_eq!(loaded.doc, expected);
    assert_eq!(std::fs::read(path).unwrap(), original);
    let reopened = load_vrs_bytes(&write_pdf(&loaded.doc).unwrap(), &Limits::DEFAULT).unwrap();
    assert_eq!(reopened.doc, loaded.doc);
    assert_eq!(reopened.notice(), None);
}

#[test]
fn frozen_refusals_keep_their_typed_reason_on_bytes_and_disk() {
    let cases = [
        "v3_future",
        "missing_version",
        "zero_version",
        "unknown_field",
        "cycle_nodes",
        "duplicate_id",
        "id_overflow",
        "opacity_range",
        "broken_mask",
        "nonchild_mask",
        "tree_depth",
        "nonfinite_number",
        "version_mismatch",
        "filtered_model",
        "incremental",
        "xref_stream",
        "missing_model",
        "future_pdf",
    ];
    for name in cases {
        let path = fixture(&format!("refused/{name}.vrs"));
        let raw = std::fs::read(&path).unwrap();
        let err = load_vrs_bytes(&raw, &Limits::DEFAULT).unwrap_err();
        assert_eq!(load_vrs_checked(&path, &Limits::DEFAULT).unwrap_err(), err, "{name}: both entry points");
        let expected = match name {
            "v3_future" | "future_pdf" => err == LoadError::NewerVersion { found: 3, supported: 2 },
            "missing_version" => err == LoadError::MissingVersion,
            "zero_version" => err == LoadError::InvalidVersion("0".into()),
            "unknown_field" => matches!(&err, LoadError::Malformed { detail, .. } if detail.contains("unknown field")),
            "cycle_nodes" => matches!(err, LoadError::Invalid(Invalid::Cycle { node: 1 })),
            "duplicate_id" => err == LoadError::Invalid(Invalid::DuplicateId { kind: "path", id: 10 }),
            "id_overflow" => err == LoadError::Invalid(Invalid::IdExhausted),
            "opacity_range" => {
                matches!(&err, LoadError::Invalid(Invalid::OutOfRange { what, value }) if what == "path 10: opacity" && *value == 2.0)
            }
            "broken_mask" => err == LoadError::Invalid(Invalid::Dangling { from: "node", id: 15, missing: 999 }),
            "nonchild_mask" => matches!(err, LoadError::Invalid(Invalid::BadMask { group: 15, .. })),
            "tree_depth" => matches!(err, LoadError::TooLarge { limit: LimitKind::TreeDepth, found: 65, max: 64 }),
            "nonfinite_number" => {
                matches!(&err, LoadError::Invalid(Invalid::NonFinite { what }) if what == "path 10: opacity")
            }
            "version_mismatch" => err == LoadError::VersionMismatch { container: 1, model: 2 },
            "filtered_model" => err == LoadError::UnsupportedPdf("model encoding".into()),
            "incremental" => err == LoadError::UnsupportedPdf("incremental update".into()),
            "xref_stream" => err == LoadError::UnsupportedPdf("compressed cross-reference".into()),
            "missing_model" => err == LoadError::NoEmbeddedModel,
            _ => unreachable!(),
        };
        assert!(expected, "{name}: unexpected refusal: {err:?}");
        assert!(!err.to_string().is_empty());
        assert_eq!(std::fs::read(path).unwrap(), raw, "{name}: original stays untouched");
    }
}

#[test]
fn frozen_inputs_obey_file_model_and_container_budgets() {
    let raw = bytes("v2/v2_masked_rotated.vrs");
    let pdf = bytes("v2/v2_masked_rotated_pdf.vrs");
    for b in [&raw, &pdf] {
        let limits = Limits { max_file_bytes: b.len() as u64 - 1, ..Limits::DEFAULT };
        assert!(matches!(load_vrs_bytes(b, &limits), Err(LoadError::TooLarge { limit: LimitKind::FileBytes, .. })));
        let limits = Limits { max_model_bytes: raw.len() - 1, ..Limits::DEFAULT };
        assert!(matches!(load_vrs_bytes(b, &limits), Err(LoadError::TooLarge { limit: LimitKind::ModelBytes, .. })));
    }
    let limits = Limits { max_pdf_objects: 1, ..Limits::DEFAULT };
    assert!(matches!(load_vrs_bytes(&pdf, &limits), Err(LoadError::TooLarge { limit: LimitKind::PdfObjects, .. })));
    let limits = Limits { max_tree_depth: 1, ..Limits::DEFAULT };
    assert!(matches!(load_vrs_bytes(&raw, &limits), Err(LoadError::TooLarge { limit: LimitKind::TreeDepth, .. })));
}
