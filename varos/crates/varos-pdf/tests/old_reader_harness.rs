//! The OLD-READER harness (DFS S5-E, ADR-0008 rule 4): proves that the reader logic every
//! `.vrs`-capable build has shipped since `7a5b3c8` — refuse a newer format BEFORE any typed
//! decode — still does its job. This does not run the old *binary*; it runs a frozen adaptation of its
//! *gate*, fed the CURRENT build's own output. S5-B raised the writer to format 2, so these checks now run in the default suite.
//! Format 3 (2026-10-04, board metadata) adds a second frozen gate: the format-2 reader's
//! `peek_version` from `f21c20e`, which must refuse every format-3 file before typed decode.
//! Format 4 (2026-10-07, artboard ids) adds a third: the format-3 reader's `peek_version` from
//! `a5f687b`, which must refuse every format-4 file before typed decode.
//!
//! Honesty note: this proves the frozen *logic*, not the old *binary*. The binary is covered by
//! Ahmed's hand test 3 (`docs/foundation/work_orders/DFS_S5_FORMAT_V2.md` §1).

use varos_core::model::{Anchor, Document, Path};

/// Frozen version gate from `ecf67f5:varos/crates/varos-core/src/file.rs:28-35`
/// `VRS_VERSION` is replaced by its old value, 1; the rest of the refusal logic is retained.
/// (the typed `VrsFile` decode that follows it in the real function never runs here — the gate
/// returns first by construction, exactly as it does in `doc_from_blob`). Diff this against
/// `git show ecf67f5:varos/crates/varos-core/src/file.rs` to check its provenance.
fn old_gate(body: &str) -> Result<(), String> {
    #[derive(serde::Deserialize)]
    struct VrsHead {
        varos: u32,
    } // serde skips unknown fields, so this reads ANY .vrs generation
    let head: VrsHead = serde_json::from_str(body).map_err(|e| format!("not a valid .vrs model: {e}"))?;
    if head.varos > 1 {
        return Err(format!("this file was saved by a newer Varos (v{}) — please update", head.varos));
    }
    Ok(())
}

/// Frozen version gate of the FORMAT-2 reader: `f21c20e:varos/crates/varos-core/src/format/mod.rs`
/// `peek_version` with `FORMAT_VERSION` replaced by its value then, 2, and the `NewerVersion`
/// `Display` text from `format/error.rs` inlined. Only the newer-version branch matters here; the
/// missing/invalid branches are kept so the adaptation stays recognisably the same function.
fn v2_gate(body: &str) -> Result<u32, String> {
    #[derive(serde::Deserialize)]
    struct Head {
        #[serde(default, deserialize_with = "present")]
        varos: Option<serde_json::Value>,
    }
    fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
        <serde_json::Value as serde::Deserialize>::deserialize(d).map(Some)
    }
    let head: Head = serde_json::from_str(body).map_err(|e| format!("malformed: {e}"))?;
    let value = head.varos.ok_or("missing version")?;
    let version = match value.as_u64() {
        Some(0) | None => return Err(format!("invalid version {value}")),
        Some(v) => u32::try_from(v).map_err(|_| format!("invalid version {value}"))?,
    };
    if version > 2 {
        return Err(format!(
            "This file needs a newer Varos. It uses file format {version}; this build supports up to 2. \
             Update Varos to open it. The file has not been changed."
        ));
    }
    Ok(version)
}
const V2_REFUSES_V3: &str = "This file needs a newer Varos. It uses file format 3; this build supports up to 2. Update Varos to open it. The file has not been changed.";

/// A minimal but non-trivial document, just enough to exercise a real save.
fn sample_doc() -> Document {
    let mut d = Document::default();
    let a = |id: u32, x: f32, y: f32| Anchor { id, p: [x, y], hin: None, hout: None, smooth: false };
    d.paths.push(Path::new(
        1,
        vec![a(1, 0.0, 0.0), a(2, 10.0, 0.0), a(3, 5.0, 10.0)],
        true,
        Some([1.0, 0.0, 0.0, 1.0]),
        None,
        1.0,
    ));
    d.ids = 3;
    d.sync_tree();
    d
}

/// Test plumbing only (NOT part of the frozen gate under test): pull the embedded model bytes out
/// of a `.vrs` PDF via its catalog `/VAROS_Model` key — every Varos PDF writer has used this exact
/// key since `b06194a`, so it is stable across the version bump this harness is about.
fn embedded_model_json(pdf_bytes: &[u8]) -> String {
    let doc = lopdf::Document::load_mem(pdf_bytes).expect("a Varos-written PDF must be readable by lopdf");
    let catalog = doc.catalog().expect("Varos PDFs always have a catalog");
    let obj = catalog.get(b"VAROS_Model").expect("Varos PDFs always set /VAROS_Model");
    let (_, o) = doc.dereference(obj).expect("catalog /VAROS_Model must dereference");
    let s = o.as_stream().expect("/VAROS_Model is a stream");
    String::from_utf8(s.content.clone()).expect("the embedded model is UTF-8 JSON")
}

/// Frozen version gate of the FORMAT-3 reader: `a5f687b:varos/crates/varos-core/src/format/mod.rs`
/// `peek_version` with `FORMAT_VERSION` replaced by its value then, 3, and the `NewerVersion` `Display`
/// text inlined — the same function as `v2_gate` with the next number (format 4, 2026-10-07, artboard
/// ids). A format-3 build must refuse every format-4 file before typed decode, so it can never open a
/// board, drop its artboard ids as unknown, and save the loss.
fn v3_gate(body: &str) -> Result<u32, String> {
    #[derive(serde::Deserialize)]
    struct Head {
        #[serde(default, deserialize_with = "present")]
        varos: Option<serde_json::Value>,
    }
    fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
        <serde_json::Value as serde::Deserialize>::deserialize(d).map(Some)
    }
    let head: Head = serde_json::from_str(body).map_err(|e| format!("malformed: {e}"))?;
    let value = head.varos.ok_or("missing version")?;
    let version = match value.as_u64() {
        Some(0) | None => return Err(format!("invalid version {value}")),
        Some(v) => u32::try_from(v).map_err(|_| format!("invalid version {value}"))?,
    };
    if version > 3 {
        return Err(format!(
            "This file needs a newer Varos. It uses file format {version}; this build supports up to 3. \
             Update Varos to open it. The file has not been changed."
        ));
    }
    Ok(version)
}
const V2_REFUSES_V4: &str = "This file needs a newer Varos. It uses file format 4; this build supports up to 2. Update Varos to open it. The file has not been changed.";
const V3_REFUSES_V4: &str = "This file needs a newer Varos. It uses file format 4; this build supports up to 3. Update Varos to open it. The file has not been changed.";

/// The v1-era gate refuses the frozen v2 and v3 files and every format-4 file (fresh and frozen)
/// before typed decode, naming the file's own number.
#[test]
fn old_reader_refuses_v2_v3_and_v4_json_before_decode() {
    let fresh = include_str!("../../varos-core/tests/fixtures/v4/v4_board_meta.vrs").to_owned();
    let v2 = include_str!("../../varos-core/tests/fixtures/v2/v2_masked_rotated.vrs");
    assert_eq!(old_gate(v2).unwrap_err(), "this file was saved by a newer Varos (v2) — please update");
    let v3 = include_str!("../../varos-core/tests/fixtures/v3/v3_board_meta.vrs");
    assert_eq!(old_gate(v3).unwrap_err(), "this file was saved by a newer Varos (v3) — please update");
    for body in [fresh.as_str(), include_str!("../../varos-core/tests/fixtures/v4/v4_board_meta.vrs")] {
        assert_eq!(old_gate(body).unwrap_err(), "this file was saved by a newer Varos (v4) — please update");
    }
}

/// The same gate proof through fresh and frozen PDF model streams (not an old binary run).
#[test]
fn old_reader_refuses_v2_v3_and_v4_pdf_before_decode() {
    let fresh = include_bytes!("../../varos-core/tests/fixtures/v4/v4_board_meta_pdf.vrs").to_vec();
    let v2 = include_bytes!("../../varos-core/tests/fixtures/v2/v2_masked_rotated_pdf.vrs");
    assert_eq!(
        old_gate(&embedded_model_json(v2)).unwrap_err(),
        "this file was saved by a newer Varos (v2) — please update"
    );
    let v3 = include_bytes!("../../varos-core/tests/fixtures/v3/v3_board_meta_pdf.vrs");
    assert_eq!(
        old_gate(&embedded_model_json(v3)).unwrap_err(),
        "this file was saved by a newer Varos (v3) — please update"
    );
    for pdf in [fresh.as_slice(), include_bytes!("../../varos-core/tests/fixtures/v4/v4_board_meta_pdf.vrs").as_slice()]
    {
        assert_eq!(
            old_gate(&embedded_model_json(pdf)).unwrap_err(),
            "this file was saved by a newer Varos (v4) — please update"
        );
    }
}

/// The format-2 reader's gate refuses frozen format-3 and fresh/frozen format-4 output (raw and PDF)
/// with its own readable "newer Varos" message.
#[test]
fn v2_reader_refuses_v3_and_v4_raw_and_pdf_before_decode() {
    let frozen_raw = include_str!("../../varos-core/tests/fixtures/v3/v3_board_meta.vrs");
    let frozen_pdf = embedded_model_json(include_bytes!("../../varos-core/tests/fixtures/v3/v3_board_meta_pdf.vrs"));
    for body in [frozen_raw, frozen_pdf.as_str()] {
        assert_eq!(v2_gate(body).unwrap_err(), V2_REFUSES_V3);
    }
    let fresh_raw = include_str!("../../varos-core/tests/fixtures/v4/v4_board_meta.vrs").to_owned();
    let fresh_pdf = embedded_model_json(include_bytes!("../../varos-core/tests/fixtures/v4/v4_board_meta_pdf.vrs"));
    let frozen_raw = include_str!("../../varos-core/tests/fixtures/v4/v4_board_meta.vrs");
    let frozen_pdf = embedded_model_json(include_bytes!("../../varos-core/tests/fixtures/v4/v4_board_meta_pdf.vrs"));
    for body in [fresh_raw.as_str(), fresh_pdf.as_str(), frozen_raw, frozen_pdf.as_str()] {
        assert_eq!(v2_gate(body).unwrap_err(), V2_REFUSES_V4);
    }
    // the inlined copy is the current core text with the numbers of a v2 build
    let core = varos_core::format::LoadError::NewerVersion { found: 3, supported: 2 }.to_string();
    assert_eq!(core, V2_REFUSES_V3);
}

/// The format-3 reader's gate refuses fresh and frozen format-4 output (raw and PDF) — so a v3 build
/// can never open a board, drop its artboard ids as unknown, and save the loss.
#[test]
fn v3_reader_refuses_v4_raw_and_pdf_before_decode() {
    let fresh_raw = include_str!("../../varos-core/tests/fixtures/v4/v4_board_meta.vrs").to_owned();
    let fresh_pdf = embedded_model_json(include_bytes!("../../varos-core/tests/fixtures/v4/v4_board_meta_pdf.vrs"));
    let frozen_raw = include_str!("../../varos-core/tests/fixtures/v4/v4_board_meta.vrs");
    let frozen_pdf = embedded_model_json(include_bytes!("../../varos-core/tests/fixtures/v4/v4_board_meta_pdf.vrs"));
    for body in [fresh_raw.as_str(), fresh_pdf.as_str(), frozen_raw, frozen_pdf.as_str()] {
        assert_eq!(v3_gate(body).unwrap_err(), V3_REFUSES_V4);
    }
    let core = varos_core::format::LoadError::NewerVersion { found: 4, supported: 3 }.to_string();
    assert_eq!(core, V3_REFUSES_V4);
}

#[test]
fn frozen_v3_gate_still_accepts_v1_v2_and_v3_headers() {
    assert_eq!(v3_gate(include_str!("../../varos-core/tests/fixtures/v1/v1_masked.vrs")), Ok(1));
    assert_eq!(v3_gate(include_str!("../../varos-core/tests/fixtures/v2/v2_masked_rotated.vrs")), Ok(2));
    assert_eq!(v3_gate(include_str!("../../varos-core/tests/fixtures/v3/v3_board_meta.vrs")), Ok(3));
}

#[test]
fn frozen_v2_gate_still_accepts_v1_and_v2_headers() {
    assert_eq!(v2_gate(include_str!("../../varos-core/tests/fixtures/v1/v1_masked.vrs")), Ok(1));
    assert_eq!(v2_gate(include_str!("../../varos-core/tests/fixtures/v2/v2_masked_rotated.vrs")), Ok(2));
}

#[test]
fn frozen_gate_still_accepts_v1_headers() {
    assert_eq!(old_gate(include_str!("../../varos-core/tests/fixtures/v1/v1_masked.vrs")), Ok(()));
    assert_eq!(
        old_gate(&embedded_model_json(include_bytes!("../../varos-core/tests/fixtures/v1/v1_masked_pdf.vrs"))),
        Ok(())
    );
}

/// Frozen v4 gate from base 7b48f2c. Typed decode is deliberately absent.
fn v4_gate(body: &str) -> Result<u32, String> {
    #[derive(serde::Deserialize)]
    struct Head {
        #[serde(default, deserialize_with = "present")]
        varos: Option<serde_json::Value>,
    }
    fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
        <serde_json::Value as serde::Deserialize>::deserialize(d).map(Some)
    }
    let head: Head = serde_json::from_str(body).map_err(|e| format!("malformed: {e}"))?;
    let value = head.varos.ok_or("missing version")?;
    let version = match value.as_u64() {
        Some(0) | None => return Err(format!("invalid version {value}")),
        Some(v) => u32::try_from(v).map_err(|_| format!("invalid version {value}"))?,
    };
    if version > 4 {
        return Err(format!(
            "This file needs a newer Varos. It uses file format {version}; this build supports up to 4. \
             Update Varos to open it. The file has not been changed."
        ));
    }
    Ok(version)
}
#[test]
fn v4_reader_refuses_v5_before_decode() {
    // Historical v5 evidence remains v5 after later writer bumps.
    let raw = include_str!("../../varos-core/tests/fixtures/v5/plain.json");
    let pdf = embedded_model_json(include_bytes!("../../varos-core/tests/fixtures/v5/plain.pdf"));
    for body in [raw, pdf.as_str(), r#"{"varos":5,"doc":42}"#] {
        assert_eq!(
            v4_gate(body).unwrap_err(),
            varos_core::format::LoadError::NewerVersion { found: 5, supported: 4 }.to_string()
        );
    }
    assert_eq!(
        v4_gate(include_str!("../../varos-core/tests/fixtures/refused/v5_future.vrs")).unwrap_err(),
        varos_core::format::LoadError::NewerVersion { found: 5, supported: 4 }.to_string()
    );
}

#[test]
fn current_next_writer_is_refused_by_frozen_v4_gate() {
    let doc = sample_doc();
    let raw = varos_core::file::doc_to_blob(&doc).unwrap();
    let pdf = embedded_model_json(&varos_pdf::write_pdf(&doc).unwrap());
    for body in [raw.as_str(), pdf.as_str()] {
        assert_eq!(
            v4_gate(body).unwrap_err(),
            varos_core::format::LoadError::NewerVersion { found: varos_core::format::FORMAT_VERSION, supported: 4 }
                .to_string()
        );
    }
}
