//! Lane F: bounded, duplicate-refusing settings envelope and pure v1 migration.
use super::{preferences::SPECS, settings::Settings};
use serde::{
    de::{Error, MapAccess, Visitor},
    Deserialize,
};
use serde_json::{Map, Value};
pub const MAX_BYTES: usize = 65536;
#[path = "settings_json.rs"]
mod settings_json;
use settings_json::UniqueValue;
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
                while let Some((key, value)) = m.next_entry::<String, UniqueValue>()? {
                    if map.insert(key, value.0).is_some() {
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
        assert!(decode(br#"{"version":2,"opaque":[{"x":1,"x":2}]}"#).is_err());
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
    let source = match fs.read_limited(path, MAX_BYTES) {
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
            let unsupported_version = parsed
                .as_ref()
                .ok()
                .and_then(|m| m.get("version"))
                .and_then(Value::as_u64)
                .is_some_and(|v| v != 1 && v != 2);
            if unsupported_version && !reset {
                return Err("Unsupported settings version; writes locked. Reset to Defaults, then Apply to preserve and repair the file".into());
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
    let actual = match fs.read_limited(path, MAX_BYTES) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e.to_string()),
    };
    if actual.as_deref() != expected {
        return Err("Settings changed while preserving evidence".into());
    }
    let outcome = super::durable::write_replace_if_unchanged(
        fs,
        path,
        &bytes,
        &super::checksum::new_nonce(),
        expected,
        MAX_BYTES,
    )
    .map_err(|e| e.reason())?;
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

#[cfg(test)]
mod frozen_fixtures {
    use super::*;
    use crate::storage::{
        durable::{Fault, FaultFs, RealFs, Step},
        testdir::TestDir,
    };
    #[test]
    fn frozen_migration_and_refusal_inputs() {
        let full = include_bytes!("../../fixtures/settings-v2/full.json");
        assert_eq!(decode(full).unwrap().0, Settings::default());
        let old = include_bytes!("../../fixtures/settings-v2/v1-off.json");
        let map = migrate_v1(envelope(old).unwrap()).unwrap();
        assert_eq!(map["future_key"]["x"], 42);
        assert_eq!(map["recovery_enabled"], false);
        assert_eq!(migrate_v1(map.clone()).unwrap(), map);
        let minimal = decode(include_bytes!("../../fixtures/settings-v2/minimal.json")).unwrap().0;
        assert!(!minimal.autosave_enabled && !minimal.recovery_enabled);
        assert_eq!(minimal.autosave_interval_seconds, 30);
        assert_eq!(minimal.preferences.history_depth, 200);
        assert!(decode(include_bytes!("../../fixtures/settings-v2/duplicate.json")).is_err());
        assert!(decode(include_bytes!("../../fixtures/settings-v2/future.json")).is_err());
        let (invalid, reasons) = decode(include_bytes!("../../fixtures/settings-v2/invalid-v2.json")).unwrap();
        assert_eq!(reasons.len(), 2);
        assert_eq!(invalid.preferences.language.requested(), "ar");
        assert_eq!(invalid.preferences.canvas_colour.to_string(), "#AB12EF");
    }
    #[test]
    fn damaged_evidence_existing_bad_and_failed_preservation_survive() {
        let dir = TestDir::new("settings-preserve");
        let path = dir.join("settings.json");
        let bytes = include_bytes!("../../fixtures/settings-v2/invalid-v2.json");
        std::fs::write(&path, bytes).unwrap();
        std::fs::write(dir.join("settings.json.bad"), b"older evidence").unwrap();
        let fs = FaultFs::new(vec![Fault::at(Step::Write { after: 0 })]);
        assert!(write_explicit(&fs, &path, &Settings::default(), Some(bytes), false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        write_explicit(&RealFs, &path, &Settings::default(), Some(bytes), false).unwrap();
        assert_eq!(std::fs::read(dir.join("settings.json.bad")).unwrap(), b"older evidence");
        assert!(dir.names().iter().filter(|n| n.starts_with("settings.preserved-")).any(|n| std::fs::read(
            dir.join(n)
        )
        .unwrap()
            == bytes));
    }
    #[test]
    fn each_prepublication_fault_keeps_effective_source() {
        for step in [Step::Create, Step::Write { after: 0 }, Step::Sync, Step::Rename] {
            let dir = TestDir::new("settings-fault");
            let path = dir.join("settings.json");
            let bytes = include_bytes!("../../fixtures/settings-v2/full.json");
            std::fs::write(&path, bytes).unwrap();
            let fs = FaultFs::new(vec![Fault::at(step)]);
            assert!(write_explicit(&fs, &path, &Settings::default(), Some(bytes), false).is_err());
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }
    }
}
