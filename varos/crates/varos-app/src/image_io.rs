//! Durable native save of the captured document/resource pair.
use std::path::Path;
use varos_core::{model::Document,images::BlobStore,format::Limits};
use crate::lifecycle::SaveOutcome;
use varos_app::storage::{checksum::new_nonce,durable::{FsPort,Fingerprint,WriteOutcome,write_replace_published,io_reason}};
pub fn save(fs:&dyn FsPort,doc:&Document,store:&BlobStore,path:&Path)->Result<(SaveOutcome,Option<Fingerprint>),String> {
    let bytes=varos_pdf::images::write_vrs(doc,store,&Limits::DEFAULT)?;
    let mut published=None;let outcome=write_replace_published(fs,path,&bytes,&new_nonce(),&mut published).map_err(|e|e.reason())?;
    Ok((match outcome {WriteOutcome::Durable=>SaveOutcome::Durable,WriteOutcome::ReplacedUnconfirmed(e)=>SaveOutcome::ReplacedUnconfirmed(io_reason(&e))},published))
}
