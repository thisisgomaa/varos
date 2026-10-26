//! Shared identity is used by the window and the saved document header.
#[test]
fn saved_generator_and_window_title_share_build_identity() {
    let app = crate::VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
    let identity = vectorcraft_format::VAROS_IDENTITY;
    assert!(identity.starts_with("Varos ") && identity.contains(" (engine VectorCraft ") && identity.contains(", build "));
    assert_eq!(crate::chrome::window_title(&app), identity);
    let doc = vectorcraft_doc::Document::new(100., 100.);
    let value: serde_json::Value = serde_json::from_slice(&vectorcraft_format::save(&doc, false)).unwrap();
    assert_eq!(value["generator"], identity);
}
