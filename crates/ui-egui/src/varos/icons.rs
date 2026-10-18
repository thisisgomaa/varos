//! Varos Lucide SVG overrides; unmatched names retain the upstream artwork.
//! Assets stay byte-for-byte copies, with ISC/MIT notices beside them.
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

pub fn source(name: &str) -> Option<egui::ImageSource<'static>> {
    static TABLE: OnceLock<HashMap<&'static str, Arc<[u8]>>> = OnceLock::new();
    let table = TABLE.get_or_init(|| {
        let originals: &[(&str, &[u8])] = &[
            ("chevron-down", include_bytes!("../../assets/varos/icons/chevron-down.svg")),
            ("chevron-left", include_bytes!("../../assets/varos/icons/chevron-left.svg")),
            ("chevron-right", include_bytes!("../../assets/varos/icons/chevron-right.svg")),
            ("copy", include_bytes!("../../assets/varos/icons/copy.svg")),
            ("dc-dist-bottom", include_bytes!("../../assets/varos/icons/align-vertical-distribute-end.svg")),
            ("dc-dist-hcenter", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-center.svg")),
            ("dc-dist-hspace", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-center.svg")),
            ("dc-dist-left", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-start.svg")),
            ("dc-dist-right", include_bytes!("../../assets/varos/icons/align-horizontal-distribute-end.svg")),
            ("dc-dist-top", include_bytes!("../../assets/varos/icons/align-vertical-distribute-start.svg")),
            ("dc-dist-vcenter", include_bytes!("../../assets/varos/icons/align-vertical-distribute-center.svg")),
            ("dc-dist-vspace", include_bytes!("../../assets/varos/icons/align-vertical-distribute-center.svg")),
            ("dc-folder", include_bytes!("../../assets/varos/icons/folder.svg")),
            ("dc-measure", include_bytes!("../../assets/varos/icons/ruler.svg")),
            ("ellipsis", include_bytes!("../../assets/varos/icons/ellipsis.svg")),
            ("file-plus", include_bytes!("../../assets/varos/icons/file-plus.svg")),
            ("folder-open", include_bytes!("../../assets/varos/icons/folder-open.svg")),
            ("frame", include_bytes!("../../assets/varos/icons/frame.svg")),
            ("hand", include_bytes!("../../assets/varos/icons/hand.svg")),
            ("history", include_bytes!("../../assets/varos/icons/history.svg")),
            ("house", include_bytes!("../../assets/varos/icons/house.svg")),
            ("lasso", include_bytes!("../../assets/varos/icons/lasso.svg")),
            ("layout-grid", include_bytes!("../../assets/varos/icons/layout-grid.svg")),
            ("link", include_bytes!("../../assets/varos/icons/link.svg")),
            ("menu", include_bytes!("../../assets/varos/icons/menu.svg")),
            ("move", include_bytes!("../../assets/varos/icons/move.svg")),
            ("pen-tool", include_bytes!("../../assets/varos/icons/pen-tool.svg")),
            ("pipette", include_bytes!("../../assets/varos/icons/pipette.svg")),
            ("plus", include_bytes!("../../assets/varos/icons/plus.svg")),
            ("ruler", include_bytes!("../../assets/varos/icons/ruler.svg")),
            ("square", include_bytes!("../../assets/varos/icons/rectangle-horizontal.svg")),
            ("square-dashed", include_bytes!("../../assets/varos/icons/square-dashed.svg")),
            ("trash-2", include_bytes!("../../assets/varos/icons/trash-2.svg")),
            ("x", include_bytes!("../../assets/varos/icons/x.svg")),
            ("zoom-in", include_bytes!("../../assets/varos/icons/zoom-in.svg")),
        ];
        originals
            .iter()
            .map(|(name, bytes)| {
                // SVG currentColor defaults to black in resvg. White is the neutral tint mask.
                let white = String::from_utf8_lossy(bytes).replace("currentColor", "white");
                (*name, Arc::from(white.into_bytes()))
            })
            .collect()
    });
    let bytes = table.get(name)?.clone();
    Some(egui::ImageSource::Bytes { uri: format!("bytes://varos/icon/{name}.svg").into(), bytes: egui::load::Bytes::Shared(bytes) })
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_override_rasterizes_and_missing_icons_fall_back() {
        for (name, _) in crate::icon_data::ICONS {
            if let Some(egui::ImageSource::Bytes { uri, bytes }) = super::source(name) {
                let egui::ImageSource::Bytes { uri: actual_uri, bytes: actual_bytes } = crate::icons::source(name) else {
                    panic!("expected Varos SVG bytes");
                };
                assert_eq!(actual_uri, uri);
                assert_eq!(actual_bytes.as_ref(), bytes.as_ref());
                let image = egui_extras::image::load_svg_bytes(bytes.as_ref(), &Default::default()).unwrap();
                assert!(image.pixels.iter().any(|p| p.a() > 0 && p.r() > 0), "{name}: must tint from white");
            }
        }
        assert!(super::source("dc-selection").is_none());
        assert!(matches!(crate::icons::source("dc-selection"), egui::ImageSource::Bytes { .. }));
    }
}
