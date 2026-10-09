use std::sync::Arc;
use varos_core::{
    format::{decode_model, encode_model, Limits},
    images::*,
    model::NodeKind,
    EditCommand, Editor,
};
fn png(color: [u8; 4]) -> Vec<u8> {
    codec::encode_png(&Pixels { budget: None, width: 2, height: 3, rgba: Arc::from(color.repeat(6)) }).unwrap()
}
fn place(ed: &mut Editor, color: [u8; 4]) -> u32 {
    links::place_bytes(ed, &png(color), [10., 20.], None, PlacementMode::Embed, None).unwrap().0
}
#[test]
fn manifest_is_bytes_free_and_history_shares_resources() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255, 0, 0, 255]);
    let key = ed.doc.images[0].blob.clone();
    let original = ed.blobs.get(&key).unwrap().original.clone().unwrap();
    let bytes = ed.blobs.retained_bytes();
    for n in 0..200 {
        let mut xf = ed.doc.images[0].xform;
        xf.e = n as f32;
        ed.try_execute(EditCommand::Image(ImageEdit::Transform { id, xform: xf, opacity: 1. })).unwrap();
    }
    assert_eq!(ed.blobs.retained_bytes(), bytes);
    assert!(Arc::ptr_eq(&original, ed.blobs.get(&key).unwrap().original.as_ref().unwrap()));
    let json = encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
    assert!(!json.contains("rgba"));
    assert!(!json.contains("base64"));
    eprintln!(
        "image undo: 200 transforms, original={} bytes, total resource={} bytes, model={} bytes",
        original.len(),
        bytes,
        json.len()
    );
    for _ in 0..200 {
        ed.undo();
    }
    assert_eq!(ed.doc.images[0].id, id);
    for _ in 0..200 {
        ed.redo();
    }
    assert_eq!(ed.doc.images[0].xform.e, 199.);
}
#[test]
fn refusal_is_atomic_and_replacements_keep_old_pixels() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255, 0, 0, 255]);
    let before = ed.clone();
    assert!(
        links::place_bytes(&mut ed, &png([0, 0, 255, 255]), [f32::NAN, 0.], None, Default::default(), None).is_err()
    );
    assert_eq!(ed.doc, before.doc);
    assert_eq!(ed.blobs, before.blobs);
    let mut staged = ed.clone();
    let mut decoded = codec::decode(&png([0, 0, 255, 255])).unwrap();
    decoded.ppi = [144.; 2];
    let image = stage(&mut staged, decoded, [0., 0.], Default::default(), None).unwrap();
    staged.try_execute(EditCommand::Image(ImageEdit::Replace { id, image })).unwrap();
    staged.undo();
    assert_eq!(staged.doc.images[0].blob, before.doc.images[0].blob);
    staged.redo();
    assert_ne!(staged.doc.images[0].blob, before.doc.images[0].blob);
    staged.undo();
    staged.try_execute(EditCommand::Image(ImageEdit::Delete { id })).unwrap();
    let pins = staged.image_pins();
    staged.blobs.collect(&pins);
    staged.undo();
    assert!(staged.blobs.get(&staged.doc.images[0].blob).is_some());
}
#[test]
fn crop_is_a_releasable_clip_group_and_one_undo() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255; 4]);
    let before = ed.doc.clone();
    ed.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds: [10., 20., 1., 1.] })).unwrap();
    let node = ed.doc.node_of_path(id).unwrap();
    assert!(matches!(ed.doc.node(node).unwrap().kind, NodeKind::Image(_)));
    assert!(ed.doc.clip_group_of(id).is_some());
    ed.undo();
    assert_eq!(ed.doc, before);
    ed.redo();
    assert!(ed.doc.clip_group_of(id).is_some());
    ed.objsel.insert(id);
    assert!(ed.clip_release_enabled());
    ed.try_execute(EditCommand::ClipRelease).unwrap();
    assert!(ed.doc.clip_group_of(id).is_none());
    ed.undo();
    assert!(ed.doc.clip_group_of(id).is_some());
}
#[test]
fn format_refuses_dangling_newer_unknown_and_malformed_metadata() {
    let mut ed = Editor::new();
    place(&mut ed, [255; 4]);
    let json = encode_model(&ed.doc, &Limits::DEFAULT).unwrap();
    assert!(decode_model(json.as_bytes(), None, &Limits::DEFAULT).is_ok());
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["varos"] = 5.into();
    assert!(decode_model(&serde_json::to_vec(&v).unwrap(), None, &Limits::DEFAULT).is_err());
    v["varos"] = varos_core::format::FORMAT_VERSION.into();
    v["doc"]["assets"] = serde_json::json!([]);
    assert!(decode_model(&serde_json::to_vec(&v).unwrap(), None, &Limits::DEFAULT).is_err());
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["doc"]["images"][0]["pixels"] = serde_json::json!([1, 2, 3]);
    assert!(decode_model(&serde_json::to_vec(&v).unwrap(), None, &Limits::DEFAULT).is_err());
    let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
    v["doc"]["images"][0]["xform"]["a"] = 0.into();
    assert!(decode_model(&serde_json::to_vec(&v).unwrap(), None, &Limits::DEFAULT).is_err());
}
#[test]
fn decode_rejects_dimensions_before_rgba_and_handles_gif_first_frame() {
    assert!(dimensions(16_385, 1).is_err());
    assert!(dimensions(6000, 6000).is_err());
    assert!(codec::decode(&vec![0; MAX_ORIGINAL + 1]).is_err());
    let gif=b"GIF89a\x01\x00\x01\x00\x80\x00\x00\xff\x00\x00\x00\x00\x00\x21\xf9\x04\x01\x00\x00\x01\x00\x2c\x00\x00\x00\x00\x01\x00\x01\x00\x00\x02\x02\x44\x01\x00\x3b";
    let d = codec::decode(gif).unwrap();
    assert_eq!(d.blob.meta.mime, Mime::Gif);
    assert!(d.notes.iter().any(|s| s.contains("First frame")));
}
#[test]
fn exif_all_eight_orientations_and_invalid_table() {
    for orientation in 1u16..=8 {
        let mut b =
            b"II\x2a\x00\x08\x00\x00\x00\x01\x00\x12\x01\x03\x00\x01\x00\x00\x00\x01\x00\x00\x00\x00\x00\x00\x00"
                .to_vec();
        b[18..20].copy_from_slice(&orientation.to_le_bytes());
        assert_eq!(orientation::exif_orientation(&b), orientation);
        b.truncate(24);
        assert_eq!(orientation::exif_orientation(&b), 1);
    }
}

#[test]
fn image_canvas_drag_escape_delete_and_trace_are_atomic() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [0, 0, 0, 255]);
    ed.tool = varos_core::ToolKind::Object;
    let before = ed.doc.clone();
    let rev = ed.rev;
    ed.pointer_down([10.5, 20.5]);
    assert!(ed.objsel.contains(&id));
    assert!(ed.transaction_open());
    ed.pointer_move([30.5, 40.5]);
    ed.escape();
    assert_eq!(ed.doc, before);
    assert_eq!(ed.rev, rev);
    ed.pointer_down([10.5, 20.5]);
    ed.pointer_move([30.5, 40.5]);
    ed.pointer_up();
    assert_eq!(ed.rev, rev + 1);
    assert_eq!(ed.doc.images[0].xform.e, 30.);
    ed.undo();
    assert_eq!(ed.doc, before);
    ed.redo();
    let before = ed.doc.clone();
    let rev = ed.rev;
    let opts = varos_core::trace::TraceOptions { noise_px: 0, ..Default::default() };
    trace::expand(&mut ed, id, &opts).unwrap();
    assert!(ed.doc.images.is_empty());
    assert!(!ed.doc.paths.is_empty());
    assert_eq!(ed.rev, rev + 1);
    ed.undo();
    assert_eq!(ed.doc, before);
    ed.objsel.insert(id);
    ed.try_execute(EditCommand::DeleteSelected).unwrap();
    assert!(ed.doc.images.is_empty());
    ed.undo();
    assert_eq!(ed.doc, before);
}
#[test]
fn resource_budget_and_200_replacements_retain_history() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255; 4]);
    for n in 0..200u16 {
        let decoded = codec::decode(&png([n as u8, (n >> 8) as u8, 0, 255])).unwrap();
        let image = stage(&mut ed, decoded, [0.; 2], Default::default(), None).unwrap();
        ed.try_execute(EditCommand::Image(ImageEdit::Replace { id, image })).unwrap();
    }
    let bytes = ed.blobs.retained_bytes();
    eprintln!(
        "image undo: 200 replacements, retained resources={bytes} bytes, originals={} bytes",
        ed.blobs.original_bytes()
    );
    for _ in 0..200 {
        ed.undo();
        assert!(ed.blobs.get(&ed.doc.images[0].blob).is_some());
    }
    for _ in 0..200 {
        ed.redo();
        assert!(ed.blobs.get(&ed.doc.images[0].blob).is_some());
    }
    assert_eq!(ed.blobs.retained_bytes(), bytes);
}
#[test]
fn protected_image_commands_refuse_before_history() {
    let mut ed = Editor::new();
    let id = place(&mut ed, [255; 4]);
    let node = ed.doc.node_of_path(id).unwrap();
    ed.layer_select_set(&[node]);
    assert!(ed.objsel.contains(&id));
    ed.layer_toggle(node);
    assert!(!ed.objsel.contains(&id));
    ed.layer_toggle(node);
    assert!(ed.objsel.contains(&id));
    let before = ed.doc.clone();
    let mut overflow = ed.doc.images[0].xform;
    overflow.a = f32::MAX;
    assert!(ed.try_execute(EditCommand::Image(ImageEdit::Transform { id, xform: overflow, opacity: 1. })).is_err());
    assert_eq!(ed.doc, before);
    let node = ed.doc.node_of_path(id).unwrap();
    ed.doc.nodes.iter_mut().find(|n| n.id == node).unwrap().locked = true;
    let before = ed.doc.clone();
    let rev = ed.rev;
    assert!(ed.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds: [0., 0., 1., 1.] })).is_err());
    assert!(ed.try_execute(EditCommand::Image(ImageEdit::Delete { id })).is_err());
    assert_eq!(ed.doc, before);
    assert_eq!(ed.rev, rev);
}

#[test]
fn clipboard_pair_survives_cross_document_cut_paste_and_gc() {
    let mut source = Editor::new();
    let id = place(&mut source, [255, 0, 0, 255]);
    source.objsel.insert(id);
    source.try_execute(EditCommand::Image(ImageEdit::Crop { id, bounds: [10., 20., 1., 1.] })).unwrap();
    let before = source.doc.clone();
    source.try_execute(EditCommand::Cut).unwrap();
    assert!(source.doc.images.is_empty());
    let clipboard = source.take_clipboard();
    assert_eq!(clipboard.image_keys().count(), 1);
    let mut dest = Editor::new();
    dest.set_clipboard(clipboard);
    dest.try_execute(EditCommand::Paste { offset: Some([50., 50.]) }).unwrap();
    assert_eq!(dest.doc.images.len(), 1);
    assert!(dest.doc.clip_group_of(dest.doc.images[0].id).is_some());
    assert!(dest.blobs.get(&dest.doc.images[0].blob).is_some());
    source.undo();
    assert_eq!(source.doc, before);
    dest.undo();
    assert!(dest.doc.images.is_empty());
    dest.redo();
    assert_eq!(dest.doc.images.len(), 1);
}

#[test]
fn named_next_migration_and_refusal_fixtures_are_frozen() {
    let limits = Limits::DEFAULT;
    let json = include_bytes!("fixtures/v6-images/embedded-crop.json");
    let loaded = decode_model(json, None, &limits).unwrap();
    use varos_core::format::{Invalid, LoadError, FORMAT_VERSION, IMAGE_VERSION};
    // The frozen v6 file migrates to the current writer; only its stamp changes (integration w2).
    assert_eq!(loaded.source_version, IMAGE_VERSION);
    assert_eq!(loaded.migrated, FORMAT_VERSION > IMAGE_VERSION);
    assert_eq!(
        encode_model(&loaded.doc, &limits).unwrap(),
        std::str::from_utf8(json).unwrap().replacen("{\"varos\":6,", &format!("{{\"varos\":{FORMAT_VERSION},"), 1)
    );
    for bytes in [
        include_bytes!("fixtures/v6-images/refused-missing-asset.json").as_slice(),
        include_bytes!("fixtures/v6-images/refused-singular-affine.json"),
    ] {
        assert!(
            matches!(decode_model(bytes,None,&limits),Err(LoadError::Invalid(Invalid::NonFinite{what})) if what=="Invalid image metadata")
        );
    }
    assert!(
        matches!(decode_model(include_bytes!("fixtures/v6-images/refused-unknown-pixels.json"),None,&limits),Err(LoadError::Malformed{detail,..}) if detail.contains("unknown field `pixels`"))
    );
    assert_eq!(
        decode_model(include_bytes!("fixtures/v6-images/refused-future.json"), None, &limits).unwrap_err(),
        LoadError::NewerVersion { found: 10, supported: FORMAT_VERSION }
    );
    let old = include_str!("fixtures/v5/cap_Butt.json");
    let migrated = decode_model(old.as_bytes(), None, &limits).unwrap();
    assert!(migrated.migrated);
    assert!(migrated.doc.images.is_empty() && migrated.doc.assets.is_empty());
    assert_eq!(
        encode_model(&migrated.doc, &limits).unwrap(),
        old.replacen("\"varos\":5", &format!("\"varos\":{FORMAT_VERSION}"), 1)
    );
}

#[test]
fn representative_undo_memory_probe() {
    let mut ed = Editor::new();
    let pixels =
        Pixels { budget: None, width: 2048, height: 2048, rgba: Arc::from([255, 0, 0, 255].repeat(2048 * 2048)) };
    let bytes = codec::encode_png(&pixels).unwrap();
    let (id, _) = links::place_bytes(&mut ed, &bytes, [0.; 2], None, Default::default(), None).unwrap();
    let baseline = ed.blobs.retained_bytes();
    for n in 0..200 {
        let mut xf = ed.doc.images[0].xform;
        xf.e = n as f32;
        ed.try_execute(EditCommand::Image(ImageEdit::Transform { id, xform: xf, opacity: 1. })).unwrap();
    }
    assert_eq!(ed.blobs.retained_bytes(), baseline);
    eprintln!(
        "RAM probe: 2048x2048 PNG, encoded={}, resources before/after 200 transforms={baseline}/{}, model={}",
        bytes.len(),
        ed.blobs.retained_bytes(),
        encode_model(&ed.doc, &Limits::DEFAULT).unwrap().len()
    );
    for n in 0..200u16 {
        let p =
            Pixels { budget: None, width: 256, height: 256, rgba: Arc::from([n as u8, 0, 0, 255].repeat(256 * 256)) };
        let bytes = codec::encode_png(&p).unwrap();
        let decoded = codec::decode(&bytes).unwrap();
        let image = stage(&mut ed, decoded, [0.; 2], Default::default(), None).unwrap();
        ed.try_execute(EditCommand::Image(ImageEdit::Replace { id, image })).unwrap();
    }
    eprintln!(
        "RAM probe: after 200 distinct 256x256 replacements resources={}, originals={}, process CPU charged={}",
        ed.blobs.retained_bytes(),
        ed.blobs.original_bytes(),
        budget::charged_bytes()
    );
    for _ in 0..200 {
        ed.undo();
        assert!(ed.blobs.get(&ed.doc.images[0].blob).is_some());
    }
    for _ in 0..200 {
        ed.redo();
    }
}

#[test]
fn jpeg_exif_normalizes_all_eight_orientations_and_swaps_ppi() {
    let rgb: Vec<u8> = vec![[255, 0, 0], [0, 255, 0], [0, 0, 255], [200, 200, 0], [0, 200, 200], [200, 0, 200]]
        .into_iter()
        .flatten()
        .collect();
    let mut jpeg = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 100)
        .encode(&rgb, 3, 2, image::ExtendedColorType::Rgb8)
        .unwrap();
    let baseline = codec::decode(&jpeg).unwrap().blob.pixels;
    for orientation in 1u16..=8 {
        let mut tiff = b"II\x2a\x00\x08\x00\x00\x00".to_vec();
        tiff.extend_from_slice(&4u16.to_le_bytes());
        for (tag, ty, value) in [(274u16, 3u16, u32::from(orientation)), (282, 5, 62), (283, 5, 70), (296, 3, 2)] {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&ty.to_le_bytes());
            tiff.extend_from_slice(&1u32.to_le_bytes());
            tiff.extend_from_slice(&value.to_le_bytes());
        }
        tiff.extend_from_slice(&0u32.to_le_bytes());
        for n in [144u32, 1, 72, 1] {
            tiff.extend_from_slice(&n.to_le_bytes());
        }
        let mut data = b"Exif\0\0".to_vec();
        data.extend_from_slice(&tiff);
        let mut source = jpeg[..2].to_vec();
        source.extend_from_slice(&[255, 225]);
        source.extend_from_slice(&((data.len() + 2) as u16).to_be_bytes());
        source.extend_from_slice(&data);
        source.extend_from_slice(&jpeg[2..]);
        let decoded = codec::decode(&source).unwrap();
        let p = &decoded.blob.pixels;
        let (w, h) = if orientation >= 5 { (2, 3) } else { (3, 2) };
        assert_eq!((p.width, p.height), (w, h));
        assert_eq!(decoded.ppi, if orientation >= 5 { [72., 144.] } else { [144., 72.] });
        for y in 0..2 {
            for x in 0..3 {
                let (tx, ty) = match orientation {
                    1 => (x, y),
                    2 => (2 - x, y),
                    3 => (2 - x, 1 - y),
                    4 => (x, 1 - y),
                    5 => (y, x),
                    6 => (1 - y, x),
                    7 => (1 - y, 2 - x),
                    8 => (y, 2 - x),
                    _ => unreachable!(),
                };
                assert_eq!(
                    &p.rgba[((ty * w + tx) * 4) as usize..][..4],
                    &baseline.rgba[((y * 3 + x) * 4) as usize..][..4],
                    "orientation {orientation}"
                );
            }
        }
    }
}
#[test]
fn all_six_decoders_have_bounded_first_frame_import() {
    let source = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(3, 2, image::Rgba([255, 0, 0, 255])));
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::Gif,
        image::ImageFormat::WebP,
        image::ImageFormat::Tiff,
        image::ImageFormat::Bmp,
    ] {
        let mut b = std::io::Cursor::new(vec![]);
        if format == image::ImageFormat::Gif {
            let mut encoder = gif::Encoder::new(&mut b, 3, 2, &[]).unwrap();
            let mut rgba = source.to_rgba8().into_raw();
            encoder.write_frame(&gif::Frame::from_rgba(3, 2, &mut rgba)).unwrap();
        } else if format == image::ImageFormat::Jpeg {
            image::DynamicImage::ImageRgb8(source.to_rgb8()).write_to(&mut b, format).unwrap();
        } else {
            source.write_to(&mut b, format).unwrap();
        }
        let decoded = codec::decode(&b.into_inner()).unwrap();
        assert_eq!((decoded.blob.pixels.width, decoded.blob.pixels.height), (3, 2));
    }
}

#[test]
fn local_links_update_relink_embed_unembed_and_missing_are_atomic() {
    let dir = std::env::temp_dir().join(format!(
        "w2-links-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir(&dir).unwrap();
    let source = dir.join("source.png");
    let replacement = dir.join("replacement.png");
    std::fs::write(&source, png([255, 0, 0, 255])).unwrap();
    std::fs::write(&replacement, png([0, 255, 0, 255])).unwrap();
    let mut ed = Editor::new();
    ed.blobs.document_dir = Some(dir.clone());
    let id = links::place_file(&mut ed, &source, [0.; 2], None, PlacementMode::Link, None).unwrap().0;
    let original = ed.doc.images[0].blob.clone();
    std::fs::write(&source, png([0, 0, 255, 255])).unwrap();
    assert_eq!(links::status(&ed.doc.images[0], &ed.blobs), links::LinkStatus::Modified);
    links::update(&mut ed, &[id], None).unwrap();
    assert_eq!(ed.doc.images[0].link.as_ref().unwrap().document_relative.as_deref(), Some("source.png"));
    assert_ne!(ed.doc.images[0].blob, original);
    ed.undo();
    assert_eq!(ed.doc.images[0].blob, original);
    assert_eq!(ed.blobs.get(&original).unwrap().pixels.rgba[0], 255);
    links::relink(&mut ed, id, &replacement, None).unwrap();
    assert_eq!(ed.blobs.get(&ed.doc.images[0].blob).unwrap().pixels.rgba[1], 255);
    ed.undo();
    assert_eq!(ed.doc.images[0].blob, original);
    ed.try_execute(EditCommand::Image(ImageEdit::Mode { id, mode: PlacementMode::Embed, link: None })).unwrap();
    let before = ed.doc.clone();
    let destination = dir.join("unembedded.png");
    links::unembed(&mut ed, id, &destination, None).unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), png([255, 0, 0, 255]));
    assert_eq!(ed.doc.images[0].placement, PlacementMode::Link);
    ed.undo();
    assert_eq!(ed.doc, before);
    assert!(links::unembed(&mut ed, id, &destination, None).is_err());
    assert_eq!(ed.doc, before);
    ed.redo();
    std::fs::remove_file(&destination).unwrap();
    assert_eq!(links::status(&ed.doc.images[0], &ed.blobs), links::LinkStatus::Missing);
    let before = ed.doc.clone();
    assert!(links::update(&mut ed, &[id], None).is_err());
    assert_eq!(ed.doc, before);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fix_round_live_image_signature_mixed_selection_and_transforms() {
    use varos_core::{
        geom::View,
        model::{Anchor, Path},
        scene::scene_signature,
        select_transform::Transform,
    };
    let mut ed = Editor::new();
    let id = place(&mut ed, [255; 4]);
    let pid = ed.doc.nid();
    let points = [[100., 100.], [120., 100.], [100., 120.]];
    let anchors =
        points.into_iter().map(|p| Anchor { id: ed.doc.nid(), p, hin: None, hout: None, smooth: false }).collect();
    ed.doc.paths.push(Path::new(pid, anchors, true, Some([1.; 4]), None, 0.));
    ed.doc.sync_tree();
    ed.select_all();
    assert!(ed.objsel.contains(&id) && ed.objsel.contains(&pid));
    assert!(ed.frame_corners().is_some());
    let signature = scene_signature(&ed, View { pan: [0.; 2], zoom: 1. }, [800, 600]);
    ed.doc.images[0].xform.e += 15.;
    assert_ne!(signature, scene_signature(&ed, View { pan: [0.; 2], zoom: 1. }, [800, 600]));
    let before = ed.doc.clone();
    ed.try_execute(EditCommand::Transform(Transform { movement: [15., 20.], ..Default::default() })).unwrap();
    assert_eq!(ed.doc.images[0].xform.e, before.images[0].xform.e + 15.);
    assert_eq!(ed.doc.paths[0].anchors[0].p, [115., 120.]);
    ed.undo();
    assert_eq!(ed.doc, before);
    ed.objsel.clear();
    ed.objsel.insert(id);
    let corners = ed.frame_corners().unwrap();
    ed.begin();
    ed.start_transform(varos_core::editor::TfHit::Scale(2), corners[2]);
    ed.pointer_move([
        corners[0][0] + (corners[2][0] - corners[0][0]) * 2.,
        corners[0][1] + (corners[2][1] - corners[0][1]) * 2.,
    ]);
    assert!((ed.doc.images[0].xform.a - before.images[0].xform.a * 2.).abs() < 0.01);
    ed.pointer_up();
    ed.undo();
    assert_eq!(ed.doc, before);
}
#[test]
fn fix_round_image_board_clip_visibility_and_zero_opacity() {
    use varos_core::scene::{build_artwork_scene, Prim};
    let mut ed = Editor::new();
    let id = place(&mut ed, [255; 4]);
    ed.doc.artboards.push(varos_core::model::Artboard::default());
    ed.doc.assign_artboard_ids();
    ed.doc.artboards[0].w = 11.;
    ed.doc.artboards[0].h = 21.;
    ed.doc.artboards[0].clip = true;
    let scene = build_artwork_scene(&ed, 1.);
    assert!(scene
        .content
        .iter()
        .any(|g| g.prims().iter().any(|p| matches!(p, Prim::Image { clip: Some([0., 0., 11., 21.]), .. }))));
    ed.doc.artboards[0].hidden = true;
    assert!(image_hidden(&ed.doc, id));
    assert!(build_artwork_scene(&ed, 1.).content.is_empty());
    ed.doc.artboards[0].hidden = false;
    ed.doc.images[0].opacity = 0.;
    assert!(build_artwork_scene(&ed, 1.).content.is_empty());
}
#[test]
fn fix_round_mixed_image_drag_copy_and_transform_cancel() {
    use varos_core::select_transform::Transform;
    let mut ed = Editor::new();
    let id = place(&mut ed, [255; 4]);
    let id2 = links::place_bytes(
        &mut ed,
        &png([0, 255, 0, 255]),
        [40., 50.],
        Some([40., 50., 30., 30.]),
        PlacementMode::Embed,
        None,
    )
    .unwrap()
    .0;
    ed.doc.images[0].xform.a = 15.;
    ed.doc.images[0].xform.d = 10.;
    ed.objsel = [id, id2].into_iter().collect();
    let before = ed.doc.clone();
    let bytes = ed.blobs.retained_bytes();
    ed.pointer_down([20., 30.]);
    ed.pointer_move([35., 45.]);
    ed.pointer_up();
    assert_eq!(ed.doc.images[0].xform.e, before.images[0].xform.e + 15.);
    assert_eq!(ed.doc.images[1].xform.e, before.images[1].xform.e + 15.);
    ed.undo();
    assert_eq!(ed.doc, before);
    ed.transform_begin();
    ed.transform_live(Transform { scale: [2., 2.], ..Default::default() });
    ed.transform_end(true);
    assert_eq!(ed.doc, before);
    ed.transform_edit(Transform { copy: true, movement: [100., 0.], ..Default::default() });
    assert_eq!(ed.doc.images.len(), 4);
    assert_eq!(ed.blobs.retained_bytes(), bytes);
    ed.undo();
    assert_eq!(ed.doc, before);
}
#[test]
fn fix_round_ordered_path_lookup_scene_probe() {
    use varos_core::model::Path;
    for count in [200, 400, 800] {
        let mut ed = Editor::new();
        for _ in 0..count {
            let id = ed.doc.nid();
            ed.doc.paths.push(Path::new(id, vec![], false, None, None, 0.));
        }
        ed.doc.sync_tree();
        let start = std::time::Instant::now();
        let scene = varos_core::scene::build_artwork_scene(&ed, 1.);
        eprintln!("ordered scene: {count} empty paths, {} us", start.elapsed().as_micros());
        assert!(scene.errors.is_empty() && scene.content.is_empty());
    }
}

/// Integration w2 (view × images): Outline mode draws an image as its box + diagonals, never pixels.
#[test]
fn outline_mode_draws_image_boxes_not_pixels() {
    use varos_core::{
        geom::View,
        scene::{build_scene_in_view_styled, Group, Prim, SceneStyle},
    };
    let mut ed = Editor::new();
    place(&mut ed, [255, 0, 0, 255]);
    let style = SceneStyle { checkerboard: [[0.; 4]; 2], outline: [0.5, 0.5, 0.5, 1.], canvas: [0.1, 0.1, 0.1, 1.] };
    let prims = |ed: &Editor| -> Vec<Prim> {
        build_scene_in_view_styled(ed, View { pan: [0.; 2], zoom: 1. }, [800, 600], style)
            .content
            .into_iter()
            .flat_map(|g| match g {
                Group::Opaque(p) => p,
                _ => vec![],
            })
            .collect()
    };
    assert!(prims(&ed).iter().any(|p| matches!(p, Prim::Image { .. })));
    ed.view_depth.outline = true;
    let outlined = prims(&ed);
    assert!(!outlined.iter().any(|p| matches!(p, Prim::Image { .. })));
    assert_eq!(
        outlined.iter().filter(|p| matches!(p, Prim::Stroke { color, .. } if *color == style.outline)).count(),
        3
    );
}
