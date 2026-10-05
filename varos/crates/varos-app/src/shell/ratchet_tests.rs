use std::path::{Path, PathBuf};

fn production_ui_files() -> Vec<PathBuf> {
    fn visit(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("read UI source directory") {
            let path = entry.expect("read UI source entry").path();
            if path.is_dir() {
                visit(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs")
                && path.file_name().is_some_and(|name| name != "tests.rs")
            {
                out.push(path);
            }
        }
    }
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = vec![src.join("ui.rs")];
    visit(&src.join("ui"), &mut files);
    files.sort();
    files
}

fn production_ui_source() -> String {
    production_ui_files()
        .into_iter()
        .map(|path| std::fs::read_to_string(path).expect("read UI source"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn rust_source_outside_icon_registry() -> String {
    fn visit(dir: &Path, out: &mut String) {
        for entry in std::fs::read_dir(dir).expect("read Rust source directory") {
            let path = entry.expect("read Rust source entry").path();
            if path.is_dir() {
                visit(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") && !path.ends_with("shell/kit/icons.rs") {
                out.push_str(&std::fs::read_to_string(path).expect("read Rust source"));
                out.push('\n');
            }
        }
    }
    let mut source = String::new();
    visit(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut source);
    source
}

fn numeric_calls(source: &str, needles: &[&str]) -> usize {
    needles
        .iter()
        .map(|needle| {
            source
                .match_indices(needle)
                .filter(|(at, _)| source[*at + needle.len()..].trim_start().starts_with(|c: char| c.is_ascii_digit()))
                .count()
        })
        .sum()
}

fn identifier_words(source: &str) -> impl Iterator<Item = &str> {
    source.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).filter(|word| !word.is_empty())
}

#[test]
fn ui_source_ratchets_only_tighten() {
    let source = production_ui_source();
    assert!(numeric_calls(&source, &["FontId::new(", "FontId::proportional(", "FontId::monospace("]) <= 20);
    assert!(numeric_calls(&source, &[".size("]) <= 33);
    let raw_colours = [
        "Color32::from_rgb(",
        "Color32::from_rgba_unmultiplied(",
        "Color32::from_gray(",
        "Color32::from_black_alpha(",
        "Color32::from_white_alpha(",
        "Color32::WHITE",
        "Color32::BLACK",
    ]
    .iter()
    .map(|needle| source.matches(needle).count())
    .sum::<usize>();
    assert!(raw_colours <= 42);
    assert!(numeric_calls(&source, &["CornerRadius::same("]) <= 22);
}

#[test]
fn ui_rs_only_shrinks_and_icons_have_one_home() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let ui_rs = std::fs::read_to_string(src.join("ui.rs")).expect("read ui.rs");
    assert!(ui_rs.lines().count() <= 958);
    let source = rust_source_outside_icon_registry();
    let legacy_prefix = ["IC", "_"].concat();
    assert!(
        !identifier_words(&source).any(|word| word.starts_with(&legacy_prefix)),
        "legacy icon identifiers belong only in shell/kit/icons.rs"
    );
}

#[test]
fn ratchets_scan_every_production_ui_module_and_exclude_test_modules() {
    let files = production_ui_files();
    assert!(files.iter().any(|path| path.ends_with("ui/bar.rs")));
    assert!(files.iter().any(|path| path.ends_with("ui/panels/align.rs")));
    assert!(!files.iter().any(|path| path.file_name().is_some_and(|name| name == "tests.rs")));
}
