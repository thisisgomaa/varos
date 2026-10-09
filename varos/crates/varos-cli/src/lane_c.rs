//! Headless Lane C hosts. Geometry operations use the existing `apply` EditCommand batch.
use serde_json::{json, Value};
use std::{ffi::OsString, io::Write, path::PathBuf, sync::atomic::AtomicBool};
pub fn run(verb: &str, args: Vec<OsString>) -> Result<Value, String> {
    let args: Vec<PathBuf> = args.into_iter().map(PathBuf::from).collect();
    match verb {
        "new-document" => {
            if args.len() != 2 {
                return Err("new-document SETTINGS.json OUTPUT.vrs".into());
            }
            let bytes = std::fs::read(&args[0]).map_err(|e| e.to_string())?;
            let settings: varos_core::new_document::Settings =
                serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let doc = settings.document()?;
            varos_bridge::templates::save_new(&doc, &args[1]).map_err(|e| e.reason)?;
            Ok(varos_core::document_setup::info(&doc))
        }
        "export-screens" => {
            if args.len() != 3 {
                return Err("export-screens INPUT.vrs SETTINGS.json OUTPUT_FOLDER".into());
            }
            let doc = varos_pdf::load_vrs(&args[0])?;
            let settings: varos_raster::screens::Advanced =
                serde_json::from_slice(&std::fs::read(&args[1]).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let scope = if settings.whole_board || doc.artboards.is_empty() {
                varos_raster::export::Scope::WholeBoard
            } else {
                varos_raster::export::Scope::AllArtboards
            };
            let assets = varos_raster::export::plan(&doc, &scope)?;
            let jobs = settings.expand(&assets, &Default::default())?;
            let cancel = AtomicBool::new(false);
            let destinations: Vec<_> = jobs.iter().map(|j| args[2].join(&j.relative)).collect();
            if destinations.iter().any(|p| p.exists()) {
                return Err("An export destination already exists; nothing written".into());
            }
            // Encode the complete plan before writing, so option/geometry errors leave no files.
            let encoded = jobs
                .iter()
                .map(|job| {
                    if job.options.format == varos_raster::export::Format::Pdf {
                        let pages = if job.pages.is_empty() { vec![&job.asset] } else { job.pages.iter().collect() };
                        let plan = varos_pdf::ExportPlan {
                            scope: varos_pdf::ExportScope::ArtworkBounds,
                            pages: pages
                                .into_iter()
                                .map(|a| varos_pdf::PageSpec {
                                    rect: a.page.rect,
                                    background: a.page.background,
                                    bleed: 0.,
                                    bleed_edges: [0.; 4],
                                })
                                .collect(),
                        };
                        varos_pdf::export_pdf_with_options(&job.asset.doc, &plan, &Default::default(), &cancel)
                            .map_err(|e| e.to_string())
                    } else {
                        let mut output = varos_raster::export::encode(&job.asset, &job.options, &cancel)?;
                        if job.options.format == varos_raster::export::Format::Svg {
                            output.bytes = job
                                .svg
                                .apply(std::str::from_utf8(&output.bytes).map_err(|e| e.to_string())?)?
                                .into_bytes();
                        }
                        Ok((output.bytes, output.report))
                    }
                })
                .collect::<Result<Vec<_>, String>>()?;
            let mut notes = vec![];
            for (dest, (bytes, report)) in destinations.iter().zip(encoded) {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut file =
                    std::fs::OpenOptions::new().write(true).create_new(true).open(dest).map_err(|e| e.to_string())?;
                file.write_all(&bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
                notes.extend(report.notes);
            }
            Ok(json!({"files":destinations,"report":{"notes":notes}}))
        }
        _ => Err("Unknown Lane C command".into()),
    }
}
