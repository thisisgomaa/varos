//! Fork-specific presentation and integrations: Varos appearance, icons, names, cursors, diagnostics. Preserve upstream lookup keys and persisted identifiers.
// Varos: agents on the same document (identity, history tags, activity, presence).
pub(crate) mod agents;
// Varos: the font menu's Arabic filter, samples and markers.
pub(crate) mod arabic_fonts;
pub(crate) mod cursors;
// Varos: concept A script-first font menu header.
pub(crate) mod font_filters;
pub(crate) mod home;
pub mod icons;
// Varos: the Paragraph panel's kashida controls.
pub(crate) mod kashida_ui;
// Varos: the Middle Eastern rows of the Character and Paragraph panels (direction, digits, harakat).
pub(crate) mod middle_eastern_ui;
pub mod names;
#[cfg(test)]
mod perf_tests;
pub(crate) mod profile;
// Varos: several icon panels open at once, stacked beside the icon column.
pub(crate) mod panel_stack;
pub(crate) mod shortcuts;
#[cfg(test)]
mod tests_agents;
#[cfg(test)]
mod tests_panel_stack;
#[cfg(test)]
mod tests_text_scale;
#[cfg(test)]
mod tests_zoom;
pub mod theme;
// Varos: General › Smooth Zoom (off: every zoom lands in one frame).
pub(crate) mod zoom;

use std::collections::HashMap;
use std::sync::OnceLock;

/// The control method the host sends (with the agent's actor) when an agent's connection closes:
/// its label number is free again ([`agents::left`]). Nothing runs and nothing replies. A client
/// sending it only marks its own claimed session as gone, which changes a label's number, nothing
/// more (actors are attribution, never access).
pub const AGENT_LEFT: &str = "varos.agent.left";

/// Project promotion commands are absent from the fork's command palette.
pub fn is_project_link(id: &str) -> bool {
    matches!(id, "help.website" | "help.appPage" | "help.github" | "help.links")
}

/// Brand a known interface label after translation. The finite, immutable table comes from
/// bundled catalogs, including their English keys. No user strings are interned or leaked.
pub fn label(s: &str) -> &str {
    if !s.contains("VectorCraft") {
        return s;
    }
    static LABELS: OnceLock<HashMap<String, String>> = OnceLock::new();
    LABELS
        .get_or_init(|| {
            let mut labels = HashMap::new();
            for lang in &crate::i18n::LANGUAGES {
                // Parse only the handful of name-bearing rows, not whole language catalogs,
                // on the first frame. The table remains independent of locale selection.
                let rows = lang.source.lines().filter(|line| line.contains("VectorCraft")).collect::<Vec<_>>().join("\n");
                for (_, source, translation) in crate::i18n::catalog::parse_entries(&rows).0 {
                    for value in [source, translation] {
                        for part in value.split('|') {
                            if part.contains("VectorCraft") {
                                labels.insert(part.to_string(), text(part));
                            }
                        }
                    }
                }
            }
            labels.insert("VectorCraft".into(), names::APP_NAME.into());
            labels
        })
        .get(s)
        .map(String::as_str)
        .unwrap_or(s)
}

/// English message templates let the English fallback brand only application prose, before
/// interpolation. File paths and other captured user data must never be renamed for display.
pub fn source_message(s: &str) -> Option<(&'static str, Vec<(&'static str, &str)>)> {
    use crate::i18n::catalog::Catalog;
    if !s.contains("VectorCraft") {
        return None;
    }
    static MESSAGES: OnceLock<Catalog> = OnceLock::new();
    MESSAGES
        .get_or_init(|| {
            let mut sources = std::collections::BTreeSet::new();
            for lang in &crate::i18n::LANGUAGES {
                for line in lang.source.lines().filter(|line| line.starts_with("@msg\t")) {
                    if let Some(source) = line.split('\t').nth(1) {
                        sources.insert(source);
                    }
                }
            }
            let rows = sources.into_iter().map(|source| format!("@msg\t{source}\t{source}\n")).collect::<String>();
            Catalog::parse(&rows)
        })
        .message(s)
}

/// Replace the product spelling in application-owned prose.
pub fn text(s: &str) -> String {
    s.replace("VectorCraft", names::APP_NAME)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_brand_label_is_replaced_without_changing_lookup_keys() {
        for lang in &crate::i18n::LANGUAGES {
            for (_, source, translated) in crate::i18n::catalog::parse_entries(lang.source).0 {
                for value in [source, translated] {
                    for part in value.split('|') {
                        assert_eq!(label(part), text(part), "{}: {part}", lang.code);
                    }
                }
            }
        }
        assert_eq!(label("VectorCraft"), names::APP_NAME);
        assert_eq!(text("VectorCraft failed: file.vectorcraft"), "Varos failed: file.vectorcraft");
    }

    #[test]
    fn messages_keep_user_paths_while_branding_application_prose() {
        assert_eq!(crate::i18n::message(crate::i18n::Lang::EN, "VectorCraft Defaults"), "Varos Defaults");
        for lang in crate::i18n::Lang::all() {
            let message = crate::i18n::message(lang, "not a VectorCraft file: /Users/me/VectorCraft.svg");
            assert!(message.contains("/Users/me/VectorCraft.svg"), "{}: {message}", lang.code());
            assert!(!message.replace("/Users/me/VectorCraft.svg", "").contains("VectorCraft"), "{}: {message}", lang.code());
        }
        assert_eq!(crate::i18n::message(crate::i18n::Lang::EN, "Saved /Users/me/VectorCraft.svg"), "Saved /Users/me/VectorCraft.svg");
    }

    #[test]
    fn user_names_and_internal_identifiers_are_unchanged() {
        assert_eq!(crate::i18n::label_or_name(crate::i18n::Lang::EN, "VectorCraft", false), "VectorCraft");
        assert_eq!(label("vectorcraft"), "vectorcraft");
        assert_eq!(label("my VectorCraft artwork.svg"), "my VectorCraft artwork.svg");
    }
}
