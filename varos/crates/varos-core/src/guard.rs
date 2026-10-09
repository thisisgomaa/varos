//! Command-boundary panic recovery; snapshots are owned by the caller.
// Adapted from VectorCraft crates/engine/src/guard.rs@a469568 (MIT OR Apache-2.0), ArtCraft Team 2026.
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineError {
    Internal { what: String },
}
impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Internal { what } => write!(f, "internal error: {what}"),
        }
    }
}
impl std::error::Error for EngineError {}
pub fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
pub fn catch_panic<T>(f: impl FnOnce() -> T) -> Result<T, EngineError> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|p| EngineError::Internal { what: panic_message(p.as_ref()) })
}
