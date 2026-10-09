use serde_json::json;
use varos_bridge::{images, mcp};
use varos_core::{
    images::{codec, links, Pixels, PlacementMode},
    Editor,
};
#[test]
fn image_discovery_is_12_only_complete_and_bytes_free() {
    for api in ["1.0", "1.1"] {
        let table = mcp::tools_for_api(api);
        assert!(!table.to_string().contains("image_action"));
        assert!(!table.to_string().contains("add_image"));
    }
    let index = mcp::list_verbs();
    let group = index["groups"].as_array().unwrap().iter().find(|g| g["tool"] == "image_action").unwrap();
    assert_eq!(group["verbs"].as_array().unwrap().len(), images::ACTIONS.len());
    for name in images::ACTIONS {
        let schema = mcp::schema("image_action", Some(name)).unwrap();
        assert_eq!(schema["properties"]["action"]["const"], *name);
    }
    assert!(mcp::decode_tool(
        "add_image",
        json!({"api":"1.2","board":"b1","request_id":"x","expected_rev":0,"path":"/tmp/image.png","bytes":"bad"})
    )
    .is_err());
}
#[test]
fn image_operations_checked_one_undo_and_metadata_only() {
    let mut ed = Editor::new();
    let bytes = codec::encode_png(&Pixels {
        budget: None,
        width: 2,
        height: 2,
        rgba: std::sync::Arc::from([0, 0, 0, 255].repeat(4)),
    })
    .unwrap();
    let id = links::place_bytes(&mut ed, &bytes, [0., 0.], None, PlacementMode::Embed, None).unwrap().0;
    let before = ed.doc.clone();
    let desc = images::run(&mut ed, images::Operation::Crop { id, bounds: [0., 0., 1., 1.] }).unwrap();
    for key in ["rgba", "base64", "original"] {
        assert!(!desc.to_string().contains(key));
    }
    ed.undo();
    assert_eq!(ed.doc, before);
    let rev = ed.rev;
    assert!(images::run(&mut ed, images::Operation::EffectsPpi { ppi: 0. }).is_err());
    assert_eq!(ed.rev, rev);
    images::run(&mut ed, images::Operation::Trace { id, options: Default::default() }).unwrap();
    assert!(ed.doc.images.is_empty());
    assert!(!ed.doc.paths.is_empty());
    ed.undo();
    assert_eq!(ed.doc, before);
}
