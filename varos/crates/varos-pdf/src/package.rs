//! Portable package copy; publication never overwrites an existing destination.
// Package operation adapted from VectorCraft engine/cmd/package.rs@a469568.
// Copyright 2026 ArtCraft Team. MIT OR Apache-2.0; see NOTICE.
use std::{io::Write, path::Path};
use varos_core::{
    format::Limits,
    images::{BlobStore, PlacementMode},
    model::Document,
};
pub fn package(doc: &Document, store: &BlobStore, destination: &Path) -> Result<(), String> {
    if !destination.is_absolute() || destination.exists() {
        return Err("Package requires a new absolute destination folder".into());
    }
    let parent = destination.parent().ok_or("Package has no parent")?;
    let stage = parent.join(format!(
        ".varos-package-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos()
    ));
    std::fs::create_dir(&stage).map_err(|e| e.to_string())?;
    let result = (|| {
        std::fs::create_dir(stage.join("Links")).map_err(|e| e.to_string())?;
        let mut copy = doc.clone();
        let mut report = String::from("Varos package\n");
        report.push_str(&crate::font_package::collect(doc, &stage)?);
        for i in &mut copy.images {
            let blob = store.get(&i.blob).ok_or("Missing package resource")?;
            let bytes = blob.original.as_ref().ok_or("Package requires full originals; proxy-only asset found")?;
            if i.placement == PlacementMode::Link {
                let status = varos_core::images::links::status(i, store);
                if status != varos_core::images::links::LinkStatus::Current {
                    return Err(format!("Package refused: image {} source is {status:?}", i.id));
                }
                let ext = match blob.meta.mime {
                    varos_core::images::Mime::Png => "png",
                    varos_core::images::Mime::Jpeg => "jpg",
                    varos_core::images::Mime::Gif => "gif",
                    varos_core::images::Mime::WebP => "webp",
                    varos_core::images::Mime::Tiff => "tiff",
                    varos_core::images::Mime::Bmp => "bmp",
                };
                let relative = format!("Links/{}.{}", i.blob.0, ext);
                let file = stage.join(&relative);
                if !file.exists() {
                    write_new(&file, bytes)?;
                }
                if varos_core::images::content_key(&std::fs::read(&file).map_err(|e| e.to_string())?) != i.blob {
                    return Err("Package verification failed".into());
                }
                let link = i.link.as_mut().ok_or("Link metadata missing")?;
                link.absolute = destination.join(&relative).to_str().ok_or("Package path must be UTF-8")?.into();
                link.document_relative = Some(relative.clone());
                link.home_relative = None;
                report.push_str(&format!("Image {}: {relative}\n", i.id));
            }
        }
        write_new(&stage.join("Document.vrs"), &crate::images::write_vrs(&copy, store, &Limits::DEFAULT)?)?;
        write_new(&stage.join("Report.txt"), report.as_bytes())?;
        if destination.exists() {
            return Err("Package destination appeared during staging".into());
        }
        std::fs::rename(&stage, destination).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&stage);
    }
    result
}
pub(crate) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(path).map_err(|e| e.to_string())?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|e| e.to_string())
}
