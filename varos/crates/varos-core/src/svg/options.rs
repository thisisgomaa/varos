//! Explicit SVG export choices; internal clip/mask identities always survive.
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Styling {
    #[default]
    Attributes,
    Inline,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub styling: Styling,
    pub decimals: u8,
    pub ids: bool,
    pub minify: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self { styling: Styling::Attributes, decimals: 4, ids: true, minify: false }
    }
}
impl Options {
    pub fn validate(&self) -> Result<(), String> {
        if self.decimals > 8 {
            Err("SVG decimals must be 0–8".into())
        } else {
            Ok(())
        }
    }
    pub fn apply(&self, source: &str) -> Result<String, String> {
        self.validate()?;
        // Generated XML only: edit attribute values without touching text or names.
        let mut out = String::new();
        let mut rest = source;
        while let Some(start) = rest.find('<') {
            out.push_str(&rest[..start]);
            rest = &rest[start..];
            let Some(end) = rest.find('>') else { return Err("Invalid generated SVG".into()) };
            let tag = &rest[..=end];
            let mut rewritten = tag.to_owned();
            if !self.ids {
                if let Some(i) = rewritten.find(" id=\"path-") {
                    if let Some(end) = rewritten[i + 5..].find('"') {
                        rewritten.replace_range(i..i + 5 + end + 1, "");
                    }
                }
            }
            let numeric = [
                "d",
                "x",
                "y",
                "width",
                "height",
                "stroke-width",
                "stroke-miterlimit",
                "stroke-dasharray",
                "stroke-dashoffset",
                "fill-opacity",
                "stroke-opacity",
                "opacity",
                "viewBox",
                "points",
            ];
            for name in numeric {
                let marker = format!(" {name}=\"");
                if let Some(i) = rewritten.find(&marker) {
                    let a = i + marker.len();
                    if let Some(n) = rewritten[a..].find('"') {
                        let value = round_numbers(&rewritten[a..a + n], self.decimals);
                        rewritten.replace_range(a..a + n, &value);
                    }
                }
            }
            if self.styling == Styling::Inline {
                let mut styles = String::new();
                for name in [
                    "fill",
                    "stroke",
                    "fill-opacity",
                    "stroke-opacity",
                    "stroke-width",
                    "fill-rule",
                    "stroke-linecap",
                    "stroke-linejoin",
                ] {
                    let marker = format!(" {name}=\"");
                    if let Some(i) = rewritten.find(&marker) {
                        let a = i + marker.len();
                        if let Some(n) = rewritten[a..].find('"') {
                            styles.push_str(&format!("{name}:{};", &rewritten[a..a + n]));
                            rewritten.replace_range(i..a + n + 1, "");
                        }
                    }
                }
                if !styles.is_empty() {
                    let i = rewritten.len() - if rewritten.ends_with("/>") { 2 } else { 1 };
                    rewritten.insert_str(i, &format!(" style=\"{styles}\""));
                }
            }
            out.push_str(&rewritten);
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        if self.minify {
            out = out.lines().map(str::trim).collect::<Vec<_>>().join("");
        }
        Ok(out)
    }
}
fn round_numbers(value: &str, decimals: u8) -> String {
    let mut out = String::new();
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() || matches!(bytes[i], b'-' | b'+' | b'.') {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            if i < bytes.len() && matches!(bytes[i], b'e' | b'E') {
                i += 1;
                if i < bytes.len() && matches!(bytes[i], b'-' | b'+') {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if let Ok(v) = value[start..i].parse::<f64>() {
                out.push_str(&format_number(v, decimals));
            } else {
                out.push_str(&value[start..i]);
            }
        } else {
            let ch = value[i..].chars().next().unwrap_or(' ');
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}
pub(crate) fn format_number(v: f64, decimals: u8) -> String {
    let mut n = format!("{v:.prec$}", prec = decimals as usize);
    if n.contains('.') {
        while n.ends_with('0') {
            n.pop();
        }
        if n.ends_with('.') {
            n.pop();
        }
    }
    if n == "-0" {
        n = "0".into();
    }
    n
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn options_change_styling_precision_ids_and_minify_without_removing_clips() {
        let src="<svg viewBox=\"0 0 100.12345 100\">\n<defs><clipPath id=\"clip-1\"/></defs>\n<path id=\"path-1-name\" d=\"M 0.12345 -0.0001 L 4.56789 7.9999\" fill=\"#ff0000\" clip-path=\"url(#clip-1)\"/>\n</svg>";
        let out = Options { styling: Styling::Inline, decimals: 2, ids: false, minify: true }.apply(src).unwrap();
        assert!(out.contains("style=\"fill:#ff0000;\""));
        assert!(out.contains("M 0.12 0 L 4.57 8"));
        assert!(out.contains("id=\"clip-1\""));
        assert!(out.contains("url(#clip-1)"));
        assert!(!out.contains("id=\"path-1"));
        assert!(!out.contains('\n'));
    }
}
