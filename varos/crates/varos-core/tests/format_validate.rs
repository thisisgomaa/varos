//! S5-C: refusal boundaries, save/load symmetry and actual model editing operations.
use serde_json::json;
use varos_core::editor::Editor;
use varos_core::format::{check_structure, decode_model, encode_model, validate, Invalid, Limits, LoadError};
use varos_core::model::{Anchor, Artboard, Document, DropPos, GroupRole, Guide, NodeKind, Paint, Path, Xform};

fn document() -> Document {
    let mut d = Document { artboards: vec![Artboard::default()], ..Document::default() };
    for id in 1..=3 {
        let a = Anchor { id: id * 10, p: [1.0, 2.0], hin: Some([0.0, 1.0]), hout: Some([3.0, 4.0]), smooth: false };
        let mut p = Path::new(id, vec![a.clone()], false, Some([0.2, 0.4, 0.6, 1.0]), Some([0.0, 0.0, 0.0, 1.0]), 2.0);
        p.holes.push(vec![Anchor { id: id * 10 + 1, ..a }]);
        p.name = Some(format!("شكل {id}"));
        d.paths.push(p);
    }
    d.ids = 100;
    d.sync_tree();
    d.guides.push(Guide { vertical: true, pos: 15.0 });
    d.nodes[0].color = Some([0.1, 0.2, 0.3, 1.0]);
    d
}
/// `d` stamped as `version`; below format 3 without the board keys (that era never wrote them).
fn raw(d: &Document, version: u32) -> Vec<u8> {
    let mut v = json!({"varos":version,"doc":d});
    if version < 3 {
        for key in ["name", "description", "tags"] {
            v["doc"].as_object_mut().unwrap().remove(key);
        }
    }
    serde_json::to_vec(&v).unwrap()
}
fn roundtrip(d: &Document) {
    let a = encode_model(d, &Limits::DEFAULT).unwrap();
    let loaded = decode_model(a.as_bytes(), None, &Limits::DEFAULT).unwrap();
    assert_eq!(encode_model(&loaded.doc, &Limits::DEFAULT).unwrap(), a);
}

#[test]
fn valid_editor_documents_pass() {
    roundtrip(&Editor::new().doc);
    let mut d = document();
    roundtrip(&d);
    let group = d.group(&[1, 2]).unwrap();
    d.set_node_xform(group, Xform { rot: 0.3, piv: [10.0, 20.0] });
    roundtrip(&d);
    d.ungroup(&[1, 2]);
    d.clip_group(&[1, 2], 2).unwrap();
    roundtrip(&d);
    d.artboards.clear();
    roundtrip(&d);
    d.artboards = vec![Artboard::default(), Artboard { w: 0.0, h: 0.0, ..Artboard::default() }];
    roundtrip(&d);
}

#[test]
fn root_paths_and_groups_permitted_by_live_model_remain_saveable() {
    let mut d = document();
    let root = d.roots[0];
    let leaf = d.node_of_path(3).unwrap();
    assert!(d.move_node_to(leaf, root, DropPos::Before));
    roundtrip(&d);
    let group = d.group(&[1, 2]).unwrap();
    assert!(d.move_node_to(group, root, DropPos::Before));
    roundtrip(&d);
}

// Enumerate every persisted float slot, including holes, optional handles and node colors.
fn floats(d: &mut Document, mut visit: impl FnMut(&mut f32)) {
    for p in &mut d.paths {
        for a in p.anchors.iter_mut().chain(p.holes.iter_mut().flatten()) {
            for v in &mut a.p {
                visit(v);
            }
            for v in a.hin.iter_mut().chain(a.hout.iter_mut()).flatten() {
                visit(v);
            }
        }
        visit(&mut p.stroke_width);
        visit(&mut p.opacity);
        for paint in [&mut p.fill, &mut p.stroke] {
            if let Paint::Solid(c) = paint {
                for v in c {
                    visit(v);
                }
            }
        }
    }
    for n in &mut d.nodes {
        visit(&mut n.xform.rot);
        for v in &mut n.xform.piv {
            visit(v);
        }
        if let Some(c) = &mut n.color {
            for v in c {
                visit(v);
            }
        }
    }
    for b in &mut d.artboards {
        for v in [&mut b.x, &mut b.y, &mut b.w, &mut b.h, &mut b.bleed] {
            visit(v);
        }
        if let Some(c) = &mut b.page_color {
            for v in c {
                visit(v);
            }
        }
    }
    for g in &mut d.guides {
        visit(&mut g.pos);
    }
    for v in &mut d.ruler_origin {
        visit(v);
    }
    visit(&mut d.snap.radius_px);
    visit(&mut d.snap.grid_spacing);
    visit(&mut d.units.ppi);
}
#[test]
fn every_persisted_float_rejects_nan_and_both_infinities_on_save() {
    let mut base = document();
    let mut count = 0;
    floats(&mut base, |_| count += 1);
    assert!(count > 90);
    for slot in 0..count {
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut d = base.clone();
            let mut i = 0;
            floats(&mut d, |v| {
                if i == slot {
                    *v = invalid;
                }
                i += 1;
            });
            assert!(matches!(validate(&d, &Limits::DEFAULT), Err(Invalid::NonFinite { .. })), "slot {slot}");
            assert!(
                matches!(
                    encode_model(&d, &Limits::DEFAULT).unwrap_err().0,
                    LoadError::Invalid(Invalid::NonFinite { .. })
                ),
                "save slot {slot}"
            );
        }
    }
}
#[test]
fn nonfinite_identity_pivot_cannot_disappear_during_serialization() {
    let mut d = document();
    d.nodes[0].xform.piv[0] = f32::NAN; // xform would be skipped because rot=0
    assert!(matches!(encode_model(&d, &Limits::DEFAULT).unwrap_err().0, LoadError::Invalid(Invalid::NonFinite { .. })));
}
#[test]
fn invalid_nested_transform_cannot_be_normalized_away() {
    let mut d = document();
    d.group(&[1, 2]).unwrap();
    let leaf = d.node_of_path(1).unwrap();
    d.set_node_xform(leaf, Xform { rot: f32::INFINITY, piv: [0.0, 0.0] });
    assert!(matches!(encode_model(&d, &Limits::DEFAULT).unwrap_err().0, LoadError::Invalid(Invalid::NonFinite { .. })));
}

#[test]
fn ranges_refused_on_load_and_save_without_mutation() {
    let base = document();
    let mut value = json!({"varos":3,"doc":base});
    // empty board keys dropped so the same blob is a valid v1, v2 and v3 file (v3 defaults them)
    for key in ["name", "description", "tags"] {
        value["doc"].as_object_mut().unwrap().remove(key);
    }
    let paths = [
        "/doc/paths/0/stroke_width",
        "/doc/paths/0/opacity",
        "/doc/paths/0/fill/0",
        "/doc/paths/0/fill/1",
        "/doc/paths/0/fill/2",
        "/doc/paths/0/fill/3",
        "/doc/paths/0/stroke/0",
        "/doc/paths/0/stroke/1",
        "/doc/paths/0/stroke/2",
        "/doc/paths/0/stroke/3",
        "/doc/nodes/0/color/0",
        "/doc/nodes/0/color/1",
        "/doc/nodes/0/color/2",
        "/doc/nodes/0/color/3",
        "/doc/artboards/0/w",
        "/doc/artboards/0/h",
        "/doc/artboards/0/bleed",
        "/doc/artboards/0/page_color/0",
        "/doc/artboards/0/page_color/1",
        "/doc/artboards/0/page_color/2",
        "/doc/artboards/0/page_color/3",
        "/doc/units/ppi",
    ];
    for path in paths {
        let saved = value.pointer(path).unwrap().clone();
        for invalid in [-0.1, 1.1] {
            if invalid > 0.0
                && (path.ends_with("width")
                    || path.ends_with("/w")
                    || path.ends_with("/h")
                    || path.ends_with("bleed")
                    || path.ends_with("ppi"))
            {
                continue;
            }
            *value.pointer_mut(path).unwrap() = json!(invalid);
            let d: Document = serde_json::from_value(value["doc"].clone()).unwrap();
            let before = d.clone();
            assert!(
                matches!(
                    encode_model(&d, &Limits::DEFAULT).unwrap_err().0,
                    LoadError::Invalid(Invalid::OutOfRange { .. })
                ),
                "{path}"
            );
            assert_eq!(d, before);
            for version in [1, 2, 3] {
                value["varos"] = json!(version);
                assert!(
                    matches!(
                        decode_model(&serde_json::to_vec(&value).unwrap(), None, &Limits::DEFAULT),
                        Err(LoadError::Invalid(Invalid::OutOfRange { .. }))
                    ),
                    "{path}"
                );
            }
        }
        *value.pointer_mut(path).unwrap() = saved;
    }
    let mut d = base;
    d.units.ppi = 0.0;
    assert!(matches!(validate(&d, &Limits::DEFAULT), Err(Invalid::OutOfRange { .. })));
    d.units.ppi = 72.0;
    d.snap.candidate_max = usize::MAX; // unused; no invented editor invariant
    roundtrip(&d);
}

#[test]
fn kind_and_mask_rules_refuse_structurally_valid_corruption() {
    for case in 0..7 {
        let mut d = document();
        let group = d.group(&[1, 2]).unwrap();
        let gi = d.nodes.iter().position(|n| n.id == group).unwrap();
        let leaf = d.node_of_path(1).unwrap();
        let li = d.nodes.iter().position(|n| n.id == leaf).unwrap();
        match case {
            0 => d.nodes[li].kind = NodeKind::Layer,   // layer inside group
            1 => d.nodes[gi].kind = NodeKind::Path(1), // leaf with children
            2 => {
                let p3 = d.node_of_path(3).unwrap();
                d.nodes.iter_mut().find(|n| n.id == p3).unwrap().kind = NodeKind::Path(1);
            }
            3 => d.nodes[gi].role = GroupRole::MaskAlpha,
            4 => d.nodes[gi].role = GroupRole::MaskLuma,
            5 => d.nodes[gi].mask_child = Some(leaf), // ordinary node with mask
            6 => {
                d.nodes[0].role = GroupRole::Clip;
                d.nodes[0].mask_child = Some(group);
            } // clip on layer
            _ => unreachable!(),
        }
        check_structure(&d, &Limits::DEFAULT).unwrap();
        assert!(validate(&d, &Limits::DEFAULT).is_err(), "case {case}");
        assert!(encode_model(&d, &Limits::DEFAULT).is_err(), "save {case}");
        for version in [1, 2] {
            assert!(decode_model(&raw(&d, version), None, &Limits::DEFAULT).is_err(), "load {case}");
        }
    }
}
#[test]
fn missing_path_leaf_is_adopted_on_save_but_refused_in_v2() {
    let mut d = document();
    d.nodes.clear();
    d.roots.clear();
    assert!(matches!(validate(&d, &Limits::DEFAULT), Err(Invalid::NotCanonical { .. })));
    assert!(decode_model(&raw(&d, 2), None, &Limits::DEFAULT).is_err());
    roundtrip(&d); // live editor can hand normalization tree-less paths
}
#[test]
fn refusal_names_the_affected_path() {
    let mut d = document();
    d.paths[1].opacity = 3.0;
    let error = encode_model(&d, &Limits::DEFAULT).unwrap_err().to_string();
    assert!(error.contains("path 2 (شكل 2): opacity"), "{error}");
}
#[test]
fn legacy_repair_does_not_hide_other_corruption() {
    let mut d = document();
    let group = d.clip_group(&[1, 2], 2).unwrap();
    d.nodes.iter_mut().find(|n| n.id == group).unwrap().mask_child = Some(9999);
    d.paths[0].opacity = -1.0;
    assert!(matches!(
        decode_model(&raw(&d, 1), None, &Limits::DEFAULT),
        Err(LoadError::Invalid(Invalid::OutOfRange { .. }))
    ));
    d.paths[0].opacity = 1.0;
    d.nodes.iter_mut().find(|n| n.id == group).unwrap().children.push(9998);
    assert!(matches!(
        decode_model(&raw(&d, 1), None, &Limits::DEFAULT),
        Err(LoadError::Invalid(Invalid::Dangling { missing: 9998, .. }))
    ));
}

#[test]
#[ignore = "manual release timing, no flaky wall-clock assertion"]
fn validate_is_linear() {
    for n in [10_000u32, 20_000, 40_000] {
        // Build a valid flat tree directly: do not include sync_tree's cost in this measurement.
        let mut d = document();
        d.paths.truncate(1);
        let path = d.paths[0].clone();
        let template = d.nodes.iter().find(|n| matches!(n.kind, NodeKind::Path(_))).unwrap().clone();
        let root = d.roots[0];
        d.nodes.truncate(1);
        d.nodes[0].children.clear();
        d.paths.clear();
        for i in 1..=n {
            let mut p = path.clone();
            p.id = i;
            d.paths.push(p);
            let mut node = template.clone();
            node.id = n + i;
            node.kind = NodeKind::Path(i);
            node.parent = Some(root);
            d.nodes[0].children.push(node.id);
            d.nodes.push(node);
        }
        let start = std::time::Instant::now();
        for _ in 0..10 {
            validate(&d, &Limits::DEFAULT).unwrap();
        }
        println!("{n} paths: {:.3} ms/validation", start.elapsed().as_secs_f64() * 100.0);
    }
}

#[test]
fn every_broken_clip_reference_is_legacy_only_and_reports_notice() {
    for missing in [None, Some(999999)] {
        let mut d = document();
        let group = d.clip_group(&[1, 2], 2).unwrap();
        d.nodes.iter_mut().find(|n| n.id == group).unwrap().mask_child = missing;
        let loaded = decode_model(&raw(&d, 1), None, &Limits::DEFAULT).unwrap();
        assert!(loaded.released_legacy_masks);
        assert!(loaded.notice().unwrap().contains("broken clipping masks released"));
        assert_eq!(loaded.doc.paths, d.paths);
        assert!(decode_model(&raw(&d, 2), None, &Limits::DEFAULT).is_err());
        assert!(encode_model(&d, &Limits::DEFAULT).is_err());
    }
}

#[test]
fn json_float_overflow_reports_affected_geometry_on_load() {
    let mut v = json!({"varos":3,"doc":document()});
    v["doc"]["paths"][0]["holes"][0][0]["hout"][1] = serde_json::from_str("1e39").unwrap();
    let error = decode_model(&serde_json::to_vec(&v).unwrap(), None, &Limits::DEFAULT).unwrap_err();
    assert!(
        matches!(&error, LoadError::Invalid(Invalid::NonFinite { what }) if what.contains("path 1") && what.contains("outgoing handle")),
        "{error:?}"
    );
}
