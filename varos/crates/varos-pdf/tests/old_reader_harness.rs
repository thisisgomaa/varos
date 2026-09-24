//! The OLD-READER harness (DFS S5-E, ADR-0008 rule 4): proves that the reader logic every
//! `.vrs`-capable build has shipped since `7a5b3c8` — refuse a newer format BEFORE any typed
//! decode — still does its job. This does not run the old *binary*; it runs a frozen copy of its
//! *gate*, fed the CURRENT build's own output. That is deliberately the whole test: once S5-B
//! raises `FORMAT_VERSION` past 1, this build's own files become "newer" to a v1-only gate.
//!
//! Honesty note: this proves the frozen *logic*, not the old *binary*. The binary is covered by
//! Ahmed's hand test 3 (`docs/foundation/work_orders/DFS_S5_FORMAT_V2.md` §1).

use varos_core::model::{Anchor, Document, Path};

/// ≤15-line VERBATIM copy of the version gate at `ecf67f5:varos/crates/varos-core/src/file.rs:28-35`
/// (the typed `VrsFile` decode that follows it in the real function never runs here — the gate
/// returns first by construction, exactly as it does in `doc_from_blob`). Diff this against
/// `git show ecf67f5:varos/crates/varos-core/src/file.rs` to check it is still exact.
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

/// The old gate refuses THIS BUILD's own raw-JSON `.vrs` output before any typed decode.
///
/// FAILS NOW: `varos_core::file::VRS_VERSION` is still 1 (S5-B has not merged), so today's writer
/// output carries `"varos":1` and the v1-only gate accepts it — there is nothing "newer" yet to
/// refuse. It PASSES once S5-B raises `FORMAT_VERSION`/`VRS_VERSION` to 2, at which point this same
/// gate correctly calls the new output "a newer Varos" and this assertion starts holding.
#[test]
#[ignore = "fails until S5-B raises the write version past 1 — see the doc comment above"]
fn old_reader_refuses_v2_json_before_decode() {
    let body = varos_core::file::doc_to_blob(&sample_doc()).expect("current writer saves");
    let err = old_gate(&body).expect_err("the current build's own output must look 'newer' to the frozen v1 gate");
    assert!(err.contains("newer"), "refusal must name the problem, got: {err}");
}

/// The same proof through the PDF container: the model embedded by THIS BUILD's PDF writer is
/// refused by the frozen gate before any typed decode. Same fails-now/passes-after-S5-B shape as
/// the JSON test above.
#[test]
#[ignore = "fails until S5-B raises the write version past 1 — see old_reader_refuses_v2_json_before_decode"]
fn old_reader_refuses_v2_pdf_before_decode() {
    let pdf_bytes = varos_pdf::write_pdf(&sample_doc()).expect("current PDF writer saves");
    let body = embedded_model_json(&pdf_bytes);
    let err = old_gate(&body).expect_err("the current build's own PDF output must look 'newer' to the frozen v1 gate");
    assert!(err.contains("newer"), "refusal must name the problem, got: {err}");
}
