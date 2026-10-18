//! Product identity shared with the desktop profile and Windows resource build.

/// Display name (crate names, commands and document extensions stay upstream-compatible).
pub const APP_NAME: &str = "Varos";
/// Native per-user settings folder.
pub const DATA_DIR_NAME: &str = APP_NAME;
/// XDG per-user settings folder.
pub const DATA_DIR_LOWER: &str = "varos";
/// Native portable settings folder; legacy marker filenames remain compatible.
pub const PORTABLE_DATA_DIR: &str = "VarosData";
