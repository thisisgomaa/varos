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

/// Deterministic xorshift; no external RNG crate offline.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Random insert/delete sequences through the convergent cache; every result
/// (or refusal) must equal a fresh cold engine's. Pools include marks, joiners,
/// ZWSP, NBSP, isolates/embeddings, digits, punctuation, tabs and emoji.
#[test]
fn convergent_edits_equal_cold_layout() {
    const POOL: &[&str] = &[
        "X",
        " ",
        "ب",
        "ل",
        "ا",
        "ـ",
        "\u{64e}",
        "\u{651}",
        "١",
        "2",
        "(",
        ")",
        "-",
        ".",
        ":",
        "\u{200c}",
        "\u{200d}",
        "\u{200b}",
        "\u{a0}",
        "\u{2067}",
        "\u{2069}",
        "\u{202b}",
        "\u{202c}",
        "e\u{301}",
        "\t",
        "👩\u{200d}💻",
        "fi",
        "شعار ",
        "Logo ",
        "  ",
        "\n",
        "ب\u{200b}ب",
    ];
    let bases = [
        "Logo شعار 12 ".repeat(40),
        "السَّلَامُ عَلَيْكُمْ ورحمة الله وبركاته، موعدنا يوم الثلاثاء الساعة 10:30 صباحًا. ".repeat(6),
        "office café (USD 12.50) — A\u{a0}B ".repeat(12),
        "A\u{2067}شعار 12\u{2069}Z ب\u{200b}ب\u{200b}ب می\u{200c}روم ".repeat(10),
        "office Logo café type v2 السلام عليكم ".repeat(15),
    ];
    let mut incremental_seen = 0;
    let mut reused_seen = 0;
    let mut edges_seen = 0;
    for (case, base) in bases.iter().enumerate() {
        for (config, (width, direction, face, end_align)) in [
            (Some(120.), Direction::Auto, Face::Inter, false),
            (Some(600.), Direction::Rtl, Face::Plex, false),
            (None, Direction::Ltr, Face::Inter, false),
            (Some(300.), Direction::Auto, Face::Plex, true),
        ]
        .into_iter()
        .enumerate()
        {
            let mut rng = Rng(0x9e37_79b9_7f4a_7c15 ^ ((case * 7 + config) as u64 + 1));
            let mut text = base.clone();
            let mut cache = Incremental::default();
            for step in 0..80 {
                let chars: Vec<usize> = text.char_indices().map(|(i, _)| i).chain([text.len()]).collect();
                let at = chars[rng.below(chars.len())];
                if rng.below(3) == 0 && at < text.len() {
                    let end = text[at..].chars().next().map_or(at, |c| at + c.len_utf8());
                    text.replace_range(at..end, "");
                } else {
                    text.insert_str(at, POOL[rng.below(POOL.len())]);
                }
                let mut r = Request::new(&text, 48., width);
                r.direction = direction;
                r.face = face;
                r.end_align = end_align;
                let warm = cache.layout(&r, || false);
                let cold = Engine::default().layout(&r);
                assert_eq!(warm, cold, "case {case} config {config} step {step}: cached != cold");
                incremental_seen += cache.counters.work.analysis_incremental;
                reused_seen += cache.counters.work.lines_reused;
                edges_seen += cache.counters.work.edge_reshapes;
            }
        }
    }
    assert!(edges_seen > 0, "edge reshaping exercised through the cache");
    assert!(incremental_seen > 300, "convergent analysis path exercised: {incremental_seen}");
    assert!(reused_seen > 1000, "line reuse exercised: {reused_seen}");
}

/// Styles and language runs that move with the edit keep the incremental path;
/// multi-paragraph splits/joins and newline edits stay equal to cold layout.
#[test]
fn convergent_edits_with_moving_styles_and_paragraphs() {
    use unicode_segmentation::UnicodeSegmentation;
    let mut rng = Rng(42);
    let mut text = "Logo شعار 12 السَّلَامُ عَلَيْكُمْ ".repeat(12);
    let g: Vec<usize> = text.grapheme_indices(true).map(|(i, _)| i).collect();
    let mut styles = vec![
        Style { range: 0..g[4], face: Face::Plex, size: 30., baseline_shift: 4., paint: 1 },
        Style { range: g[40]..g[120], face: Face::Inter, size: 60., baseline_shift: -3., paint: 2 },
    ];
    let mut runs = vec![LanguageRun { range: g[130]..g[200], language: "ur".into(), script: None }];
    let mut cache = Incremental::default();
    let mut incremental = 0;
    for step in 0..80 {
        let chars: Vec<usize> = text.char_indices().map(|(i, _)| i).chain([text.len()]).collect();
        let at = chars[rng.below(chars.len())];
        let insert = ["X", "ب ", " ", "\u{64e}", "\n", "12"][rng.below(6)];
        let (removed, added) = if rng.below(3) == 0 && at < text.len() {
            let end = text[at..].chars().next().map_or(at, |c| at + c.len_utf8());
            text.replace_range(at..end, "");
            (end - at, 0)
        } else {
            text.insert_str(at, insert);
            (0, insert.len())
        };
        // Hosts keep ranges on grapheme boundaries: shift, then snap down.
        let bounds: Vec<usize> = text.grapheme_indices(true).map(|(i, _)| i).chain([text.len()]).collect();
        let shift = |p: usize| {
            let p = if p > at { (p + added).saturating_sub(removed).max(at) } else { p };
            bounds[bounds.partition_point(|b| *b <= p) - 1]
        };
        for s in &mut styles {
            s.range = shift(s.range.start)..shift(s.range.end);
        }
        for r in &mut runs {
            r.range = shift(r.range.start)..shift(r.range.end);
        }
        styles.retain(|s| !s.range.is_empty());
        runs.retain(|r| !r.range.is_empty());
        let mut r = Request::new(&text, 40., Some(300.));
        r.styles = styles.clone();
        r.language_runs = runs.clone();
        let warm = cache.layout(&r, || false);
        let cold = Engine::default().layout(&r);
        assert!(cold.is_ok(), "step {step}: fixture ranges must stay valid");
        assert_eq!(warm, cold, "step {step}");
        incremental += cache.counters.work.analysis_incremental;
    }
    assert!(incremental > 20, "moving styles kept the incremental path: {incremental}");
}

/// Counters prove locality: a middle character edit in one long paragraph reshapes
/// a handful of level runs and re-fits a handful of lines; a width-only change
/// reuses the analysis (no shaping) and re-fits every line; cached == cold.
#[test]
fn long_paragraph_counters_prove_reuse() {
    // Varied word lengths (a periodic motif can legitimately cascade to the end).
    const WORDS: &[&str] = &["Logo", "شعار", "12", "السلام", "عليكم", "office", "café", "ورحمة", "v2", "الله", "type"];
    let mut rng = Rng(7);
    let text: String = (0..900).map(|_| format!("{} ", WORDS[rng.below(WORDS.len())])).collect();
    let mut cache = Incremental::default();
    let r = Request::new(&text, 48., Some(600.));
    let first = cache.layout(&r, || false).unwrap();
    let lines = first.lines.len();
    assert!(lines > 100);
    assert_eq!(cache.counters.work.analysis_full, 1);
    let mut edited = text.clone();
    edited.insert(text.len() / 2 + 1, 'X');
    let r = Request::new(&edited, 48., Some(600.));
    let layout = cache.layout(&r, || false).unwrap();
    assert_eq!(layout, Engine::default().layout(&r).unwrap());
    let w = cache.counters.work.clone();
    assert_eq!((w.analysis_incremental, w.analysis_full), (1, 0));
    assert!(w.runs_shaped <= 6, "{w:?}");
    assert!(w.lines_refit <= 16 && w.lines_reused + w.lines_refit == layout.lines.len(), "{w:?}");
    // Lines after the convergence point were reused, not only the prefix.
    let prefix = layout.lines.iter().take_while(|l| l.range.end < text.len() / 2 - 30).count();
    assert!(w.lines_reused > prefix, "{w:?} prefix={prefix}");
    // Width-only change: same analysis, zero shaping, every line re-fit.
    let mut r = r.clone();
    r.width = Some(300.);
    let layout = cache.layout(&r, || false).unwrap();
    assert_eq!(layout, Engine::default().layout(&r).unwrap());
    let w = cache.counters.work.clone();
    assert_eq!((w.analysis_reused, w.runs_shaped, w.lines_reused), (1, 0, 0), "{w:?}");
    assert_eq!(w.lines_refit, layout.lines.len());
    // Paint-only change in a long paragraph: no work at all.
    r.styles = vec![Style { range: 0..4, face: Face::Inter, size: 48., baseline_shift: 0., paint: 7 }];
    cache.layout(&r, || false).unwrap();
    r.styles[0].paint = 8;
    cache.layout(&r, || false).unwrap();
    assert_eq!((cache.counters.reshaped, cache.counters.hits), (0, 1));
}

/// Cancellation at every polling point of an incremental long-paragraph update
/// publishes nothing and leaves the cache usable (next result equals cold).
#[test]
fn cancellation_inside_line_fitting_keeps_cache() {
    let text = "Logo شعار 12 ".repeat(200);
    let mut cache = Incremental::default();
    cache.layout(&Request::new(&text, 48., Some(600.)), || false).unwrap();
    let ids = cache.identities();
    let mut edited = text.clone();
    edited.insert(0, 'X');
    let r = Request::new(&edited, 48., Some(600.));
    // Count the polls of a full update without cancelling (on a scratch clone of state).
    let mut probe = Incremental::default();
    probe.layout(&Request::new(&text, 48., Some(600.)), || false).unwrap();
    probe.layout(&r, || false).unwrap();
    let polls = probe.counters.work.cancel_checks;
    assert!(polls > 100, "line fitting polls: {polls}");
    for k in [1, 2, 3, 5, 8, polls / 2, polls - 2, polls - 1] {
        let calls = std::cell::Cell::new(0);
        let result = cache.layout(&r, || {
            calls.set(calls.get() + 1);
            calls.get() >= k
        });
        assert_eq!(result, Err("cancelled"), "poll {k}");
        assert_eq!(cache.source(), text);
        assert_eq!(cache.identities(), ids);
    }
    assert_eq!(cache.layout(&r, || false).unwrap(), Engine::default().layout(&r).unwrap());
    assert_eq!(cache.counters.work.analysis_incremental, 1, "cancelled updates left the old analysis intact");
}

#[test]
fn edits_down_to_empty_and_back() {
    let mut cache = Incremental::default();
    let mut text = String::from("Logo شعار\nب");
    while !text.is_empty() {
        text.pop();
        let r = Request::new(&text, 48., Some(120.));
        assert_eq!(cache.layout(&r, || false), Engine::default().layout(&r));
    }
    for c in "شعار Logo\n\nX".chars() {
        text.push(c);
        let r = Request::new(&text, 48., Some(120.));
        assert_eq!(cache.layout(&r, || false), Engine::default().layout(&r));
    }
}

/// Review fix (2026-10-07): on an unsafe (reshaped) line edge the greedy fitter
/// reads widths/unsafe flags up to a look-ahead unit G, then refits backwards to a
/// smaller final end. A cached line may be reused only if everything it READ (up to
/// G and the flag at G's end) precedes the edited window, not merely its final end.
/// ZWSP-joined Arabic gives non-monotonic edge widths; edits are placed just after
/// line ends so the window starts inside such look-ahead units.
#[test]
fn lookahead_beyond_final_line_end_is_a_dependency() {
    let units = ["بِبَّ", "السَّلَامُ", "ب\u{200d}ب", "قُرْآنٌ", "سـلام", "بب"];
    // Latin tokens and spaces keep level runs and cut segments short, so the
    // edited window is local and earlier cached lines are reuse candidates.
    let text: String =
        (0..36).map(|i| format!("{}\u{200b}{} v{i} ", units[i % units.len()], units[(i + 2) % units.len()])).collect();
    let mut guarded = 0;
    let mut beyond = 0;
    let mut checked = 0;
    for width in (60..=420).step_by(40) {
        let mut base = Request::new(&text, 48., Some(width as f32));
        base.face = Face::Plex;
        base.language = "ar";
        let cold_base = Engine::default().layout(&base).unwrap();
        // Edit right after each line end (inside the next line's first units).
        for line in cold_base.lines.iter().take(cold_base.lines.len().saturating_sub(1)) {
            let mut cache = Incremental::default();
            cache.layout(&base, || false).unwrap();
            beyond += cache.counters.work.lookahead_beyond_last;
            let ends: Vec<usize> = text[line.range.end..]
                .char_indices()
                .map(|(i, _)| line.range.end + i)
                .take_while(|at| *at < line.range.end + 48)
                .collect();
            for (k, at) in ends.into_iter().enumerate() {
                let insert = ["ب", "\u{64e}", "\u{200b}", "X"][k % 4];
                let mut edited = text.clone();
                edited.insert_str(at, insert);
                let mut r = base.clone();
                r.text = &edited;
                let warm = cache.layout(&r, || false);
                assert_eq!(warm, Engine::default().layout(&r), "width {width} edit {insert:?} at {at}");
                guarded += cache.counters.work.lookahead_guarded;
                // Back to the base text, also equal to cold.
                assert_eq!(cache.layout(&base, || false).unwrap(), cold_base, "width {width} undo at {at}");
                guarded += cache.counters.work.lookahead_guarded;
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "edits checked: {checked}");
    assert!(beyond > 0, "fixture must produce look-ahead beyond a final line end: {beyond}");
    assert!(guarded > 0, "fixture must reach the look-ahead dependency guard: {guarded}");
}
