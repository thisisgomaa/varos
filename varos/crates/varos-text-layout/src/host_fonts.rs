//! Explicit desktop host discovery, outside core and the byte-fed shaping engine.
//! Bounded, immutable process snapshot. Missing/exchanged saved hashes are refused by the adapter.
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use varos_text::{FaceId, FallbackPolicy, FontFace, FontSet};
static SNAPSHOT: OnceLock<Result<FontSet, String>> = OnceLock::new();
pub fn snapshot() -> Result<FontSet, String> {
    SNAPSHOT.get_or_init(discover).clone()
}
fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(target_os = "macos")]
    {
        roots.push(PathBuf::from("/System/Library/Fonts/Supplemental"));
        roots.push(PathBuf::from("/Library/Fonts"));
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Library/Fonts"));
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(windir) = std::env::var_os("WINDIR") {
            roots.push(PathBuf::from(windir).join("Fonts"));
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        roots.push(PathBuf::from("/usr/share/fonts"));
    }
    roots
}
fn files(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 || out.len() >= 512 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(Result::ok).map(|e| e.path()).take(1024).collect();
    paths.sort();
    for path in paths {
        if out.len() >= 512 {
            break;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            files(&path, depth + 1, out);
        } else if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
            && meta.len() <= 16 * 1024 * 1024
        {
            out.push(path);
        }
    }
}
fn discover() -> Result<FontSet, String> {
    let mut faces = super::bundled_fonts()?.faces().to_vec();
    let mut paths = Vec::new();
    for root in roots() {
        files(&root, 0, &mut paths);
    }
    let mut bytes_total = 0;
    for path in paths {
        if faces.len() >= 128 || bytes_total >= 128 * 1024 * 1024 {
            break;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        bytes_total += bytes.len();
        let mut db = fontdb::Database::new();
        db.load_font_data(bytes.clone());
        let mut loaded = db.faces();
        let Some(face) = loaded.next() else {
            continue;
        };
        if loaded.next().is_some() || face.style != fontdb::Style::Normal || ![400, 500, 600].contains(&face.weight.0) {
            continue;
        }
        let Some((family, _)) = face.families.first() else {
            continue;
        };
        if family.len() > 256
            || faces.iter().any(|f| f.family == family && f.weight == face.weight.0)
            || varos_text::Engine::validate_font(&bytes).is_err()
        {
            continue;
        }
        // Engine font family identities have a static lifetime; this bounded snapshot lives until exit.
        let family: &'static str = Box::leak(family.clone().into_boxed_str());
        if let Ok(face) = FontFace::new(family, face.weight.0, bytes.into()) {
            faces.push(face);
        }
    }
    Ok(FontSet::new(faces, FallbackPolicy { common: vec![FaceId(1), FaceId(0)], scripts: vec![] })?)
}
