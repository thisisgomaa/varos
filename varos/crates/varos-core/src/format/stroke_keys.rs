//! Keys-only v5 style gate. Values are ignored, including null/default-looking styles.
use super::{Invalid, LoadError};
use serde::{
    de::{IgnoredAny, MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
struct Paths(bool);
impl<'de> Deserialize<'de> for Paths {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Paths;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("paths")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Paths, A::Error> {
                let mut found = false;
                while let Some(PathKeys(p)) = seq.next_element()? {
                    found |= p;
                }
                Ok(Paths(found))
            }
        }
        d.deserialize_seq(V)
    }
}
struct PathKeys(bool);
impl<'de> Deserialize<'de> for PathKeys {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = PathKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("path keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<PathKeys, A::Error> {
                let mut found = false;
                while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                    found |= key == "stroke_style";
                    map.next_value::<IgnoredAny>()?;
                }
                Ok(PathKeys(found))
            }
        }
        d.deserialize_map(V)
    }
}
pub(super) fn refuse(json: &[u8], version: u32) -> Result<(), LoadError> {
    #[derive(Deserialize)]
    struct Doc {
        #[serde(default)]
        paths: Option<Paths>,
    }
    #[derive(Deserialize)]
    struct Head {
        doc: Doc,
    }
    let head: Head = serde_json::from_slice(json).map_err(|e| LoadError::malformed(&e))?;
    if head.doc.paths.is_some_and(|p| p.0) {
        Err(Invalid::FieldNotInFormat { field: "stroke_style", version }.into())
    } else {
        Ok(())
    }
}
