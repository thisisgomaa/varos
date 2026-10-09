//! Explicit local-file imports and link refresh. Watcher events never replace accepted artwork.
// Link-operation contracts adapted from VectorCraft engine/cmd/links.rs@a469568.
// Copyright 2026 ArtCraft Team. MIT OR Apache-2.0; see NOTICE.
use super::*;
use crate::{Editor,EditCommand};
use std::{io::{Read,Write},path::Path};
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum LinkStatus {Embedded,Current,Missing,Modified,ProxyOnly}
pub fn read_original(path:&Path)->Result<Vec<u8>,String> {
    if !path.is_absolute()||path.as_os_str().len()>4096{return Err("Image path must be a bounded absolute local path".into());}
    let mut options=std::fs::OpenOptions::new();options.read(true);
    #[cfg(unix)] {use std::os::unix::fs::OpenOptionsExt;options.custom_flags(libc::O_NONBLOCK);}
    let mut file=options.open(path).map_err(|e|e.to_string())?;let meta=file.metadata().map_err(|e|e.to_string())?;
    if !meta.is_file()||meta.len()>MAX_ORIGINAL as u64{return Err("Image source must be a regular file no larger than 16 MiB".into());}
    let mut bytes=Vec::new();bytes.try_reserve_exact(meta.len() as usize).map_err(|e|e.to_string())?;
    (&mut file).take(MAX_ORIGINAL as u64+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len()>MAX_ORIGINAL{return Err("Image grew beyond the 16 MiB source limit".into());}Ok(bytes)
}
pub fn locator(path:&Path,bytes:&[u8],home:Option<&Path>)->Result<LinkInfo,String> {
    let path=path.canonicalize().map_err(|e|e.to_string())?;let meta=path.metadata().map_err(|e|e.to_string())?;
    let stamp=meta.modified().ok().and_then(|t|t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d|format!("{}:{}",d.as_secs(),d.subsec_nanos())).unwrap_or_default();
    let absolute=path.to_str().ok_or("Image path must be UTF-8")?.to_owned();
    Ok(LinkInfo{home_relative:home.and_then(|home|path.strip_prefix(home).ok()).and_then(|p|p.to_str()).map(str::to_owned),document_relative:None,absolute,accepted_mtime:stamp,byte_size:bytes.len(),hash:content_key(bytes)})
}
pub fn place_bytes(ed:&mut Editor,bytes:&[u8],at:[f32;2],bounds:Option<[f32;4]>,mode:PlacementMode,link:Option<LinkInfo>)->Result<(u32,Vec<String>),String> {
    let before=ed.clone();let result=(||{
        let decoded=codec::decode(bytes)?;let notes=decoded.notes.clone();
        let mut image=stage(ed,decoded,at,mode,link)?;
        if let Some([x,y,w,h])=bounds {image.xform=ImageAffine{a:w/image.px_w as f32,b:0.,c:0.,d:h/image.px_h as f32,e:x,f:y};}
        ed.try_execute(EditCommand::Image(ImageEdit::Add{image,parent:None}))?;
        let id=ed.doc.images.last().ok_or("Image placement did not publish")?.id;Ok((id,notes))
    })();if result.is_err(){*ed=before;}result
}
pub fn place_file(ed:&mut Editor,path:&Path,at:[f32;2],bounds:Option<[f32;4]>,mode:PlacementMode,home:Option<&Path>)->Result<(u32,Vec<String>),String> {
    let bytes=read_original(path)?;let info=locator(path,&bytes,home)?;place_bytes(ed,&bytes,at,bounds,mode,Some(info))
}
pub fn status(i:&ImageObject,store:&BlobStore)->LinkStatus {
    if i.placement==PlacementMode::Embed{return LinkStatus::Embedded;}
    let Some(l)=&i.link else{return LinkStatus::Missing};
    match read_original(Path::new(&l.absolute)) {Err(_)=>if store.get(&i.blob).is_some_and(|b|b.original.is_none()){LinkStatus::ProxyOnly}else{LinkStatus::Missing},Ok(b)=>if content_key(&b)!=l.hash{LinkStatus::Modified}else{LinkStatus::Current}}
}
pub fn relink(ed:&mut Editor,id:u32,path:&Path,home:Option<&Path>)->Result<Vec<String>,String> {
    let old=ed.doc.images.iter().find(|i|i.id==id).ok_or("Image not found")?.clone();
    let bytes=read_original(path)?;let link=locator(path,&bytes,home)?;let decoded=codec::decode(&bytes)?;let notes=decoded.notes.clone();let before=ed.clone();
    let result=(||{let image=stage(ed,decoded,[0.,0.],old.placement,Some(link))?;ed.try_execute(EditCommand::Image(ImageEdit::Replace{id,image}))})();
    if let Err(e)=result {*ed=before;return Err(e);}Ok(notes)
}
pub fn update(ed:&mut Editor,ids:&[u32],home:Option<&Path>)->Result<(),String> {
    let mut staged=ed.clone();let mut commands=vec![];
    for &id in ids {let i=staged.doc.images.iter().find(|i|i.id==id).ok_or("Image not found")?.clone();let l=i.link.as_ref().ok_or("Image has no source")?;let path=Path::new(&l.absolute);let bytes=read_original(path)?;let info=locator(path,&bytes,home)?;let decoded=codec::decode(&bytes)?;let image=stage(&mut staged,decoded,[0.,0.],i.placement,Some(info))?;commands.push(EditCommand::Image(ImageEdit::Replace{id,image}));}
    staged.execute_batch(commands).map_err(|e|e.reason)?;
    ed.publish_batch(staged,true);Ok(())
}
pub fn unembed(ed:&mut Editor,id:u32,destination:&Path,home:Option<&Path>)->Result<(),String> {
    let i=ed.doc.images.iter().find(|i|i.id==id).ok_or("Image not found")?;let b=ed.blobs.get(&i.blob).ok_or("Image unavailable")?;let bytes=b.original.as_ref().ok_or("Cannot unembed proxy-only image")?;
    if !destination.is_absolute(){return Err("Unembed destination must be absolute".into());}
    let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(destination).map_err(|e|e.to_string())?;
    if let Err(e)=file.write_all(bytes).and_then(|_|file.sync_all()) {let _=std::fs::remove_file(destination);return Err(e.to_string());}
    let link=locator(destination,bytes,home)?;ed.try_execute(EditCommand::Image(ImageEdit::Mode{id,mode:PlacementMode::Link,link:Some(link)}))
}
