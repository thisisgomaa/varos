//! Lane F: additive shortcut overrides by command ID; canvas context preserves text editing.
use crate::{
    app_command::AppCommand,
    command_registry::{self, Command},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use varos_app::shell::{
    kit::{self, Control},
    tokens as t,
};
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    pub version: u32,
    pub bindings: BTreeMap<String, Option<Chord>>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chord {
    pub key: String,
    pub primary: bool,
    pub shift: bool,
    pub alt: bool,
}
impl Chord {
    pub fn from_accel(a: crate::menus::Accel) -> Self {
        Self { key: format!("{:?}", a.code), primary: a.cmd, shift: a.shift, alt: a.alt }
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        let parts = text.split('+').collect::<Vec<_>>();
        let key = parts.last().ok_or("Enter a key")?.to_string();
        let c = Self {
            key,
            primary: parts.contains(&"Primary"),
            shift: parts.contains(&"Shift"),
            alt: parts.contains(&"Alt"),
        };
        if parts[..parts.len() - 1].iter().any(|p| !matches!(*p, "Primary" | "Shift" | "Alt")) {
            return Err("Use Primary, Shift, Alt and a physical key name".into());
        }
        if !crate::shortcuts::parity::BINDINGS.iter().any(|b| b.key == c.key)
            && !matches!(c.key.as_str(), "KeyK" | "Backspace" | "Delete")
        {
            return Err("Unsupported physical key".into());
        }
        if c.primary && c.key == "Space" || c.alt && c.key == "Tab" {
            return Err("This chord is reserved by the operating system".into());
        }
        Ok(c)
    }
    pub fn text(&self) -> String {
        format!(
            "{}{}{}{}",
            if self.primary { "Primary+" } else { "" },
            if self.alt { "Alt+" } else { "" },
            if self.shift { "Shift+" } else { "" },
            self.key
        )
    }
}
impl Overrides {
    pub fn effective(&self, c: &Command) -> Option<Chord> {
        self.bindings.get(&c.id).cloned().unwrap_or_else(|| c.accel.map(Chord::from_accel))
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("Unsupported shortcut version".into());
        }
        let rows = command_registry::commands();
        let mut seen = BTreeMap::new();
        for c in &rows {
            if let Some(chord) = self.effective(c) {
                Chord::parse(&chord.text())?;
                if let Some(other) = seen.insert(chord.text(), c.label) {
                    return Err(format!("{} conflicts with {other}", c.label));
                }
            }
        }
        Ok(())
    }
    pub fn save(&self) -> Result<(), String> {
        self.validate()?;
        let layout = varos_app::storage::paths::AppLayout::current().ok_or("App data folder unavailable")?;
        let path = layout.settings().with_file_name("shortcuts.json");
        let mut combined = match std::fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() > 65536 {
                    return Err("Shortcut file exceeds 64 KiB".into());
                }
                let existing: Self =
                    serde_json::from_slice(&bytes).map_err(|_| "Shortcut file damaged; original retained")?;
                if existing.version != 1 {
                    return Err("Unsupported shortcut version; writes locked".into());
                }
                existing
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self { version: 1, ..Default::default() },
            Err(e) => return Err(e.to_string()),
        };
        for (k, v) in &self.bindings {
            combined.bindings.insert(k.clone(), v.clone());
        }
        combined.validate()?;
        let bytes = serde_json::to_vec_pretty(&combined).map_err(|e| e.to_string())?;
        if bytes.len() > 65536 {
            return Err("Shortcut file exceeds 64 KiB".into());
        }
        match varos_app::storage::durable::write_replace(
            &varos_app::storage::durable::RealFs,
            &path,
            &bytes,
            "shortcuts",
        )
        .map_err(|e| e.reason())?
        {
            varos_app::storage::durable::WriteOutcome::Durable => Ok(()),
            varos_app::storage::durable::WriteOutcome::ReplacedUnconfirmed(e) => {
                Err(format!("Shortcut bytes replaced; durability unconfirmed: {e}"))
            }
        }
    }
    pub fn load() -> Self {
        let fallback = Self { version: 1, bindings: BTreeMap::new() };
        let Some(layout) = varos_app::storage::paths::AppLayout::current() else { return fallback };
        let path = layout.settings().with_file_name("shortcuts.json");
        std::fs::read(path)
            .ok()
            .filter(|b| b.len() <= 65536)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|s: &Self| s.version == 1)
            .unwrap_or(fallback)
    }
}
#[derive(Default)]
pub struct EditorState {
    pub effective: Overrides,
    pub draft: Option<Overrides>,
    search: String,
}
impl EditorState {
    pub fn draw(&mut self, ui: &mut egui::Ui, error: &mut Option<String>, commands: &mut Vec<AppCommand>) {
        if self.draft.is_none() {
            self.draft = Some(self.effective.clone());
        }
        let (rect, _) = ui.allocate_exact_size(egui::vec2(t::DOC_SHEET_W, t::FIELD_H), egui::Sense::hover());
        let edit = kit::field::text_field(
            ui,
            kit::field::TextField {
                id: ui.id().with("shortcut-search"),
                rect,
                value: &self.search,
                font: t::small(),
                framed: true,
                open: false,
                hint: "Search commands",
            },
            |s| Ok::<_, &str>(s.to_string()),
        );
        if let Some(s) = edit.commit {
            self.search = s;
        }
        let Some(draft) = &mut self.draft else { return };
        for c in command_registry::commands()
            .into_iter()
            .filter(|c| c.label.to_lowercase().contains(&self.search.to_lowercase()))
        {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(c.label).font(t::small()).color(t::TEXT));
                let value = draft.effective(&c).map(|c| c.text()).unwrap_or_default();
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(t::DOC_SHEET_FIELD_W, t::FIELD_H), egui::Sense::hover());
                let edit = kit::field::text_field(
                    ui,
                    kit::field::TextField {
                        id: ui.id().with(&c.id),
                        rect,
                        value: &value,
                        font: t::small(),
                        framed: true,
                        open: false,
                        hint: "Unbound",
                    },
                    |s| {
                        if s.is_empty() {
                            Ok(None)
                        } else {
                            Chord::parse(s).map(Some).map_err(|_| "Invalid or reserved chord")
                        }
                    },
                );
                if let Some(chord) = edit.commit {
                    draft.bindings.insert(c.id.clone(), chord);
                    *error = draft.validate().err();
                }
            });
        }
        ui.label(
            egui::RichText::new("Canvas shortcuts; text fields retain their native editing keys.")
                .font(t::small())
                .color(t::MUTED),
        );
        if kit::menu_row(ui, Control::new(ui.id().with("reset-shortcuts"), "Reset to Illustrator defaults")).activated {
            for c in command_registry::commands() {
                draft.bindings.insert(c.id, c.accel.map(Chord::from_accel));
            }
        }
        if kit::menu_row(ui, Control::new(ui.id().with("apply-shortcuts"), "Apply")).activated {
            match draft.validate() {
                Ok(()) => commands.push(AppCommand::ApplyShortcuts(draft.clone())),
                Err(e) => *error = Some(e),
            }
        }
    }
    pub fn route(&self, code: winit::keyboard::KeyCode, m: varos_core::Mods) -> Option<crate::menus::MenuCmd> {
        let chord = Chord { key: format!("{code:?}"), primary: m.ctrl, shift: m.shift, alt: m.alt };
        command_registry::commands()
            .into_iter()
            .find(|c| self.effective.effective(c) == Some(chord.clone()) && self.effective.bindings.contains_key(&c.id))
            .map(|c| c.handler)
    }
    pub fn suppresses_default(&self, code: winit::keyboard::KeyCode, m: varos_core::Mods) -> bool {
        let chord = Chord { key: format!("{code:?}"), primary: m.ctrl, shift: m.shift, alt: m.alt };
        command_registry::commands().iter().any(|c| {
            c.accel.map(Chord::from_accel) == Some(chord.clone())
                && self.effective.bindings.contains_key(&c.id)
                && self.effective.effective(c) != Some(chord.clone())
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicts_unbind_and_reserved() {
        let mut o = Overrides { version: 1, ..Default::default() };
        o.bindings.insert("edit.copy".into(), Some(Chord::parse("Primary+KeyX").unwrap()));
        assert!(o.validate().unwrap_err().contains("conflicts"));
        o.bindings.insert("edit.cut".into(), None);
        assert!(o.validate().is_ok());
        assert!(Chord::parse("Unknown+KeyK").is_err());
    }
}

pub fn key_code(key: &str) -> Option<winit::keyboard::KeyCode> {
    use winit::keyboard::KeyCode as K;
    Some(match key {
        "ArrowDown" => K::ArrowDown,
        "ArrowLeft" => K::ArrowLeft,
        "ArrowRight" => K::ArrowRight,
        "ArrowUp" => K::ArrowUp,
        "Backspace" => K::Backspace,
        "BracketLeft" => K::BracketLeft,
        "BracketRight" => K::BracketRight,
        "Delete" => K::Delete,
        "Digit0" => K::Digit0,
        "Digit1" => K::Digit1,
        "Digit2" => K::Digit2,
        "Digit3" => K::Digit3,
        "Digit5" => K::Digit5,
        "Digit6" => K::Digit6,
        "Digit7" => K::Digit7,
        "Digit8" => K::Digit8,
        "Enter" => K::Enter,
        "Equal" => K::Equal,
        "Escape" => K::Escape,
        "F12" => K::F12,
        "KeyA" => K::KeyA,
        "KeyC" => K::KeyC,
        "KeyD" => K::KeyD,
        "KeyE" => K::KeyE,
        "KeyG" => K::KeyG,
        "KeyH" => K::KeyH,
        "KeyI" => K::KeyI,
        "KeyJ" => K::KeyJ,
        "KeyK" => K::KeyK,
        "KeyL" => K::KeyL,
        "KeyM" => K::KeyM,
        "KeyN" => K::KeyN,
        "KeyO" => K::KeyO,
        "KeyP" => K::KeyP,
        "KeyQ" => K::KeyQ,
        "KeyR" => K::KeyR,
        "KeyS" => K::KeyS,
        "KeyU" => K::KeyU,
        "KeyV" => K::KeyV,
        "KeyW" => K::KeyW,
        "KeyX" => K::KeyX,
        "KeyY" => K::KeyY,
        "KeyZ" => K::KeyZ,
        "Minus" => K::Minus,
        "Numpad0" => K::Numpad0,
        "NumpadAdd" => K::NumpadAdd,
        "NumpadSubtract" => K::NumpadSubtract,
        "Quote" => K::Quote,
        "Semicolon" => K::Semicolon,
        "Slash" => K::Slash,
        "Space" => K::Space,
        "Tab" => K::Tab,
        _ => return None,
    })
}
