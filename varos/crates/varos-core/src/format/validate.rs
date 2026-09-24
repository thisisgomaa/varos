// OWNED BY S5-C — this stub is replaced wholesale by the semantic validator (work order §4 S5-C).
//! Semantic validation (work order §3 step 8): version-agnostic rules on the canonical v2 state —
//! masks, parentage and kinds, finiteness and ranges. Runs on every load (after migration) and on
//! every save (on the normalized clone). Never mutates.

use super::error::Invalid;
use super::limits::Limits;
use crate::model::Document;

/// Check what the user drew. `Ok(())` in this stub; S5-C fills in the rules.
pub fn validate(doc: &Document, limits: &Limits) -> Result<(), Invalid> {
    let _ = (doc, limits);
    Ok(())
}
