//! Byte-fed, headless text layout. No host font or locale discovery.
#![forbid(unsafe_code)]
pub mod composer;
mod engine;
mod engine_support;
pub mod kashida;
mod labels;
pub mod metrics;
mod text_types;
pub use converge::WorkCounters;
pub use labels::*;
mod converge;
mod fonts;
pub mod incremental;
pub mod outlines;
pub mod paths;
pub use engine::*;
pub use fonts::*;

#[cfg(test)]
fn test_fonts() -> FontSet {
    FontSet::new(
        vec![
            FontFace::new("Inter", 400, include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice().into()).unwrap(),
            FontFace::new(
                "IBM Plex Sans Arabic",
                400,
                include_bytes!("../assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice().into(),
            )
            .unwrap(),
        ],
        FallbackPolicy { common: vec![FaceId(1), FaceId(0)], scripts: vec![] },
    )
    .unwrap()
}
