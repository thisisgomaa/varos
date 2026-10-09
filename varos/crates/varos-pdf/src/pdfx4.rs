//! Lane C: constrained PDF/X-4 preset. Vector-only until image profile preflight exists.
use varos_core::model::Document;
pub(crate) fn preflight(doc: &Document, marks: crate::PdfMarks) -> Result<(), String> {
    let profile = doc.output_profile.as_ref().ok_or("PDF/X-4 requires an ICC output profile")?.parse()?;
    if profile.profile_class != moxcms::ProfileClass::OutputDevice
        || !matches!(profile.color_space, moxcms::DataColorSpace::Rgb | moxcms::DataColorSpace::Cmyk)
    {
        return Err("PDF/X-4 requires an RGB/CMYK printer output profile".into());
    }
    if profile.color_space==moxcms::DataColorSpace::Rgb && (doc.colour_mode==varos_core::colour_management::ColourMode::Cmyk || doc.paths.iter().flat_map(|p|[&p.fill,&p.stroke]).chain(doc.swatches.iter().map(|s|&s.paint)).any(|p|matches!(p,varos_core::model::Paint::Managed(m) if matches!(m.colour,varos_core::colour_management::Colour::Cmyk {..}|varos_core::colour_management::Colour::Spot {..})))) {return Err("CMYK/spot artwork requires a CMYK printer output profile for PDF/X-4".into());}
    if !doc.images.is_empty() {
        return Err("PDF/X-4 image profile preflight is not implemented; export a regular PDF".into());
    }
    if marks.page_info {
        return Err("PDF/X-4 page-info font embedding is not implemented".into());
    }
    Ok(())
}
pub(crate) fn finish(pdf: &mut lopdf::Document) -> Result<(), String> {
    use lopdf::{dictionary, Object, Stream};
    pdf.version = "1.6".into();
    let rgb = moxcms::ColorProfile::new_srgb().encode().map_err(|e| e.to_string())?;
    let profile = pdf.add_object(Stream::new(dictionary! {"N"=>3}, rgb));
    let rgb_space = Object::Array(vec![Object::Name(b"ICCBased".to_vec()), profile.into()]);
    // Calibrate all RGB operators, including transparency form resources.
    for object in pdf.objects.values_mut() {
        let dict = match object {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => Some(&mut s.dict),
            _ => None,
        };
        if let Some(dict) = dict {
            if let Ok(Object::Dictionary(res)) = dict.get_mut(b"Resources") {
                if !res.has(b"ColorSpace") {
                    res.set("ColorSpace", dictionary! {});
                }
                if let Ok(Object::Dictionary(spaces)) = res.get_mut(b"ColorSpace") {
                    spaces.set("DefaultRGB", rgb_space.clone());
                }
            }
        }
    }
    let xmp = r#"<?xpacket begin="﻿" id="W5M0MpCehiHzreSzNTczkc9d"?><x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:pdfxid="http://www.npes.org/pdfx/ns/id/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:pdf="http://ns.adobe.com/pdf/1.3/" pdfxid:GTS_PDFXVersion="PDF/X-4" pdf:Producer="Varos"><dc:format>application/pdf</dc:format><dc:title><rdf:Alt><rdf:li xml:lang="x-default">Varos artwork</rdf:li></rdf:Alt></dc:title></rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;
    let metadata =
        pdf.add_object(Stream::new(dictionary! {"Type"=>"Metadata","Subtype"=>"XML"}, xmp.as_bytes().to_vec()));
    let root = pdf.trailer.get(b"Root").and_then(Object::as_reference).map_err(|e| e.to_string())?;
    let catalog = pdf.get_object_mut(root).and_then(Object::as_dict_mut).map_err(|e| e.to_string())?;
    catalog.set("Metadata", metadata);
    let info=pdf.add_object(dictionary! {"Title"=>Object::string_literal("Varos artwork"),"Producer"=>Object::string_literal("Varos"),"GTS_PDFXVersion"=>Object::string_literal("PDF/X-4"),"Trapped"=>Object::Name(b"False".to_vec())});
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?;
    let date = utc_date(secs.as_secs());
    let info_dict = pdf.get_object_mut(info).and_then(Object::as_dict_mut).map_err(|e| e.to_string())?;
    info_dict.set("CreationDate", Object::string_literal(date.as_bytes()));
    info_dict.set("ModDate", Object::string_literal(date.as_bytes()));
    pdf.trailer.set("Info", info);
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    secs.as_nanos().hash(&mut hash);
    format!("{:?}", pdf.catalog().map_err(|e| e.to_string())?).hash(&mut hash);
    let a = hash.finish();
    a.hash(&mut hash);
    let id = [a.to_be_bytes(), hash.finish().to_be_bytes()].concat();
    let id = Object::String(id, lopdf::StringFormat::Hexadecimal);
    pdf.trailer.set("ID", Object::Array(vec![id.clone(), id]));
    Ok(())
}

// Gregorian UTC conversion for the current export timestamp; no ambient colour state.
fn utc_date(secs: u64) -> String {
    let mut days = secs / 86400;
    let mut year = 1970u64;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    loop {
        let n = if leap(year) { 366 } else { 365 };
        if days < n {
            break;
        }
        days -= n;
        year += 1;
    }
    let months = [31, if leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    for n in months {
        if days < n {
            break;
        }
        days -= n;
        month += 1;
    }
    format!(
        "D:{year:04}{month:02}{:02}{:02}{:02}{:02}Z",
        days + 1,
        (secs % 86400) / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
