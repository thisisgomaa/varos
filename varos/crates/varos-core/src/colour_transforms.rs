//! Lane C: explicit editor-owned ICC transforms. One content key and at most three executors.
//! Conversion layouts and default intent are fixed by Screen/Proof; names are not conversion settings.
use crate::{
    colour_management::{IccProfile, Screen},
    colour_preview::Proof,
};
use std::cell::RefCell;
#[derive(Clone, Default)]
pub(crate) struct Cache(RefCell<Entry>);
#[derive(Clone, Default)]
struct Entry {
    data: String,
    screen: Option<Result<Screen, String>>,
    proof: Option<Result<Proof, String>>,
    #[cfg(test)]
    builds: usize,
}
impl Cache {
    fn entry(&self, profile: &IccProfile) -> Result<std::cell::RefMut<'_, Entry>, String> {
        // Validate metadata even on a hit; profile names do not affect colour conversion.
        if profile.name.trim().is_empty() || profile.name.len() > 256 || profile.name.chars().any(char::is_control) {
            return Err("invalid ICC profile name".into());
        }
        if profile.data.len() > 8 * 1024 * 1024 {
            return Err("ICC profile exceeds 4 MiB".into());
        }
        let mut entry = self.0.borrow_mut();
        if entry.data != profile.data {
            *entry = Entry { data: profile.data.clone(), ..Default::default() };
        }
        Ok(entry)
    }
    pub(crate) fn screen(&self, profile: &IccProfile) -> Result<Screen, String> {
        let mut entry = self.entry(profile)?;
        if entry.screen.is_none() {
            entry.screen = Some(Screen::new(profile));
            #[cfg(test)]
            {
                entry.builds += 1;
            }
        }
        entry.screen.as_ref().cloned().ok_or_else(|| "missing screen transform".to_owned())?
    }
    pub(crate) fn proof(&self, profile: &IccProfile) -> Result<Proof, String> {
        let mut entry = self.entry(profile)?;
        if entry.proof.is_none() {
            entry.proof = Some(Proof::new(profile));
            #[cfg(test)]
            {
                entry.builds += 1;
            }
        }
        entry.proof.as_ref().cloned().ok_or_else(|| "missing proof transform".to_owned())?
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_reuses_transforms_and_content_change_evicts_bounded_entry() {
        let mut ed = crate::Editor::new();
        let profile = IccProfile::new("sRGB".into(), &moxcms::ColorProfile::new_srgb().encode().unwrap()).unwrap();
        ed.doc.output_profile = Some(profile.clone());
        ed.colour_preview.proof = true;
        let style = crate::scene::SceneStyle { checkerboard: [[1.; 4]; 2], outline: [0.; 4], canvas: [1.; 4] };
        for i in 0..8 {
            crate::scene::build_scene_in_view_styled(
                &ed,
                crate::geom::View { pan: [i as f32; 2], zoom: 1. + i as f32 },
                [100, 100],
                style,
            );
        }
        assert_eq!(ed.colour_transforms.0.borrow().builds, 2);
        let renamed = IccProfile { name: "Renamed".into(), ..profile.clone() };
        ed.colour_transforms.screen(&renamed).unwrap();
        assert_eq!(ed.colour_transforms.0.borrow().builds, 2);
        let mut bytes = profile.bytes().unwrap();
        bytes[12..16].copy_from_slice(b"prtr");
        let changed = IccProfile::new("Printer".into(), &bytes).unwrap();
        ed.colour_transforms.screen(&changed).unwrap();
        let entry = ed.colour_transforms.0.borrow();
        assert_eq!(entry.builds, 1);
        assert!(entry.proof.is_none());
        assert_eq!(entry.data, changed.data);
    }
    #[test]
    fn invalid_profile_result_is_cached_without_stale_success() {
        let cache = Cache::default();
        let bad = IccProfile { name: "Bad".into(), data: "00".repeat(128) };
        for _ in 0..3 {
            assert!(cache.screen(&bad).is_err());
        }
        assert_eq!(cache.0.borrow().builds, 1);
    }
}
