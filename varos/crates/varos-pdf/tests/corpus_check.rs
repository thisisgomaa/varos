//! Read-only personal-corpus acceptance. Point VAROS_CORPUS_DIR at a dedicated document folder:
//! every regular .vrs/.json file is treated as a Varos document. Symlinks are reported and skipped.
//! The explicit ignored run FAILS for missing configuration, an empty corpus, traversal errors or
//! any refused file; an ordinary CI run does not claim personal-corpus acceptance.

use std::path::{Path, PathBuf};

fn walk(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    if !std::fs::symlink_metadata(dir)?.file_type().is_dir() {
        return Err(std::io::Error::other("corpus root must be a real directory, not a symlink"));
    }
    let mut pending = vec![dir.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_symlink() {
                println!("SKIPPED symlink {}", path.display());
            } else if kind.is_dir() {
                pending.push(path);
            } else if kind.is_file()
                && path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vrs") || e.eq_ignore_ascii_case("json"))
            {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn check(dir: &Path) -> Result<usize, String> {
    let files = walk(dir).map_err(|e| format!("corpus traversal failed: {e}"))?;
    if files.is_empty() {
        return Err(format!("no .vrs/.json documents under {}; acceptance is unverified", dir.display()));
    }
    let mut refused = 0;
    for path in &files {
        // Read only. Never save, normalize on disk, or repair originals here.
        match varos_pdf::load_vrs_checked(path, &varos_core::format::Limits::DEFAULT) {
            Ok(loaded) => {
                println!("OK       {}{}", path.display(), loaded.notice().map_or(String::new(), |n| format!(" — {n}")))
            }
            Err(reason) => {
                refused += 1;
                println!("REFUSED  {} — {reason}", path.display());
            }
        }
    }
    println!("{} OK, {refused} refused, {} total under {}", files.len() - refused, files.len(), dir.display());
    if refused > 0 {
        Err(format!("{refused} refused document(s); personal-corpus acceptance blocks the S5 merge until resolved"))
    } else {
        Ok(files.len())
    }
}

#[test]
#[ignore = "requires VAROS_CORPUS_DIR pointing at personal Varos documents; not fixture/CI acceptance"]
fn corpus_check() {
    let dir =
        std::env::var_os("VAROS_CORPUS_DIR").expect("set VAROS_CORPUS_DIR to a dedicated personal-document folder");
    check(Path::new(&dir)).unwrap();
}

struct Temp(PathBuf);
impl Temp {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("varos-corpus-{}-{name}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn missing_empty_and_refused_corpora_fail_without_modifying_files() {
    let dir = Temp::new("refusals");
    assert!(check(&dir.0.join("missing")).unwrap_err().contains("traversal"));
    assert!(check(&dir.0).unwrap_err().contains("no .vrs/.json"));
    let valid = varos_pdf::write_pdf(&varos_core::model::Document::default()).unwrap();
    let path = dir.0.join("valid.vrs");
    std::fs::write(&path, &valid).unwrap();
    assert_eq!(check(&dir.0), Ok(1));
    let bad = dir.0.join("future.json");
    let bytes = br#"{"varos":3,"doc":42}"#;
    std::fs::write(&bad, bytes).unwrap();
    assert!(check(&dir.0).unwrap_err().contains("1 refused"));
    assert_eq!(std::fs::read(path).unwrap(), valid);
    assert_eq!(std::fs::read(bad).unwrap(), bytes);
}

#[test]
fn walker_includes_nested_case_insensitive_documents() {
    let dir = Temp::new("nested");
    let nested = dir.0.join("nested");
    std::fs::create_dir(&nested).unwrap();
    std::fs::write(nested.join("a.VRS"), []).unwrap();
    std::fs::write(nested.join("b.JSON"), []).unwrap();
    std::fs::write(nested.join("notes.txt"), []).unwrap();
    assert_eq!(walk(&dir.0).unwrap(), vec![nested.join("a.VRS"), nested.join("b.JSON")]);
}

#[cfg(unix)]
#[test]
fn walker_does_not_follow_symlink_cycles_or_external_files() {
    use std::os::unix::fs::symlink;
    let dir = Temp::new("symlinks");
    symlink(&dir.0, dir.0.join("cycle")).unwrap();
    symlink(dir.0.join("absent"), dir.0.join("linked.vrs")).unwrap();
    assert!(walk(&dir.0).unwrap().is_empty());
    assert!(walk(&dir.0.join("cycle")).is_err());
}
