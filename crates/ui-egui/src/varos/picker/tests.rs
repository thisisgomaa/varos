// Varos colour picker v3 — ported from thisisgomaa/varos (MIT OR Apache-2.0).
//! Headless tests of the picker: its maths (triangle ↔ HSV, the ring, harmony rules, modes, hex),
//! its caches, one undo step per gesture (and none without a change), the mixed selection, the
//! cluster, Escape, its preferences and an idle frame asking for no repaint.

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use serde_json::json;
use vectorcraft_color::{Color, Paint};
use vectorcraft_engine::Session;

use super::wheel::{CENTER, RING_BAND, RING_R, TRIANGLE_R, in_triangle, pos_sv, ring_hit, ring_hue, ring_pos, sv_pos, vertices};
use super::*;

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
    app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    app.run("paint.setFill", json!({"color": "#3366cc"})).unwrap();
    app
}

fn undo_len(app: &VectorcraftApp) -> usize {
    app.session.doc().unwrap().history.undo.len()
}

fn fill_hex(app: &VectorcraftApp) -> String {
    crate::panels::current_paints(app).0.color().map(|c| c.to_hex()).unwrap_or_default()
}

/// One frame of the picker, 260 points wide, with `events`; returns the texts drawn and their
/// centres.
fn frame(ctx: &egui::Context, app: &mut VectorcraftApp, events: Vec<Event>) -> Vec<(String, Pos2)> {
    fn texts(s: &egui::Shape, out: &mut Vec<(String, Pos2)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos + t.galley.rect.center().to_vec2())),
            egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }
    let input = egui::RawInput { events, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(260.0, 900.0))), ..Default::default() };
    let mut out = ctx.run_ui(input, |ui| show(app, ui));
    out.textures_delta.clear();
    let mut v = vec![];
    out.shapes.iter().for_each(|c| texts(&c.shape, &mut v));
    v
}

/// The picker's rect as last drawn.
fn picker_rect(ctx: &egui::Context) -> Rect {
    ctx.data(|d| d.get_temp::<(Rect, u64)>(rect_id())).expect("the picker was drawn").0
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Press at `from`, move through `path` (a frame each), release at the last point.
fn drag(ctx: &egui::Context, app: &mut VectorcraftApp, from: Pos2, path: &[Pos2]) {
    frame(ctx, app, vec![Event::PointerMoved(from), button(from, true)]);
    for p in path {
        frame(ctx, app, vec![Event::PointerMoved(*p)]);
    }
    let end = path.last().copied().unwrap_or(from);
    frame(ctx, app, vec![button(end, false)]);
    frame(ctx, app, vec![]);
}

fn click(ctx: &egui::Context, app: &mut VectorcraftApp, at: Pos2) {
    drag(ctx, app, at, &[]);
}

/// The wheel's centre on screen (Wheel tab).
fn centre(ctx: &egui::Context) -> Pos2 {
    picker_rect(ctx).min + vec2(CENTER[0], HEADER_H + CENTER[1])
}

// ---------- maths ----------

#[test]
fn triangle_and_hsv_round_trip_at_every_hue_and_the_vertices() {
    for hi in 0..24 {
        let h = hi as f32 / 24.0;
        let [hue, white, black] = vertices(h, TRIANGLE_R);
        // The hue vertex is on the circle at the hue; white and black follow at +120° and +240°.
        assert!((ring_hue(Pos2::ZERO, hue) - h).abs() < 1e-4 || (ring_hue(Pos2::ZERO, hue) - h).abs() > 1.0 - 1e-4, "{h}");
        assert!((hue.to_vec2().length() - TRIANGLE_R).abs() < 1e-3);
        assert!(
            (ring_hue(Pos2::ZERO, white) - (h + 1.0 / 3.0).rem_euclid(1.0))
                .abs()
                .min(1.0 - (ring_hue(Pos2::ZERO, white) - (h + 1.0 / 3.0).rem_euclid(1.0)).abs())
                < 1e-4
        );
        assert!(sv_pos(h, TRIANGLE_R, 1.0, 1.0).distance(hue) < 1e-3, "full colour at the hue vertex");
        assert!(sv_pos(h, TRIANGLE_R, 0.0, 1.0).distance(white) < 1e-3, "white vertex");
        for s in [0.0, 0.5, 1.0] {
            assert!(sv_pos(h, TRIANGLE_R, s, 0.0).distance(black) < 1e-3, "black vertex");
        }
        for s in [0.05, 0.3, 0.5, 0.8, 1.0] {
            for v in [0.1, 0.4, 0.75, 1.0] {
                let p = sv_pos(h, TRIANGLE_R, s, v);
                let [s2, v2] = pos_sv(h, TRIANGLE_R, p);
                assert!((s - s2).abs() < 1e-3 && (v - v2).abs() < 1e-3, "h {h}: ({s}, {v}) → ({s2}, {v2})");
            }
        }
    }
    // Inside the triangle, not in the ring's hole around it.
    assert!(in_triangle(0.3, TRIANGLE_R, sv_pos(0.3, TRIANGLE_R, 0.5, 0.5)));
    assert!(!in_triangle(0.3, TRIANGLE_R, ring_pos(Pos2::ZERO, TRIANGLE_R - 2.0, 0.3 + 1.0 / 6.0)), "between two vertices, near the ring");
    // The triangle turns with the hue: a third of a turn moves the hue vertex onto white's place.
    assert!(vertices(0.0, TRIANGLE_R)[1].distance(vertices(1.0 / 3.0, TRIANGLE_R)[0]) < 1e-3);
    // Outside points clamp to the closest edge.
    let [s, v] = pos_sv(0.0, TRIANGLE_R, pos2(500.0, 0.0));
    assert!((s - 1.0).abs() < 1e-3 && (v - 1.0).abs() < 1e-3, "beyond the hue vertex: {s} {v}");
    let [_, v] = pos_sv(0.0, TRIANGLE_R, ring_pos(Pos2::ZERO, 500.0, 2.0 / 3.0));
    assert!(v < 1e-3, "beyond the black vertex");
}

#[test]
fn the_ring_puts_red_at_three_oclock_counter_clockwise_and_hits_its_band() {
    let c = pos2(100.0, 100.0);
    assert!(ring_hue(c, pos2(150.0, 100.0)).abs() < 1e-6, "red at three o'clock");
    assert!((ring_hue(c, pos2(100.0, 50.0)) - 0.25).abs() < 1e-6, "a quarter turn is up (counter-clockwise)");
    assert!((ring_hue(c, pos2(50.0, 100.0)) - 0.5).abs() < 1e-6);
    assert!((ring_hue(c, pos2(100.0, 150.0)) - 0.75).abs() < 1e-6);
    for i in 0..36 {
        let h = i as f32 / 36.0;
        let p = ring_pos(c, RING_R - RING_BAND / 2.0, h);
        assert!(ring_hit(c, p, RING_R, RING_BAND));
        let back = ring_hue(c, p);
        assert!((back - h).abs() < 1e-4 || (back - h).abs() > 1.0 - 1e-4, "{h} → {back}");
    }
    assert!(ring_hit(c, pos2(100.0 + RING_R, 100.0), RING_R, RING_BAND), "outer edge");
    assert!(ring_hit(c, pos2(100.0 + RING_R - RING_BAND, 100.0), RING_R, RING_BAND), "inner edge");
    assert!(!ring_hit(c, pos2(100.0 + RING_R + 0.5, 100.0), RING_R, RING_BAND), "outside");
    assert!(!ring_hit(c, pos2(100.0 + RING_R - RING_BAND - 0.5, 100.0), RING_R, RING_BAND), "inside the hole");
    assert!(!ring_hit(c, c, RING_R, RING_BAND));
    // The triangle fits inside the ring's hole.
    const { assert!(TRIANGLE_R < RING_R - RING_BAND) };
}

#[test]
fn harmony_rules_link_the_hues_and_make_six_chips() {
    let base = [0.1, 0.8, 0.9];
    let hues = |rule| harmony::linked(rule, base).iter().map(|c| (c[0] * 360.0).round() as i32).collect::<Vec<_>>();
    assert_eq!(hues(Rule::Complementary), [36, 216]);
    assert_eq!(hues(Rule::Analogous), [36, 6, 66]);
    assert_eq!(hues(Rule::Split), [36, 186, 246]);
    assert_eq!(hues(Rule::Triadic), [36, 156, 276]);
    assert_eq!(hues(Rule::Tetradic), [36, 96, 216, 276]);
    assert_eq!(hues(Rule::Square), [36, 126, 216, 306]);
    assert_eq!(harmony::linked(Rule::None, base), vec![base]);
    let mono = harmony::linked(Rule::Mono, base);
    assert_eq!(mono.len(), 4);
    assert!(mono.iter().all(|c| c[0] == base[0] && c[1] == base[1]), "one hue");
    assert!(mono.windows(2).all(|w| w[1][2] < w[0][2]), "darker steps");
    for rule in [Rule::Complementary, Rule::Analogous, Rule::Split, Rule::Triadic, Rule::Tetradic, Rule::Square, Rule::Mono, Rule::None] {
        let chips = harmony::swatches(rule, base);
        assert_eq!(chips.len(), 6, "{rule:?}");
        assert_eq!(chips[0], base, "{rule:?}: the base first");
        assert_eq!(harmony::variations(rule, base).len(), 5 * harmony::linked(rule, base).len(), "{rule:?}");
        assert!(harmony::variations(rule, base).iter().flatten().all(|v| (0.0..=1.0).contains(v)));
    }
    // A black base keeps the Mono chips distinct (clamped brightness, no duplicates).
    let dark = harmony::swatches(Rule::Mono, [0.0, 0.5, 0.05]);
    assert!(dark.windows(2).all(|w| w[0] != w[1]));
}

#[test]
fn every_mode_round_trips_edges_greys_and_colours() {
    for mode in sliders::MODES {
        for c in [
            [0., 0., 0.],
            [1., 1., 1.],
            [0.5, 0.5, 0.5],
            [22. / 255., 22. / 255., 22. / 255.],
            [0., 35. / 255., 30. / 255.],
            [0.9, 0.2, 0.6],
            [0., 1., 0.],
            [1., 0., 0.],
        ] {
            let out = sliders::rgb(mode, sliders::values(mode, c));
            assert!(c.iter().zip(out).all(|(a, b)| (a - b).abs() <= 1.5 / 255.), "{mode:?}: {c:?} -> {out:?}");
        }
        assert_eq!(sliders::readout(&State::default(), mode).len(), if mode == Mode::Web { 1 } else { sliders::channels(mode).len() });
    }
    // HSB keeps the hue of a grey; Web snaps to the six levels.
    let mut st = State { hsva: [0.5, 0.0, 0.5, 1.0], ..Default::default() };
    assert_eq!(sliders::values_for(&st, Mode::Hsb)[0].round(), 180.0);
    sliders::adopt_channel(&mut st, Mode::Web, 0, 80.0);
    assert!((st.rgb()[0] - 102.0 / 255.0).abs() < 1e-4);
    // The CMYK and Lab sliders are naive (not colour managed): pure black is K 100, and an edit
    // writes sRGB.
    let st = State { hsva: [0.0, 0.0, 0.0, 1.0], ..Default::default() };
    assert_eq!(sliders::values_for(&st, Mode::Cmyk), [0.0, 0.0, 0.0, 100.0]);
    let mut st = State::default();
    sliders::adopt_channel(&mut st, Mode::Cmyk, 3, 50.0);
    assert!(matches!(st.out_color(), Color::Rgb { .. }));
    assert!(same(st.rgb(), [0.5, 0.0, 0.0]), "{:?}", st.rgb());
}

#[test]
fn hex_reads_three_six_and_eight_digits_and_writes_six() {
    assert_eq!(fields::hex_digits([1.0, 0.5, 0.0]), "FF8000");
    assert_eq!(fields::hex_digits([0.0, 0.0, 0.0]), "000000");
    assert_eq!(fields::parse_hex("FF8000"), Some(([1.0, 128.0 / 255.0, 0.0], None)));
    assert_eq!(fields::parse_hex("#ff8000"), fields::parse_hex("FF8000"));
    assert_eq!(fields::parse_hex(" fff "), Some(([1.0, 1.0, 1.0], None)));
    assert_eq!(fields::parse_hex("00000080"), Some(([0.0, 0.0, 0.0], Some(128.0 / 255.0))));
    for bad in ["", "zz", "12345", "1234567", "#gg0000", "ff80٠٠", "+12345"] {
        assert_eq!(fields::parse_hex(bad), None, "{bad:?}");
    }
    for c in [[0.2, 0.4, 0.6], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0]] {
        let ([r, g, b], _) = fields::parse_hex(&fields::hex_digits(c)).unwrap();
        assert!(same([r, g, b], c));
    }
}

#[test]
fn meshes_are_rebuilt_only_when_the_size_or_hue_changes() {
    let mut cache = wheel::Cache::default();
    let ring = cache.ring(RING_R).vertices.as_ptr();
    let tri = cache.triangle(0.25, TRIANGLE_R).vertices.as_ptr();
    for _ in 0..32 {
        assert_eq!(cache.ring(RING_R).vertices.as_ptr(), ring);
        assert_eq!(cache.triangle(0.25, TRIANGLE_R).vertices.as_ptr(), tri);
    }
    assert_eq!(cache.builds, (1, 1));
    cache.triangle(0.5, TRIANGLE_R);
    assert_eq!(cache.builds, (1, 2), "a new hue re-tessellates the triangle only");
    cache.ring(RING_R - 1.0);
    assert_eq!(cache.builds, (2, 2));
    let start = std::time::Instant::now();
    for i in 0..1000 {
        cache.triangle(i as f32 / 1000.0, TRIANGLE_R);
    }
    assert!(start.elapsed() / 1000 < std::time::Duration::from_millis(1), "triangle rebuild under 1 ms");
    let mut tracks = sliders::Cache::default();
    let v = sliders::values(Mode::Rgb, [0.4, 0.6, 0.8]);
    let first = tracks.get(Mode::Rgb, v)[0].vertices.as_ptr();
    for _ in 0..8 {
        assert_eq!(tracks.get(Mode::Rgb, v)[0].vertices.as_ptr(), first);
    }
    assert_eq!(tracks.get(Mode::Cmyk, sliders::values(Mode::Cmyk, [0.4, 0.6, 0.8])).len(), 4);
    assert_eq!(tracks.builds, 2);
}

// ---------- the panel ----------

#[test]
fn a_drag_on_the_triangle_is_one_undo_step_and_a_press_without_change_is_none() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let before = (undo_len(&app), fill_hex(&app));
    // Idle frames write nothing.
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!((undo_len(&app), fill_hex(&app)), before, "idle");
    let c = centre(&ctx);
    let st: State = ctx.data(|d| d.get_temp(state_id())).unwrap();
    // A press and release on the colour's own marker changes nothing: no step.
    let marker = c + sv_pos(st.hsva[0], TRIANGLE_R, st.hsva[1], st.hsva[2]).to_vec2();
    click(&ctx, &mut app, marker);
    assert_eq!((undo_len(&app), fill_hex(&app)), before, "no step without a change");
    // A press outside the ring and the triangle starts no gesture.
    click(&ctx, &mut app, c + vec2(-RING_R + 3.0, -RING_R + 3.0));
    assert_eq!(undo_len(&app), before.0);
    // A drag across the triangle: live on the art while dragging, one step on release.
    let white = c + vertices(st.hsva[0], TRIANGLE_R)[1].to_vec2() * 0.9;
    let black = c + vertices(st.hsva[0], TRIANGLE_R)[2].to_vec2() * 0.9;
    frame(&ctx, &mut app, vec![Event::PointerMoved(marker), button(marker, true)]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(white)]);
    let live = fill_hex(&app);
    assert_ne!(live, before.1, "the art shows the colour while dragging");
    frame(&ctx, &mut app, vec![Event::PointerMoved(black)]);
    assert_ne!(fill_hex(&app), live);
    frame(&ctx, &mut app, vec![button(black, false)]);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(undo_len(&app), before.0 + 1, "one undo step for the drag");
    let after = fill_hex(&app);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(fill_hex(&app), before.1, "undo restores the colour");
    app.run("edit.redo", json!({})).unwrap();
    assert_eq!(fill_hex(&app), after);
    // A drag on the ring changes the hue, again one step.
    let at = |h: f32| ring_pos(c, RING_R - RING_BAND / 2.0, h);
    drag(&ctx, &mut app, at(0.1), &[at(0.3), at(0.5)]);
    assert_eq!(undo_len(&app), before.0 + 2);
    let hue = Color::from_hex(fill_hex(&app).trim_start_matches('#')).unwrap().to_hsb()[0];
    assert!((hue - 180.0).abs() < 2.0, "{hue}");
}

#[test]
fn a_mixed_selection_is_written_only_by_a_change() {
    let mut app = app();
    let b = app.run("shape.rectangle", json!({"x": 80, "y": 10, "width": 50, "height": 50})).unwrap()["id"].clone();
    app.run("select.all", json!({})).unwrap();
    app.run("paint.setFill", json!({"color": "#00ff00", "ids": [b]})).unwrap();
    let ctx = egui::Context::default();
    let undo = undo_len(&app);
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(crate::panels::mixed_paints(&app, &ctx), (true, false));
    assert_eq!(undo_len(&app), undo, "a mixed selection is left alone");
    let texts = frame(&ctx, &mut app, vec![]);
    assert!(!texts.iter().any(|(t, _)| t.len() == 6 && t.chars().all(|c| c.is_ascii_hexdigit())), "no hex for mixed: {texts:?}");
    let c = centre(&ctx);
    let at = |h: f32| ring_pos(c, RING_R - RING_BAND / 2.0, h);
    drag(&ctx, &mut app, at(0.6), &[at(0.62)]);
    assert_eq!(undo_len(&app), undo + 1, "one step");
    assert_eq!(crate::panels::mixed_paints(&app, &ctx), (false, false), "both objects take the colour");
}

#[test]
fn hex_field_commits_one_step_and_eight_digits_set_the_alpha() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    let texts = frame(&ctx, &mut app, vec![]);
    let field = texts.iter().find(|(t, _)| t == "3366CC").unwrap_or_else(|| panic!("hex drawn: {texts:?}")).1;
    let undo = undo_len(&app);
    let key = |k, modifiers| Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers };
    let type_hex = |app: &mut VectorcraftApp, text: &str| {
        click(&ctx, app, field);
        frame(&ctx, app, vec![key(egui::Key::A, Modifiers::COMMAND), Event::Text(text.into())]);
        frame(&ctx, app, vec![key(egui::Key::Enter, Modifiers::NONE)]);
        frame(&ctx, app, vec![]);
    };
    type_hex(&mut app, "ff8000");
    assert_eq!(fill_hex(&app), "#ff8000");
    assert_eq!(undo_len(&app), undo + 1);
    // The same colour again: no step.
    type_hex(&mut app, "FF8000");
    assert_eq!(undo_len(&app), undo + 1);
    // Eight digits: colour and alpha, one step.
    type_hex(&mut app, "00000080");
    assert_eq!(fill_hex(&app), "#000000");
    assert_eq!(undo_len(&app), undo + 2, "colour and alpha are one step");
    let alpha = read_alpha(&app, true).unwrap();
    assert!((alpha - 128.0 / 255.0).abs() < 0.01, "{alpha}");
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(fill_hex(&app), "#ff8000");
    assert!((read_alpha(&app, true).unwrap() - 1.0).abs() < 1e-6);
}

#[test]
fn the_alpha_slider_sets_the_fills_opacity_in_one_step() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(read_alpha(&app, true), Some(1.0));
    let r = picker_rect(&ctx);
    // The row under the wheel: `#`, the hex field, then the alpha track.
    let y = r.top() + HEADER_H + wheel::BODY_H + fields::ROW_H / 2.0;
    let undo = undo_len(&app);
    let id = ctx.viewport(|vp| {
        vp.prev_pass
            .widgets
            .layers()
            .flat_map(|(_, w)| w.iter())
            .find(|w| w.rect.width() == 80.0 && (w.rect.center().y - y).abs() < 4.0)
            .map(|w| w.rect)
    });
    let track = id.expect("the alpha slider");
    drag(&ctx, &mut app, pos2(track.right() - 8.0, y), &[pos2(track.center().x, y), pos2(track.left() + 7.0 + 0.25 * (track.width() - 14.0), y)]);
    let alpha = read_alpha(&app, true).unwrap();
    assert!((alpha - 0.25).abs() < 0.02, "{alpha}");
    assert_eq!(undo_len(&app), undo + 1, "one step");
    assert_eq!(fill_hex(&app), "#3366cc", "the colour stays");
    // The stroke's opacity is its own.
    assert_eq!(read_alpha(&app, false), Some(1.0));
}

#[test]
fn the_cluster_focuses_swaps_and_sets_none() {
    let mut app = app();
    app.run("paint.setStroke", json!({"color": "#000000", "focus": false})).unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let o = picker_rect(&ctx).min + vec2(0.0, HEADER_H);
    assert!(app.session.fill_active);
    // The stroke ring, outside the fill circle.
    let ring = o + vec2(cluster::STROKE_CENTER[0], cluster::STROKE_CENTER[1] - cluster::STROKE_R + 2.0);
    assert_eq!(cluster::target_at(o, ring, true), Some(false));
    click(&ctx, &mut app, ring);
    assert!(!app.session.fill_active, "the stroke is focused");
    let fill = o + vec2(cluster::FILL_CENTER[0] + 8.0, cluster::FILL_CENTER[1] + 8.0);
    click(&ctx, &mut app, fill);
    assert!(app.session.fill_active, "the fill is focused");
    // Swap.
    let undo = undo_len(&app);
    click(&ctx, &mut app, o + vec2(222.0, 204.0));
    assert_eq!((fill_hex(&app).as_str(), undo_len(&app)), ("#000000", undo + 1), "swapped");
    // None for the focused one.
    click(&ctx, &mut app, o + vec2(182.0, 238.0));
    assert!(matches!(crate::panels::current_paints(&app).0, Paint::None));
}

#[test]
fn escape_over_the_picker_closes_the_panel_and_never_undoes() {
    let mut app = app();
    app.run("window.panel", json!({"panel": "color"})).unwrap();
    let shown = |app: &VectorcraftApp| crate::menus::checked(app, "window.panel", &json!({"panel": "color"}));
    assert_eq!(shown(&app), Some(true));
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    let undo = undo_len(&app);
    let inside = picker_rect(&ctx).center();
    let esc = Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
    // Away from the picker, Escape is not the picker's.
    let input = egui::RawInput { events: vec![Event::PointerMoved(pos2(1000.0, 1000.0)), esc.clone()], ..Default::default() };
    let _ = ctx.run_ui(input, |ui| {
        assert!(!escape(&mut app, ui.ctx()));
        assert!(ui.input(|i| i.key_pressed(egui::Key::Escape)), "left for the others");
    });
    assert_eq!(shown(&app), Some(true));
    frame(&ctx, &mut app, vec![Event::PointerMoved(inside)]);
    let _ = ctx.run_ui(egui::RawInput { events: vec![esc], ..Default::default() }, |ui| {
        assert!(escape(&mut app, ui.ctx()), "Escape over the picker is its own");
        assert!(!ui.input(|i| i.key_pressed(egui::Key::Escape)), "consumed");
    });
    assert_eq!(shown(&app), Some(false), "closed");
    assert_eq!(undo_len(&app), undo, "nothing undone");
}

#[test]
fn a_double_click_on_a_proxy_opens_the_picker_for_it_not_the_modal() {
    let mut app = app();
    assert_ne!(crate::menus::checked(&app, "window.panel", &json!({"panel": "color"})), Some(true));
    open_for(&mut app, true);
    assert!(!app.session.fill_active);
    assert!(app.ui.dialog.is_none());
    assert_eq!(crate::menus::checked(&app, "window.panel", &json!({"panel": "color"})), Some(true));
    // Already showing: it stays open and changes focus only.
    open_for(&mut app, false);
    assert!(app.session.fill_active);
    assert_eq!(crate::menus::checked(&app, "window.panel", &json!({"panel": "color"})), Some(true));
}

#[test]
fn tabs_drawer_and_recent_colours_are_kept_and_old_profiles_load() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.ui.varos_picker.tab = Tab::Sliders;
    app.ui.varos_picker.mode = Mode::Cmyk;
    app.ui.varos_picker.harmony = Rule::Triadic;
    app.ui.varos_picker.drawer_open = true;
    app.run("paint.setFill", json!({"color": "#123456"})).unwrap();
    frame(&ctx, &mut app, vec![]);
    assert_eq!(app.ui.varos_picker.recent.first().map(String::as_str), Some("#123456"), "recent colours are kept");
    let saved: crate::UiState = serde_json::from_value(serde_json::to_value(&app.ui).unwrap()).unwrap();
    let saved = saved.sanitized();
    assert_eq!(saved.varos_picker, app.ui.varos_picker);
    // A new session shows the kept recent colours before any paint command.
    let mut fresh = VectorcraftApp::new(Session::new(), Default::default());
    fresh.ui = saved;
    assert!(fresh.session.recent_colors.is_empty());
    assert_eq!(drawer::recent(&fresh).first().map(Color::to_hex).as_deref(), Some("#123456"));
    // Profiles from before the picker, and malformed values, load with the defaults.
    let old: crate::UiState = serde_json::from_value(json!({"place_link": false})).unwrap();
    assert_eq!((old.varos_picker, old.place_link), (Prefs::default(), false));
    for junk in [json!(7), json!("wheel"), json!(null), json!([1, 2])] {
        let ui: crate::UiState = serde_json::from_value(json!({"varos_picker": junk, "place_link": false})).unwrap();
        assert_eq!((ui.varos_picker, ui.place_link), (Prefs::default(), false), "{junk}");
    }
    let ui: crate::UiState =
        serde_json::from_value(json!({"varos_picker": {"tab": "gradient", "mode": "lab", "drawer_open": "yes", "recent": ["#ff0000"]}})).unwrap();
    assert_eq!((ui.varos_picker.tab, ui.varos_picker.mode, ui.varos_picker.drawer_open), (Tab::Wheel, Mode::Lab, false), "per-field defaults");
    assert_eq!(ui.varos_picker.recent, ["#ff0000"]);
}

#[test]
fn an_idle_picker_asks_for_no_repaint() {
    let mut app = app();
    let ctx = egui::Context::default();
    egui_extras::install_image_loaders(&ctx);
    for tab in [Tab::Wheel, Tab::Sliders, Tab::Harmony] {
        app.ui.varos_picker.tab = tab;
        for _ in 0..4 {
            frame(&ctx, &mut app, vec![]);
        }
        frame(&ctx, &mut app, vec![]);
        assert!(!ctx.has_requested_repaint(), "{tab:?}: idle frames asked for a repaint: {:?}", ctx.repaint_causes());
    }
}

#[test]
fn the_picker_renders_every_tab_at_240_points_in_a_harness() {
    for (tab, drawer) in [(Tab::Wheel, false), (Tab::Sliders, true), (Tab::Harmony, false), (Tab::Harmony, true)] {
        let mut app = app();
        app.ui.varos_picker.tab = tab;
        app.ui.varos_picker.drawer_open = drawer;
        let mut fonts = false;
        let mut h = egui_kittest::Harness::builder().with_size(vec2(W, 900.0)).build_ui_state(
            move |ui, app: &mut VectorcraftApp| {
                if !std::mem::replace(&mut fonts, true) {
                    crate::theme::install_fonts(ui.ctx());
                    egui_extras::install_image_loaders(ui.ctx());
                    return;
                }
                show(app, ui);
            },
            app,
        );
        h.run();
        let rect = picker_rect(&h.ctx);
        assert_eq!(rect.width(), W, "{tab:?}");
        assert_eq!(rect.height(), height(&h.state().ui.varos_picker), "{tab:?}");
    }
}

#[test]
fn the_sliders_tab_drags_a_channel_in_one_step_and_types_a_value() {
    let mut app = app();
    app.ui.varos_picker.tab = Tab::Sliders;
    app.ui.varos_picker.mode = Mode::Rgb;
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    let texts = frame(&ctx, &mut app, vec![]);
    for l in ["R", "G", "B"] {
        assert!(texts.iter().any(|(t, _)| t == l), "{l}: {texts:?}");
    }
    let o = picker_rect(&ctx).min + vec2(0.0, HEADER_H);
    let y = o.y + sliders::HEADER_H + sliders::ROW_H / 2.0;
    let left = o.x + PAD + 18.0;
    let undo = undo_len(&app);
    drag(&ctx, &mut app, pos2(left + 10.0, y), &[pos2(left + 70.0, y), pos2(left + sliders::SLIDER_W, y)]);
    assert_eq!(undo_len(&app), undo + 1);
    assert_eq!(fill_hex(&app), "#ff66cc", "R at its maximum");
    let value = frame(&ctx, &mut app, vec![]).into_iter().find(|(t, _)| t == "255").expect("the R value").1;
    let key = |k, modifiers| Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers };
    click(&ctx, &mut app, value);
    frame(&ctx, &mut app, vec![key(egui::Key::A, Modifiers::COMMAND), Event::Text("0".into())]);
    frame(&ctx, &mut app, vec![key(egui::Key::Enter, Modifiers::NONE)]);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(fill_hex(&app), "#0066cc");
    assert_eq!(undo_len(&app), undo + 2);
}

#[test]
fn a_harmony_chip_and_a_recent_chip_set_the_target_in_one_step() {
    let mut app = app();
    app.ui.varos_picker.tab = Tab::Harmony;
    app.ui.varos_picker.harmony = Rule::Complementary;
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let o = picker_rect(&ctx).min + vec2(0.0, HEADER_H + harmony::STRIP_H + wheel::BODY_H);
    let undo = undo_len(&app);
    // The second chip is the complement of #3366cc.
    click(&ctx, &mut app, o + vec2(PAD + harmony::SWATCH + 4.0 + harmony::SWATCH / 2.0, harmony::ROW_H / 2.0));
    assert_eq!(fill_hex(&app), "#cc9933");
    assert_eq!(undo_len(&app), undo + 1);
    // The first recent chip is the colour before.
    app.run("paint.setFill", json!({"color": "#3366cc"})).unwrap();
    frame(&ctx, &mut app, vec![]);
    let r = picker_rect(&ctx);
    let undo = undo_len(&app);
    let row = r.bottom() - drawer::ROW_H / 2.0;
    click(&ctx, &mut app, pos2(r.left() + PAD + 16.0 + 4.0 + 8.0, row));
    assert_eq!(fill_hex(&app), "#cc9933", "the second recent colour");
    assert_eq!(undo_len(&app), undo + 1);
}

// ---------- review fixes (2026-10-20) ----------

fn stroke_hex(app: &VectorcraftApp) -> String {
    crate::panels::current_paints(app).1.color().map(|c| c.to_hex()).unwrap_or_default()
}

fn esc() -> Event {
    Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// Fix 1: Escape mid-drag commits what the drag previewed (one step) and leaves no interaction
/// open; so does putting the panel away mid-drag (the picker not drawn).
#[test]
fn escape_or_hiding_mid_drag_commits_the_gesture_and_closes_the_interaction() {
    for hide in [false, true] {
        let mut app = app();
        app.run("window.panel", json!({"panel": "color"})).unwrap();
        let ctx = egui::Context::default();
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![]);
        let (undo, before) = (undo_len(&app), fill_hex(&app));
        let c = centre(&ctx);
        let at = |h: f32| ring_pos(c, RING_R - RING_BAND / 2.0, h);
        frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.1)), button(at(0.1), true)]);
        frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.3))]);
        let previewed = fill_hex(&app);
        assert_ne!(previewed, before);
        assert!(app.session.in_interaction(), "the drag previews in an interaction");
        if hide {
            // Two frames without the picker (the panel put away), the button still held.
            for _ in 0..2 {
                let _ = ctx.run_ui(egui::RawInput::default(), |ui| {
                    escape(&mut app, ui.ctx());
                });
            }
        } else {
            let _ = ctx.run_ui(egui::RawInput { events: vec![esc()], ..Default::default() }, |ui| {
                assert!(escape(&mut app, ui.ctx()));
            });
            assert_ne!(crate::menus::checked(&app, "window.panel", &json!({"panel": "color"})), Some(true), "closed");
        }
        assert!(!app.session.in_interaction(), "hide {hide}: no interaction left open");
        assert_eq!((undo_len(&app), fill_hex(&app)), (undo + 1, previewed.clone()), "hide {hide}: one step, the previewed colour");
        // A later edit elsewhere doesn't replace it.
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 20, "height": 20})).unwrap();
        app.run("edit.undo", json!({})).unwrap();
        app.run("select.all", json!({})).unwrap();
        assert_eq!(fill_hex(&app), previewed, "hide {hide}");
    }
}

/// Fix 2: X pressed during a drag on the Fill: the drag keeps previewing and commits on the Fill;
/// the Stroke is untouched and stays in front as asked.
#[test]
fn a_focus_change_mid_drag_keeps_the_target_the_drag_began_on() {
    let mut app = app();
    app.run("paint.setStroke", json!({"color": "#000000", "focus": false})).unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let undo = undo_len(&app);
    let c = centre(&ctx);
    let at = |h: f32| ring_pos(c, RING_R - RING_BAND / 2.0, h);
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.1)), button(at(0.1), true)]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.2))]);
    app.run("paint.toggleActive", json!({})).unwrap();
    assert!(!app.session.fill_active);
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.5))]);
    let live = fill_hex(&app);
    assert_eq!(stroke_hex(&app), "#000000", "the stroke isn't previewed");
    frame(&ctx, &mut app, vec![button(at(0.5), false)]);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(fill_hex(&app), live, "committed on the fill");
    assert_ne!(live, "#3366cc");
    assert_eq!(stroke_hex(&app), "#000000", "the stroke untouched");
    assert_eq!(undo_len(&app), undo + 1);
    assert!(!app.session.fill_active, "the stroke stays in front");
    let hue = Color::from_hex(live.trim_start_matches('#')).unwrap().to_hsb()[0];
    assert!((hue - 180.0).abs() < 2.0, "{hue}");
}

/// Fix 3: on a mixed selection, pressing the shown marker without moving writes nothing.
#[test]
fn pressing_the_marker_of_a_mixed_selection_writes_nothing() {
    let mut app = app();
    let b = app.run("shape.rectangle", json!({"x": 80, "y": 10, "width": 50, "height": 50})).unwrap()["id"].clone();
    app.run("select.all", json!({})).unwrap();
    app.run("paint.setFill", json!({"color": "#00ff00", "ids": [b]})).unwrap();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let undo = undo_len(&app);
    let c = centre(&ctx);
    let st: State = ctx.data(|d| d.get_temp(state_id())).unwrap();
    for at in [c + sv_pos(st.hsva[0], TRIANGLE_R, st.hsva[1], st.hsva[2]).to_vec2(), ring_pos(c, RING_R - RING_BAND / 2.0, st.hsva[0])] {
        click(&ctx, &mut app, at);
        assert_eq!(undo_len(&app), undo, "no step");
        assert_eq!(crate::panels::mixed_paints(&app, &ctx), (true, false), "still mixed");
        assert!(!app.session.in_interaction());
    }
}

/// Fix 4: × and Escape close the Color panel wherever it shows: a floating group (not just
/// activated), its card, and upstream's pop-out.
#[test]
fn close_puts_the_panel_away_from_a_floating_group_a_card_and_a_pop_out() {
    let shown = |app: &VectorcraftApp| crate::menus::checked(app, "window.panel", &json!({"panel": "color"})) == Some(true);
    // Floating.
    let mut app = app();
    app.run("window.panel.float", json!({"panel": "color"})).unwrap();
    assert!(crate::panel_docking::floats(&app.ui, "color"));
    assert!(shown(&app));
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    let r = picker_rect(&ctx);
    let x = pos2(r.left() + W - PAD - TAB_W / 2.0, r.top() + HEADER_H / 2.0);
    click(&ctx, &mut app, x);
    assert!(!shown(&app), "× closes the floating Color panel");
    // Its card (the icon's default).
    let mut app = self::app();
    app.run("window.panel", json!({"panel": "color"})).unwrap();
    assert!(shown(&app));
    close(&mut app);
    assert!(!shown(&app), "card closed");
    // Upstream's pop-out.
    let mut app = self::app();
    app.ui.open_panel = Some("color".into());
    if shown(&app) {
        close(&mut app);
        assert!(!shown(&app), "pop-out closed");
    }
}

/// Fix 5: an alpha drag over objects whose fills sit at different places in their stacks
/// previews on all of them and commits one step.
#[test]
fn an_alpha_drag_previews_every_object_whatever_its_stack() {
    let mut app = app();
    let b = app.run("shape.rectangle", json!({"x": 80, "y": 10, "width": 50, "height": 50})).unwrap()["id"].as_u64().unwrap();
    app.run("paint.setFill", json!({"color": "#3366cc", "ids": [b]})).unwrap();
    app.run("appearance.addFill", json!({"ids": [b]})).unwrap();
    app.run("select.all", json!({})).unwrap();
    assert_eq!(alpha_groups(&app, true).len(), 2, "two stack positions");
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let undo = undo_len(&app);
    let r = picker_rect(&ctx);
    let y = r.top() + HEADER_H + wheel::BODY_H + fields::ROW_H / 2.0;
    let track = ctx
        .viewport(|vp| {
            vp.prev_pass
                .widgets
                .layers()
                .flat_map(|(_, w)| w.iter())
                .find(|w| w.rect.width() == 80.0 && (w.rect.center().y - y).abs() < 4.0)
                .map(|w| w.rect)
        })
        .expect("the alpha slider");
    let at = |a: f32| pos2(track.left() + 7.0 + a * (track.width() - 14.0), y);
    let opacities = |app: &VectorcraftApp| {
        alpha_groups(app, true)
            .into_iter()
            .map(|(i, ids)| {
                let st = app.session.active().unwrap();
                st.doc.node(vectorcraft_doc::NodeId(ids[0])).unwrap().appearance.items[i].opacity()
            })
            .collect::<Vec<_>>()
    };
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.9)), button(at(0.9), true)]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.5))]);
    let live = opacities(&app);
    assert!(live.iter().all(|o| (o - 0.5).abs() < 0.02), "live on every object: {live:?}");
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.3))]);
    frame(&ctx, &mut app, vec![button(at(0.3), false)]);
    frame(&ctx, &mut app, vec![]);
    let done = opacities(&app);
    assert!(done.iter().all(|o| (o - 0.3).abs() < 0.02), "{done:?}");
    assert_eq!(undo_len(&app), undo + 1, "one step");
    app.run("edit.undo", json!({})).unwrap();
    assert!(opacities(&app).iter().all(|o| (o - 1.0).abs() < 1e-6));
}

/// Fix 6: movement delivered in the release frame is where the commit lands (ring, channel, alpha).
#[test]
fn the_release_frames_movement_is_committed() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    frame(&ctx, &mut app, vec![]);
    let c = centre(&ctx);
    let at = |h: f32| ring_pos(c, RING_R - RING_BAND / 2.0, h);
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.1)), button(at(0.1), true)]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.3))]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(at(0.5)), button(at(0.5), false)]);
    frame(&ctx, &mut app, vec![]);
    let hue = Color::from_hex(fill_hex(&app).trim_start_matches('#')).unwrap().to_hsb()[0];
    assert!((hue - 180.0).abs() < 2.0, "ring: {hue}");
    // A channel of the Sliders tab.
    app.ui.varos_picker.tab = Tab::Sliders;
    app.ui.varos_picker.mode = Mode::Rgb;
    frame(&ctx, &mut app, vec![]);
    let o = picker_rect(&ctx).min + vec2(0.0, HEADER_H);
    let y = o.y + sliders::HEADER_H + sliders::ROW_H / 2.0;
    let left = o.x + PAD + 18.0;
    frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(left + 20.0, y)), button(pos2(left + 20.0, y), true)]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(left + 60.0, y))]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(pos2(left + sliders::SLIDER_W, y)), button(pos2(left + sliders::SLIDER_W, y), false)]);
    frame(&ctx, &mut app, vec![]);
    assert!(fill_hex(&app).starts_with("#ff"), "R at the release point: {}", fill_hex(&app));
}

#[test]
fn gradient_tab_and_gradient_focus_open_upstream_panel_once() {
    let mut app = app();
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![]);
    let r = picker_rect(&ctx);
    let at = r.min + vec2(PAD + 3. * (TAB_W + TAB_GAP) + TAB_W / 2., HEADER_H / 2.);
    click(&ctx, &mut app, at);
    assert!(crate::varos::panel_stack::is_open(&app, "gradient") || crate::panel_docking::showing(&app.ui, "gradient"));
    let stops = json!([{"offset":0,"color":"#000000"},{"offset":1,"color":"#ffffff"}]);
    app.run("paint.setFill", json!({"gradient":{"stops":stops}})).unwrap();
    frame(&ctx, &mut app, vec![]);
    assert!(crate::varos::panel_stack::is_open(&app, "gradient") || crate::panel_docking::showing(&app.ui, "gradient"));
    let before = serde_json::to_value(&app.ui).unwrap();
    frame(&ctx, &mut app, vec![]);
    assert_eq!(serde_json::to_value(&app.ui).unwrap(), before, "idle gradient focus must not reopen or move panels");
}
