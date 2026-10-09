//! Lane B: the picker only adapts its persisted UI rule to pure core harmony maths.
use varos_app::storage::layout::HarmonyRule as Rule;
fn core(rule: Rule) -> varos_core::colour_guide::Rule {
    use varos_core::colour_guide::Rule as C;
    match rule {
        Rule::Complementary => C::Complementary,
        Rule::Analogous => C::Analogous,
        Rule::Split => C::Split,
        Rule::Triadic => C::Triadic,
        Rule::Tetradic => C::Tetradic,
        Rule::Square => C::Square,
        Rule::Mono => C::Mono,
        Rule::None => C::None,
    }
}
pub(super) fn linked(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    varos_core::colour_guide::linked(core(rule), base)
}
pub(super) fn swatches(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    varos_core::colour_guide::swatches(core(rule), base)
}
pub(super) fn variations(rule: Rule, base: [f32; 3]) -> Vec<[f32; 3]> {
    varos_core::colour_guide::variations(core(rule), base)
}
