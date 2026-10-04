//! The `.vrs` format spine (ADR-0008, `docs/reference/VRS_FORMAT.md`): the format number, the
//! version gate, declared limits, typed errors, the structural precheck, migrations and the save-side
//! checks. Pure: bytes in, `Document` out (and back). The PDF container lives in `varos-pdf`.
//!
//! Load pipeline (`decode_model`): model size cap → version gate (header-only parse, before any typed
//! decode) → newer-format keys refused in older files (keys-only scan) → strict typed decode
//! (`deny_unknown_fields`; serde_json's 128-level depth limit) → `check_structure` → `validate` → canonical check (format 2+) →
//! migration (older formats) → `validate`.
//! Save (`encode_model`) runs `check_structure` → the same normalizer on a CLONE → `validate` → size
//! cap, so this build never writes a file it would refuse to read. The caller's document is never
//! mutated, and a refusal never touches the file on disk.

pub mod error;
pub mod limits;
pub mod migrate;
pub mod structure;
pub mod validate;

pub use error::{Invalid, LoadError, SaveRefused};
pub use limits::{LimitKind, Limits};
pub use migrate::{migrate_v1_to_v2, migrate_v2_to_v3};
pub use structure::check_structure;
pub use validate::validate;

use crate::model::Document;
use serde::{Deserialize, Deserializer, Serialize};
use std::io::Read;
use std::path::Path;

/// The format this build writes (the wrapper key `varos` and the PDF catalog's `/VAROS_SchemaVersion`).
/// 3 (2026-10-04): board metadata — `doc.name`, `doc.description`, `doc.tags` (ADR-0008 amendment).
pub const FORMAT_VERSION: u32 = 3;
/// The first format whose writer emits the board metadata keys (`name`, `description`, `tags`).
pub const BOARD_META_VERSION: u32 = 3;
/// The oldest format this build reads (older ones are migrated up in memory).
pub const MIN_READ_VERSION: u32 = 1;
/// Shown after opening a file that was migrated from an older format.
pub const MIGRATION_NOTICE: &str = "Opened an older file. Saving will update its format.";
/// The notice for a v1 file whose broken clipping masks were released in memory. Unlike a plain
/// migration this is a CONTENT change, so the app opens such a tab dirty (A4) and Save writes the current format.
pub const RELEASED_MASKS_NOTICE: &str = "Opened an older file with broken clipping masks released. All remaining artwork was kept. The original file has not been changed; saving will update it.";

/// The on-disk envelope `{"varos": N, "doc": {…}}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VrsFile {
    #[allow(dead_code)] // read by `peek_version`; kept here so the envelope is decoded strictly
    varos: u32,
    doc: Document,
}
/// The same envelope for writing, without cloning the document again.
#[derive(Serialize)]
struct VrsFileRef<'a> {
    varos: u32,
    doc: &'a Document,
}

/// A successfully opened model.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// The document, in the current format's canonical form.
    pub doc: Document,
    /// The format number the file was written in.
    pub source_version: u32,
    /// True when an older format was migrated up in memory (the file on disk is untouched).
    pub migrated: bool,
    /// Broken v1 clips released in memory; the caller must surface notice().
    pub released_legacy_masks: bool,
}
impl Loaded {
    /// The notice to show after opening, if any.
    pub fn notice(&self) -> Option<&'static str> {
        if self.released_legacy_masks {
            Some(RELEASED_MASKS_NOTICE)
        } else {
            self.migrated.then_some(MIGRATION_NOTICE)
        }
    }
}

/// Read only the format number, without decoding the document. Missing → `MissingVersion`; zero,
/// negative, fractional, too large or not a number → `InvalidVersion`; newer than this build →
/// `NewerVersion`. Input that is not a JSON object at all → `NotAVarosFile`.
pub fn peek_version(json: &[u8]) -> Result<u32, LoadError> {
    #[derive(Deserialize)]
    struct Head {
        // `Some(Value::Null)` for an explicit `null`, `None` only when the key is absent.
        #[serde(default, deserialize_with = "present")]
        varos: Option<serde_json::Value>,
    } // no deny_unknown_fields: the rest of the file is skipped (serde_json's iterative skip)
    fn present<'de, D: Deserializer<'de>>(d: D) -> Result<Option<serde_json::Value>, D::Error> {
        serde_json::Value::deserialize(d).map(Some)
    }

    if json.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
        return Err(LoadError::NotAVarosFile);
    }
    let head: Head = serde_json::from_slice(json).map_err(|e| LoadError::malformed(&e))?;
    let value = head.varos.ok_or(LoadError::MissingVersion)?;
    let version = match value.as_u64() {
        Some(0) | None => return Err(LoadError::InvalidVersion(value.to_string())),
        Some(v) => u32::try_from(v).map_err(|_| LoadError::InvalidVersion(value.to_string()))?,
    };
    if version > FORMAT_VERSION {
        return Err(LoadError::NewerVersion { found: version, supported: FORMAT_VERSION });
    }
    Ok(version)
}

/// Decode a model blob (the raw-JSON `.vrs` body, or the JSON embedded in the PDF container).
/// `container_version` is the PDF catalog's `/VAROS_SchemaVersion` when there is one; it must match
/// the model's own number. The input bytes are never modified.
pub fn decode_model(json: &[u8], container_version: Option<u32>, limits: &Limits) -> Result<Loaded, LoadError> {
    if json.len() > limits.max_model_bytes {
        return Err(LoadError::TooLarge {
            limit: LimitKind::ModelBytes,
            found: json.len() as u64,
            max: limits.max_model_bytes as u64,
        });
    }
    let version = peek_version(json)?;
    if let Some(container) = container_version {
        if container != version {
            return Err(LoadError::VersionMismatch { container, model: version });
        }
    }
    if version < BOARD_META_VERSION {
        refuse_newer_keys(json, version)?; // keys only, before any typed decode
    }
    let file: VrsFile = serde_json::from_slice(json).map_err(|e| LoadError::malformed(&e))?;
    let mut doc = file.doc;
    let released_legacy_masks = version == 1 && migrate::release_broken_clips(&mut doc);
    check_structure(&doc, limits)?;
    validate::authored(&doc)?;
    let migrated = version < FORMAT_VERSION;
    let doc = if version == 1 {
        // v1 is the one non-canonical era: its documented normalizations ARE the v1→v2 migration.
        let doc = migrate::migrate(doc, version, FORMAT_VERSION, limits)?;
        check_structure(&doc, limits)?; // what migration produced must be saveable as-is
        validate(&doc, limits)?;
        doc
    } else {
        // v2 and later were written canonical by Varos: check that BEFORE migrating, so a v2 file gets
        // exactly the strictness it had when v2 was current.
        validate(&doc, limits)?;
        let doc = canonical(doc)?;
        if migrated {
            let doc = migrate::migrate(doc, version, FORMAT_VERSION, limits)?;
            check_structure(&doc, limits)?;
            validate(&doc, limits)?;
            doc
        } else {
            doc
        }
    };
    Ok(Loaded { doc, source_version: version, migrated, released_legacy_masks })
}

/// A file that claims a format older than [`BOARD_META_VERSION`] must not carry the board keys: no
/// writer of that format emitted them, so their presence is an unknown field (ADR-0008: unknown fields
/// fail closed), not data to keep — whatever its value (`"name": 42`, `null`, a nested object). The
/// typed decode would default them (or fail on a wrong type as merely "damaged"), so this keys-only
/// scan runs right after the version gate, BEFORE any typed decode, for older files only. It reads
/// the top-level keys of `doc` and skips every value (serde's `IgnoredAny`, inside serde_json's
/// 128-level depth limit, after the model byte cap) — nothing is built. A `doc` that is not an object
/// is left for the typed decode to refuse.
fn refuse_newer_keys(json: &[u8], version: u32) -> Result<(), LoadError> {
    use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
    use std::fmt;

    /// The first board key found in `doc`, if any.
    #[derive(Default)]
    struct DocKeys(Option<&'static str>);
    impl<'de> Deserialize<'de> for DocKeys {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = DocKeys;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("any JSON value")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<DocKeys, A::Error> {
                    let mut found = None;
                    while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                        map.next_value::<IgnoredAny>()?;
                        if found.is_none() {
                            found = ["name", "description", "tags"].into_iter().find(|k| *k == key);
                        }
                    }
                    Ok(DocKeys(found))
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<DocKeys, A::Error> {
                    while seq.next_element::<IgnoredAny>()?.is_some() {}
                    Ok(DocKeys(None))
                }
                fn visit_bool<E>(self, _: bool) -> Result<DocKeys, E> {
                    Ok(DocKeys(None))
                }
                fn visit_i64<E>(self, _: i64) -> Result<DocKeys, E> {
                    Ok(DocKeys(None))
                }
                fn visit_u64<E>(self, _: u64) -> Result<DocKeys, E> {
                    Ok(DocKeys(None))
                }
                fn visit_f64<E>(self, _: f64) -> Result<DocKeys, E> {
                    Ok(DocKeys(None))
                }
                fn visit_str<E>(self, _: &str) -> Result<DocKeys, E> {
                    Ok(DocKeys(None))
                }
                fn visit_unit<E>(self) -> Result<DocKeys, E> {
                    Ok(DocKeys(None))
                }
            }
            d.deserialize_any(V)
        }
    }
    #[derive(Deserialize)]
    struct Head {
        #[serde(default)]
        doc: DocKeys,
    } // no deny_unknown_fields: `varos` and anything else are skipped here; the typed decode is strict
    let head: Head = serde_json::from_slice(json).map_err(|e| LoadError::malformed(&e))?;
    match head.doc.0 {
        Some(field) => Err(Invalid::FieldNotInFormat { field, version }.into()),
        None => Ok(()),
    }
}

/// Current-format input must already be in the form this build's writer produces: the normalizer may
/// only re-order path storage, fix `active_layer` and raise a stale id counter. Anything else means the
/// file was not written by Varos (or was edited), and is refused rather than silently repaired.
fn canonical(doc: Document) -> Result<Document, LoadError> {
    if !doc.groups.is_empty() {
        return Err(Invalid::LegacyInV2 { what: "group registry" }.into());
    }
    if !doc.group_of.is_empty() {
        return Err(Invalid::LegacyInV2 { what: "group membership map" }.into());
    }
    let nodes = doc.nodes.clone();
    let roots = doc.roots.clone();
    let mut pids: Vec<u32> = doc.paths.iter().map(|p| p.id).collect();
    let canon = migrate::normalize(doc)?; // a clip demotion is refused there as BadMask
    if canon.roots != roots {
        return Err(Invalid::NotCanonical { what: "root layer list" }.into());
    }
    if canon.nodes != nodes {
        return Err(Invalid::NotCanonical { what: "layer tree" }.into());
    }
    let mut after: Vec<u32> = canon.paths.iter().map(|p| p.id).collect();
    pids.sort_unstable();
    after.sort_unstable();
    if after != pids {
        return Err(Invalid::NotCanonical { what: "path list" }.into());
    }
    Ok(canon)
}

/// Serialize a document as the current format. Runs `check_structure`, normalizes a CLONE (a clip
/// the normalizer would demote is refused), re-checks the structure and runs `validate` on that clone,
/// enforces the model size cap, and checks serde can read the bytes back as a final backstop.
/// Authored values are also checked before normalization, with object-specific diagnostics. `doc` is
/// never mutated.
pub fn encode_model(doc: &Document, limits: &Limits) -> Result<String, SaveRefused> {
    check_structure(doc, limits).map_err(SaveRefused)?;
    validate::authored(doc).map_err(|e| SaveRefused(e.into()))?;
    let norm = migrate::normalize(doc.clone()).map_err(SaveRefused)?;
    check_structure(&norm, limits).map_err(SaveRefused)?; // adoption may add nodes
    validate(&norm, limits).map_err(|i| SaveRefused(i.into()))?;
    let out = serde_json::to_string(&VrsFileRef { varos: FORMAT_VERSION, doc: &norm })
        .map_err(|e| SaveRefused(LoadError::malformed(&e)))?;
    if out.len() > limits.max_model_bytes {
        return Err(SaveRefused(LoadError::TooLarge {
            limit: LimitKind::ModelBytes,
            found: out.len() as u64,
            max: limits.max_model_bytes as u64,
        }));
    }
    // Backstop for anything serde writes but cannot read back (a non-finite float serializes as
    // `null`): the saved bytes must at least decode as a typed model. `validate` names such values
    // precisely; this only guarantees nothing unreadable ever reaches the disk.
    serde_json::from_str::<VrsFile>(&out)
        .map_err(|_| SaveRefused(Invalid::NonFinite { what: "a number in this document".into() }.into()))?;
    Ok(out)
}

/// Read a whole file, refusing it before reading when it is over `max_file_bytes`, and reading at most
/// `max_file_bytes + 1` bytes even if the file grows meanwhile.
pub fn read_bounded(path: &Path, limits: &Limits) -> Result<Vec<u8>, LoadError> {
    let too_large = |found: u64| LoadError::TooLarge { limit: LimitKind::FileBytes, found, max: limits.max_file_bytes };
    let file = std::fs::File::open(path).map_err(|e| LoadError::Io(e.to_string()))?;
    let len = file.metadata().map_err(|e| LoadError::Io(e.to_string()))?.len();
    if len > limits.max_file_bytes {
        return Err(too_large(len));
    }
    let mut buf = Vec::with_capacity(len as usize);
    file.take(limits.max_file_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| LoadError::Io(e.to_string()))?;
    if buf.len() as u64 > limits.max_file_bytes {
        return Err(too_large(buf.len() as u64));
    }
    Ok(buf)
}
