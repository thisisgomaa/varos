use std::sync::{Arc,atomic::AtomicBool};
use varos_core::{Editor,images::{self,Pixels,PlacementMode},format::Limits};
fn editor()->Editor {let bytes=images::codec::encode_png(&Pixels{width:4,height:4,rgba:Arc::from([255,0,0,128].repeat(16))}).unwrap();let mut ed=Editor::new();images::links::place_bytes(&mut ed,&bytes,[0.;2],Some([0.,0.,72.,72.]),PlacementMode::Embed,None).unwrap();ed}
#[test]fn native_binary_streams_roundtrip_and_missing_original_refuses() {
    let ed=editor();let bytes=varos_pdf::images::write_vrs(&ed.doc,&ed.blobs,&Limits::DEFAULT).unwrap();let loaded=varos_pdf::load_vrs_bytes(&bytes,&Limits::DEFAULT).unwrap();assert_eq!(loaded.doc,ed.doc);assert_eq!(loaded.blobs,ed.blobs);
    let mut pdf=lopdf::Document::load_mem(&bytes).unwrap();let root=pdf.trailer.get(b"Root").unwrap().as_reference().unwrap();let assets=pdf.get_object_mut(root).unwrap().as_dict_mut().unwrap().get_mut(b"VAROS_Assets").unwrap().as_dict_mut().unwrap();assets.get_mut(ed.doc.assets[0].key.0.as_bytes()).unwrap().as_dict_mut().unwrap().remove(b"Original");let mut corrupt=vec![];pdf.save_to(&mut corrupt).unwrap();assert!(varos_pdf::load_vrs_bytes(&corrupt,&Limits::DEFAULT).is_err());
    assert!(varos_pdf::write_pdf_checked(&ed.doc,&Limits::DEFAULT).is_err());
}
#[test]fn pdf_xobject_smask_downsampling_and_no_editable_export() {
    let ed=editor();let pages=vec![varos_pdf::PageSpec{rect:[0.,0.,72.,72.],background:None,bleed:0.,bleed_edges:[0.;4]}];
    let (bytes,report)=varos_pdf::images::export_pdf(&ed.doc,&ed.blobs,&pages,2.,false,&AtomicBool::new(false)).unwrap();let pdf=lopdf::Document::load_mem(&bytes).unwrap();assert!(pdf.catalog().unwrap().get(b"VAROS_Model").is_err());
    let images:Vec<_>=pdf.objects.values().filter_map(|o|o.as_stream().ok()).filter(|s|s.dict.get(b"Subtype").is_ok_and(|o|o.as_name().is_ok_and(|n|n==b"Image"))).collect();assert_eq!(images.len(),2);assert!(images.iter().any(|s|s.dict.has(b"SMask")));assert!(images.iter().all(|s|s.dict.get(b"Width").unwrap().as_i64().unwrap()==2));assert!(report.notes.iter().any(|n|n.kind=="image_downsample"));
}
#[test]fn svg_portable_upright_image_and_clip_validate() {
    let mut ed=editor();let id=ed.doc.images[0].id;ed.try_execute(varos_core::EditCommand::Image(images::ImageEdit::Crop{id,bounds:[0.,0.,20.,20.]})).unwrap();
    let plan=varos_core::svg::ExportPlan{scope:varos_core::svg::ExportScope::WholeBoard,pages:vec![varos_core::svg::PageSpec{rect:[0.,0.,72.,72.],background:None,artboard:None,name:String::new()}]};
    let (files,_)=images::svg::export(&ed.doc,&ed.blobs,&plan,false,&AtomicBool::new(false)).unwrap();let text=std::str::from_utf8(&files[0].bytes).unwrap();assert!(text.contains("<image "));assert!(text.contains("data:image/png;base64,"));assert!(text.contains("clipPath"));resvg::usvg::Tree::from_data(&files[0].bytes,&resvg::usvg::Options::default()).unwrap();
}
