//! Lane F: recursive duplicate refusal keeps opaque additive JSON lossless.
use serde::{
    de::{Error, MapAccess, SeqAccess, Visitor},
    Deserialize,
};
use serde_json::{Map, Value};
pub(super) struct UniqueValue(pub Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut source: M) -> Result<Self::Value, M::Error> {
                let mut map = Map::new();
                while let Some((key, value)) = source.next_entry::<String, UniqueValue>()? {
                    if map.insert(key, value.0).is_some() {
                        return Err(M::Error::custom("Duplicate settings key"));
                    }
                }
                Ok(UniqueValue(Value::Object(map)))
            }
            fn visit_seq<S: SeqAccess<'de>>(self, mut source: S) -> Result<Self::Value, S::Error> {
                let mut values = vec![];
                while let Some(value) = source.next_element::<UniqueValue>()? {
                    values.push(value.0);
                }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_bool<E: Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Bool(v)))
            }
            fn visit_i64<E: Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::from(v)))
            }
            fn visit_u64<E: Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::from(v)))
            }
            fn visit_f64<E: Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| UniqueValue(Value::Number(n)))
                    .ok_or_else(|| E::custom("Nonfinite settings number"))
            }
            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(v.into())))
            }
            fn visit_string<E: Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::String(v)))
            }
            fn visit_unit<E: Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(Value::Null))
            }
        }
        d.deserialize_any(V)
    }
}
