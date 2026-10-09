//! Headless image workflows use the same checked edits, immutable resources and container writer.
use std::path::Path;
use varos_core::{Editor,EditCommand,images::{self,ImageEdit,PlacementMode},format::Limits};
fn load(path:&str)->Result<Editor,String> {let mut ed=Editor::new();if path!="new"{let loaded=varos_pdf::load_vrs_checked(Path::new(path),&Limits::DEFAULT).map_err(|e|e.to_string())?;ed.replace_doc(loaded.doc);ed.blobs=loaded.blobs;}Ok(ed)}
fn save(ed:&Editor,path:&str)->Result<(),String> {let bytes=if ed.doc.images.is_empty(){varos_pdf::write_pdf_checked(&ed.doc,&Limits::DEFAULT)?}else{varos_pdf::images::write_vrs(&ed.doc,&ed.blobs,&Limits::DEFAULT)?};varos_core::file::write_atomic(Path::new(path),&bytes)}
pub fn run(args:Vec<String>)->Result<serde_json::Value,String> {
    let verb=args.first().ok_or("image expects place, describe, crop, relink, update, embed, unembed, trace, effects-ppi, or rasterize")?.as_str();
    let input=args.get(1).ok_or("input .vrs or new required")?;let mut ed=load(input)?;
    let get=|n|args.get(n).map(String::as_str).ok_or("missing image argument");
    let id=|n|->Result<u32,String>{get(n)?.parse().map_err(|_|"image id must be u32".into())};
    let home=varos_bridge::files::account_home().ok();
    match verb {
        "describe"=>return Ok(varos_bridge::images::describe(&ed)),
        "place"=>{let mode=if args.iter().any(|a|a=="--link"){PlacementMode::Link}else{PlacementMode::Embed};images::links::place_file(&mut ed,Path::new(get(2)?),[0.;2],None,mode,home.as_deref())?;save(&ed,get(3)?)?;}
        "crop"=>{let bounds: [f32;4]=serde_json::from_str(get(3)?).map_err(|e|e.to_string())?;ed.try_execute(EditCommand::Image(ImageEdit::Crop{id:id(2)?,bounds}))?;save(&ed,get(4)?)?;}
        "relink"=>{images::links::relink(&mut ed,id(2)?,Path::new(get(3)?),home.as_deref())?;save(&ed,get(4)?)?;}
        "update"=>{images::links::update(&mut ed,&[id(2)?],home.as_deref())?;save(&ed,get(3)?)?;}
        "embed"=>{ed.try_execute(EditCommand::Image(ImageEdit::Mode{id:id(2)?,mode:PlacementMode::Embed,link:None}))?;save(&ed,get(3)?)?;}
        "unembed"=>{images::links::unembed(&mut ed,id(2)?,Path::new(get(3)?),home.as_deref())?;save(&ed,get(4)?)?;}
        "effects-ppi"=>{ed.try_execute(EditCommand::Image(ImageEdit::EffectsPpi(get(2)?.parse().map_err(|_|"invalid ppi")?)))?;save(&ed,get(3)?)?;}
        _=>return Err("Unknown or unavailable image workflow".into()),
    }
    Ok(varos_bridge::images::describe(&ed))
}
