//! Local-user trust (owner decision 2026-10-08). Scopes describe capabilities, not grants.
use serde::{Deserialize, Serialize};

pub const LOCK_FILE: &str = "profiles.lock";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scopes {
    pub read: bool,
    pub edit: bool,
    pub destructive: bool,
    pub history: bool,
    #[serde(default)]
    pub files: bool,
}
impl Scopes {
    pub const ALL: Self = Self { read: true, edit: true, destructive: true, history: true, files: true };
    pub fn names(&self) -> String {
        [
            ("read", self.read),
            ("edit", self.edit),
            ("destructive", self.destructive),
            ("history", self.history),
            ("files", self.files),
        ]
        .iter()
        .filter(|(_, on)| *on)
        .map(|(n, _)| *n)
        .collect::<Vec<_>>()
        .join(",")
    }
}
