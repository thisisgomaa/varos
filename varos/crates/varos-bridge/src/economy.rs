//! Opt-in wire normalization. No document IDs are allocated here; original DTOs stay intact.
use crate::{dto::*, MAX_OPS, MAX_TARGETS};
use serde::Deserialize;
use serde_json::{json, Map, Value};

const DEFAULT_KEYS: &[&str] = &["parent", "fill", "stroke", "stroke_width", "radius", "opacity"];
/// API 1.2 edit inherits the API 1.1 wire economy without changing older tools.
pub(crate) fn edit_enabled(api: &str) -> bool {
    matches!(api, "1.1" | "1.2")
}
fn invalid(reason: impl Into<String>) -> Error {
    Error::new("invalid_argument", reason)
}
#[derive(Clone)]
pub(crate) struct Leaf {
    pub op: Operation,
    pub index: usize,
    pub location: Vec<String>,
}
impl Leaf {
    pub fn error(&self, mut e: Error) -> Error {
        e.op_index = Some(self.index);
        e.location = self.location.clone();
        e
    }
}
fn numeric_nulls(m: &Map<String, Value>) -> Result<(), Error> {
    for key in ["parent", "radius", "stroke_width", "opacity"] {
        if m.get(key).is_some_and(Value::is_null) {
            return Err(invalid(format!("{key} cannot be null")));
        }
    }
    Ok(())
}
fn tuple(v: &Value) -> Result<Value, Error> {
    let Some(a) = v.as_array() else {
        return Ok(v.clone());
    };
    let kind = a.first().and_then(Value::as_str).ok_or_else(|| invalid("tuple kind required"))?;
    let (mut m, mut pos) = match kind {
        "rect" | "ellipse" if a.len() >= 2 => (json!({"verb":"add_shape","kind":kind,"bounds":a[1]}), 2),
        "path" if a.len() >= 3 => {
            let points = a[1].as_array().ok_or_else(|| invalid("path points must be an array"))?;
            if !(2..=1000).contains(&points.len()) {
                return Err(invalid("path needs 2..1000 points"));
            }
            (
                json!({"verb":"add_path","anchors":points.iter().map(|p| json!({"p":p})).collect::<Vec<_>>(),"closed":a[2]}),
                3,
            )
        }
        _ => return Err(invalid("invalid creation tuple")),
    };
    if let Some(fill) = a.get(pos).filter(|v| !v.is_object()) {
        if !fill.is_null() && !fill.is_string() {
            return Err(invalid("tuple fill must be color or null"));
        }
        m["fill"] = fill.clone();
        pos += 1;
        if kind == "rect" {
            if let Some(radius) = a.get(pos).filter(|v| v.is_number()) {
                m["radius"] = radius.clone();
                pos += 1;
            }
        }
    }
    if let Some(options) = a.get(pos) {
        let options = options.as_object().ok_or_else(|| invalid("tuple trailing options must be an object"))?;
        for (key, value) in options {
            let allowed = match kind {
                "rect" => {
                    &["parent", "insert", "local", "name", "fill", "stroke", "stroke_width", "opacity", "radius"][..]
                }
                "ellipse" => &["parent", "insert", "local", "name", "fill", "stroke", "stroke_width", "opacity"][..],
                _ => &["parent", "local", "name", "fill", "stroke", "stroke_width", "opacity"][..],
            };
            if !allowed.contains(&key.as_str()) || m.get(key).is_some() {
                return Err(invalid(format!("unknown or duplicate tuple option {key}")));
            }
            m[key] = value.clone();
        }
        pos += 1;
    }
    if pos != a.len() {
        return Err(invalid("extra tuple slots"));
    }
    Ok(m)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Repeat {
    verb: String,
    ops: Vec<Value>,
    count: usize,
    dx: f32,
    dy: f32,
}

/// First normalize and bound all work, before accessing the allocator/staging editor.
pub(crate) fn expand(edit: &Edit) -> Result<Vec<Leaf>, Error> {
    let economy = edit_enabled(&edit.api);
    if !economy && (edit.defaults.is_some() || edit.receipt.is_some()) {
        return Err(invalid("defaults and receipt require API 1.1 or 1.2"));
    }
    if edit.receipt.as_deref().is_some_and(|r| r != "ids") {
        return Err(invalid("receipt must be ids"));
    }
    if edit.ops.is_empty() || edit.ops.len() > MAX_OPS {
        return Err(Error::new("limit_exceeded", "edit needs 1..100 operations"));
    }
    let defaults = match &edit.defaults {
        None => Map::new(),
        Some(v) => {
            let m = v.as_object().ok_or_else(|| invalid("defaults must be an object"))?;
            if m.keys().any(|k| !DEFAULT_KEYS.contains(&k.as_str())) {
                return Err(invalid("unknown defaults field"));
            }
            numeric_nulls(m)?;
            // Validate even unused defaults, using the existing typed/color validators.
            let mut probe = m.clone();
            probe.insert("verb".into(), json!("add_shape"));
            probe.insert("kind".into(), json!("rect"));
            probe.insert("bounds".into(), json!([0, 0, 1, 1]));
            let _: Operation = serde_json::from_value(Value::Object(probe)).map_err(|e| invalid(e.to_string()))?;
            for key in ["fill", "stroke"] {
                if let Some(v) = m.get(key) {
                    crate::service::paint(&serde_json::from_value(v.clone()).map_err(|e| invalid(e.to_string()))?)?;
                }
            }
            for key in ["radius", "stroke_width", "opacity"] {
                if let Some(v) = m.get(key) {
                    let n = v.as_f64().ok_or_else(|| invalid("numeric default required"))?;
                    if !n.is_finite() || !(n as f32).is_finite() || n < 0.0 || (key == "opacity" && n > 1.0) {
                        return Err(invalid(format!("invalid default {key}")));
                    }
                }
            }
            if let Some(p) = m.get("parent") {
                let p = p.as_str().ok_or_else(|| invalid("parent must be node:N"))?;
                let n = p
                    .strip_prefix("node:")
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|n| *n > 0)
                    .ok_or_else(|| invalid("parent must be node:N"))?;
                if p != format!("node:{n}") {
                    return Err(invalid("parent must be canonical node:N"));
                }
            }
            m.clone()
        }
    };
    let mut out = Vec::new();
    let mut targets = 0;
    for (index, op) in edit.ops.iter().enumerate() {
        walk(
            op,
            &edit.api,
            economy,
            edit.api == "1.2",
            &defaults,
            0,
            [0.0, 0.0],
            "",
            index,
            &[],
            &mut out,
            &mut targets,
        )?;
    }
    if !economy && targets > MAX_TARGETS {
        return Err(Error::new("limit_exceeded", "edit exceeds 1000 explicit targets"));
    }
    if edit.api != "1.2" && out.iter().any(|l| matches!(l.op, Operation::DocumentSetup { .. })) {
        return Err(Error::new("unsupported", "document_setup requires API 1.2"));
    }
    Ok(out)
}
#[allow(clippy::too_many_arguments)]
fn walk(
    v: &Value,
    api: &str,
    economy: bool,
    construction: bool,
    defaults: &Map<String, Value>,
    depth: usize,
    delta: [f32; 2],
    suffix: &str,
    index: usize,
    location: &[String],
    out: &mut Vec<Leaf>,
    targets: &mut usize,
) -> Result<(), Error> {
    let result = (|| {
        if v.get("verb").and_then(Value::as_str) == Some("repeat") {
            if !economy {
                return Err(Error::new("unsupported", "repeat requires API 1.1 or 1.2"));
            }
            let r: Repeat = serde_json::from_value(v.clone()).map_err(|e| invalid(e.to_string()))?;
            debug_assert_eq!(r.verb, "repeat");
            if depth >= 4
                || !(1..=100).contains(&r.count)
                || r.ops.is_empty()
                || r.ops.len() > 100
                || !r.dx.is_finite()
                || !r.dy.is_finite()
            {
                return Err(invalid("repeat needs count 1..100, 1..100 children, finite delta and nesting <=4"));
            }
            for i in 0..r.count {
                let offset = [delta[0] + i as f32 * r.dx, delta[1] + i as f32 * r.dy];
                for (j, child) in r.ops.iter().enumerate() {
                    let mut path = location.to_vec();
                    path.push(format!("instance:{i}"));
                    path.push(format!("op:{j}"));
                    walk(
                        child,
                        api,
                        economy,
                        construction,
                        defaults,
                        depth + 1,
                        offset,
                        &format!("{suffix}_{i}"),
                        index,
                        &path,
                        out,
                        targets,
                    )?;
                }
            }
            return Ok(());
        }
        if !economy && v.is_array() {
            return Err(invalid("tuples require API 1.1 or 1.2"));
        }
        let mut normalized = tuple(v)?;
        let m = normalized.as_object_mut().ok_or_else(|| invalid("operation must be an object or creation tuple"))?;
        if api == "1.2" && m.contains_key("op") {
            if m.contains_key("verb") {
                return Err(invalid("use op or verb, not both"));
            }
            if let Some(verb) = m.remove("op") {
                m.insert("verb".into(), verb);
            }
        }
        let creation = matches!(m.get("verb").and_then(Value::as_str), Some("add_shape" | "add_path"));
        if depth > 0 && !creation {
            return Err(invalid("repeat children must be creation operations or repeats"));
        }
        if economy && creation {
            numeric_nulls(m)?;
            for (k, val) in defaults {
                if k == "radius" && m.get("kind").and_then(Value::as_str) != Some("rect") {
                    continue;
                }
                m.entry(k.clone()).or_insert_with(|| val.clone());
            }
        }
        let verb = m.get("verb").and_then(Value::as_str).ok_or_else(|| invalid("verb required"))?;
        if api != "1.2" && (verb == "set_stroke_style" || m.contains_key("stroke_style")) {
            return Err(Error::new("unsupported", "stroke_style requires explicit API 1.2"));
        }
        // ---- Lane D: opt-in drawing verb expansion ----
        if !crate::EDIT_VERBS.contains(&verb)
            && !(construction && crate::drawing::VERBS.contains(&verb))
            && !(api == "1.2" && verb == "set_stroke_style")
            && !(construction && (crate::CONSTRUCTION_VERBS.contains(&verb) || verb == "trace_rgba"))
            && !(api == "1.2"
                && ["transform", "magic_wand", "eyedropper", "isolation", "layers", "tool_options"].contains(&verb))
            && ![
                "document_setup",
                "clip",
                "release_clip",
                "view",
                "object",
                "distribute_mode",
                "distribute_spacing",
                "anchor_type",
                "insert_anchor",
                "delete_anchor",
            ]
            .contains(&verb)
        {
            return Err(Error::new("unsupported", "edit verb is not enabled in this slice"));
        }
        let mut op: Operation = serde_json::from_value(normalized).map_err(|e| invalid(e.to_string()))?;
        match &mut op {
            Operation::AddShape { bounds, local, .. } => {
                bounds[0] += delta[0];
                bounds[1] += delta[1];
                if let Some(l) = local {
                    l.push_str(suffix);
                }
            }
            Operation::AddPath { anchors, local, .. } => {
                for a in anchors {
                    for p in std::iter::once(&mut a.p).chain(a.hin.iter_mut()).chain(a.hout.iter_mut()) {
                        p[0] += delta[0];
                        p[1] += delta[1];
                    }
                }
                if let Some(l) = local {
                    l.push_str(suffix);
                }
            }
            _ => {}
        }
        *targets += op.ids().len() + usize::from(creation && economy);
        if economy && (out.len() >= MAX_OPS || *targets > MAX_TARGETS) {
            return Err(Error::new("limit_exceeded", "expanded edit exceeds 100 operations or 1000 targets"));
        }
        out.push(Leaf { op, index, location: location.to_vec() });
        Ok(())
    })();
    result.map_err(|mut e: Error| {
        if e.op_index.is_none() {
            e.op_index = Some(index);
            e.location = location.to_vec();
        }
        e
    })
}

/// Count leaf work using symbolic identities. This catches layer/group expansion and earlier
/// creations/groups without allocating a path, node or anchor in the staging editor.
pub(crate) fn preflight_targets(doc: &varos_core::model::Document, leaves: &[Leaf]) -> Result<(), Error> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut units: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for p in &doc.paths {
        let id = format!("path:{}", p.id);
        units.insert(id.clone(), BTreeSet::from([id]));
    }
    for n in &doc.nodes {
        units.insert(format!("node:{}", n.id), doc.node_paths(n.id).iter().map(|p| format!("path:{p}")).collect());
    }
    let mut local_names = BTreeSet::new();
    let mut work = 0;
    for (i, leaf) in leaves.iter().enumerate() {
        let result = (|| {
            let (local, creation, parent) = match &leaf.op {
                Operation::AddShape { local, parent, .. } | Operation::AddPath { local, parent, .. } => {
                    (local, true, parent.clone())
                }
                Operation::Group { local, .. }
                | Operation::AddArtboard { local, .. }
                | Operation::DuplicateArtboard { local, .. } => (local, false, None),
                _ => (&None, false, None),
            };
            if let Some(name) = local {
                crate::design::local_name(name)?;
                if !local_names.insert(name.clone()) {
                    return Err(invalid("duplicate request-local name"));
                }
            }
            let members: BTreeSet<String> =
                leaf.op.ids().iter().filter_map(|id| units.get(id)).flatten().cloned().collect();
            work += members.len() + usize::from(creation);
            if work > MAX_TARGETS {
                return Err(Error::new("limit_exceeded", "batch expanded targets exceed 1000"));
            }
            if creation {
                let id = format!("generated:{i}");
                if let Some(parent) = units.get_mut(&parent.unwrap_or_else(|| format!("node:{}", doc.active_layer))) {
                    parent.insert(id.clone());
                }
                if let Some(local) = local {
                    units.insert(local.clone(), BTreeSet::from([id]));
                }
            } else if let Some(local) = local {
                units.insert(local.clone(), members.clone());
            }
            if matches!(leaf.op, Operation::Delete { .. }) {
                for values in units.values_mut() {
                    values.retain(|id| !members.contains(id));
                }
            }
            Ok(())
        })();
        result.map_err(|e| leaf.error(e))?;
    }
    Ok(())
}
