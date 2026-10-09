//! Lane E host presentation helpers, GPU-independent.
pub fn canvas_rgba(rgb: [u8; 3]) -> [f32; 4] {
    [rgb[0] as f32 / 255.0, rgb[1] as f32 / 255.0, rgb[2] as f32 / 255.0, 1.0]
}
pub fn signature(scene: u64, rgb: [u8; 3]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    scene.hash(&mut h);
    rgb.hash(&mut h);
    h.finish()
}
