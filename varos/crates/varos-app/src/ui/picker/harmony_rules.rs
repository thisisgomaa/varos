//! Pure port of da05aca's picker.rs Harmony::offsets/harmony_set.
//! Hue offsets and the clamped Mono brightness multipliers are preserved verbatim.
use varos_app::storage::layout::HarmonyRule as Rule;

pub(super) fn offsets(rule: Rule) -> &'static [f32] {
    match rule {
        Rule::Complementary => &[180.0],
        Rule::Analogous => &[-30.0, 30.0],
        Rule::Split => &[150.0, 210.0],
        Rule::Triadic => &[120.0, 240.0],
        Rule::Tetradic => &[60.0, 180.0, 240.0],
        Rule::Square => &[90.0, 180.0, 270.0],
        _ => &[],
    }
}
pub(super) fn linked(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    match rule {
        Rule::None => vec![base],
        Rule::Mono => {
            [1.0, 0.78, 0.56, 0.36].iter().map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)]).collect()
        }
        _ => {
            let mut v = vec![base];
            for off in offsets(rule) {
                v.push([(base[0] + off / 360.0).rem_euclid(1.0), base[1], base[2]]);
            }
            v
        }
    }
}
/// Six chips: original linked set first, then tones from the original Mono progression.
/// None keeps only the base hue; Mono continues its brightness steps without duplicate chips.
pub(super) fn swatches(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    if rule == Rule::None {
        return std::iter::once(base)
            .chain([0.78, 0.56, 0.36, 0.22, 0.12].iter().map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)]))
            .collect();
    }
    if rule == Rule::Mono {
        return [1.0, 0.78, 0.56, 0.36, 0.22, 0.12]
            .iter()
            .map(|k| [base[0], base[1], (base[2] * k).clamp(0.06, 1.0)])
            .fold(Vec::new(), |mut out, c| {
                if !out.contains(&c) {
                    out.push(c);
                }
                out
            });
    }
    let mut out = linked(rule, base);
    let seeds = out.clone();
    for k in [0.78, 0.56, 0.36] {
        for c in &seeds {
            if out.len() == 6 {
                return out;
            }
            out.push([c[0], c[1], (c[2] * k).clamp(0.06, 1.0)]);
        }
    }
    out
}
