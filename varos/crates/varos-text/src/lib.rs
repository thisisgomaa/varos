//! Byte-fed, headless text layout. No host font or locale discovery.
#![forbid(unsafe_code)]
mod engine;
mod labels;
pub use converge::WorkCounters;
pub use labels::*;
mod converge;
mod fonts;
pub mod incremental;
pub mod outlines;
pub use engine::*;
pub use fonts::*;

#[cfg(test)]
fn test_fonts() -> FontSet {
    FontSet::new(
        vec![
            FontFace::new(
                "Inter",
                400,
                include_bytes!("../../varos-app/assets/fonts/Inter-Regular.ttf").as_slice().into(),
            )
            .unwrap(),
            FontFace::new(
                "IBM Plex Sans Arabic",
                400,
                include_bytes!("../../varos-app/assets/fonts/IBMPlexSansArabic-Regular.ttf").as_slice().into(),
            )
            .unwrap(),
        ],
        FallbackPolicy { common: vec![FaceId(1), FaceId(0)], scripts: vec![] },
    )
    .unwrap()
}
