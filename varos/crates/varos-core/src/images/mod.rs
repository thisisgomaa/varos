//! Phase 3 image metadata, resource ownership and pure edits (no UI/GPU dependencies).
mod metadata;
mod store;
pub mod codec;
pub mod orientation;
mod edits;
pub mod links;
pub mod svg;
pub use metadata::*;
pub use store::*;
pub use edits::*;
use crate::model::{Document,NodeKind};
use std::collections::HashSet;
pub fn validate(doc:&Document)->Result<(),String> {
    if !doc.raster_effects_ppi.is_finite()||!(1.0..=2400.).contains(&doc.raster_effects_ppi) {return Err("Invalid raster effects ppi".into());}
    let mut keys=HashSet::new();let mut bytes=0usize;
    for a in &doc.assets {
        if !a.key.valid()||!keys.insert(a.key.clone())||a.encoded_len>MAX_ORIGINAL||!(1..=8).contains(&a.orientation)||a.proxy_w>256||a.proxy_h>256 {return Err("Invalid image manifest".into());}
        dimensions(a.px_w,a.px_h)?;dimensions(a.proxy_w,a.proxy_h)?;
        bytes=bytes.checked_add(a.encoded_len).ok_or("Asset size overflow")?;
    }
    if bytes>MAX_DOCUMENT_ORIGINALS {return Err("Document image originals exceed 32 MiB".into());}
    let mut ids=HashSet::new();let mut referenced=HashSet::new();
    for i in &doc.images {
        if !ids.insert(i.id)||!i.blob.valid()||!keys.contains(&i.blob)||!i.xform.valid()||!i.ppi.iter().all(|v|v.is_finite()&&*v>0.)||!i.opacity.is_finite()||!(0.0..=1.).contains(&i.opacity) {return Err("Invalid image metadata".into());}
        dimensions(i.px_w,i.px_h)?;
        let Some(a)=doc.assets.iter().find(|a|a.key==i.blob) else {return Err("Dangling image asset".into());};
        if (a.px_w,a.px_h)!=(i.px_w,i.px_h) {return Err("Image and manifest dimensions disagree".into());}
        if i.placement==PlacementMode::Link&&i.link.is_none() {return Err("Linked image has no locator".into());}
        if let Some(l)=&i.link {
            if l.absolute.len()>4096||!std::path::Path::new(&l.absolute).is_absolute()||l.accepted_mtime.len()>64||l.byte_size>MAX_ORIGINAL||l.hash!=i.blob {return Err("Invalid image link".into());}
            for p in [&l.home_relative,&l.document_relative].into_iter().flatten() { if p.len()>4096||std::path::Path::new(p).components().any(|c|!matches!(c,std::path::Component::Normal(_))) {return Err("Invalid relative image locator".into());} }
        }
        let leaves:Vec<_>=doc.nodes.iter().filter(|n|n.kind==NodeKind::Image(i.id)).collect();
        if leaves.len()!=1||!leaves[0].children.is_empty()||!leaves[0].xform.is_identity() {return Err("Invalid image leaf ownership/transform".into());}
        referenced.insert(i.blob.clone());
    }
    if referenced!=keys||doc.nodes.iter().any(|n|matches!(n.kind,NodeKind::Image(id) if !ids.contains(&id))) {return Err("Dangling or unused image reference".into());}
    Ok(())
}

/// Strict older-reader gate before typed decoding. The bounded model has already passed the size cap.
pub fn refuse_older_keys(json:&[u8],version:u32)->Result<(),crate::format::LoadError> {
    if version>=crate::format::IMAGE_VERSION {return Ok(());}
    let v:serde_json::Value=serde_json::from_slice(json).map_err(|e|crate::format::LoadError::malformed(&e))?;
    let doc=&v["doc"];
    if ["images","assets","raster_effects_ppi"].iter().any(|k|doc.get(k).is_some()) || doc["nodes"].as_array().is_some_and(|nodes|nodes.iter().any(|n|n["kind"].get("Image").is_some())) {
        return Err(crate::format::Invalid::FieldNotInFormat{field:"image object",version}.into());
    }
    Ok(())
}
/// Back-to-front mixed leaf traversal. Existing path-only documents retain their historical order.
pub fn paint_order(doc:&Document)->Vec<NodeKind> {
    fn visit(doc:&Document,id:u32,out:&mut Vec<NodeKind>) {if let Some(n)=doc.node(id) {for &c in n.children.iter().rev(){visit(doc,c,out);}if matches!(n.kind,NodeKind::Image(_)|NodeKind::Path(_)){out.push(n.kind);}}}
    if doc.images.is_empty() {return doc.paint_list().map(|(_,p)|NodeKind::Path(p.id)).collect();}
    let mut out=Vec::new();for &r in doc.roots.iter().rev(){visit(doc,r,&mut out);}out
}
pub fn world_corners(doc:&Document,i:&ImageObject)->[[f32;2];4] {
    let mut p=[[0.,0.],[i.px_w as f32,0.],[i.px_w as f32,i.px_h as f32],[0.,i.px_h as f32]].map(|p|i.xform.apply(p));
    let mut node=doc.node_of_path(i.id);while let Some(id)=node {let Some(n)=doc.node(id) else {break};p=p.map(|p|n.xform.apply(p));node=n.parent;}p
}
pub fn corner_rect(p:[[f32;2];4])->(f32,f32,f32,f32) { (p.iter().map(|p|p[0]).fold(f32::INFINITY,f32::min),p.iter().map(|p|p[1]).fold(f32::INFINITY,f32::min),p.iter().map(|p|p[0]).fold(f32::NEG_INFINITY,f32::max),p.iter().map(|p|p[1]).fold(f32::NEG_INFINITY,f32::max)) }
fn ancestor_flag(doc:&Document,id:u32,locked:bool)->bool {let mut cur=doc.node_of_path(id);while let Some(id)=cur {let Some(n)=doc.node(id)else{break};if if locked{n.locked}else{n.hidden}{return true;}cur=n.parent;}false}
pub fn image_hidden(doc:&Document,id:u32)->bool {ancestor_flag(doc,id,false)}
pub fn image_locked(doc:&Document,id:u32)->bool {ancestor_flag(doc,id,true)}
