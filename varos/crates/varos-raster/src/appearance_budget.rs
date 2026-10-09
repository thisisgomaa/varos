//! Lane A: refuse excessive CPU temporary surfaces before any layer allocation.
//! Integration w3: composites run on the render lane's f32 layer stack (`layer_scene`), so the
//! charge is its real storage: the f32 root copy + backdrop + 8-bit scratch (36 B/px) once, then a
//! f32 layer plus f32 mask coverage (20 B/px) per temporary surface.
use varos_core::Group;
const BUDGET: u64 = 1024 * 1024 * 1024;
fn surfaces(groups: &[Group]) -> u64 {
    groups
        .iter()
        .map(|g| match g {
            Group::Composite { members, mask, .. } => {
                let content = 1 + surfaces(members);
                mask.as_ref().map_or(content, |m| content.max(2 + surfaces(m)))
            }
            Group::Clip { members, .. } => 2 + surfaces(members),
            Group::Isolated { .. } | Group::Overprint { .. } => 1,
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}
pub fn check(groups: &[Group], size: [u32; 2]) -> Result<(), String> {
    let temporary = surfaces(groups);
    let per_pixel = if temporary == 0 { 0 } else { 36 + 20 * temporary };
    if temporary > 12 || u64::from(size[0]).saturating_mul(u64::from(size[1])).saturating_mul(per_pixel) > BUDGET {
        Err("limit_exceeded: Appearance CPU layer depth/1 GiB temporary-surface budget; reduce export dimensions or mask nesting".into())
    } else {
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budget_refuses_before_allocating_and_accepts_small_nested_masks() {
        let g = vec![Group::Composite { opacity: 0.5, members: vec![], mask: Some(vec![]) }];
        assert!(check(&g, [64, 64]).is_ok());
        assert!(check(&g, [8000, 8000]).is_err());
        assert!(check(&[], [8000, 8000]).is_ok());
    }
}
