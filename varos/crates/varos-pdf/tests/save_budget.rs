//! A1: nothing Varos saves may be refused when it is reopened. The writer's output must fit the
//! reader's own limits (`Limits::DEFAULT`, the one source); `write_pdf_checked` decides at save
//! time — never at reopen — from cheap writer-side counts, and runs the full reopen decode only
//! when a count is within 10 % of its limit.
use varos_core::format::Limits;
use varos_core::model::{Anchor, Document, Path};
use varos_pdf::{load_vrs_bytes, write_pdf, write_pdf_checked_report};

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}

/// `n` closed squares, each filled and stroked with a TRANSLUCENT stroke (the knockout-group case,
/// the most PDF objects a path can cost).
fn knockout_doc(n: u32) -> Document {
    let mut d = Document::default();
    for i in 1..=n {
        let (x, y) = ((i % 200) as f32 * 4.0, (i / 200) as f32 * 4.0);
        let a = 4 * i + 1_000_000;
        let anchors = vec![anc(a, x, y), anc(a + 1, x + 3.0, y), anc(a + 2, x + 3.0, y + 3.0), anc(a + 3, x, y + 3.0)];
        d.paths.push(Path::new(i, anchors, true, Some([0.2, 0.4, 0.8, 1.0]), Some([0.0, 0.0, 0.0, 0.5]), 1.0));
    }
    d.ids = 4 * n + 1_000_004;
    d.sync_tree();
    d
}

#[test]
fn knockouts_share_objects_and_an_ordinary_save_skips_the_decoder() {
    let n = 2_000;
    let doc = knockout_doc(n);
    let (bytes, check) = write_pdf_checked_report(&doc, &Limits::DEFAULT).unwrap();
    assert!(!check.decoded, "an ordinary save never runs the full decode: {check:?}");
    // One Form XObject per knockout; their ExtGStates, /Group and /Resources are shared per page.
    assert!(check.objects <= n as usize + 16, "objects per knockout must stay ~1: {check:?}");
    assert!(check.tokens < Limits::DEFAULT.max_pdf_tokens() / 10, "{check:?}");
    assert_eq!(load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap().doc.paths.len(), n as usize, "checked reopen");
    assert_eq!(bytes, write_pdf(&doc).unwrap(), "the gate writes exactly the native bytes");
}

#[test]
fn a_near_limit_save_runs_the_decoder_and_an_over_limit_one_is_refused_by_the_writer_count() {
    let doc = knockout_doc(200);
    let (written, normal) = write_pdf_checked_report(&doc, &Limits::DEFAULT).unwrap();
    // Within 10 % of a limit (here the file size): the full reopen decode is the final word, and it passes.
    let near = Limits { max_file_bytes: written.len() as u64 * 105 / 100, ..Limits::DEFAULT };
    let (bytes, check) = write_pdf_checked_report(&doc, &near).unwrap();
    assert!(check.decoded, "{check:?}");
    assert!(load_vrs_bytes(&bytes, &near).is_ok());
    // Over the limit: refused from the writer's own count, before any decode or disk write.
    let over = Limits { max_pdf_objects: normal.objects, ..Limits::DEFAULT };
    let err = write_pdf_checked_report(&doc, &over).unwrap_err();
    assert!(!err.decoded, "{err:?}");
    assert!(err.reason.starts_with("This document can't be saved:") && err.reason.contains("limit"), "{err:?}");
    assert!(load_vrs_bytes(&write_pdf(&doc).unwrap(), &over).is_err(), "the reader would indeed refuse it");
    // The token budget is enforced from the same shared constant.
    let tokens = Limits { max_pdf_objects: normal.tokens / Limits::PDF_TOKENS_PER_OBJECT, ..Limits::DEFAULT };
    let err = write_pdf_checked_report(&doc, &tokens).unwrap_err();
    assert!(!err.decoded && err.reason.contains("can't be saved"), "{err:?}");
}

#[test]
fn the_decoded_stream_budget_is_checked_by_the_writer_and_triggers_the_decode_near_it() {
    let doc = knockout_doc(50);
    let (written, normal) = write_pdf_checked_report(&doc, &Limits::DEFAULT).unwrap();
    assert!(!normal.decoded && normal.model_bytes > 0);
    // Small decoded-stream budget, generous everything else: refused by the save check, not the reopen.
    let small = Limits { max_decoded_stream_bytes: normal.model_bytes - 1, ..Limits::DEFAULT };
    let err = write_pdf_checked_report(&doc, &small).unwrap_err();
    assert!(!err.decoded, "{err:?}");
    assert!(err.reason.starts_with("This document can't be saved:"), "{err:?}");
    assert!(load_vrs_bytes(&written, &small).is_err(), "the reader would indeed refuse it");
    // The model at 95 % of the budget: the full reopen decode runs, and it passes.
    let near = Limits { max_decoded_stream_bytes: normal.model_bytes * 100 / 95, ..Limits::DEFAULT };
    let (bytes, check) = write_pdf_checked_report(&doc, &near).unwrap();
    assert!(check.decoded, "{check:?}");
    assert!(load_vrs_bytes(&bytes, &near).is_ok());
}

/// The largest document the model limits allow (~66 s in a debug build, ~20 s in release), so it is
/// opt-in: `cargo test --release -p varos-pdf --test save_budget -- --ignored`.
#[test]
#[ignore = "slow: maximum-size document; run with -- --ignored (release recommended)"]
fn the_largest_allowed_knockout_document_saves_and_reopens() {
    // + Layer 1 = exactly max_nodes nodes.
    let n = Limits::DEFAULT.max_nodes as u32 - 1;
    let doc = knockout_doc(n);
    let t = std::time::Instant::now();
    let plain = write_pdf(&doc).unwrap();
    let write = t.elapsed();
    let t = std::time::Instant::now();
    let (bytes, check) = write_pdf_checked_report(&doc, &Limits::DEFAULT).unwrap_or_else(|e| panic!("{}", e.reason));
    println!("{n} knockouts: write {write:?}, checked save {:?}, {check:?}", t.elapsed());
    assert_eq!(bytes, plain);
    let reopened = load_vrs_bytes(&bytes, &Limits::DEFAULT).unwrap_or_else(|e| panic!("reopen refused: {e:?}"));
    assert_eq!(reopened.doc.paths.len(), n as usize);
}
