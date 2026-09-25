//! DFS S5-B — the `.vrs` format spine, headless: format 2 on every save, the version gate before any
//! typed decode, strict decoding, declared limits, the structural precheck (cycles, dangling and
//! duplicate ids, depth, id headroom), the v1→v2 migration, and save-side symmetry (this build never
//! writes what it would refuse to read). Pure model/IO — no Renderer, no EventLoop.

use serde_json::{json, Value};
use varos_core::file::{doc_from_blob, doc_to_blob, VRS_VERSION};
use varos_core::format::{
    decode_model, encode_model, peek_version, Invalid, LimitKind, Limits, LoadError, SaveRefused, FORMAT_VERSION,
    MIGRATION_NOTICE, MIN_READ_VERSION,
};
use varos_core::model::{Anchor, Artboard, Document, GroupRole, Guide, Node, NodeKind, Path, Xform};

// ───────────────────────────── builders ─────────────────────────────

fn anc(id: u32, x: f32, y: f32) -> Anchor {
    Anchor { id, p: [x, y], hin: None, hout: None, smooth: false }
}
/// A closed triangle path with anchor ids `id*10+1..=id*10+3`.
fn tri(id: u32, x: f32, y: f32) -> Path {
    let a = id * 10;
    Path::new(
        id,
        vec![anc(a + 1, x, y), anc(a + 2, x + 40.0, y), anc(a + 3, x + 20.0, y + 30.0)],
        true,
        Some([0.2, 0.4, 0.6, 1.0]),
        Some([0.0, 0.0, 0.0, 1.0]),
        2.0,
    )
}
fn group_node(id: u32, parent: Option<u32>, children: Vec<u32>) -> Node {
    Node {
        id,
        kind: NodeKind::Group,
        name: format!("Group {id}"),
        parent,
        children,
        hidden: false,
        locked: false,
        color: None,
        clip_exempt: false,
        xform: Xform::default(),
        role: GroupRole::Normal,
        mask_child: None,
    }
}
/// `n` triangles adopted into Layer 1 (what every real commit leaves behind).
fn doc_with(n: u32) -> Document {
    let mut d = Document::default();
    for i in 1..=n {
        d.paths.push(tri(i, 50.0 * i as f32, 10.0));
    }
    d.ids = n * 10 + 3;
    d.sync_tree();
    d
}
fn grouped() -> Document {
    let mut d = doc_with(3);
    d.group(&[1, 2]).expect("group");
    d
}
/// Two boards, a clip group (path 2 masks path 1) and a rotated top-level unit (path 3).
fn masked_rotated() -> Document {
    let mut d = doc_with(3);
    d.artboards = vec![
        Artboard { x: 0.0, y: 0.0, w: 400.0, h: 300.0, ..Default::default() },
        Artboard { x: 500.0, y: 0.0, w: 200.0, h: 200.0, name: "B".into(), ..Default::default() },
    ];
    d.guides.push(Guide { vertical: true, pos: 120.0 });
    let clip = d.clip_group(&[1, 2], 2).expect("clip group");
    assert_eq!(d.node(clip).unwrap().role, GroupRole::Clip);
    let leaf3 = d.node_of_path(3).unwrap();
    d.set_node_xform(leaf3, Xform { rot: 0.5, piv: [170.0, 20.0] });
    d.sync_tree();
    d
}
fn boardless() -> Document {
    let mut d = doc_with(2);
    d.artboards.clear();
    d
}
/// Every valid document this file builds — `every_saved_doc_reopens` runs over all of them.
fn corpus() -> Vec<(&'static str, Document)> {
    vec![
        ("default", Document::default()),
        ("three paths", doc_with(3)),
        ("grouped", grouped()),
        ("masked + rotated", masked_rotated()),
        ("boardless", boardless()),
        ("unsynced raw push", {
            let mut d = Document::default();
            d.paths.push(tri(1, 0.0, 0.0));
            d.ids = 13;
            d // no sync_tree: the save-side normalizer adopts it on the clone
        }),
        ("stale id counter", {
            let mut d = doc_with(2);
            d.ids = 0;
            d
        }),
    ]
}

fn enc(d: &Document) -> String {
    encode_model(d, &Limits::DEFAULT).unwrap_or_else(|e| panic!("encode: {e}"))
}
fn dec(s: &str) -> Result<varos_core::format::Loaded, LoadError> {
    decode_model(s.as_bytes(), None, &Limits::DEFAULT)
}
fn as_value(s: &str) -> Value {
    serde_json::from_str(s).unwrap()
}
/// The blob of `d`, re-stamped as `version`.
fn blob_as(d: &Document, version: u32) -> Value {
    let mut v = as_value(&enc(d));
    v["varos"] = json!(version);
    v
}
fn dec_value(v: &Value) -> Result<varos_core::format::Loaded, LoadError> {
    dec(&v.to_string())
}
fn invalid(r: Result<varos_core::format::Loaded, LoadError>) -> Invalid {
    match r {
        Err(LoadError::Invalid(i)) => i,
        other => panic!("expected LoadError::Invalid, got {other:?}"),
    }
}
fn save_invalid(d: &Document) -> Invalid {
    match encode_model(d, &Limits::DEFAULT) {
        Err(SaveRefused(LoadError::Invalid(i))) => i,
        other => panic!("expected SaveRefused(Invalid), got {other:?}"),
    }
}
fn tiny(f: impl FnOnce(&mut Limits)) -> Limits {
    let mut l = Limits::DEFAULT;
    f(&mut l);
    l
}

// ───────────────────────────── version stamp & gate ─────────────────────────────

#[test]
fn new_saves_write_format_2() {
    assert_eq!(FORMAT_VERSION, 2);
    assert_eq!(VRS_VERSION, FORMAT_VERSION, "the old constant is an alias");
    assert_eq!(MIN_READ_VERSION, 1);
    for (name, d) in corpus() {
        let s = enc(&d);
        assert!(s.starts_with(r#"{"varos":2,"doc":{"#), "{name}: the wrapper says format 2, got {}", &s[..20]);
        assert_eq!(doc_to_blob(&d).unwrap(), s, "{name}: doc_to_blob delegates to encode_model");
        assert_eq!(peek_version(s.as_bytes()), Ok(2));
    }
}

#[test]
fn v1_blob_migrates_and_reports_notice() {
    let d = grouped();
    let loaded = dec_value(&blob_as(&d, 1)).expect("v1 loads");
    assert_eq!(loaded.source_version, 1);
    assert!(loaded.migrated);
    assert_eq!(loaded.notice(), Some(MIGRATION_NOTICE));
    assert_eq!(loaded.notice(), Some("Opened an older file. Saving will update its format."));
    assert_eq!(loaded.doc, d, "a current-shape v1 file migrates to the same document");
    // the frozen legacy fixture migrates too (registry → tree)
    let legacy =
        std::fs::read(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/legacy_groups.vrs"))
            .unwrap();
    let l = decode_model(&legacy, None, &Limits::DEFAULT).expect("legacy v1 loads");
    assert!(l.migrated && l.doc.groups.is_empty() && l.doc.group_of.is_empty());
    assert!(enc(&l.doc).starts_with(r#"{"varos":2,"#), "saving the migrated file writes format 2");
}

#[test]
fn v2_blob_loads_without_migration() {
    for (name, d) in corpus() {
        let loaded = dec(&enc(&d)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(loaded.source_version, 2, "{name}");
        assert!(!loaded.migrated, "{name}");
        assert_eq!(loaded.notice(), None, "{name}");
    }
    let d = masked_rotated();
    assert_eq!(dec(&enc(&d)).unwrap().doc, d, "a canonical document round-trips exactly");
}

#[test]
fn newer_version_refused_before_typed_decode() {
    // `"doc": 42` would be `Malformed` if the typed decode ran first
    let e = dec(r#"{"varos":3,"doc":42}"#).unwrap_err();
    assert_eq!(e, LoadError::NewerVersion { found: 3, supported: 2 });
    let msg = e.to_string();
    assert!(msg.contains("newer") && msg.contains("file format 3") && msg.contains("up to 2"), "{msg}");
    // a newer version wins over an inconsistent container number too
    let e = decode_model(br#"{"varos":9999,"doc":{}}"#, Some(2), &Limits::DEFAULT).unwrap_err();
    assert!(matches!(e, LoadError::NewerVersion { found: 9999, .. }), "{e:?}");
}

#[test]
fn missing_zero_negative_fractional_huge_versions_refused() {
    assert_eq!(dec(r#"{"doc":{}}"#).unwrap_err(), LoadError::MissingVersion);
    for (raw, shown) in [
        ("0", "0"),
        ("-1", "-1"),
        ("1.5", "1.5"),
        ("2.0", "2.0"),
        ("4294967296", "4294967296"),
        ("18446744073709551615", "18446744073709551615"),
        (r#""2""#, r#""2""#),
        ("null", "null"),
        ("true", "true"),
        ("[2]", "[2]"),
    ] {
        let e = dec(&format!(r#"{{"varos":{raw},"doc":42}}"#)).unwrap_err();
        assert_eq!(e, LoadError::InvalidVersion(shown.into()), "version {raw}");
    }
    // not a JSON object at all
    assert_eq!(dec("not json at all").unwrap_err(), LoadError::NotAVarosFile);
    assert_eq!(dec("").unwrap_err(), LoadError::NotAVarosFile);
    assert_eq!(dec("[1,2]").unwrap_err(), LoadError::NotAVarosFile);
    // a truncated object is damaged, not foreign
    assert!(matches!(dec(r#"{"varos":2,"doc":{"#).unwrap_err(), LoadError::Malformed { .. }));
    // the message is plain English
    assert!(LoadError::MissingVersion.to_string().contains("no Varos format number"));
}

/// One edit to a JSON blob.
type Poke = Box<dyn Fn(&mut Value)>;

#[test]
fn unknown_field_fails_closed_envelope_document_node() {
    let d = masked_rotated();
    let rotated = d.nodes.iter().position(|n| !n.xform.is_identity()).expect("a rotated node");
    for version in [1, 2] {
        let base = blob_as(&d, version);
        assert!(dec_value(&base).is_ok(), "v{version}: the unmodified blob loads");
        let spots: Vec<(&str, Poke)> = vec![
            ("envelope", Box::new(|v| v["extra"] = json!(1))),
            ("document", Box::new(|v| v["doc"]["future_key"] = json!(true))),
            ("node", Box::new(|v| v["doc"]["nodes"][0]["soft_mask"] = json!({"a":1}))),
            ("path", Box::new(|v| v["doc"]["paths"][0]["gradient"] = json!(null))),
            ("anchor", Box::new(|v| v["doc"]["paths"][0]["anchors"][0]["w"] = json!(1.0))),
            ("artboard", Box::new(|v| v["doc"]["artboards"][0]["margin"] = json!(4))),
            ("snap", Box::new(|v| v["doc"]["snap"]["new_snap"] = json!(true))),
            ("units", Box::new(|v| v["doc"]["units"]["dpi"] = json!(300))),
            ("guide", Box::new(|v| v["doc"]["guides"][0]["color"] = json!(1))),
            ("xform", Box::new(move |v| v["doc"]["nodes"][rotated]["xform"]["skew"] = json!(0.1))),
            ("unknown enum variant", Box::new(|v| v["doc"]["nodes"][0]["kind"] = json!("Blob"))),
            ("unknown role", Box::new(|v| v["doc"]["nodes"][0]["role"] = json!("SoftLuma"))),
        ];
        for (spot, poke) in spots {
            let mut v = base.clone();
            poke(&mut v);
            match dec_value(&v) {
                Err(LoadError::Malformed { detail: m, .. }) => {
                    assert!(m.contains("unknown") || m.contains("variant"), "v{version} {spot}: {m}")
                }
                other => panic!("v{version} {spot}: expected Malformed, got {other:?}"),
            }
        }
    }
}

#[test]
fn v1_missing_artboards_gets_legacy_board() {
    let mut v = blob_as(&doc_with(2), 1);
    v["doc"].as_object_mut().unwrap().remove("artboards");
    let l = dec_value(&v).expect("pre-artboard v1 loads");
    assert_eq!(l.doc.artboards.len(), 1, "the legacy single page");
    assert!(!l.doc.artboards[0].clip, "legacy pages bleed (never clip)");
}

#[test]
fn explicit_empty_artboards_stays_boardless_v1_and_v2() {
    let d = boardless();
    for version in [1, 2] {
        let v = blob_as(&d, version);
        assert_eq!(v["doc"]["artboards"], json!([]));
        let l = dec_value(&v).unwrap();
        assert!(l.doc.artboards.is_empty(), "v{version}: [] stays boardless");
    }
}

#[test]
fn container_version_mismatch_refused() {
    let s = enc(&doc_with(1));
    let e = decode_model(s.as_bytes(), Some(1), &Limits::DEFAULT).unwrap_err();
    assert_eq!(e, LoadError::VersionMismatch { container: 1, model: 2 });
    assert!(e.to_string().contains("format 1") && e.to_string().contains("format 2"));
    assert!(decode_model(s.as_bytes(), Some(2), &Limits::DEFAULT).is_ok());
    let v1 = blob_as(&doc_with(1), 1).to_string();
    assert!(decode_model(v1.as_bytes(), Some(1), &Limits::DEFAULT).is_ok(), "a v1 PDF with a v1 catalog");
    assert_eq!(
        decode_model(v1.as_bytes(), Some(2), &Limits::DEFAULT).unwrap_err(),
        LoadError::VersionMismatch { container: 2, model: 1 }
    );
}

#[test]
fn json_depth_over_limit_refused() {
    // serde_json's built-in recursion limit (128) is active: a 200-deep value where a value is parsed
    let deep = format!(r#"{{"varos":{}{},"doc":{{}}}}"#, "[".repeat(200), "]".repeat(200));
    match dec(&deep) {
        Err(LoadError::Malformed { detail: m, .. }) => assert!(m.contains("recursion limit"), "{m}"),
        other => panic!("expected Malformed(recursion limit), got {other:?}"),
    }
    // just under the limit parses (and is then refused as an invalid version, not as too deep)
    let ok_depth = format!(r#"{{"varos":{}{},"doc":{{}}}}"#, "[".repeat(100), "]".repeat(100));
    assert!(matches!(dec(&ok_depth), Err(LoadError::InvalidVersion(_))));
    // a million levels of junk anywhere else neither overflows the stack nor hangs
    let n = 1_000_000;
    let junk = format!(r#"{{"varos":2,"junk":{}{},"doc":{{}}}}"#, "[".repeat(n), "]".repeat(n));
    assert!(matches!(dec(&junk), Err(LoadError::Malformed { .. })));
    let junk = format!(r#"{{"varos":2,"doc":{{"paths":{}{}}}}}"#, "[".repeat(n), "]".repeat(n));
    assert!(matches!(dec(&junk), Err(LoadError::Malformed { .. })));
}

#[test]
fn model_bytes_over_limit_refused() {
    let d = doc_with(3);
    let s = enc(&d);
    let small = tiny(|l| l.max_model_bytes = 64);
    let e = decode_model(s.as_bytes(), None, &small).unwrap_err();
    assert_eq!(e, LoadError::TooLarge { limit: LimitKind::ModelBytes, found: s.len() as u64, max: 64 });
    assert!(e.to_string().contains("editable model exceeds the 64 bytes limit"), "{e}");
    assert!(e.to_string().contains("(found: 2.") && e.to_string().contains(" KiB)."), "{e}");
    match encode_model(&d, &small) {
        Err(SaveRefused(LoadError::TooLarge { limit: LimitKind::ModelBytes, .. })) => {}
        other => panic!("save must refuse too, got {other:?}"),
    }
    let e32 = LoadError::TooLarge { limit: LimitKind::ModelBytes, found: 40 << 20, max: 32 << 20 };
    assert!(e32.to_string().contains("exceeds the 32 MiB limit"), "{e32}");
}

// ───────────────────────────── structure ─────────────────────────────

#[test]
fn node_cycle_refused_without_hang() {
    // two groups that own each other, detached from every root
    let mut d = doc_with(1);
    d.nodes.push(group_node(100, Some(101), vec![101]));
    d.nodes.push(group_node(101, Some(100), vec![100]));
    let v = blob_as_raw(&d, 2);
    assert!(matches!(invalid(dec_value(&v)), Invalid::Cycle { .. }));
    assert!(matches!(invalid(dec_value(&blob_as_raw(&d, 1))), Invalid::Cycle { .. }));
    assert!(matches!(save_invalid(&d), Invalid::Cycle { .. }), "save refuses too");
    // a node that is its own child
    let mut d = doc_with(1);
    d.nodes.push(group_node(100, Some(100), vec![100]));
    assert_eq!(save_invalid(&d), Invalid::Cycle { node: 100 });
}

/// The raw serde JSON of `d` (bypassing the save-side checks), stamped as `version` — for building
/// hostile inputs that this build's writer would refuse to produce.
fn blob_as_raw(d: &Document, version: u32) -> Value {
    json!({ "varos": version, "doc": serde_json::to_value(d).unwrap() })
}

#[test]
fn legacy_group_parent_cycle_refused_without_hang() {
    let mut v = blob_as_raw(&doc_with(1), 1);
    v["doc"]["nodes"] = json!([]);
    v["doc"]["roots"] = json!([]);
    v["doc"]["groups"] = json!([{"id":5,"name":"A","parent":6},{"id":6,"name":"B","parent":5}]);
    v["doc"]["group_of"] = json!({"1":5});
    assert!(matches!(invalid(dec_value(&v)), Invalid::Cycle { node: 5 | 6 }));
    v["doc"]["groups"] = json!([{"id":5,"name":"A","parent":5}]);
    assert_eq!(invalid(dec_value(&v)), Invalid::Cycle { node: 5 });
    // an acyclic registry with a parent that names no group is legal v1 data (hangs on the host layer)
    v["doc"]["groups"] = json!([{"id":5,"name":"A","parent":77}]);
    let l = dec_value(&v).expect("a dangling legacy parent migrates onto the host layer");
    assert!(l.migrated && l.doc.groups.is_empty());
    // registry chains count against the tree depth
    let chain: Vec<Value> = (0..70u32)
        .map(|i| json!({"id": 1000 + i, "name": "g", "parent": if i == 0 { None } else { Some(999 + i) }}))
        .collect();
    v["doc"]["groups"] = json!(chain);
    v["doc"]["group_of"] = json!({"1": 1069});
    assert!(matches!(dec_value(&v), Err(LoadError::TooLarge { limit: LimitKind::TreeDepth, .. })));
}

#[test]
fn duplicate_ids_refused() {
    let mut d = doc_with(2);
    d.paths[1].id = 1;
    assert_eq!(invalid(dec_value(&blob_as_raw(&d, 2))), Invalid::DuplicateId { kind: "path", id: 1 });
    assert_eq!(save_invalid(&d), Invalid::DuplicateId { kind: "path", id: 1 });

    let mut d = doc_with(1);
    let mut twin = group_node(1, None, vec![]);
    twin.kind = NodeKind::Layer;
    d.nodes.push(twin);
    assert_eq!(save_invalid(&d), Invalid::DuplicateId { kind: "node", id: 1 });

    let mut v = blob_as_raw(&doc_with(1), 1);
    v["doc"]["groups"] = json!([{"id":5,"name":"A","parent":null},{"id":5,"name":"B","parent":null}]);
    assert_eq!(invalid(dec_value(&v)), Invalid::DuplicateId { kind: "legacy group", id: 5 });

    // cross-kind reuse is legal: path 1, anchor 1 and Layer node 1 coexist (R10)
    let mut d = Document::default();
    d.paths.push(Path::new(1, vec![anc(1, 0.0, 0.0), anc(2, 10.0, 0.0), anc(3, 0.0, 10.0)], true, None, None, 1.0));
    d.ids = 3;
    d.sync_tree();
    assert!(d.nodes.iter().any(|n| n.id == 1) && d.paths[0].id == 1 && d.paths[0].anchors[0].id == 1);
    assert_eq!(dec(&enc(&d)).unwrap().doc, d);
}

#[test]
fn dangling_leaf_refused() {
    let mut d = doc_with(2);
    let leaf = d.node_of_path(2).unwrap();
    d.paths.retain(|p| p.id != 2); // the leaf now points at nothing
    let e = invalid(dec_value(&blob_as_raw(&d, 2)));
    assert_eq!(e, Invalid::Dangling { from: "node", id: leaf, missing: 2 });
    assert_eq!(invalid(dec_value(&blob_as_raw(&d, 1))), e, "v1 is refused too, not pruned");
    assert_eq!(save_invalid(&d), e);

    let mut d = doc_with(1);
    d.nodes[0].children.push(999);
    assert_eq!(save_invalid(&d), Invalid::Dangling { from: "node", id: 1, missing: 999 });
    let mut d = doc_with(1);
    d.roots.push(42);
    assert_eq!(save_invalid(&d), Invalid::Dangling { from: "root list", id: 42, missing: 42 });
    assert!(LoadError::Invalid(save_invalid(&d)).to_string().contains("root layer list refers to node 42"));

    // ownership: parent link disagrees with the children list / no owner / two owners
    let mut d = doc_with(1);
    d.nodes.push(group_node(100, Some(1), vec![]));
    assert!(matches!(save_invalid(&d), Invalid::BadParentage { node: 100, .. }));
    let mut d = doc_with(1);
    d.nodes.push(group_node(100, None, vec![]));
    assert!(matches!(save_invalid(&d), Invalid::BadParentage { node: 100, .. }));
    let mut d = doc_with(1);
    let leaf = d.nodes[0].children[0];
    d.nodes[0].children.push(leaf);
    assert!(matches!(save_invalid(&d), Invalid::BadParentage { .. }));
}

/// A chain of `depth` nested groups under Layer 1 with path 1's leaf at the bottom.
fn nested(depth: u32) -> Document {
    let mut d = doc_with(1);
    let leaf = d.node_of_path(1).unwrap();
    let mut parent = 1u32; // Layer 1
    d.nodes[0].children.clear();
    for i in 0..depth {
        let id = 1000 + i;
        d.nodes.push(group_node(id, Some(parent), vec![]));
        let pi = d.nodes.iter().position(|n| n.id == parent).unwrap();
        d.nodes[pi].children.push(id);
        parent = id;
    }
    let pi = d.nodes.iter().position(|n| n.id == parent).unwrap();
    d.nodes[pi].children.push(leaf);
    let li = d.nodes.iter().position(|n| n.id == leaf).unwrap();
    d.nodes[li].parent = Some(parent);
    d.ids = d.ids.max(1000 + depth);
    d
}

#[test]
fn tree_depth_over_limit_refused() {
    // layer (1) + 62 groups + leaf (1) = 64 levels: at the limit, fine
    let ok = nested(62);
    assert_eq!(dec(&enc(&ok)).unwrap().doc, ok);
    let deep = nested(63);
    match encode_model(&deep, &Limits::DEFAULT) {
        Err(SaveRefused(LoadError::TooLarge { limit: LimitKind::TreeDepth, found: 65, max: 64 })) => {}
        other => panic!("expected TreeDepth 65/64, got {other:?}"),
    }
    let e = dec_value(&blob_as_raw(&deep, 2)).unwrap_err();
    assert_eq!(e, LoadError::TooLarge { limit: LimitKind::TreeDepth, found: 65, max: 64 });
    // tiny limit
    let l = tiny(|l| l.max_tree_depth = 3);
    assert!(matches!(
        decode_model(enc(&nested(2)).as_bytes(), None, &l),
        Err(LoadError::TooLarge { limit: LimitKind::TreeDepth, found: 4, max: 3 })
    ));
}

#[test]
fn counts_over_limits_refused() {
    let d = masked_rotated(); // 3 paths, 9 anchors, 2 artboards, 1 layer + 1 group + 3 leaves
    let s = enc(&d);
    let check = |l: Limits, kind: LimitKind, found: u64, max: u64| {
        assert_eq!(
            decode_model(s.as_bytes(), None, &l).unwrap_err(),
            LoadError::TooLarge { limit: kind, found, max },
            "{kind:?}"
        );
        assert!(matches!(encode_model(&d, &l), Err(SaveRefused(LoadError::TooLarge { limit, .. })) if limit == kind));
    };
    check(tiny(|l| l.max_nodes = 4), LimitKind::Nodes, 5, 4);
    check(tiny(|l| l.max_paths = 2), LimitKind::Paths, 3, 2);
    check(tiny(|l| l.max_anchors = 8), LimitKind::Anchors, 9, 8);
    check(tiny(|l| l.max_artboards = 1), LimitKind::Artboards, 2, 1);
    // holes count as anchors
    let mut h = doc_with(1);
    h.paths[0].holes.push(vec![anc(90, 1.0, 1.0), anc(91, 2.0, 1.0)]);
    assert!(matches!(
        encode_model(&h, &tiny(|l| l.max_anchors = 4)),
        Err(SaveRefused(LoadError::TooLarge { limit: LimitKind::Anchors, found: 5, max: 4 }))
    ));
    assert!(decode_model(s.as_bytes(), None, &Limits::DEFAULT).is_ok());
}

#[test]
fn id_exhaustion_refused() {
    let mut d = doc_with(1);
    d.ids = u32::MAX;
    assert_eq!(save_invalid(&d), Invalid::IdExhausted);
    assert_eq!(invalid(dec_value(&blob_as_raw(&d, 2))), Invalid::IdExhausted);
    // an id in use at the top of the range, with a low counter
    let mut d = doc_with(1);
    d.paths[0].anchors[0].id = u32::MAX;
    assert_eq!(invalid(dec_value(&blob_as_raw(&d, 1))), Invalid::IdExhausted);
    // the migration's own allocations are counted: a legacy file with 3 paths needs 5 ids + 1 headroom
    let mut v = blob_as_raw(&doc_with(3), 1);
    v["doc"]["nodes"] = json!([]);
    v["doc"]["roots"] = json!([]);
    v["doc"]["groups"] = json!([{"id":5,"name":"A","parent":null}]);
    v["doc"]["group_of"] = json!({"1":5});
    v["doc"]["ids"] = json!(u32::MAX - 6);
    assert_eq!(
        invalid(dec_value(&v)),
        Invalid::IdExhausted,
        "no headroom left once the migration has allocated its ids"
    );
    v["doc"]["ids"] = json!(u32::MAX - 7);
    let l = dec_value(&v).expect("just enough headroom");
    assert!(l.doc.ids < u32::MAX);
    assert!(Invalid::IdExhausted.to_string().contains("id counter"));
}

#[test]
fn stale_ids_counter_raised() {
    for version in [1, 2] {
        let mut v = blob_as(&doc_with(2), version);
        v["doc"]["ids"] = json!(0);
        let l = dec_value(&v).expect("a stale counter is repaired, not refused");
        let max_used = 23; // path 2's last anchor
        assert!(l.doc.ids >= max_used, "v{version}: ids raised to {}", l.doc.ids);
        let mut doc = l.doc;
        let fresh = doc.nid();
        assert!(doc.nodes.iter().all(|n| n.id != fresh), "v{version}: the next id is really free");
    }
    // stale counter + adoption: the normalizer must not hand out a node id already in use
    let mut d = Document::default();
    d.paths.push(tri(1, 0.0, 0.0));
    d.ids = 0; // Layer node 1 exists; nid() would return 1 again
    let l = dec(&enc(&d)).unwrap();
    let mut ids: Vec<u32> = l.doc.nodes.iter().map(|n| n.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), l.doc.nodes.len(), "node ids stay unique");
}

// ───────────────────────────── masks & migration ─────────────────────────────

/// `masked_rotated` with the clip's `mask_child` pointed at a node that is NOT its direct child.
fn broken_clip() -> (Document, u32) {
    let mut d = masked_rotated();
    let clip = d.nodes.iter().find(|n| n.role == GroupRole::Clip).unwrap().id;
    let outsider = d.node_of_path(3).unwrap(); // a leaf directly under the layer
    let ci = d.nodes.iter().position(|n| n.id == clip).unwrap();
    d.nodes[ci].mask_child = Some(outsider);
    (d, clip)
}

#[test]
fn v1_invalid_clip_refused_not_demoted() {
    let (d, clip) = broken_clip();
    let want = Invalid::BadMask { group: clip, reason: "its mask shape is no longer inside it" };
    assert_eq!(invalid(dec_value(&blob_as_raw(&d, 1))), want, "v1: refused, never silently demoted");
    assert_eq!(invalid(dec_value(&blob_as_raw(&d, 2))), want, "v2: refused too");
    // a mask id that names no node at all
    let mut v = blob_as_raw(&d, 1);
    let ci = d.nodes.iter().position(|n| n.id == clip).unwrap();
    v["doc"]["nodes"][ci]["mask_child"] = json!(4242);
    let dangling = Invalid::Dangling { from: "node", id: clip, missing: 4242 };
    assert_eq!(invalid(dec_value(&v)), dangling, "refused by the structure check, before any migration");
    // review P3-1: with a tree-less path to adopt, the migration would otherwise hand out node 4242 to it
    // and the clip would silently bind to an arbitrary shape
    let mut v2 = blob_as_raw(&d, 1);
    v2["doc"]["nodes"][ci]["mask_child"] = json!(4242);
    v2["doc"]["ids"] = json!(4241);
    v2["doc"]["paths"].as_array_mut().unwrap().push(serde_json::to_value(tri(9, 0.0, 0.0)).unwrap());
    assert_eq!(invalid(dec_value(&v2)), dangling);
    let mut sd = d.clone();
    sd.nodes[ci].mask_child = Some(4242);
    assert_eq!(save_invalid(&sd), dangling, "save refuses it too");
    // save refuses it and leaves the editor's document untouched
    let before = d.clone();
    assert_eq!(save_invalid(&d), want);
    assert_eq!(d, before);
    assert!(LoadError::Invalid(want).to_string().contains(&format!("clipping group {clip} is broken")));
}

#[test]
fn v1_masks_and_xform_preserved_through_migration() {
    let d = masked_rotated();
    let l = dec_value(&blob_as(&d, 1)).expect("v1 masked+rotated loads");
    assert!(l.migrated);
    assert_eq!(l.doc, d, "roles, mask_child and xform survive the migration unchanged");
    let clip = l.doc.nodes.iter().find(|n| n.role == GroupRole::Clip).expect("still a clip");
    assert!(clip.mask_child.is_some());
    assert!(l.doc.is_mask_source(2), "the mask still shapes the clip");
    assert!(!l.doc.unit_xform(3).is_identity(), "rotation kept");
    // the migrated document saves as v2 and round-trips byte-stably
    let a = enc(&l.doc);
    let b = enc(&dec(&a).unwrap().doc);
    assert_eq!(a, b);
}

#[test]
fn sync_tree_is_noop_on_canonical_v2() {
    for (name, d) in corpus() {
        let doc = dec(&enc(&d)).unwrap().doc;
        let mut again = doc.clone();
        again.sync_tree();
        assert_eq!(again, doc, "{name}: sync_tree changes nothing on a canonical v2 document");
    }
    // v2 input the writer could not have produced is refused, not repaired
    let mut raw_push = doc_with(1);
    raw_push.paths.push(tri(2, 0.0, 0.0)); // no leaf
    assert_eq!(invalid(dec_value(&blob_as_raw(&raw_push, 2))), Invalid::NotCanonical { what: "layer tree" });
    assert!(dec_value(&blob_as_raw(&raw_push, 1)).is_ok(), "v1: adopting tree-less paths is a documented migration");
    let mut empty_group = doc_with(1);
    empty_group.nodes.push(group_node(100, Some(1), vec![]));
    empty_group.nodes[0].children.push(100);
    assert_eq!(invalid(dec_value(&blob_as_raw(&empty_group, 2))), Invalid::NotCanonical { what: "layer tree" });
    let mut legacy = blob_as_raw(&doc_with(1), 2);
    legacy["doc"]["groups"] = json!([{"id":5,"name":"A","parent":null}]);
    assert_eq!(invalid(dec_value(&legacy)), Invalid::LegacyInV2 { what: "group registry" });
    // but a stale active_layer is a preference the load may fix
    let mut v = blob_as(&doc_with(1), 2);
    v["doc"]["active_layer"] = json!(777);
    assert_eq!(dec_value(&v).unwrap().doc.active_layer, 1);
}

#[test]
fn save_refuses_non_finite_and_leaves_doc_unchanged() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut d = doc_with(2);
        d.paths[1].anchors[0].p[0] = bad;
        let before = format!("{d:?}");
        let e = encode_model(&d, &Limits::DEFAULT).expect_err("a non-finite value is never written");
        assert!(e.to_string().starts_with("This document can't be saved: "), "{e}");
        assert!(e.to_string().ends_with(". It is still open."), "{e}");
        assert_eq!(format!("{d:?}"), before, "the editor's document is untouched");
        assert!(doc_to_blob(&d).is_err(), "the string API refuses too");
    }
    // what serde would have written reads back as damaged, never as a document
    let mut d = doc_with(1);
    d.paths[0].opacity = f32::NAN;
    let raw = blob_as_raw(&d, 2).to_string();
    assert!(raw.contains(r#""opacity":null"#));
    assert!(matches!(dec(&raw), Err(LoadError::Malformed { .. })));
}

#[test]
fn every_saved_doc_reopens() {
    let mut docs = corpus();
    docs.push(("nested at the depth limit", nested(62)));
    let (_, clip) = broken_clip();
    let mut released = masked_rotated();
    released.release_clip(clip);
    docs.push(("released clip", released));
    for (name, d) in docs {
        let a = encode_model(&d, &Limits::DEFAULT).unwrap_or_else(|e| panic!("{name}: save refused: {e}"));
        let l = decode_model(a.as_bytes(), None, &Limits::DEFAULT).unwrap_or_else(|e| panic!("{name}: reopen: {e}"));
        let b = encode_model(&l.doc, &Limits::DEFAULT).unwrap();
        assert_eq!(a, b, "{name}: save → reopen → save is byte-stable");
        assert_eq!(doc_from_blob(&a).unwrap(), l.doc, "{name}: doc_from_blob delegates to decode_model");
    }
}

#[test]
fn limits_default_values_match_documented_numbers() {
    let l = Limits::DEFAULT;
    assert_eq!(l, Limits::default());
    assert_eq!(l.max_file_bytes, 256 * 1024 * 1024, "256 MiB file");
    assert_eq!(l.max_model_bytes, 32 * 1024 * 1024, "32 MiB model");
    assert_eq!(l.max_pdf_objects, 100_000);
    assert_eq!(l.max_decoded_stream_bytes, 64 * 1024 * 1024, "64 MiB decoded streams");
    assert_eq!(l.max_pdf_depth, 64);
    assert_eq!(l.max_nodes, 40_000, "lowered from 100,000 after measurement (R4)");
    assert_eq!(l.max_paths, 40_000, "lowered from 100,000 after measurement (R4)");
    assert_eq!(l.max_anchors, 1_000_000);
    assert_eq!(l.max_artboards, 1_000);
    assert_eq!(l.max_tree_depth, 64);
}

#[test]
fn load_vrs_reads_bounded_and_keeps_its_string_api() {
    let p = std::env::temp_dir().join(format!("varos-format-v2-{}-big.vrs", std::process::id()));
    std::fs::write(&p, enc(&doc_with(2))).unwrap();
    assert!(varos_core::file::load_vrs(&p).is_ok());
    let small = tiny(|l| l.max_file_bytes = 16);
    match varos_core::format::read_bounded(&p, &small) {
        Err(LoadError::TooLarge { limit: LimitKind::FileBytes, max: 16, .. }) => {}
        other => panic!("expected FileBytes refusal, got {other:?}"),
    }
    let _ = std::fs::remove_file(&p);
    let missing = varos_core::file::load_vrs(&p).unwrap_err();
    assert!(missing.contains("could not be read"), "{missing}");
}

/// R4: time to load and save at the caps (S3 pays the save cost on every snapshot). Run with
/// `cargo test -p varos-core --release --test format_v2 -- --ignored --nocapture decode_timing_at_caps`.
#[test]
#[ignore]
fn decode_timing_at_caps() {
    let cap = Limits::DEFAULT.max_nodes as u32 - 1; // + Layer 1 = exactly max_nodes nodes
    for n in [10_000u32, 25_000, cap] {
        let mut d = Document::default();
        for i in 1..=n {
            d.paths.push(Path::new(i, vec![anc(n + 3 * i, 0.0, 0.0)], false, None, None, 1.0));
        }
        d.ids = 4 * n + 3;
        d.sync_tree();
        let t = std::time::Instant::now();
        let s = enc(&d);
        let save = t.elapsed();
        let t = std::time::Instant::now();
        let l = dec(&s).expect("loads at the cap");
        let load = t.elapsed();
        assert_eq!(l.doc.paths.len(), n as usize);
        println!(
            "{n:>6} paths / {:>6} nodes, {:>5.1} MiB: encode {:>8.1} ms, decode {:>8.1} ms",
            l.doc.nodes.len(),
            s.len() as f64 / (1024.0 * 1024.0),
            save.as_secs_f64() * 1e3,
            load.as_secs_f64() * 1e3
        );
    }
}

/// Review P2-1: user-facing text never leaks parser internals (type names, field lists, serde words).
#[test]
fn user_messages_hide_parser_internals() {
    let base = blob_as(&masked_rotated(), 2);
    let mut inputs: Vec<String> = vec![
        r#"{"varos":2,"doc":42}"#.into(),
        r#"{"varos":2,"doc":{"#.into(),
        format!(r#"{{"varos":{}{},"doc":{{}}}}"#, "[".repeat(200), "]".repeat(200)),
    ];
    let pokes: [fn(&mut Value); 6] = [
        |v| v["extra"] = json!(1),
        |v| v["doc"]["bogus"] = json!(1),
        |v| v["doc"]["ids"] = json!("x"),
        |v| v["doc"]["paths"][0]["stroke_width"] = json!(null),
        |v| v["doc"]["nodes"][0]["kind"] = json!("Blob"),
        |v| {
            v["doc"].as_object_mut().unwrap().remove("paths");
        },
    ];
    for poke in pokes {
        let mut v = base.clone();
        poke(&mut v);
        inputs.push(v.to_string());
    }
    let mut messages: Vec<String> = inputs.iter().map(|s| dec(s).expect_err("refused").to_string()).collect();
    let mut nan = doc_with(1);
    nan.paths[0].anchors[0].p[0] = f32::NAN;
    messages.push(encode_model(&nan, &Limits::DEFAULT).unwrap_err().to_string());
    for m in &messages {
        for bad in ["struct", "expected", "serde", "field", "u32", "f32", "`", "stroke_width", "move_art_with_ab"] {
            assert!(!m.contains(bad), "user message leaks {bad:?}: {m}");
        }
    }
    assert!(messages[0].ends_with("(line 1, column 19)."), "{}", messages[0]);
    assert_eq!(
        messages.last().unwrap(),
        "This document can't be saved: a number in this document has a value that is not a finite number. It is still open."
    );
    // the raw parser text is still kept for logs
    match dec(r#"{"varos":2,"doc":42}"#) {
        Err(LoadError::Malformed { line: 1, column: 19, detail }) => assert!(detail.contains("expected")),
        other => panic!("{other:?}"),
    }
}
