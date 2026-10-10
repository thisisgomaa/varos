//! Keys-only next-version live corner gate. Values are ignored, including null/default-looking styles.
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
                    if key == "stroke_style" {
                        found |= map.next_value::<StyleKeys>()?.0;
                    } else if key == "stack" {
                        // integration w3: an appearance stroke entry (v10) carries a full StrokeStyle;
                        // its v11 `width_profile` must be refused in a v10 file too.
                        let stack = map.next_value::<serde_json::Value>()?;
                        found |= stack.as_array().is_some_and(|items| {
                            items.iter().any(|i| i["Stroke"]["style"].get("width_profile").is_some())
                        });
                    } else {
                        found |= key == "effects";
                        map.next_value::<IgnoredAny>()?;
                    }
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
        Err(Invalid::FieldNotInFormat { field: "effects/width_profile", version }.into())
    } else {
        Ok(())
    }
}

struct StyleKeys(bool);
impl<'de> Deserialize<'de> for StyleKeys {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = StyleKeys;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("stroke style")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<StyleKeys, A::Error> {
                let mut found = false;
                while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                    found |= key == "width_profile";
                    map.next_value::<IgnoredAny>()?;
                }
                Ok(StyleKeys(found))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<StyleKeys, E> {
                Ok(StyleKeys(false))
            }
        }
        d.deserialize_any(V)
    }
}
