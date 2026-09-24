//! App-owned storage (DFS S2/S3, work order `DFS_S2_S3_START_RECENTS_RECOVERY.md` §3).
//!
//! Everything that touches the user's disk on the app's behalf lives here, behind small pure
//! functions and one fault-injectable file-system port ([`durable::FsPort`]) so the safety
//! properties are proven by headless tests:
//! - [`paths`] — the ONE app-data resolver (`VAROS_DATA_DIR` → per-OS folder; never the working dir).
//! - [`durable`] — the durable replacement writer (unique temp → sync → atomic rename → dir sync).
//! - [`checksum`] — CRC-32 and unique nonces for temp/recovery names.
//! - [`time_text`] — "saved at 14:32" and "3 min ago" text.
//! - [`recents`] — the 20-item deduplicated Recent-documents list.
//! - [`settings`] — app-wide settings (the Recovery on/off switch).
pub mod checksum;
pub mod durable;
pub mod paths;
pub mod recents;
pub mod recovery;
pub mod settings;
pub mod time_text;

#[cfg(test)]
pub(crate) mod testdir;
