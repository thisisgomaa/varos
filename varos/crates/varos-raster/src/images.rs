//! Image scene consumer; shares transforms, clipping, alpha and filtering with vector groups.
use std::sync::Arc;
use tiny_skia::{Pixmap,PixmapPaint,FilterQuality,Transform};
use varos_core::{images::{Pixels,BlobStore},model::Document,Editor,build_scene};
pub(crate) fn draw(p:&Pixels,c:[[f32;2];4],opacity:f32,dst:&mut Pixmap,view:Transform) {
    let mut rgba=p.rgba.to_vec();
    for px in rgba.chunks_exact_mut(4) {let a=u16::from(px[3]);for v in &mut px[..3]{*v=((u16::from(*v)*a+127)/255) as u8;}}
    let Some(size)=tiny_skia::IntSize::from_wh(p.width,p.height) else {return};
    let Some(src)=Pixmap::from_vec(rgba,size) else {return};
    let w=p.width as f32;let h=p.height as f32;
    let image=Transform::from_row((c[1][0]-c[0][0])/w,(c[1][1]-c[0][1])/w,(c[3][0]-c[0][0])/h,(c[3][1]-c[0][1])/h,c[0][0],c[0][1]);
    dst.draw_pixmap(0,0,src.as_ref(),&PixmapPaint{opacity,quality:FilterQuality::Bilinear,..Default::default()},view.pre_concat(image),None);
}
pub fn rasterize_with_images(doc:&Document,blobs:&BlobStore,size:[u32;2],pan:[f32;2],ppu:f32,background:Option<[f32;4]>)->Result<crate::Raster,String> {
    if size.contains(&0)||size[0] as u64*size[1] as u64>32_000_000||!ppu.is_finite()||ppu<=0.||!pan.iter().all(|v|v.is_finite()){return Err("Invalid or oversized raster target".into());}
    let mut ed=Editor::new();ed.replace_doc(doc.clone());ed.blobs=blobs.clone();let scene=build_scene(&ed,ppu);
    if !scene.errors.is_empty(){return Err(scene.errors.join("; "));}
    let mut dst=Pixmap::new(size[0],size[1]).ok_or("Raster allocation refused")?;
    if let Some(c)=background {dst.fill(tiny_skia::Color::from_rgba(c[0],c[1],c[2],c[3]).ok_or("Invalid background")?);}
    super::draw_groups(&scene.content,&mut dst,Transform::from_row(ppu,0.,0.,ppu,pan[0],pan[1]));
    Ok(crate::Raster{width:dst.width(),height:dst.height(),pixels:dst.take(),errors:vec![]})
}
pub fn rasterize_object(ed:&mut Editor,id:u32,ppi:f32,background:Option<[f32;4]>)->Result<(),String> {
    if !ppi.is_finite()||!(1.0..=2400.).contains(&ppi){return Err("Invalid rasterize ppi".into());}
    let i=ed.doc.images.iter().find(|i|i.id==id).ok_or("Image not found")?;
    let [x,y,w,h]=i.xform.bounds(i.px_w,i.px_h);let scale=ppi/72.;
    let raster=rasterize_with_images(&ed.doc,&ed.blobs,[(w*scale).ceil() as u32,(h*scale).ceil() as u32],[-x*scale,-y*scale],scale,background)?;
    let mut rgba=raster.pixels;for p in rgba.chunks_exact_mut(4) {let a=p[3] as u32;for v in &mut p[..3] {*v=if a==0{0}else{((*v as u32*255+a/2)/a).min(255) as u8};}}
    let bytes=varos_core::images::codec::encode_png(&Pixels{width:raster.width,height:raster.height,rgba:Arc::from(rgba)})?;
    let mut decoded=varos_core::images::codec::decode(&bytes)?;decoded.ppi=[ppi;2];
    let before=ed.clone();let image=varos_core::images::stage(ed,decoded,[x,y],Default::default(),None)?;
    if let Err(e)=ed.try_execute(varos_core::EditCommand::Image(varos_core::images::ImageEdit::Replace{id,image})) {*ed=before;return Err(e);}Ok(())
}
