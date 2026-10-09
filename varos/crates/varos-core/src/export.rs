//! Export diagnostics stay outside the persisted document and undo snapshots.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportReport {
    pub notes: Vec<ExportNote>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportNote {
    pub kind: String,
    pub object_id: Option<u32>,
    pub message: String,
}
