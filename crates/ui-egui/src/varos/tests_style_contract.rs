//! Source-level guard for UI styling bypasses. This is deliberately a lexical Rust scan, not a
//! formatting/line-number check. Comments and strings are opaque; test-only items are skipped.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    file: String,
    expression: String,
}
#[derive(Clone, Debug)]
struct Token {
    text: String,
}

fn tokens(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes.get(i..i + 2) == Some(b"//") {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if bytes.get(i..i + 2) == Some(b"/*") {
            i += 2;
            let mut depth = 1;
            while i < bytes.len() && depth > 0 {
                if bytes.get(i..i + 2) == Some(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes.get(i..i + 2) == Some(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            continue;
        }
        // Raw strings, including byte raw strings. Keep them opaque so fixture text cannot
        // masquerade as production syntax. Literal content is retained only for exact keys.
        let raw = if bytes[i] == b'r' {
            Some(i + 1)
        } else if bytes.get(i..i + 2) == Some(b"br") {
            Some(i + 2)
        } else {
            None
        };
        if let Some(mut quote) = raw {
            while bytes.get(quote) == Some(&b'#') {
                quote += 1;
            }
            if bytes.get(quote) == Some(&b'"') {
                let hashes = quote - raw.unwrap();
                i = quote + 1;
                while i < bytes.len() {
                    if bytes[i] == b'"' && (0..hashes).all(|h| bytes.get(i + 1 + h) == Some(&b'#')) {
                        i += 1 + hashes;
                        break;
                    }
                    i += 1;
                }
                out.push(Token { text: source[start..i].to_owned() });
                continue;
            }
        }
        if bytes[i] == b'"' {
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i = (i + 2).min(bytes.len());
                } else if bytes[i] == b'"' {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }
        } else if bytes[i] == b'\'' && bytes.get(i + 2) == Some(&b'\'') {
            i += 3;
        } else if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
        } else if bytes[i].is_ascii_digit() {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'.')) {
                i += 1;
            }
        } else {
            // Advance complete UTF-8 characters; identifiers outside ASCII are not styling API
            // names, but must not make a byte slice panic.
            i += source[i..].chars().next().unwrap().len_utf8();
        }
        out.push(Token { text: source[start..i].to_owned() });
    }
    out
}
fn balanced(tokens: &[Token], at: usize, open: &str, close: &str) -> usize {
    let mut depth = 0;
    for (i, token) in tokens.iter().enumerate().skip(at) {
        if token.text == open {
            depth += 1;
        }
        if token.text == close {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
    }
    tokens.len()
}
fn production_tokens(source: &str) -> Vec<Token> {
    let all = tokens(source);
    let mut out = Vec::new();
    let mut i = 0;
    while i < all.len() {
        if all[i].text == "#" && all.get(i + 1).is_some_and(|t| t.text == "[") {
            let end = balanced(&all, i + 1, "[", "]");
            let attr = all[i..end].iter().map(|t| t.text.as_str()).collect::<String>();
            if matches!(attr.as_str(), "#[cfg(test)]" | "#[test]") {
                i = end;
                // Skip additional attributes on the same item, such as #[path = "tests.rs"].
                while all.get(i).is_some_and(|t| t.text == "#") && all.get(i + 1).is_some_and(|t| t.text == "[") {
                    i = balanced(&all, i + 1, "[", "]");
                }
                while i < all.len() && !matches!(all[i].text.as_str(), "{" | ";") {
                    i += 1;
                }
                i = if all.get(i).is_some_and(|t| t.text == "{") { balanced(&all, i, "{", "}") } else { (i + 1).min(all.len()) };
                continue;
            }
        }
        out.push(all[i].clone());
        i += 1;
    }
    out
}
fn scan(file: &str, source: &str) -> Vec<Finding> {
    let tokens = production_tokens(source);
    let mut findings = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        // Painter rectangles take their corner radius as the second argument. Walk nested
        // expressions in the first argument so a Rect constructor's commas do not confuse it.
        if matches!(token.text.as_str(), "rect" | "rect_filled" | "rect_stroke")
            && i > 0
            && tokens[i - 1].text == "."
            && tokens.get(i + 1).is_some_and(|t| t.text == "(")
        {
            let end = balanced(&tokens, i + 1, "(", ")");
            let mut depth = 0;
            for j in i + 2..end {
                match tokens[j].text.as_str() {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    "," if depth == 0 => {
                        if let Some(radius) = tokens.get(j + 1)
                            && radius.text.parse::<f64>().is_ok_and(|n| n > 0.0)
                            && tokens.get(j + 2).is_some_and(|t| t.text == ",")
                        {
                            findings.push(Finding { file: file.into(), expression: format!("{}(..,{},..)", token.text, radius.text) });
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
        let numeric_call =
            matches!(token.text.as_str(), "proportional" | "monospace" | "semibold" | "mono" | "corner_radius" | "size" | "same" | "new")
                && tokens.get(i + 1).is_some_and(|t| t.text == "(")
                && tokens.get(i + 2).is_some_and(|t| t.text.as_bytes().first().is_some_and(u8::is_ascii_digit));
        let font_id = i >= 3 && tokens[i - 3].text == "FontId" && tokens[i - 2].text == ":" && tokens[i - 1].text == ":";
        let radius = i >= 3 && tokens[i - 3].text == "CornerRadius" && tokens[i - 2].text == ":" && tokens[i - 1].text == ":";
        let method = i >= 1 && tokens[i - 1].text == ".";
        let relevant_call = numeric_call
            && match token.text.as_str() {
                "proportional" | "monospace" | "new" => font_id,
                "same" => radius,
                "corner_radius" | "size" => method,
                "semibold" | "mono" => true,
                _ => false,
            };
        let shadow = token.text == "Shadow" && tokens.get(i + 1).is_some_and(|t| t.text == "{");
        if relevant_call || shadow {
            let start = if font_id || radius { i - 3 } else { i };
            let end = if shadow { balanced(&tokens, i + 1, "{", "}") } else { balanced(&tokens, i + 1, "(", ")") };
            let expression = tokens[start..end].iter().map(|t| t.text.as_str()).collect();
            findings.push(Finding { file: file.into(), expression });
        }
    }
    findings
}
fn source_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            source_files(&path, out);
        } else if path.extension().is_some_and(|s| s == "rs") {
            out.push(path);
        }
    }
}
#[test]
fn production_ui_styling_uses_shared_definitions_or_reviewed_exceptions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let mut files = vec![];
    source_files(&src, &mut files);
    let mut actual = BTreeMap::<Finding, usize>::new();
    for path in files {
        let file = path.strip_prefix(&src).unwrap().to_string_lossy().replace('\\', "/");
        let name = path.file_name().unwrap().to_string_lossy();
        if file == "theme.rs"
            || file == "varos/theme.rs"
            || file == "varos/theme_components.rs"
            || name.starts_with("tests")
            || name.ends_with("_tests.rs")
            || name.contains("capture")
            || file.contains("/tests/")
        {
            continue;
        }
        for finding in scan(&file, &std::fs::read_to_string(&path).unwrap()) {
            *actual.entry(finding).or_default() += 1;
        }
    }
    // Compile the reviewed policy together with its scanner; a later policy edit cannot
    // accidentally be interpreted by an older test binary.
    let entries: serde_json::Value =
        serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/varos/UI_STYLE_EXCEPTIONS.json"))).unwrap();
    let mut allowed = BTreeMap::new();
    for entry in entries.as_array().expect("style exceptions must be an array") {
        let finding = Finding { file: entry["file"].as_str().unwrap().into(), expression: entry["expression"].as_str().unwrap().into() };
        assert!(!entry["reason"].as_str().unwrap_or_default().trim().is_empty(), "exception requires a reason: {finding:?}");
        let count = entry["occurrences"].as_u64().expect("exception requires an occurrence count");
        assert!(count > 0);
        assert!(allowed.insert(finding, count as usize).is_none(), "duplicate style exception");
    }
    assert_eq!(actual, allowed, "styling exceptions must match exact expressions and occurrence counts; review every new or stale bypass");
}
#[test]
fn lexical_contract_ignores_comments_strings_and_test_items_without_hiding_later_production() {
    let source = r###"
        // FontId::proportional(99.0)
        const EXAMPLE: &str = r#"CornerRadius::same(44)"#;
        #[cfg(test)] mod tests { fn preview() { FontId::proportional(55.0); } }
        fn real() { FontId::proportional(12.0); }
        #[cfg(test)] use thing::FontId;
        fn later() { value.corner_radius(3); }
    "###;
    let found = scan("sample.rs", source);
    assert_eq!(found.len(), 2);
    assert!(found.iter().any(|f| f.expression == "FontId::proportional(12.0)"));
    assert!(found.iter().any(|f| f.expression == "corner_radius(3)"));
}
#[test]
fn lexical_contract_keys_are_format_independent_and_do_not_flag_shared_roles() {
    assert_eq!(scan("a.rs", "FontId :: proportional ( 12.0 )"), scan("a.rs", "FontId::proportional(12.0)"));
    assert!(scan("a.rs", "FontId::proportional(LOOK.control_font); value.corner_radius(LOOK.control_radius);").is_empty());
    assert!(!scan("a.rs", "Shadow { offset: [0, 4], ..Default::default() }").is_empty());
}

#[test]
fn painter_radius_contract_skips_nested_rect_arguments_and_square_surfaces() {
    let found = scan(
        "sample.rs",
        "p.rect_filled(Rect::from_min_size(pos2(1,2), vec2(3,4)), 3.0, fill); p.rect_filled(r, 0.0, fill); p.rect_stroke(r, LOOK.control_radius, stroke, kind);",
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].expression, "rect_filled(..,3.0,..)");
}
