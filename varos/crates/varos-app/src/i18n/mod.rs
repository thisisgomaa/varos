//! Lane F: provisional chrome translations. Authored document text is never normalized.
use std::{borrow::Cow, collections::BTreeMap, sync::OnceLock};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    En,
    Ar,
}
impl Locale {
    pub fn from_tag(tag: &str) -> Self {
        if tag.split(['-', '_', '.', '@']).next().is_some_and(|s| s.eq_ignore_ascii_case("ar")) {
            Self::Ar
        } else {
            Self::En
        }
    }
    pub fn directional(self, ltr: &str, rtl: &str) -> String {
        if self == Self::Ar { rtl } else { ltr }.into()
    }
}
pub fn resolve(requested: &str, system: &str) -> Locale {
    Locale::from_tag(if requested == "system" { system } else { requested })
}
pub fn system_locale() -> String {
    for name in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(value) = std::env::var(name) {
            if !value.is_empty() {
                return value;
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        // GUI launches do not inherit LANG. Read the user's OS preference once at startup.
        if let Ok(out) = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLocale"]).output() {
            if out.status.success() {
                return String::from_utf8_lossy(&out.stdout).trim().into();
            }
        }
    }
    #[cfg(windows)]
    {
        let mut name = [0_u16; 85];
        // SAFETY: Win32 writes at most the supplied buffer length and returns its written count.
        let count = unsafe { windows::Win32::Globalization::GetUserDefaultLocaleName(&mut name) };
        if count > 1 {
            if let Some(slice) = name.get(..count as usize - 1) {
                if let Ok(name) = String::from_utf16(slice) {
                    return name;
                }
            }
        }
    }
    "en".into()
}
pub fn set(ctx: &egui::Context, locale: Locale) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("lane-f-locale"), locale));
}
pub fn locale(ctx: &egui::Context) -> Locale {
    ctx.data(|d| d.get_temp(egui::Id::new("lane-f-locale"))).unwrap_or_default()
}
fn catalog() -> &'static BTreeMap<&'static str, &'static str> {
    static CATALOG: OnceLock<BTreeMap<&'static str, &'static str>> = OnceLock::new();
    CATALOG.get_or_init(|| include_str!("ar.tsv").lines().filter_map(|l| l.split_once('\t')).collect())
}
pub fn contains(english: &str) -> bool {
    catalog().contains_key(english)
}
pub fn translate<'a>(ctx: &egui::Context, english: &'a str) -> Cow<'a, str> {
    if locale(ctx) == Locale::Ar {
        if let Some(ar) = catalog().get(english) {
            return Cow::Borrowed(ar);
        }
        if let Some((label, shortcut)) = english.rsplit_once(" (") {
            if english.ends_with(')') && catalog().contains_key(label) {
                return Cow::Owned(message(
                    ctx,
                    "{label} ({shortcut})",
                    &[("label", catalog()[label]), ("shortcut", &shortcut[..shortcut.len() - 1])],
                ));
            }
        }
    }
    Cow::Borrowed(english)
}
/// Numeric fields alone normalize keyboard digits; names retain exact authored bytes.
pub fn latin_digits(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{0660}'..='\u{0669}' => char::from_u32('0' as u32 + c as u32 - 0x660).unwrap_or(c),
            '\u{06f0}'..='\u{06f9}' => char::from_u32('0' as u32 + c as u32 - 0x6f0).unwrap_or(c),
            '\u{066b}' => '.',
            _ => c,
        })
        .collect()
}
static NATIVE_ARABIC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Native menus are rebuilt at launch (Preferences advertises restart for language).
pub fn configure_native(requested: &str) {
    NATIVE_ARABIC.store(resolve(requested, &system_locale()) == Locale::Ar, std::sync::atomic::Ordering::Relaxed);
}
pub fn native_label(english: &str) -> &str {
    if NATIVE_ARABIC.load(std::sync::atomic::Ordering::Relaxed) {
        catalog().get(english).copied().unwrap_or(english)
    } else {
        english
    }
}

/// Format a complete catalog sentence; translators control argument order.
pub fn message(ctx: &egui::Context, template: &str, values: &[(&str, &str)]) -> String {
    let translated = translate(ctx, template);
    let mut out = String::new();
    let mut rest = translated.as_ref();
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find('}').map(|n| start + n) else {
            out.push_str(&rest[start..]);
            return out;
        };
        let key = &rest[start + 1..end];
        if let Some((_, value)) = values.iter().find(|(name, _)| *name == key) {
            out.push_str(value);
        } else {
            out.push_str(&rest[start..=end]);
        }
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_and_sentence_arguments_roundtrip() {
        use crate::storage::{
            preferences::{Language, SPECS},
            settings::Settings,
        };
        let mut settings = Settings::default();
        SPECS.iter().find(|s| s.key == "language").unwrap().set(&mut settings, serde_json::json!("ar")).unwrap();
        assert_eq!(settings.preferences.language, Language::Ar);
        assert_eq!(serde_json::to_string(&settings.preferences.language).unwrap(), "\"ar\"");
        let ctx = egui::Context::default();
        set(&ctx, Locale::Ar);
        assert_eq!(message(&ctx, "Remove {name}", &[("name", "{name} لوحة")]), "إزالة {name} لوحة");
        for spec in SPECS {
            for copy in [spec.label, spec.category, spec.timing] {
                assert!(catalog().contains_key(copy), "missing preference copy: {copy}");
            }
        }
        for panel in crate::shell::registry::PanelId::DOCKABLE {
            assert!(catalog().contains_key(panel.title()), "{}", panel.title());
        }
    }
    #[test]
    fn locale_and_digits_policy() {
        assert_eq!(resolve("system", "ar_EG.UTF-8"), Locale::Ar);
        assert_eq!(resolve("en", "ar-SA"), Locale::En);
        assert_eq!(latin_digits("١٢٣٫٤۵"), "123.45");
    }
    #[test]
    fn catalogs_are_complete_and_logical() {
        let en: Vec<_> = include_str!("en.tsv").lines().map(|l| l.split_once('\t').unwrap().0).collect();
        let ar: Vec<_> = include_str!("ar.tsv").lines().map(|l| l.split_once('\t').unwrap()).collect();
        assert_eq!(en.len(), catalog().len(), "duplicates or missing translation");
        for (key, value) in ar {
            assert!(en.contains(&key));
            assert!(!value.is_empty());
            let placeholders = |text: &str| {
                let mut keys: Vec<String> = text
                    .split('{')
                    .skip(1)
                    .filter_map(|part| part.split_once('}').map(|(key, _)| key.to_owned()))
                    .collect();
                keys.sort();
                keys
            };
            assert_eq!(placeholders(key), placeholders(value), "sentence arguments: {key}");
            assert!(!value
                .chars()
                .any(|c| matches!(c,'\u{200e}'|'\u{200f}'|'\u{fb50}'..='\u{fdff}'|'\u{fe70}'..='\u{feff}')));
        }
    }
}
