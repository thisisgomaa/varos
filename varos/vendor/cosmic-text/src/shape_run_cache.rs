#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};
use core::ops::Range;

use crate::{AttrsOwned, HashMap, ShapeGlyph};

/// Key for caching shape runs.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ShapeRunKey {
    pub rtl: bool,
    pub pre_context: String,
    pub post_context: String,
    pub text: String,
    pub default_attrs: AttrsOwned,
    pub attrs_spans: Vec<(Range<usize>, AttrsOwned)>,
}

/// A helper structure for caching shape runs.
#[derive(Clone, Default)]
pub struct ShapeRunCache {
    age: u64,
    bytes: usize,
    cache: HashMap<ShapeRunKey, (u64, Vec<ShapeGlyph>)>,
}

impl ShapeRunCache {
    /// Estimated retained payload bytes; hash table allocator overhead excluded.
    pub fn payload_bytes(&self) -> usize {
        self.bytes
    }

    /// Get cache item, updating age if found
    pub fn get(&mut self, key: &ShapeRunKey) -> Option<&Vec<ShapeGlyph>> {
        self.cache.get_mut(key).map(|(age, glyphs)| {
            *age = self.age;
            &*glyphs
        })
    }

    /// Insert cache item with current age
    pub fn insert(&mut self, key: ShapeRunKey, glyphs: Vec<ShapeGlyph>) {
        // Conservative payload budget plus an entry ceiling for table overhead.
        let size = entry_bytes(&key, &glyphs);
        if size > 8 * 1024 * 1024 {
            return;
        }
        if self.cache.len() >= 4096 || self.bytes + size > 8 * 1024 * 1024 {
            self.cache.clear();
            self.bytes = 0;
        }
        if let Some((old_key, (_, old_glyphs))) = self.cache.remove_entry(&key) {
            self.bytes -= entry_bytes(&old_key, &old_glyphs);
        }
        self.bytes += size;
        self.cache.insert(key, (self.age, glyphs));
    }

    /// Remove anything in the cache with an age older than `keep_ages`
    pub fn trim(&mut self, keep_ages: u64) {
        self.cache
            .retain(|_key, (age, _glyphs)| *age + keep_ages >= self.age);
        self.bytes = self.cache.iter().map(|(k, (_, g))| entry_bytes(k, g)).sum();
        // Increase age
        self.age += 1;
    }
}

impl core::fmt::Debug for ShapeRunCache {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("ShapeRunCache").finish()
    }
}

fn entry_bytes(key: &ShapeRunKey, glyphs: &Vec<ShapeGlyph>) -> usize {
    fn attrs_bytes(a: &AttrsOwned) -> usize {
        let family = match &a.family_owned {
            crate::FamilyOwned::Name(s) => s.len(),
            _ => 0,
        };
        family
            + a.language.as_ref().map_or(0, |l| l.as_str().len())
            + a.font_features.features.capacity() * core::mem::size_of::<crate::Feature>()
    }
    core::mem::size_of::<ShapeRunKey>()
        + key.text.capacity()
        + key.pre_context.capacity()
        + key.post_context.capacity()
        + attrs_bytes(&key.default_attrs)
        + key.attrs_spans.capacity() * core::mem::size_of::<(Range<usize>, AttrsOwned)>()
        + key
            .attrs_spans
            .iter()
            .map(|(_, a)| attrs_bytes(a))
            .sum::<usize>()
        + glyphs.capacity() * core::mem::size_of::<ShapeGlyph>()
}
