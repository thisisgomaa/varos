//! Bounded first-frame image decoding and physical resolution.
// Bounded entry adapted from PhotoCraft codecs/lib.rs:39-55@4cb7cf3.
// Copyright 2026 ArtCraft Team. MIT OR Apache-2.0; see NOTICE.
use super::*;
use image::{ImageDecoder,ImageFormat,ImageReader,DynamicImage,ImageEncoder};
use std::{io::Cursor,sync::Arc};
#[derive(Clone,Debug,PartialEq)]
pub struct Decoded {pub blob:Blob,pub ppi:[f32;2],pub notes:Vec<String>}
pub fn decode(bytes:&[u8])->Result<Decoded,String> {
    if bytes.is_empty()||bytes.len()>MAX_ORIGINAL {return Err("Image original exceeds 16 MiB or is empty".into());}
    let format=image::guess_format(bytes).map_err(|e|e.to_string())?;
    let mime=match format { ImageFormat::Png=>Mime::Png,ImageFormat::Jpeg=>Mime::Jpeg,ImageFormat::Gif=>Mime::Gif,ImageFormat::WebP=>Mime::WebP,ImageFormat::Tiff=>Mime::Tiff,ImageFormat::Bmp=>Mime::Bmp,_=>return Err("Unsupported image codec".into()) };
    let mut limits=image::Limits::default();limits.max_image_width=Some(MAX_AXIS);limits.max_image_height=Some(MAX_AXIS);limits.max_alloc=Some(256*1024*1024);
    let mut reader=ImageReader::with_format(Cursor::new(bytes),format);reader.limits(limits);
    let (mut pixels,orientation)=if format==ImageFormat::Gif {
        (decode_gif(bytes)?,image::metadata::Orientation::NoTransforms)
    } else {
        let mut decoder=reader.into_decoder().map_err(|e|e.to_string())?;
        let (w,h)=decoder.dimensions();let size=dimensions(w,h)?;
        if decoder.total_bytes().checked_add(size as u64).is_none_or(|n|n>256*1024*1024) {return Err("Image decode scratch budget exceeded".into());}
        let o=if format==ImageFormat::Tiff {image::metadata::Orientation::from_exif(super::orientation::exif_orientation(bytes) as u8).unwrap_or(image::metadata::Orientation::NoTransforms)}else{decoder.orientation().map_err(|e|e.to_string())?};
        (DynamicImage::from_decoder(decoder).map_err(|e|e.to_string())?,o)
    };
    let recipe=orientation.to_exif() as u32;
    let mut ppi=resolution(bytes,format).unwrap_or([72.,72.]);
    let fallback=resolution(bytes,format).is_none();
    pixels.apply_orientation(orientation);
    if recipe>=5 {ppi.swap(0,1);}
    let pixels=pixels.to_rgba8();let (w,h)=pixels.dimensions();
    let factor=256.0/w.max(h) as f64;
    let pw=if factor<1.0 {(w as f64*factor).round().max(1.0) as u32}else{w};
    let ph=if factor<1.0 {(h as f64*factor).round().max(1.0) as u32}else{h};
    let proxy=image::imageops::resize(&pixels,pw,ph,image::imageops::FilterType::Triangle);
    let meta=AssetMeta{key:content_key(bytes),mime,encoded_len:bytes.len(),orientation:recipe,px_w:w,px_h:h,proxy_w:pw,proxy_h:ph};
    let mut notes=Vec::new();if fallback {notes.push("Missing/invalid file ppi; using 72 ppi".into());}
    if matches!(format,ImageFormat::Gif|ImageFormat::WebP|ImageFormat::Tiff) {notes.push("First frame/page imported; animation/multipage content is not editable".into());}
    notes.push("RGB interpretation; ICC/CMYK proofing is not supported".into());
    Ok(Decoded{blob:Blob{meta,original:Some(Arc::from(bytes)),pixels:Arc::new(Pixels{width:w,height:h,rgba:Arc::from(pixels.into_raw())}),proxy:Arc::new(Pixels{width:pw,height:ph,rgba:Arc::from(proxy.into_raw())})},ppi,notes})
}
pub fn encode_png(pixels:&Pixels)->Result<Vec<u8>,String> {
    if dimensions(pixels.width,pixels.height)?!=pixels.rgba.len(){return Err("Invalid pixels".into());}
    let mut bytes=Vec::new();image::codecs::png::PngEncoder::new(&mut bytes).write_image(&pixels.rgba,pixels.width,pixels.height,image::ExtendedColorType::Rgba8).map_err(|e|e.to_string())?;
    if bytes.len()>MAX_ORIGINAL {return Err("Encoded image exceeds 16 MiB".into());}Ok(bytes)
}
fn resolution(b:&[u8],format:ImageFormat)->Option<[f32;2]> {
    let valid=|x:f32,y:f32| (x.is_finite()&&y.is_finite()&&x>0.&&y>0.).then_some([x,y]);
    match format {
        ImageFormat::Png=>{
            let mut at=8usize;while let Some(h)=b.get(at..at.checked_add(8)?) {let len=u32::from_be_bytes(h[..4].try_into().ok()?) as usize;let data=b.get(at+8..at.checked_add(8)?.checked_add(len)?)?;
                if &h[4..]==b"pHYs"&&data.len()==9&&data[8]==1 {let x=u32::from_be_bytes(data[..4].try_into().ok()?) as f32*0.0254;let y=u32::from_be_bytes(data[4..8].try_into().ok()?) as f32*0.0254;return valid(x,y);}
                at=at.checked_add(len)?.checked_add(12)?;}
            None
        }
        ImageFormat::Jpeg=>{
            let mut at=2usize;while b.get(at)==Some(&255) {let marker=*b.get(at+1)?;if marker==0xda||marker==0xd9 {break;}let len=u16::from_be_bytes(b.get(at+2..at+4)?.try_into().ok()?) as usize;if len<2{return None;}let data=b.get(at+4..at.checked_add(2)?.checked_add(len)?)?;
                if marker==0xe0&&data.starts_with(b"JFIF\0")&&data.len()>=12 {let factor=match data[7]{1=>1.,2=>2.54,_=>return None};return valid(u16::from_be_bytes(data[8..10].try_into().ok()?) as f32*factor,u16::from_be_bytes(data[10..12].try_into().ok()?) as f32*factor);}
                at=at.checked_add(len)?.checked_add(2)?;}
            None
        }
        ImageFormat::Bmp if b.len()>=46=>valid(i32::from_le_bytes(b[38..42].try_into().ok()?) as f32*0.0254,i32::from_le_bytes(b[42..46].try_into().ok()?) as f32*0.0254),
        _=>None,
    }
}

fn decode_gif(bytes:&[u8])->Result<DynamicImage,String> {
    let w=u16::from_le_bytes(bytes.get(6..8).ok_or("Truncated GIF")?.try_into().map_err(|_|"Truncated GIF")?) as u32;
    let h=u16::from_le_bytes(bytes.get(8..10).ok_or("Truncated GIF")?.try_into().map_err(|_|"Truncated GIF")?) as u32;
    let len=dimensions(w,h)?;
    let mut opts=gif::DecodeOptions::new();opts.set_color_output(gif::ColorOutput::RGBA);opts.check_frame_consistency(true);
    opts.set_memory_limit(gif::MemoryLimit::Bytes(std::num::NonZeroU64::new(128*1024*1024).ok_or("Invalid GIF limit")?));
    let mut decoder=opts.read_info(Cursor::new(bytes)).map_err(|e|e.to_string())?;
    let f=decoder.read_next_frame().map_err(|e|e.to_string())?.ok_or("GIF has no frame")?;
    let mut out=vec![0;len];
    for y in 0..f.height as usize {let from=y*f.width as usize*4;let to=((y+f.top as usize)*w as usize+f.left as usize)*4;let count=f.width as usize*4;
        out.get_mut(to..to+count).ok_or("GIF frame exceeds bounds")?.copy_from_slice(f.buffer.get(from..from+count).ok_or("Invalid GIF frame")?);}
    Ok(DynamicImage::ImageRgba8(image::RgbaImage::from_raw(w,h,out).ok_or("Invalid GIF pixels")?))
}
pub fn resize_pixels(p:&Pixels,size:[u32;2])->Result<Pixels,String> {
    dimensions(size[0],size[1])?;
    let src=image::RgbaImage::from_raw(p.width,p.height,p.rgba.to_vec()).ok_or("Invalid pixels")?;
    let dst=image::imageops::resize(&src,size[0],size[1],image::imageops::FilterType::Triangle);
    Ok(Pixels{width:size[0],height:size[1],rgba:Arc::from(dst.into_raw())})
}
