//! Ahmed's hand test 0 (`docs/foundation/work_orders/DFS_S5_FORMAT_V2.md` §1): a required, not
//! optional, precondition run on the Mac BEFORE any S5 code reaches `main`. It walks every real
//! `.vrs`/`.json` file under `VAROS_CORPUS_DIR` and reports each one OK or refused-with-reason. It
//! never writes anything — this is read-only reconnaissance over Ahmed's own files, never a repair.
//!
//! Run with (see also `VRS_FORMAT.md` §Corpus check):
//!   VAROS_CORPUS_DIR=~/Documents CARGO_TARGET_DIR=/home/user/varos/target-s5 \
//!     cargo test -p varos-pdf --test corpus_check -- --ignored --nocapture
//!
//! Skipped (not failed) when `VAROS_CORPUS_DIR` is unset, so it never runs in ordinary CI.

use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrs") || e.eq_ignore_ascii_case("json")) {
            out.push(path);
        }
    }
}

#[test]
#[ignore = "run explicitly: needs VAROS_CORPUS_DIR pointed at real .vrs files, and is a hand test, not CI"]
fn corpus_check() {
    let Ok(dir) = std::env::var("VAROS_CORPUS_DIR") else {
        eprintln!("VAROS_CORPUS_DIR is unset — skipping (this is a hand test, not a CI gate)");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut files = Vec::new();
    walk(&dir, &mut files);
    files.sort();

    if files.is_empty() {
        println!("no .vrs/.json files found under {}", dir.display());
        return;
    }

    let mut ok = 0usize;
    let mut refused = 0usize;
    for path in &files {
        // Read-only: `load_vrs` never mutates the file. Never `save_vrs` here.
        match varos_pdf::load_vrs(path) {
            Ok(_doc) => {
                ok += 1;
                println!("OK       {}", path.display());
            }
            Err(reason) => {
                refused += 1;
                println!("REFUSED  {} — {reason}", path.display());
            }
        }
    }
    println!("---");
    println!("{ok} OK, {refused} refused, {} total under {}", files.len(), dir.display());
    if refused > 0 {
        println!(
            "Any refusal of one of your own files blocks the merge (DFS_S5_FORMAT_V2.md §1, hand test 0) \
             — send this output before S5 reaches main."
        );
    }
}
