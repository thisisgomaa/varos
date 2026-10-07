use varos_text_spike::{incremental::*, *};
#[test]
fn edited_paragraph_cache_matches_cold_and_preserves_neighbors() {
    let mut cache = Incremental::default();
    let text = "Logo شعار 12\nالسَّلَامُ عَلَيْكُمْ\nA\u{a0}B";
    let r = Request::new(text, 48., Some(120.));
    let first = cache.layout(&r, || false).unwrap();
    assert_eq!(first, Engine::default().layout(&r).unwrap());
    assert_eq!(cache.counters.reshaped, 3);
    let ids = cache.identities();
    let text2 = text.replacen("السَّلَامُ", "السَّلَامُ X", 1);
    let r = Request::new(&text2, 48., Some(120.));
    let edited = cache.layout(&r, || false).unwrap();
    assert_eq!(edited, Engine::default().layout(&r).unwrap());
    assert_eq!(cache.counters.reshaped, 1);
    assert_eq!(cache.counters.hits, 2);
    assert_eq!(cache.identities()[1], ids[1], "edited paragraph retains identity");
    assert_eq!(cache.identities()[0], ids[0]);
    assert_eq!(cache.identities()[2], ids[2]);
    for width in [600., 120., 600.] {
        let mut r = r.clone();
        r.width = Some(width);
        let l = cache.layout(&r, || false).unwrap();
        assert_eq!(l, Engine::default().layout(&r).unwrap());
    }
    for face in [Face::Plex, Face::Inter] {
        let mut r = r.clone();
        r.face = face;
        assert_eq!(cache.layout(&r, || false).unwrap(), Engine::default().layout(&r).unwrap());
    }
    for lang in ["ar", "fa", "ur", "und"] {
        let mut r = r.clone();
        r.language = lang;
        assert_eq!(cache.layout(&r, || false).unwrap(), Engine::default().layout(&r).unwrap());
    }
}
#[test]
fn cancellation_eviction_split_join_and_paint() {
    let mut cache = Incremental::new(1);
    let mut r = Request::new("Logo\nشعار", 48., Some(120.));
    assert_eq!(cache.layout(&r, || false).unwrap(), Engine::default().layout(&r).unwrap());
    assert_eq!(cache.counters.cache_bytes, 0);
    assert_eq!(cache.counters.evicted, 2);
    let ids = cache.identities();
    r.text = "changed";
    assert_eq!(cache.layout(&r, || true), Err("cancelled"));
    assert_eq!(cache.source(), "Logo\nشعار");
    assert_eq!(cache.identities(), ids);
    let mut cache = Incremental::default();
    for text in ["Logo\nشعار", "Logo شعار", "Logo\nشعار\n", ""] {
        r.text = text;
        assert_eq!(cache.layout(&r, || false).unwrap(), Engine::default().layout(&r).unwrap());
    }
    r.text = "Logo";
    r.styles = vec![Style { range: 0..4, face: Face::Inter, size: 48., baseline_shift: 0., paint: 1 }];
    cache.layout(&r, || false).unwrap();
    r.styles[0].paint = 2;
    cache.layout(&r, || false).unwrap();
    assert_eq!(cache.counters.reshaped, 0);
}

#[test]
fn mid_update_cancellation_and_invalid_ranges_never_publish() {
    let mut cache = Incremental::default();
    let mut r = Request::new("A\nB\nC", 48., Some(120.));
    cache.layout(&r, || false).unwrap();
    let ids = cache.identities();
    let before = cache.source().to_owned();
    r.text = "X\nY\nZ";
    let calls = std::cell::Cell::new(0);
    assert_eq!(
        cache.layout(&r, || {
            calls.set(calls.get() + 1);
            calls.get() >= 4
        }),
        Err("cancelled")
    );
    assert_eq!(cache.source(), before);
    assert_eq!(cache.identities(), ids);
    r.styles = vec![Style { range: 0..999, face: Face::Inter, size: 48., baseline_shift: 0., paint: 0 }];
    assert_eq!(cache.layout(&r, || false), Err("invalid style"));
    assert_eq!(cache.source(), before);
    r.styles.clear();
    r.language_runs = vec![LanguageRun { range: 50..60, language: "ar".into(), script: None }];
    assert_eq!(cache.layout(&r, || false), Err("invalid language range"));
    assert_eq!(cache.source(), before);
}

#[test]
fn shaped_span_reuse_retains_context_attributes_and_offsets() {
    let mut engine = Engine::default();
    for text in [
        "Logo شعار 12 Logo شعار 12",
        "XXXX Logo شعار 12 Logo شعار 12",
        "A ببب Z",
        "A بب Z",
        "A ببب Z",
        "ب\u{200b}ب\u{200b}ب",
    ] {
        for language in ["ar", "ur", "und", "ar"] {
            for direction in [Direction::Auto, Direction::Ltr, Direction::Rtl] {
                let mut r = Request::new(text, 48., Some(120.));
                r.language = language;
                r.direction = direction;
                assert_eq!(engine.layout(&r).unwrap(), Engine::default().layout(&r).unwrap());
                r.width = Some(600.);
                r.face = Face::Plex;
                assert_eq!(engine.layout(&r).unwrap(), Engine::default().layout(&r).unwrap());
            }
        }
    }
    let (hits, misses) = engine.shape_span_cache_stats();
    assert!(hits > 0 && misses > 0);
    assert!(engine.cache_bytes() < 17 * 1024 * 1024);
}
