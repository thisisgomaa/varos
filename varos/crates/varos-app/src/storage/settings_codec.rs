//! Lane F: bounded, duplicate-refusing settings envelope and pure v1 migration.
use super::{preferences::SPECS, settings::Settings};
use serde::{
    de::{Error, MapAccess, Visitor},
    Deserialize,
};
use serde_json::{Map, Value};
pub const MAX_BYTES: usize = 65536;
struct Unique(Map<String, Value>);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a settings object without duplicate keys")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut m: M) -> Result<Unique, M::Error> {
                let mut map = Map::new();
                while let Some((key, value)) = m.next_entry::<String, Value>()? {
                    if map.insert(key, value).is_some() {
                        return Err(M::Error::custom("Duplicate settings key"));
                    }
                }
                Ok(Unique(map))
            }
        }
        d.deserialize_map(V)
    }
}
pub fn envelope(bytes: &[u8]) -> Result<Map<String, Value>, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Settings exceed 64 KiB".into());
    }
    serde_json::from_slice::<Unique>(bytes).map(|u| u.0).map_err(|e| e.to_string())
}
/// Version 1 migrates in memory only; opaque additive fields survive repeated migration.
pub fn migrate_v1(mut map: Map<String, Value>) -> Result<Map<String, Value>, String> {
    let version = map.get("version").and_then(Value::as_u64).ok_or("Settings require an integer version")?;
    if version == 1 {
        if !map.get("recovery_enabled").is_some_and(Value::is_boolean) {
            return Err("v1 recovery_enabled must be boolean".into());
        }
        map.insert("version".into(), Value::from(2));
        for spec in SPECS {
            map.entry(spec.key.to_string()).or_insert_with(|| spec.value(&Settings::default()));
        }
    }
    Ok(map)
}
pub fn decode(bytes: &[u8]) -> Result<(Settings, Vec<String>), String> {
    let map = migrate_v1(envelope(bytes)?)?;
    if map.get("version").and_then(Value::as_u64) != Some(2) {
        return Err("Unsupported settings version; writes locked".into());
    }
    let mut s = Settings::default();
    let mut invalid = vec![];
    for spec in SPECS {
        if let Some(value) = map.get(spec.key) {
            if let Err(e) = spec.set(&mut s, value.clone()) {
                invalid.push(format!("{}: {e}", spec.key));
            }
        }
    }
    if let Some(v) = map.get("paste_remembers_layers") {
        match v.as_bool() {
            Some(v) => s.paste_remembers_layers = v,
            None => invalid.push("paste_remembers_layers: expected boolean".into()),
        }
    }
    Ok((s, invalid))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixtures_migrate_preserve_refuse() {
        for flag in [true, false] {
            let bytes = format!(r#"{{"version":1,"recovery_enabled":{flag},"unknown":{{"x":42}}}}"#);
            let map = migrate_v1(envelope(bytes.as_bytes()).unwrap()).unwrap();
            assert_eq!(migrate_v1(map.clone()).unwrap(), map);
            assert_eq!(map["unknown"]["x"], 42);
            assert_eq!(decode(bytes.as_bytes()).unwrap().0.recovery_enabled, flag);
        }
        assert!(decode(br#"{"version":2,"version":2}"#).is_err());
        assert!(decode(br#"{"version":"2"}"#).is_err());
        assert!(decode(br#"{"version":1,"recovery_enabled":0}"#).is_err());
        let (s, invalid) =
            decode(br#"{"version":2,"keyboard_increment_pt":"bad","history_depth":4,"default_units":"mm"}"#).unwrap();
        assert_eq!(invalid.len(), 2);
        assert_eq!(s.preferences.default_units, super::super::preferences::Units::Mm);
    }
}
/// Explicit Apply/repair: compare the source bytes, preserve damaged evidence before replacement,
/// and never overwrite a future version without an explicit Reset followed by Apply.
pub fn write_explicit(
    fs: &dyn super::durable::FsPort,
    path: &std::path::Path,
    settings: &Settings,
    expected: Option<&[u8]>,
    reset: bool,
) -> Result<(super::durable::WriteOutcome, Vec<u8>), String> {
    use std::io::Write;
    let source = match fs.read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if source.as_deref() != expected {
        return Err("Settings changed on disk; reopen the draft".into());
    }
    super::preferences::validate(settings)?;
    let mut map = Map::new();
    if let Some(bytes) = source.as_deref() {
        let parsed = envelope(bytes);
        let decoded = decode(bytes);
        let damaged = match &decoded {
            Err(_) => true,
            Ok((_, invalid)) => !invalid.is_empty(),
        };
        if damaged {
            if !reset {
                return Err(
                    "Settings require repair. Reset to Defaults, then Apply to preserve and repair the file".into()
                );
            }
            let backup = path.with_file_name(format!("settings.preserved-{}.json", super::checksum::new_nonce()));
            let mut file = fs.create_new(&backup).map_err(|e| format!("Couldn't preserve settings: {e}"))?;
            file.write_all(bytes)
                .and_then(|()| file.sync_all())
                .map_err(|e| format!("Couldn't preserve settings: {e}"))?;
            drop(file);
            fs.sync_dir(path.parent().ok_or("Settings folder unavailable")?)
                .map_err(|e| format!("Couldn't confirm preserved settings: {e}"))?;
        }
        if let Ok(existing) = parsed {
            if matches!(existing.get("version").and_then(Value::as_u64), Some(1 | 2)) {
                map = existing;
            }
        }
    }
    let owned = serde_json::to_value(settings).map_err(|e| e.to_string())?;
    if let Some(owned) = owned.as_object() {
        map.extend(owned.clone());
    }
    map.insert("version".into(), Value::from(2));
    let bytes = serde_json::to_vec_pretty(&map).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err("Settings exceed 64 KiB".into());
    }
    // Recheck after preservation as well: a stale repair cannot replace a new external file.
    let actual = match fs.read(path) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if actual.as_deref() != expected {
        return Err("Settings changed while preserving evidence".into());
    }
    let outcome =
        super::durable::write_replace(fs, path, &bytes, &super::checksum::new_nonce()).map_err(|e| e.reason())?;
    Ok((outcome, bytes))
}
#[cfg(test)]
mod writer_tests {
    use super::*;
    use crate::storage::{
        durable::{Fault, FaultFs, RealFs, Step, WriteOutcome},
        testdir::TestDir,
    };
    #[test]
    fn source_compare_future_lock_and_preserved_reset() {
        let d = TestDir::new("preferences-repair");
        let path = d.join("settings.json");
        let future = br#"{"version":99,"future":42}"#;
        std::fs::write(&path, future).unwrap();
        assert!(write_explicit(&RealFs, &path, &Settings::default(), Some(future), false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), future);
        assert!(write_explicit(&RealFs, &path, &Settings::default(), Some(b"stale"), true).is_err());
        assert!(matches!(
            write_explicit(&RealFs, &path, &Settings::default(), Some(future), true).unwrap().0,
            WriteOutcome::Durable
        ));
        assert!(d.names().iter().any(|name| name.starts_with("settings.preserved-")));
    }
    #[test]
    fn write_failure_and_unconfirmed_do_not_claim_success() {
        let d = TestDir::new("preferences-faults");
        let path = d.join("settings.json");
        let bytes = br#"{"version":2,"recovery_enabled":false}"#;
        std::fs::write(&path, bytes).unwrap();
        let fs = FaultFs::new(vec![Fault::at(Step::Write { after: 0 })]);
        assert!(write_explicit(&fs, &path, &Settings::default(), Some(bytes), false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let fs = FaultFs::new(vec![Fault::at(Step::SyncDir)]);
        assert!(matches!(
            write_explicit(&fs, &path, &Settings::default(), Some(bytes), false).unwrap().0,
            WriteOutcome::ReplacedUnconfirmed(_)
        ));
    }
}
