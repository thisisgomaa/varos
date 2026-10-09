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
mod gradient_keys;
pub mod limits;
pub mod migrate;
mod stroke_keys;
// ---- Lane C ----
// ---- Lane B w3-effects ----
mod effect_keys;
// ---- end Lane B w3-effects ----
mod corner_keys;
pub mod structure;
pub mod validate;

pub use error::{Invalid, LoadError, SaveRefused};
pub use limits::{LimitKind, Limits};
pub use migrate::{
    migrate_v11_to_v12, migrate_v12_to_v13, migrate_v1_to_v2, migrate_v2_to_v3, migrate_v3_to_v4, migrate_v4_to_v5,
    migrate_v5_to_v6, migrate_v6_to_v7, migrate_v7_to_v8, migrate_v8_to_v9, migrate_v9_to_v10, readable_versions,
};
pub use structure::check_structure;
pub use validate::validate;

use crate::model::Document;
use serde::{Deserialize, Deserializer, Serialize};
use std::io::Read;
use std::path::Path;

/// The format this build writes (the wrapper key `varos` and the PDF catalog's `/VAROS_SchemaVersion`).
/// 3 (2026-10-04): board metadata — `doc.name`, `doc.description`, `doc.tags` (ADR-0008 amendment).
/// 4 (2026-10-07): stable artboard ids — `doc.artboards[].id` (ADR-0008 amendment, Bridge slice 3).
/// 5 (2026-10-08): stroke styles — `doc.paths[].stroke_style`.
/// 6 (2026-10-09, wave 2): raster images — `doc.images`, `doc.assets`, `doc.raster_effects_ppi`,
/// `NodeKind::Image` (w2-images).
pub const IMAGE_VERSION: u32 = 6;
/// 7 (2026-10-09, wave 2): gradient paints, swatch references and `doc.swatches` (w2-gradients).
pub const GRADIENT_VERSION: u32 = 7;
/// 8 (2026-10-09): editable text — `doc.text_boxes`, `NodeKind::Text` (wave-2 text lane), stamped
/// first so files saved before wave-2 stage 2 stay valid.
pub const TEXT_FORMAT_VERSION: u32 = 8;
/// The last format before editable text (the declared version a pre-text file may carry).
pub const PRE_TEXT_FORMAT_VERSION: u32 = GRADIENT_VERSION;
/// 9 (2026-10-09, wave 2): Live Corners — `doc.paths[].corners` (w2-export-paths), plus the
/// app lane's container-level embedded preview in the same bump.
pub const CORNERS_VERSION: u32 = 9;
/// Lane F: the optional PDF-catalog Quick Look preview (`/VAROS_Preview` + `/VAROS_PreviewVersion`)
/// is container-only (no model key, no reader impact on the JSON body); folded into the v9 bump.
pub const PREVIEW_FORMAT_VERSION: u32 = CORNERS_VERSION;
// ---- Lane A ----
/// 10 (2026-10-10, wave 3): appearance — `doc.paths[].stack`, `doc.nodes[].look`, `MaskAlpha` role.
pub const APPEARANCE_VERSION: u32 = 10;
mod appearance_keys;
// ---- Lane B w3-effects ----
/// 11 (2026-10-10, wave 3): live vector effects — `doc.paths[].effects`, `stroke_style.width_profile`.
pub const EFFECTS_VERSION: u32 = 11;
// ---- w3-cmyk ----
/// 12 (2026-10-10, wave 3): explicit colour sources — managed paints, `colour_mode`, `output_profile`.
pub const COLOUR_VERSION: u32 = 12;
// ---- Lane E: Phase 11 ----
/// 13 (2026-10-10, wave 3): live objects — `NodeKind::Live{Blend,Repeat,Envelope}`.
pub const LIVE_VERSION: u32 = 13;
// ---- Lane H ----
/// 14 (2026-10-10, wave 3): typography — `doc.typography` (named styles, OpenType features, threaded
/// area text / type on a path; text P5–P8).
pub const TYPOGRAPHY_VERSION: u32 = 14;
pub const FORMAT_VERSION: u32 = TYPOGRAPHY_VERSION;
/// The first format whose writer emits the board metadata keys (`name`, `description`, `tags`).
pub const BOARD_META_VERSION: u32 = 3;
/// The first format whose writer emits a stable `id` on every artboard.
pub const ARTBOARD_ID_VERSION: u32 = 4;
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
    // ---- w2-images ----
    pub blobs: crate::images::BlobStore,
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
        if !self.blobs.load_notes.is_empty() {
            Some("Some image originals could not fit in the decoded cache; proxy previews are shown. Original streams are retained.")
        } else if self.released_legacy_masks {
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
    // ---- Lane A ----
    if version < APPEARANCE_VERSION {
        appearance_keys::refuse(json, version)?;
    }
    if version < ARTBOARD_ID_VERSION {
        refuse_newer_keys(json, version)?; // keys only, before any typed decode
    }
    if version < GRADIENT_VERSION {
        gradient_keys::refuse(json, version)?;
    }
    // ---- Lane C ----
    // ---- Lane B w3-effects ----
    if version < EFFECTS_VERSION {
        effect_keys::refuse(json, version)?;
    }
    // ---- end Lane B w3-effects ----
    if version < CORNERS_VERSION {
        corner_keys::refuse(json, version)?;
    }
    if version < 5 {
        stroke_keys::refuse(json, version)?;
    }
    // ---- w2-images ----
    crate::images::refuse_older_keys(json, version)?;
    crate::text_format::refuse_legacy_text(json, version)?;
    // ---- w3-cmyk ----
    crate::colour_format::refuse(json, version)?;
    // ---- Lane E: Phase 11 ----
    crate::live::refuse_older_keys(json, version)?;
    // ---- Lane H ----
    crate::typography_format::refuse(json, version)?;
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
        // exactly the strictness it had when v2 was current (artboard ids exist from format 4 only).
        if version >= ARTBOARD_ID_VERSION {
            validate(&doc, limits)?;
        } else {
            validate::before_artboard_ids(&doc)?;
        }
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
    Ok(Loaded { doc, blobs: Default::default(), source_version: version, migrated, released_legacy_masks })
}

/// A file that claims a format older than the one that introduced a key must not carry it: no writer
/// of that format emitted it, so its presence is an unknown field (ADR-0008: unknown fields fail
/// closed), not data to keep — whatever its value (`"name": 42`, `null`, a nested object). The keys:
/// the board metadata (`doc.name`, `doc.description`, `doc.tags`, format 3) and the artboard id
/// (`doc.artboards[].id`, format 4). The typed decode would default them (or fail on a wrong type as
/// merely "damaged"), so this keys-only scan runs right after the version gate, BEFORE any typed
/// decode, for older files only. It reads the top-level keys of `doc` and the keys of each artboard
/// object, skipping every value (serde's `IgnoredAny`, inside serde_json's 128-level depth limit, after
/// the model byte cap) — nothing is built. A `doc` (or artboard list) of the wrong shape is left for
/// the typed decode to refuse.
fn refuse_newer_keys(json: &[u8], version: u32) -> Result<(), LoadError> {
    use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
    use std::fmt;

    /// Skip any JSON value; for a map, report whether it has an `id` key (an artboard's id).
    #[derive(Default)]
    struct HasId(bool);
    /// The first board key found, and whether any artboard carries an `id`.
    #[derive(Default)]
    struct Found {
        board: Option<&'static str>,
        artboard_id: bool,
    }

    macro_rules! skip_scalars {
        ($t:ty, $v:expr) => {
            fn visit_bool<E>(self, _: bool) -> Result<$t, E> {
                Ok($v)
            }
            fn visit_i64<E>(self, _: i64) -> Result<$t, E> {
                Ok($v)
            }
            fn visit_u64<E>(self, _: u64) -> Result<$t, E> {
                Ok($v)
            }
            fn visit_f64<E>(self, _: f64) -> Result<$t, E> {
                Ok($v)
            }
            fn visit_str<E>(self, _: &str) -> Result<$t, E> {
                Ok($v)
            }
            fn visit_unit<E>(self) -> Result<$t, E> {
                Ok($v)
            }
        };
    }

    impl<'de> Deserialize<'de> for HasId {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = HasId;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("any JSON value")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<HasId, A::Error> {
                    let mut found = false;
                    while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                        map.next_value::<IgnoredAny>()?;
                        found |= key == "id";
                    }
                    Ok(HasId(found))
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<HasId, A::Error> {
                    while seq.next_element::<IgnoredAny>()?.is_some() {}
                    Ok(HasId(false))
                }
                skip_scalars!(HasId, HasId(false));
            }
            d.deserialize_any(V)
        }
    }
    /// The artboard list: does any artboard object carry an `id`?
    struct AnyArtboardId(bool);
    impl<'de> Deserialize<'de> for AnyArtboardId {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = AnyArtboardId;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("any JSON value")
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<AnyArtboardId, A::Error> {
                    let mut found = false;
                    while let Some(HasId(id)) = seq.next_element::<HasId>()? {
                        found |= id;
                    }
                    Ok(AnyArtboardId(found))
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<AnyArtboardId, A::Error> {
                    while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                    Ok(AnyArtboardId(false))
                }
                skip_scalars!(AnyArtboardId, AnyArtboardId(false));
            }
            d.deserialize_any(V)
        }
    }
    impl<'de> Deserialize<'de> for Found {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            struct V;
            impl<'de> Visitor<'de> for V {
                type Value = Found;
                fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    f.write_str("any JSON value")
                }
                fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Found, A::Error> {
                    let mut found = Found::default();
                    while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
                        if key == "artboards" {
                            let AnyArtboardId(id) = map.next_value()?;
                            found.artboard_id |= id;
                            continue;
                        }
                        map.next_value::<IgnoredAny>()?;
                        if found.board.is_none() {
                            found.board = ["name", "description", "tags"].into_iter().find(|k| *k == key);
                        }
                    }
                    Ok(found)
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Found, A::Error> {
                    while seq.next_element::<IgnoredAny>()?.is_some() {}
                    Ok(Found::default())
                }
                skip_scalars!(Found, Found::default());
            }
            d.deserialize_any(V)
        }
    }
    #[derive(Deserialize)]
    struct Head {
        #[serde(default)]
        doc: Found,
    } // no deny_unknown_fields: `varos` and anything else are skipped here; the typed decode is strict
    let head: Head = serde_json::from_slice(json).map_err(|e| LoadError::malformed(&e))?;
    match head.doc {
        Found { board: Some(field), .. } if version < BOARD_META_VERSION => {
            Err(Invalid::FieldNotInFormat { field, version }.into())
        }
        Found { artboard_id: true, .. } if version < ARTBOARD_ID_VERSION => {
            Err(Invalid::FieldNotInFormat { field: "artboard id", version }.into())
        }
        _ => Ok(()),
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
    let mut norm = migrate::normalize(doc.clone()).map_err(SaveRefused)?;
    // format 4: a page built in memory without an id gets one, and a stale active index is clamped —
    // the same forms `Editor::commit` keeps, so this only matters for documents assembled in code.
    norm.assign_artboard_ids();
    migrate::clamp_active(&mut norm);
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

// ---- Lane B w3-effects ----
pub use migrate::migrate_v10_to_v11;
// ---- end Lane B w3-effects ----
