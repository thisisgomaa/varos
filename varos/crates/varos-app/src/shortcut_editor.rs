//! Lane F: additive shortcut overrides by command ID; canvas context preserves text editing.
// ---- Lane F: shaped chrome ----
use varos_app::shell::kit::text::ShapedUi as _;
// ---- end Lane F ----
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
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Overrides {
    pub version: u32,
    pub bindings: BTreeMap<String, Option<Chord>>,
}
impl Default for Overrides {
    fn default() -> Self {
        Self { version: 1, bindings: BTreeMap::new() }
    }
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
        if key_code(&c.key).is_none() {
            return Err("Unsupported physical key".into());
        }
        if (c.primary && c.key == "Space") || (c.alt && c.key == "Tab") {
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
    pub fn reset_defaults(&mut self) {
        for c in command_registry::commands() {
            if c.id == "shortcut.temporary-hand" {
                self.bindings.remove(&c.id);
            } else {
                self.bindings.insert(c.id, c.accel.map(Chord::from_accel));
            }
        }
    }
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
            if c.id == "shortcut.temporary-hand" && self.effective(c) != c.accel.map(Chord::from_accel) {
                return Err("Temporary Hand uses a held Space gesture and cannot be rebound".into());
            }
            if let Some(chord) = self.effective(c) {
                Chord::parse(&chord.text())?;
                if let Some(other) = seen.insert(chord.text(), c.label) {
                    return Err(format!("{} conflicts with {other}", c.label));
                }
            }
        }
        Ok(())
    }
    /// Additive durable write pinned to the bytes loaded with this draft.
    pub fn save_at(
        &self,
        fs: &dyn varos_app::storage::durable::FsPort,
        path: &std::path::Path,
        expected: Option<&[u8]>,
    ) -> Result<(Self, Vec<u8>), String> {
        self.validate()?;
        let source = match fs.read_limited(path, 65536) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if source.as_deref() != expected {
            return Err("Shortcuts changed on disk; reopen the draft".into());
        }
        let mut combined = match source {
            Some(bytes) => Self::decode(&bytes)?,
            None => Self::default(),
        };
        combined.bindings.extend(self.bindings.clone());
        combined.validate()?;
        let bytes = serde_json::to_vec_pretty(&combined).map_err(|e| e.to_string())?;
        if bytes.len() > 65536 {
            return Err("Shortcut file exceeds 64 KiB".into());
        }
        match varos_app::storage::durable::write_replace_if_unchanged(
            fs,
            path,
            &bytes,
            &varos_app::storage::checksum::new_nonce(),
            expected,
            65536,
        )
        .map_err(|e| e.reason())?
        {
            varos_app::storage::durable::WriteOutcome::Durable => Ok((combined, bytes)),
            varos_app::storage::durable::WriteOutcome::ReplacedUnconfirmed(e) => {
                Err(format!("Shortcut bytes replaced; durability unconfirmed. Reconcile disk before retry: {e}"))
            }
        }
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 65536 {
            return Err("Shortcut file exceeds 64 KiB".into());
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| "Shortcut file damaged; original retained")?;
        value.validate()?;
        Ok(value)
    }
    pub fn load_at(path: &std::path::Path) -> (Self, Option<Vec<u8>>, Option<String>) {
        match varos_app::storage::durable::FsPort::read_limited(&varos_app::storage::durable::RealFs, path, 65536) {
            Ok(bytes) => match Self::decode(&bytes) {
                Ok(value) => (value, Some(bytes), None),
                Err(e) => (Self::default(), Some(bytes), Some(e)),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Self::default(), None, None),
            Err(e) => (Self::default(), None, Some(e.to_string())),
        }
    }
}
#[derive(Default)]
pub struct EditorState {
    pub effective: Overrides,
    pub draft: Option<Overrides>,
    search: String,
    pub generation: u64,
    pub draft_generation: u64,
}
impl EditorState {
    pub fn draw(&mut self, ui: &mut egui::Ui, error: &mut Option<String>, commands: &mut Vec<AppCommand>) {
        if self.draft.is_none() {
            self.draft = Some(self.effective.clone());
            self.draft_generation = self.generation;
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
                ui.shaped_label(egui::RichText::new(c.label).font(t::small()).color(t::TEXT));
                let value = draft.effective(&c).map(|c| c.text()).unwrap_or_default();
                if c.id == "shortcut.temporary-hand" {
                    ui.shaped_label(egui::RichText::new("Space · held gesture").font(t::small()).color(t::MUTED));
                    return;
                }
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
        ui.shaped_label(
            egui::RichText::new("Canvas shortcuts; text fields retain their native editing keys.")
                .font(t::small())
                .color(t::MUTED),
        );
        if kit::menu_row(ui, Control::new(ui.id().with("reset-shortcuts"), "Reset to Illustrator defaults")).activated {
            draft.reset_defaults();
        }
        if kit::menu_row(ui, Control::new(ui.id().with("apply-shortcuts"), "Apply")).activated {
            match draft.validate() {
                Ok(()) if !kit::field::any_open(ui.ctx()) => {
                    commands.push(AppCommand::ApplyShortcuts(draft.clone(), self.draft_generation))
                }
                Ok(()) => *error = Some("Finish valid field edits before Apply".into()),
                Err(e) => *error = Some(e),
            }
        }
    }
    pub fn route(&self, code: winit::keyboard::KeyCode, m: varos_core::Mods) -> Option<crate::menus::MenuCmd> {
        let chord = Chord { key: format!("{code:?}"), primary: m.ctrl, shift: m.shift, alt: m.alt };
        command_registry::commands()
            .into_iter()
            .find(|c| {
                c.id != "shortcut.temporary-hand"
                    && self.effective.effective(c) != c.accel.map(Chord::from_accel)
                    && self.effective.effective(c) == Some(chord.clone())
                    && self.effective.bindings.contains_key(&c.id)
            })
            .map(|c| c.handler)
    }
    pub fn suppresses_default(&self, code: winit::keyboard::KeyCode, m: varos_core::Mods) -> bool {
        let chord = Chord { key: format!("{code:?}"), primary: m.ctrl, shift: m.shift, alt: m.alt };
        command_registry::commands().iter().any(|c| {
            c.id != "shortcut.temporary-hand"
                && c.accel.map(Chord::from_accel) == Some(chord.clone())
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
        key if key.starts_with("Key") => match key {
            "KeyB" => K::KeyB,
            "KeyF" => K::KeyF,
            "KeyT" => K::KeyT,
            "KeyU" => K::KeyU,
            "KeyW" => K::KeyW,
            _ => return None,
        },
        "Digit4" => K::Digit4,
        "Digit9" => K::Digit9,
        "F1" => K::F1,
        "F2" => K::F2,
        "F3" => K::F3,
        "F4" => K::F4,
        "F5" => K::F5,
        "F6" => K::F6,
        "F7" => K::F7,
        "F8" => K::F8,
        "F9" => K::F9,
        "F10" => K::F10,
        "F11" => K::F11,
        _ => return None,
    })
}

#[cfg(test)]
mod persistence_tests {
    use super::*;
    #[test]
    fn additive_unknown_binding_is_preserved_and_stale_or_future_files_refuse() {
        let root = std::env::temp_dir().join(format!("varos-shortcuts-{}", varos_app::storage::checksum::new_nonce()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("shortcuts.json");
        let original = br#"{"version":1,"bindings":{"future.command":null}}"#;
        std::fs::write(&path, original).unwrap();
        let mut draft = Overrides::default();
        draft.bindings.insert("edit.copy".into(), Some(Chord::parse("Primary+KeyB").unwrap()));
        assert!(draft.save_at(&varos_app::storage::durable::RealFs, &path, Some(b"stale")).is_err());
        let (saved, bytes) = draft.save_at(&varos_app::storage::durable::RealFs, &path, Some(original)).unwrap();
        assert_eq!(saved.bindings["future.command"], None);
        assert_eq!(Overrides::decode(&bytes).unwrap(), saved);
        let editor = EditorState { effective: saved, ..Default::default() };
        assert!(matches!(
            editor.route(winit::keyboard::KeyCode::KeyB, varos_core::Mods { ctrl: true, ..Default::default() }),
            Some(crate::menus::MenuCmd::Key(_))
        ));
        assert!(editor
            .suppresses_default(winit::keyboard::KeyCode::KeyC, varos_core::Mods { ctrl: true, ..Default::default() }));
        let future = br#"{"version":99,"bindings":{}}"#;
        std::fs::write(&path, future).unwrap();
        assert!(draft.save_at(&varos_app::storage::durable::RealFs, &path, Some(future)).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), future);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod fix_round_tests {
    use super::*;
    #[test]
    fn reset_and_legacy_reset_keep_held_space_and_default_repeat_routing() {
        let mut state = EditorState::default();
        state.effective.reset_defaults();
        assert!(state.effective.validate().is_ok());
        assert!(!state.effective.bindings.contains_key("shortcut.temporary-hand"));
        state.effective.bindings.insert("shortcut.temporary-hand".into(), Some(Chord::parse("Space").unwrap()));
        for code in
            [winit::keyboard::KeyCode::Space, winit::keyboard::KeyCode::ArrowLeft, winit::keyboard::KeyCode::ArrowRight]
        {
            assert!(state.route(code, varos_core::Mods::default()).is_none());
            assert!(!state.suppresses_default(code, varos_core::Mods::default()));
        }
    }
}
