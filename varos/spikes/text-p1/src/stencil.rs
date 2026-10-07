//! CPU model of a separate winding attachment, never shared clip/parity bits.
//! Integer pixel-center proof only; GPU antialiasing is deliberately not claimed.
use crate::outlines::{Outline, Point};
#[derive(Clone, Copy, Debug)]
pub enum Rule {
    EvenOdd,
    NonZero,
}
#[derive(Debug, PartialEq)]
pub struct Coverage {
    pub alpha: Vec<u8>,
    pub stencil_after: Vec<u8>,
    pub clip_after: Vec<u8>,
}
pub fn cover(
    outlines: &[Outline],
    width: usize,
    height: usize,
    clip: &[u8],
    alpha: u8,
    rule: Rule,
) -> Result<Coverage, &'static str> {
    let pixels = width.checked_mul(height).ok_or("resource limit")?;
    if pixels > 1_000_000 || clip.len() != pixels {
        return Err("resource limit");
    }
    let rings: Vec<_> = outlines.iter().map(|o| o.flatten(0.005)).collect();
    let edges: usize = rings.iter().flatten().map(Vec::len).sum();
    if edges > 100_000 || edges.saturating_mul(pixels) > 50_000_000 {
        return Err("resource limit");
    }
    let mut result = vec![0; pixels];
    let mut stencil = vec![0u8; pixels];
    for glyph in rings {
        // Validate signed winding BEFORE increment/decrement wrap. Never draw an
        // unsafe glyph partially: caller receives no result on resource refusal.
        let counts: Vec<i32> =
            (0..pixels).map(|i| winding(&glyph, [(i % width) as f32 + 0.5, (i / width) as f32 + 0.5])).collect();
        if counts.iter().any(|c| c.unsigned_abs() > 255) {
            return Err("winding overflow");
        }
        for (s, count) in stencil.iter_mut().zip(counts) {
            match rule {
                Rule::NonZero => {
                    for _ in 0..count.unsigned_abs() {
                        *s = if count > 0 { s.wrapping_add(1) } else { s.wrapping_sub(1) };
                    }
                }
                Rule::EvenOdd => *s = (count.unsigned_abs() % 2) as u8,
            }
        }
        for i in 0..pixels {
            if stencil[i] != 0 && clip[i] & 0x02 != 0 {
                result[i] = alpha;
            }
            stencil[i] = 0;
        }
    }
    Ok(Coverage { alpha: result, stencil_after: stencil, clip_after: clip.to_vec() })
}
fn winding(rings: &[Vec<Point>], p: Point) -> i32 {
    let mut count = 0;
    for ring in rings {
        for (a, b) in ring.iter().zip(ring.iter().cycle().skip(1)).take(ring.len()) {
            let cross = (b[0] - a[0]) * (p[1] - a[1]) - (p[0] - a[0]) * (b[1] - a[1]);
            if a[1] <= p[1] && b[1] > p[1] && cross > 0. {
                count += 1;
            }
            if a[1] > p[1] && b[1] <= p[1] && cross < 0. {
                count -= 1;
            }
        }
    }
    count
}
