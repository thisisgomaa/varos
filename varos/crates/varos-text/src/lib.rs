//! Byte-fed, headless text layout. No host font or locale discovery.
//!
//! The default `cosmic` feature is the layout engine. Without it only the
//! backend-neutral [`kashida`] policy remains, for hosts that shape with their
//! own HarfRust and face.
#![forbid(unsafe_code)]
#[cfg(feature = "cosmic")]
pub mod composer;
#[cfg(feature = "cosmic")]
mod engine;
#[cfg(feature = "cosmic")]
mod engine_support;
pub mod kashida;
#[cfg(feature = "cosmic")]
mod labels;
#[cfg(feature = "cosmic")]
pub mod metrics;
#[cfg(feature = "cosmic")]
mod text_types;
#[cfg(feature = "cosmic")]
pub use converge::WorkCounters;
#[cfg(feature = "cosmic")]
pub use labels::*;
#[cfg(feature = "cosmic")]
mod converge;
#[cfg(feature = "cosmic")]
mod fonts;
#[cfg(feature = "cosmic")]
pub mod incremental;
#[cfg(feature = "cosmic")]
pub mod outlines;
#[cfg(feature = "cosmic")]
pub mod paths;
#[cfg(feature = "cosmic")]
pub use engine::*;
#[cfg(feature = "cosmic")]
pub use fonts::*;

#[cfg(all(test, feature = "cosmic"))]
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
