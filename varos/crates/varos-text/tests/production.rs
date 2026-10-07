mod common;
use common::*;
use varos_text::*;

fn key(text: &str, role: u32) -> LabelKey {
    LabelKey {
        text: text.into(),
        face: LATIN,
        size_bits: 12f32.to_bits(),
        width_bits: None,
        role,
        ppp_bits: 1f32.to_bits(),
    }
}
#[test]
fn byte_fed_identity_and_invalid_ids_are_rejected() {
    let bad = FontFace::new("Wrong family", 400, INTER.into()).unwrap();
    assert!(Engine::new(FontSet::new(vec![bad], FallbackPolicy::default()).unwrap()).is_err());
    let mut e = engine();
    let mut r = Request::new("name", 12., None);
    r.face = FaceId(99);
    assert_eq!(e.layout(&r), Err("invalid face id"));
    assert_eq!(incremental(1024).layout(&r, || false), Err("invalid face id"));
    assert!(FontSet::new(
        vec![FontFace::new("Inter", 400, INTER.into()).unwrap()],
        FallbackPolicy { common: vec![FaceId(1)], scripts: vec![] }
    )
    .is_err());
}
#[test]
fn font_order_and_script_policy_are_data_driven() {
    let fonts = FontSet::new(
        vec![
            FontFace::new("IBM Plex Sans Arabic", 400, PLEX.into()).unwrap(),
            FontFace::new("Inter", 400, INTER.into()).unwrap(),
        ],
        FallbackPolicy { common: vec![FaceId(1)], scripts: vec![(*b"Arab", vec![FaceId(0)])] },
    )
    .unwrap();
    assert!(FontSet::new(
        fonts.faces().to_vec(),
        FallbackPolicy { common: vec![], scripts: vec![(*b"arab", vec![FaceId(0)])] },
    )
    .is_err());
    let mut e = Engine::new(fonts.clone()).unwrap();
    let mut r = Request::new("شعار", 12., None);
    r.face = FaceId(1);
    let l = e.layout(&r).unwrap();
    assert!(l.lines[0].glyphs.iter().all(|g| g.face == FaceId(0)));
    assert!(outlines::layout_outlines(&fonts, &l).unwrap().iter().any(|o| !o.commands.is_empty()));
}
#[test]
fn labels_obey_entry_payload_lru_and_age_limits() {
    let mut e = engine();
    let mut cache = LabelCache::new(&e);
    let different = FontSet::new(font_set(true).faces().to_vec(), FallbackPolicy::default()).unwrap();
    let mut changed_engine = Engine::new(different).unwrap();
    assert!(cache.layout(&mut changed_engine, &key("", 0), 0).is_err());
    for n in 0..LABEL_ENTRIES as u32 {
        cache.layout(&mut e, &key("", n), 1).unwrap();
    }
    assert_eq!(cache.stats().0, LABEL_ENTRIES);
    cache.get(&key("", 0), 2).unwrap();
    cache.layout(&mut e, &key("", LABEL_ENTRIES as u32), 2).unwrap();
    assert!(cache.get(&key("", 1), 2).is_none());
    assert!(cache.get(&key("", 0), 2).is_some());
    for n in 0..512 {
        cache.layout(&mut e, &key(&"Logo شعار ".repeat(12), n), 3).unwrap();
        assert!(cache.stats().1 <= LABEL_BYTES);
    }
    cache.expire(604);
    assert_eq!(cache.stats(), (0, 0));
    assert!(cache.layout(&mut e, &key(&"X".repeat(4097), 0), 605).is_err());
}
#[test]
fn elision_and_bidi_selection_keep_logical_graphemes() {
    let mut e = engine();
    let r = Request::new("السَّلَامُ Café logo شعار", 12., None);
    let elided = e.elide(&r, 60.).unwrap();
    assert!(elided.source.ends_with('…'));
    assert!(elided.lines[0].width <= 60.);
    assert!(r.text.starts_with(elided.source.trim_end_matches('…')));
    let l = e.layout(&r).unwrap();
    let rects = l.selection_rects(0..r.text.len());
    assert!(rects.len() > 1 && rects.iter().all(|r| r[0] <= r[2] && r[1] < r[3]));
    let first = l.carets.first().unwrap();
    let last = l.caret_move(first, CaretMove::End).unwrap();
    assert!(last.x >= first.x);
    assert_eq!(l.caret_move(last, CaretMove::Home), Some(first));
    assert!(l.caret_move(first, CaretMove::Right).is_some());
    assert_eq!(e.elide(&r, f32::NAN), Err("invalid elision width"));
}
