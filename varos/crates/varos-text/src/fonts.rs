use sha2::{Digest, Sha256};
use std::sync::Arc;

/// Index into an immutable FontSet; meaningful only with that snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FaceId(pub usize);

/// ISO 15924 script tag, independent of the shaping backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Script([u8; 4]);
impl std::str::FromStr for Script {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 4 || !s.bytes().all(|b| b.is_ascii_alphabetic()) {
            return Err("invalid script");
        }
        Ok(Self(s.as_bytes().try_into().unwrap()))
    }
}
impl Script {
    pub(crate) fn cosmic(self) -> cosmic_text::harfrust::Script {
        std::str::from_utf8(&self.0).unwrap().parse().unwrap()
    }
}
#[derive(Clone, Debug)]
pub struct FontFace {
    pub family: &'static str,
    pub weight: u16,
    pub bytes: Arc<[u8]>,
    pub content_hash: [u8; 32],
}
impl FontFace {
    pub fn new(family: &'static str, weight: u16, bytes: Arc<[u8]>) -> Result<Self, &'static str> {
        crate::Engine::validate_font(&bytes)?;
        if family.is_empty() || ![300, 400, 500, 600].contains(&weight) {
            return Err("invalid font metadata");
        }
        let content_hash = Sha256::digest(&bytes).into();
        Ok(Self { family, weight, bytes, content_hash })
    }
}
/// Per-script ordered candidates, applied to whole joining segments.
#[derive(Clone, Debug, Default)]
pub struct FallbackPolicy {
    pub common: Vec<FaceId>,
    pub scripts: Vec<([u8; 4], Vec<FaceId>)>,
}
#[derive(Clone, Debug)]
pub struct FontSet {
    faces: Vec<FontFace>,
    fallback: FallbackPolicy,
    snapshot_hash: [u8; 32],
}
impl FontSet {
    pub fn new(faces: Vec<FontFace>, fallback: FallbackPolicy) -> Result<Self, &'static str> {
        let mut set = Self { faces, fallback, snapshot_hash: [0; 32] };
        set.validate()?;
        let mut hash = Sha256::new();
        for face in &set.faces {
            hash.update((face.family.len() as u64).to_le_bytes());
            hash.update(face.family.as_bytes());
            hash.update(face.weight.to_le_bytes());
            hash.update(face.content_hash);
        }
        hash.update((set.fallback.common.len() as u64).to_le_bytes());
        for id in &set.fallback.common {
            hash.update((id.0 as u64).to_le_bytes());
        }
        for (script, ids) in &set.fallback.scripts {
            hash.update(script);
            hash.update((ids.len() as u64).to_le_bytes());
            for id in ids {
                hash.update((id.0 as u64).to_le_bytes());
            }
        }
        set.snapshot_hash = hash.finalize().into();
        Ok(set)
    }
    pub fn snapshot_hash(&self) -> [u8; 32] {
        self.snapshot_hash
    }
    pub fn faces(&self) -> &[FontFace] {
        &self.faces
    }
    pub fn face(&self, id: FaceId) -> Option<&FontFace> {
        self.faces.get(id.0)
    }
    pub(crate) fn fallback_for(&self, script: Option<[u8; 4]>) -> &[FaceId] {
        self.fallback
            .scripts
            .iter()
            .find(|(tag, _)| Some(*tag) == script)
            .map_or(&self.fallback.common, |(_, faces)| faces)
    }
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        if self.faces.is_empty() || self.faces.len() > 256 {
            return Err("invalid font count");
        }
        for (i, face) in self.faces.iter().enumerate() {
            if self.faces[..i].iter().any(|f| f.family == face.family && f.weight == face.weight) {
                return Err("duplicate family and weight");
            }
            if face.family.is_empty() || ![300, 400, 500, 600].contains(&face.weight) {
                return Err("invalid font metadata");
            }
            crate::Engine::validate_font(&face.bytes)?;
            if Sha256::digest(&face.bytes)[..] != face.content_hash {
                return Err("font hash mismatch");
            }
        }
        if self
            .fallback
            .scripts
            .iter()
            .any(|(tag, _)| !tag[0].is_ascii_uppercase() || !tag[1..].iter().all(|b| b.is_ascii_lowercase()))
        {
            return Err("invalid fallback script");
        }
        if self
            .fallback
            .common
            .iter()
            .chain(self.fallback.scripts.iter().flat_map(|(_, ids)| ids))
            .any(|id| self.face(*id).is_none())
        {
            return Err("invalid fallback face");
        }
        Ok(())
    }
    pub(crate) fn cosmic_fallback(&self) -> ByteFallback {
        ByteFallback {
            common: self.fallback.common.iter().map(|id| self.faces[id.0].family).collect(),
            scripts: self
                .fallback
                .scripts
                .iter()
                .map(|(tag, ids)| {
                    (
                        std::str::from_utf8(tag).unwrap_or("").to_owned(),
                        ids.iter().map(|id| self.faces[id.0].family).collect(),
                    )
                })
                .collect(),
        }
    }
}
pub(crate) struct ByteFallback {
    common: Vec<&'static str>,
    scripts: Vec<(String, Vec<&'static str>)>,
}
impl cosmic_text::Fallback for ByteFallback {
    fn common_fallback(&self) -> &[&'static str] {
        &self.common
    }
    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }
    fn script_fallback(&self, script: unicode_script::Script, _: &str) -> &[&'static str] {
        self.scripts.iter().find(|(tag, _)| tag == script.short_name()).map_or(&[], |(_, families)| families)
    }
}
