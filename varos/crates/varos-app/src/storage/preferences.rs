//! Lane F: typed settings v2 descriptors; independent of document serialization.
use super::settings::Settings;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub keyboard_increment_pt: f32,
    pub default_units: Units,
    pub gpu_preference: GpuPreference,
    pub history_depth: usize,
    pub language: Language,
    pub canvas_colour: CanvasColour,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    Px,
    Pt,
    Pc,
    Mm,
    Cm,
    In,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuPreference {
    Auto,
    LowPower,
    HighPerformance,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    System,
    En,
    /// Previously selected catalog unavailable in this build; retained with English fallback.
    Unavailable {
        catalog: [u8; 64],
        len: u8,
    },
}
impl Language {
    pub fn requested(&self) -> &str {
        match self {
            Self::System => "system",
            Self::En => "en",
            Self::Unavailable { catalog, len } => {
                std::str::from_utf8(&catalog[..usize::from(*len).min(64)]).unwrap_or("system")
            }
        }
    }
}
impl Serialize for Language {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.requested())
    }
}
impl<'de> Deserialize<'de> for Language {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let value = String::deserialize(d)?;
        match value.as_str() {
            "system" => Ok(Self::System),
            "en" => Ok(Self::En),
            _ if !value.is_empty()
                && value.len() <= 64
                && value.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_".contains(&b)) =>
            {
                let mut catalog = [0; 64];
                catalog[..value.len()].copy_from_slice(value.as_bytes());
                Ok(Self::Unavailable { catalog, len: value.len() as u8 })
            }
            _ => Err(serde::de::Error::custom("Use a catalog identifier up to 64 ASCII characters")),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasColour {
    MatchUi,
    White,
    Custom([u8; 3]),
}
impl Serialize for CanvasColour {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
impl<'de> Deserialize<'de> for CanvasColour {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "match_ui" => Ok(Self::MatchUi),
            "white" => Ok(Self::White),
            _ => {
                if s.len() != 7 || !s.starts_with('#') || !s[1..].bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(serde::de::Error::custom("Use match_ui, white or #RRGGBB"));
                }
                let mut rgb = [0; 3];
                for (i, c) in rgb.iter_mut().enumerate() {
                    *c = u8::from_str_radix(&s[1 + i * 2..3 + i * 2], 16).map_err(serde::de::Error::custom)?;
                }
                Ok(Self::Custom(rgb))
            }
        }
    }
}
impl CanvasColour {
    /// Integration w2: Lane E's presets map onto the one stored value — the UI default canvas
    /// (#141313, the renderer's historic background) is `MatchUi`, pure white is `White`.
    pub fn from_rgb(rgb: [u8; 3]) -> Self {
        if rgb == crate::shell::tokens::CANVAS_DARK {
            Self::MatchUi
        } else if rgb == [255; 3] {
            Self::White
        } else {
            Self::Custom(rgb)
        }
    }
}
/// The RGB a stored canvas colour paints (Lane E furniture + Lane F pasteboard).
pub fn canvas_rgb(colour: CanvasColour) -> [u8; 3] {
    match colour {
        CanvasColour::MatchUi => crate::shell::tokens::CANVAS_DARK,
        CanvasColour::White => [255; 3],
        CanvasColour::Custom(rgb) => rgb,
    }
}
impl std::fmt::Display for CanvasColour {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MatchUi => f.write_str("match_ui"),
            Self::White => f.write_str("white"),
            Self::Custom(c) => write!(f, "#{:02X}{:02X}{:02X}", c[0], c[1], c[2]),
        }
    }
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            keyboard_increment_pt: 1.0,
            default_units: Units::Px,
            gpu_preference: GpuPreference::HighPerformance,
            history_depth: 200,
            language: Language::System,
            canvas_colour: CanvasColour::MatchUi,
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Key {
    Increment,
    Units,
    Gpu,
    History,
    Recovery,
    Autosave,
    Interval,
    Language,
    Canvas,
}
#[derive(Clone, Copy, Debug)]
pub enum Control {
    Number,
    Boolean,
    Choice(&'static [&'static str]),
    Colour,
}
pub struct SettingSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub category: &'static str,
    pub typed: Key,
    pub control: Control,
    pub timing: &'static str,
}
pub const SPECS: &[SettingSpec] = &[
    SettingSpec {
        key: "keyboard_increment_pt",
        label: "Keyboard increment (pt)",
        category: "General",
        typed: Key::Increment,
        control: Control::Number,
        timing: "Next nudge; Shift ×10; px = pt at 72 ppi",
    },
    SettingSpec {
        key: "default_units",
        label: "Default units",
        category: "General",
        typed: Key::Units,
        control: Control::Choice(&["px", "pt", "pc", "mm", "cm", "in"]),
        timing: "New blank documents",
    },
    SettingSpec {
        key: "gpu_preference",
        label: "Graphics processor",
        category: "Performance",
        typed: Key::Gpu,
        control: Control::Choice(&["auto", "low_power", "high_performance"]),
        timing: "Restart required; adapter hint",
    },
    SettingSpec {
        key: "history_depth",
        label: "History steps",
        category: "Performance",
        typed: Key::History,
        control: Control::Number,
        timing: "Next settled history boundary; older steps are discarded",
    },
    SettingSpec {
        key: "recovery_enabled",
        label: "Recovery copies",
        category: "Saving",
        typed: Key::Recovery,
        control: Control::Boolean,
        timing: "Keeps separate recovery copies",
    },
    SettingSpec {
        key: "autosave_enabled",
        label: "Autosave to file",
        category: "Saving",
        typed: Key::Autosave,
        control: Control::Boolean,
        timing: "Saves changes to the open file",
    },
    SettingSpec {
        key: "autosave_interval_seconds",
        label: "After inactivity (seconds)",
        category: "Saving",
        typed: Key::Interval,
        control: Control::Number,
        timing: "30–1800 seconds",
    },
    SettingSpec {
        key: "language",
        label: "Language",
        category: "Interface",
        typed: Key::Language,
        control: Control::Choice(&["system", "en"]),
        timing: "Restart required; System falls back to English",
    },
    SettingSpec {
        key: "canvas_colour",
        label: "Canvas colour",
        category: "Interface",
        typed: Key::Canvas,
        control: Control::Colour,
        timing: "Pasteboard only; artboards and export unchanged",
    },
];
impl SettingSpec {
    pub fn value(&self, s: &Settings) -> serde_json::Value {
        use serde_json::json;
        match self.typed {
            Key::Increment => json!(s.preferences.keyboard_increment_pt),
            Key::Units => json!(s.preferences.default_units),
            Key::Gpu => json!(s.preferences.gpu_preference),
            Key::History => json!(s.preferences.history_depth),
            Key::Recovery => json!(s.recovery_enabled),
            Key::Autosave => json!(s.autosave_enabled),
            Key::Interval => json!(s.autosave_interval_seconds),
            Key::Language => json!(s.preferences.language),
            Key::Canvas => json!(s.preferences.canvas_colour),
        }
    }
    pub fn set(&self, s: &mut Settings, v: serde_json::Value) -> Result<(), String> {
        fn decode<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Result<T, String> {
            serde_json::from_value(v).map_err(|e| e.to_string())
        }
        match self.typed {
            Key::Increment => {
                let n: f32 = decode(v)?;
                if !n.is_finite() || !(0.001..=1296.0).contains(&n) {
                    return Err("Use 0.001–1296 points".into());
                }
                s.preferences.keyboard_increment_pt = n;
            }
            Key::History => {
                let n: usize = decode(v)?;
                if !(5..=200).contains(&n) {
                    return Err("Use 5–200 steps".into());
                }
                s.preferences.history_depth = n;
            }
            Key::Interval => {
                let n: u64 = decode(v)?;
                if !super::settings::valid_autosave_interval(n) {
                    return Err("Use 30–1800 seconds".into());
                }
                s.autosave_interval_seconds = n;
            }
            Key::Units => s.preferences.default_units = decode(v)?,
            Key::Gpu => s.preferences.gpu_preference = decode(v)?,
            Key::Recovery => s.recovery_enabled = decode(v)?,
            Key::Autosave => s.autosave_enabled = decode(v)?,
            Key::Language => s.preferences.language = decode(v)?,
            Key::Canvas => s.preferences.canvas_colour = decode(v)?,
        }
        Ok(())
    }
}
pub fn validate(s: &Settings) -> Result<(), String> {
    let mut copy = *s;
    for spec in SPECS {
        spec.set(&mut copy, spec.value(s)).map_err(|e| format!("{}: {e}", spec.key))?;
    }
    Ok(())
}
impl Units {
    pub fn core(self) -> varos_core::Unit {
        match self {
            Self::Px => varos_core::Unit::Px,
            Self::Pt => varos_core::Unit::Pt,
            Self::Pc => varos_core::Unit::Pica,
            Self::Mm => varos_core::Unit::Mm,
            Self::Cm => varos_core::Unit::Cm,
            Self::In => varos_core::Unit::In,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_validation() {
        let mut s = Settings::default();
        for spec in SPECS {
            assert!(spec.set(&mut s, spec.value(&Settings::default())).is_ok());
            assert!(spec.set(&mut s, serde_json::Value::Null).is_err());
        }
        for n in [0.0, 1297.0, f32::INFINITY] {
            s.preferences.keyboard_increment_pt = n;
            assert!(validate(&s).is_err());
        }
        assert_eq!(serde_json::from_str::<CanvasColour>("\"#aBcD12\"").unwrap().to_string(), "#ABCD12");
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;
    #[test]
    fn setting_bounds_wrong_types_and_unavailable_catalog_roundtrip() {
        for (key, valid, invalid) in [
            (
                "keyboard_increment_pt",
                vec![serde_json::json!(0.001), serde_json::json!(1296)],
                vec![serde_json::json!(0.0009), serde_json::json!(1296.1), serde_json::json!("1")],
            ),
            (
                "history_depth",
                vec![serde_json::json!(5), serde_json::json!(200)],
                vec![serde_json::json!(4), serde_json::json!(1001), serde_json::json!(5.5)],
            ),
            (
                "autosave_interval_seconds",
                vec![serde_json::json!(30), serde_json::json!(1800)],
                vec![serde_json::json!(29), serde_json::json!(1801), serde_json::json!(-1)],
            ),
        ] {
            let spec = SPECS.iter().find(|s| s.key == key).unwrap();
            for value in valid {
                assert!(spec.set(&mut Settings::default(), value).is_ok());
            }
            for value in invalid {
                assert!(spec.set(&mut Settings::default(), value).is_err());
            }
        }
        let mut settings = Settings::default();
        settings.preferences.language = serde_json::from_str("\"ar-eg\"").unwrap();
        let bytes = serde_json::to_vec(&settings.preferences.language).unwrap();
        assert_eq!(serde_json::from_slice::<Language>(&bytes).unwrap().requested(), "ar-eg");
        assert!(serde_json::from_str::<CanvasColour>("\"#12🍎\"").is_err());
        assert!(serde_json::from_str::<GpuPreference>("\"software\"").is_err());
        for recovery in [false, true] {
            for autosave in [false, true] {
                settings.recovery_enabled = recovery;
                settings.autosave_enabled = autosave;
                assert!(validate(&settings).is_ok());
            }
        }
    }
}
