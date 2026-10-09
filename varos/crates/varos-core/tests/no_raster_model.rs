//! ADR-0014: image bytes live outside Document and its undo clone stacks.
//! Walk field TYPES, including absent Option/Vec payloads, rather than sample JSON values.
use std::collections::{HashMap, HashSet};
use syn::{GenericArgument, Item, PathArguments, Type};
fn walk(ty: &Type, defs: &HashMap<String, Vec<Type>>, seen: &mut HashSet<String>) {
    match ty {
        Type::Path(p) => {
            for seg in &p.path.segments {
                if ["crate", "gradient", "swatches", "appearance", "effects", "width_profile", "stroke"]
                    .iter()
                    .any(|v| seg.ident == *v)
                {
                    continue;
                }
                let name = seg.ident.to_string();
                assert!(
                    !["u8", "i8", "Bytes", "ByteBuf", "Value"].contains(&name.as_str()),
                    "raster-capable model type: {name}"
                );
                if let PathArguments::AngleBracketed(a) = &seg.arguments {
                    for arg in &a.args {
                        if let GenericArgument::Type(t) = arg {
                            walk(t, defs, seen);
                        }
                    }
                }
                if let Some(fields) = defs.get(&name) {
                    if seen.insert(name.clone()) {
                        for t in fields {
                            walk(t, defs, seen);
                        }
                    }
                } else {
                    assert!(
                        ["Vec", "HashMap", "Option", "String", "bool", "u32", "usize", "f32", "f64"]
                            .contains(&name.as_str()),
                        "classify new model type before allowing it: {name}"
                    );
                }
            }
        }
        Type::Array(a) => walk(&a.elem, defs, seen),
        Type::Tuple(t) => {
            for t in &t.elems {
                walk(t, defs, seen);
            }
        }
        _ => panic!("classify new model type form before allowing it"),
    }
}
#[test]
fn serde_document_node_path_have_no_byte_blobs() {
    let mut defs = HashMap::new();
    for source in [
        include_str!("../src/model.rs"),
        include_str!("../src/text.rs"),
        include_str!("../src/images/metadata.rs"),
        include_str!("../src/geom.rs"),
        include_str!("../src/units.rs"),
        include_str!("../src/stroke.rs"),
        include_str!("../src/gradient.rs"),
        include_str!("../src/swatches.rs"),
        include_str!("../src/live_corners.rs"),
        // ---- Lane A: walk stack/look payloads, never whitelist their contents ----
        include_str!("../src/appearance.rs"),
        include_str!("../src/effects.rs"),
        include_str!("../src/width_profile.rs"),
    ] {
        for item in syn::parse_file(source).unwrap().items {
            match item {
                Item::Struct(s) => {
                    defs.insert(s.ident.to_string(), s.fields.iter().map(|f| f.ty.clone()).collect());
                }
                Item::Enum(e) => {
                    defs.insert(
                        e.ident.to_string(),
                        e.variants.iter().flat_map(|v| v.fields.iter().map(|f| f.ty.clone())).collect(),
                    );
                }
                Item::Macro(m) if m.mac.path.is_ident("wire_enum") => {
                    defs.insert(m.mac.tokens.to_string().split(',').next().unwrap().trim().to_owned(), vec![]);
                }
                Item::Type(t) => {
                    defs.insert(t.ident.to_string(), vec![*t.ty]);
                }
                _ => {}
            }
        }
    }
    for name in ["Document", "Node", "Path"] {
        walk(&syn::parse_str::<Type>(name).unwrap(), &defs, &mut HashSet::new());
    }
}
