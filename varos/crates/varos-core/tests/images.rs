use std::sync::Arc;
use varos_core::{images::*,Editor,EditCommand,format::{encode_model,decode_model,Limits},model::NodeKind};
fn png(color:[u8;4])->Vec<u8> {codec::encode_png(&Pixels{width:2,height:3,rgba:Arc::from(color.repeat(6))}).unwrap()}
fn place(ed:&mut Editor,color:[u8;4])->u32 {links::place_bytes(ed,&png(color),[10.,20.],None,PlacementMode::Embed,None).unwrap().0}
#[test]fn manifest_is_bytes_free_and_history_shares_resources() {
    let mut ed=Editor::new();let id=place(&mut ed,[255,0,0,255]);let key=ed.doc.images[0].blob.clone();let original=ed.blobs.get(&key).unwrap().original.clone().unwrap();
    let bytes=ed.blobs.retained_bytes();
    for n in 0..200 {let mut xf=ed.doc.images[0].xform;xf.e=n as f32;ed.try_execute(EditCommand::Image(ImageEdit::Transform{id,xform:xf,opacity:1.})).unwrap();}
    assert_eq!(ed.blobs.retained_bytes(),bytes);assert!(Arc::ptr_eq(&original,ed.blobs.get(&key).unwrap().original.as_ref().unwrap()));
    let json=encode_model(&ed.doc,&Limits::DEFAULT).unwrap();assert!(!json.contains("rgba"));assert!(!json.contains("base64"));
    eprintln!("image undo: 200 transforms, original={} bytes, total resource={} bytes, model={} bytes",original.len(),bytes,json.len());
    for _ in 0..200{ed.undo();}assert_eq!(ed.doc.images[0].id,id);
    for _ in 0..200{ed.redo();}assert_eq!(ed.doc.images[0].xform.e,199.);
}
#[test]fn refusal_is_atomic_and_replacements_keep_old_pixels() {
    let mut ed=Editor::new();let id=place(&mut ed,[255,0,0,255]);let before=ed.clone();
    assert!(links::place_bytes(&mut ed,&png([0,0,255,255]),[f32::NAN,0.],None,Default::default(),None).is_err());assert_eq!(ed.doc,before.doc);assert_eq!(ed.blobs,before.blobs);
    let mut staged=ed.clone();let mut decoded=codec::decode(&png([0,0,255,255])).unwrap();decoded.ppi=[144.;2];let image=stage(&mut staged,decoded,[0.,0.],Default::default(),None).unwrap();staged.try_execute(EditCommand::Image(ImageEdit::Replace{id,image})).unwrap();
    staged.undo();assert_eq!(staged.doc.images[0].blob,before.doc.images[0].blob);staged.redo();assert_ne!(staged.doc.images[0].blob,before.doc.images[0].blob);
    staged.undo();staged.try_execute(EditCommand::Image(ImageEdit::Delete{id})).unwrap();let pins=staged.image_pins();staged.blobs.collect(&pins);staged.undo();assert!(staged.blobs.get(&staged.doc.images[0].blob).is_some());
}
#[test]fn crop_is_a_releasable_clip_group_and_one_undo() {
    let mut ed=Editor::new();let id=place(&mut ed,[255;4]);let before=ed.doc.clone();ed.try_execute(EditCommand::Image(ImageEdit::Crop{id,bounds:[10.,20.,1.,1.]})).unwrap();
    let node=ed.doc.node_of_path(id).unwrap();assert!(matches!(ed.doc.node(node).unwrap().kind,NodeKind::Image(_)));assert!(ed.doc.clip_group_of(id).is_some());
    ed.undo();assert_eq!(ed.doc,before);ed.redo();assert!(ed.doc.clip_group_of(id).is_some());
}
#[test]fn format_refuses_dangling_newer_unknown_and_malformed_metadata() {
    let mut ed=Editor::new();place(&mut ed,[255;4]);let json=encode_model(&ed.doc,&Limits::DEFAULT).unwrap();assert!(decode_model(json.as_bytes(),None,&Limits::DEFAULT).is_ok());
    let mut v:serde_json::Value=serde_json::from_str(&json).unwrap();v["varos"]=5.into();assert!(decode_model(&serde_json::to_vec(&v).unwrap(),None,&Limits::DEFAULT).is_err());v["varos"]=varos_core::format::FORMAT_VERSION.into();v["doc"]["assets"]=serde_json::json!([]);assert!(decode_model(&serde_json::to_vec(&v).unwrap(),None,&Limits::DEFAULT).is_err());
    let mut v:serde_json::Value=serde_json::from_str(&json).unwrap();v["doc"]["images"][0]["pixels"]=serde_json::json!([1,2,3]);assert!(decode_model(&serde_json::to_vec(&v).unwrap(),None,&Limits::DEFAULT).is_err());
    let mut v:serde_json::Value=serde_json::from_str(&json).unwrap();v["doc"]["images"][0]["xform"]["a"]=0.into();assert!(decode_model(&serde_json::to_vec(&v).unwrap(),None,&Limits::DEFAULT).is_err());
}
#[test]fn decode_rejects_dimensions_before_rgba_and_handles_gif_first_frame() {
    assert!(dimensions(16_385,1).is_err());assert!(dimensions(6000,6000).is_err());assert!(codec::decode(&vec![0;MAX_ORIGINAL+1]).is_err());
    let gif=b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\x00\x00\x00\x00\x00\x21\xf9\x04\x01\x00\x00\x01\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b";
    let d=codec::decode(gif).unwrap();assert_eq!(d.blob.meta.mime,Mime::Gif);assert!(d.notes.iter().any(|s|s.contains("First frame")));
}
#[test]fn exif_all_eight_orientations_and_invalid_table() {
    for orientation in 1u16..=8 {let mut b=b"II\x2a\x00\x08\x00\x00\x00\x01\x00\x12\x01\x03\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\x00".to_vec();b[18..20].copy_from_slice(&orientation.to_le_bytes());assert_eq!(orientation::exif_orientation(&b),orientation);b.truncate(24);assert_eq!(orientation::exif_orientation(&b),1);}
}
