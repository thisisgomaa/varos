//! CMYK is naive subtractive sRGB; Lab is sRGB/XYZ D65, without profiles or colour management.
use super::*;
pub(super) use varos_app::storage::layout::PickerMode as Mode;
pub(super) const MODES: [Mode; 6] = [Mode::Hsb, Mode::Hsl, Mode::Rgb, Mode::Cmyk, Mode::Lab, Mode::Web];
pub(super) const NAMES: [&str; 6] = ["HSB", "HSL", "RGB", "CMYK", "Lab", "Web"];
pub(super) fn channels(mode: Mode) -> &'static [(&'static str, f32, f32)] {
    match mode {
        Mode::Hsb => &[("H", 0., 360.), ("S", 0., 100.), ("B", 0., 100.)],
        Mode::Hsl => &[("H", 0., 360.), ("S", 0., 100.), ("L", 0., 100.)],
        Mode::Rgb | Mode::Web => &[("R", 0., 255.), ("G", 0., 255.), ("B", 0., 255.)],
        Mode::Cmyk => &[("C", 0., 100.), ("M", 0., 100.), ("Y", 0., 100.), ("K", 0., 100.)],
        Mode::Lab => &[("L", 0., 100.), ("a", -128., 127.), ("b", -128., 127.)],
    }
}
pub(super) fn values(mode: Mode, c: Rgba) -> [f32; 4] {
    let [h, s, v] = rgb_to_hsv(c);
    match mode {
        Mode::Hsb => [h * 360., s * 100., v * 100., 0.],
        Mode::Hsl => {
            let l = v * (1. - s / 2.);
            let d = l.min(1. - l);
            [h * 360., if d > 0. { (v - l) / d * 100. } else { 0. }, l * 100., 0.]
        }
        Mode::Rgb | Mode::Web => [c[0] * 255., c[1] * 255., c[2] * 255., 0.],
        Mode::Cmyk => {
            let k = 1. - v;
            if v == 0. {
                [0., 0., 0., 100.]
            } else {
                [(1. - c[0] - k) / v * 100., (1. - c[1] - k) / v * 100., (1. - c[2] - k) / v * 100., k * 100.]
            }
        }
        Mode::Lab => {
            let lin = |v: f32| if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
            let [r, g, b] = [lin(c[0]), lin(c[1]), lin(c[2])];
            let f = |v: f32| if v > 216. / 24389. { v.cbrt() } else { v * 24389. / 3132. + 16. / 116. };
            let x = f((0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047);
            let y = f(0.2126729 * r + 0.7151522 * g + 0.072175 * b);
            let z = f((0.0193339 * r + 0.119192 * g + 0.9503041 * b) / 1.08883);
            [116. * y - 16., 500. * (x - y), 200. * (y - z), 0.]
        }
    }
}
pub(super) fn rgb(mode: Mode, v: [f32; 4], alpha: f32) -> Rgba {
    let c = match mode {
        Mode::Hsb => hsv_to_rgb(v[0] / 360., v[1] / 100., v[2] / 100.),
        Mode::Hsl => {
            let l = v[2] / 100.;
            let b = l + v[1] / 100. * l.min(1. - l);
            hsv_to_rgb(v[0] / 360., if b > 0. { 2. * (1. - l / b) } else { 0. }, b)
        }
        Mode::Rgb | Mode::Web => [v[0] / 255., v[1] / 255., v[2] / 255.],
        Mode::Cmyk => [
            (1. - v[0] / 100.) * (1. - v[3] / 100.),
            (1. - v[1] / 100.) * (1. - v[3] / 100.),
            (1. - v[2] / 100.) * (1. - v[3] / 100.),
        ],
        Mode::Lab => {
            let y = (v[0] + 16.) / 116.;
            let x = y + v[1] / 500.;
            let z = y - v[2] / 200.;
            let f = |v: f32| if v > 6. / 29. { v * v * v } else { (v - 16. / 116.) * 3132. / 24389. };
            let [x, y, z] = [f(x) * 0.95047, f(y), f(z) * 1.08883];
            let enc = |v: f32| if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1. / 2.4) - 0.055 };
            [
                enc(3.2404542 * x - 1.5371385 * y - 0.4985314 * z),
                enc(-0.969266 * x + 1.8760108 * y + 0.041556 * z),
                enc(0.0556434 * x - 0.2040259 * y + 1.0572252 * z),
            ]
        }
    };
    [c[0].clamp(0., 1.), c[1].clamp(0., 1.), c[2].clamp(0., 1.), alpha]
}
#[cfg(test)]
pub(super) fn changed(mode: Mode, c: Rgba, index: usize, value: f32) -> Rgba {
    let mut v = values(mode, c);
    v[index] = if mode == Mode::Web { (value / 51.).round() * 51. } else { value };
    rgb(mode, v, c[3])
}
pub(super) fn readout(mode: Mode, c: Rgba, v: [f32; 4]) -> Vec<(&'static str, String)> {
    if mode == Mode::Web {
        return vec![("", hex_of(c))];
    }
    channels(mode).iter().enumerate().map(|(i, (name, _, _))| (*name, format!("{:.0}", v[i]))).collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_mode_round_trips_edges_greys_and_colours() {
        for mode in MODES {
            for c in [
                [0., 0., 0., 1.],
                [1., 1., 1., 1.],
                [0.5, 0.5, 0.5, 0.4],
                [22. / 255., 22. / 255., 22. / 255., 1.],
                [24. / 255., 24. / 255., 24. / 255., 1.],
                [0., 35. / 255., 30. / 255., 1.],
                [0.9, 0.2, 0.6, 1.],
                [0., 1., 0., 1.],
                [1., 0., 0., 1.],
            ] {
                let out = rgb(mode, values(mode, c), c[3]);
                assert!(c.iter().zip(out).all(|(a, b)| (a - b).abs() <= 1. / 255.), "{mode:?}: {c:?} -> {out:?}");
            }
        }
    }
    #[test]
    fn hue_wrap_web_snap_and_readout() {
        assert!(same_color(rgb(Mode::Hsb, [360., 100., 100., 0.], 1.), rgb(Mode::Hsb, [0., 100., 100., 0.], 1.)));
        assert_eq!(changed(Mode::Web, [0., 0., 0., 1.], 0, 80.)[0], 102. / 255.);
        for mode in MODES {
            assert_eq!(
                readout(mode, [0.2, 0.3, 0.4, 1.], values(mode, [0.2, 0.3, 0.4, 1.])).len(),
                if mode == Mode::Web { 1 } else { channels(mode).len() }
            );
        }
    }
}

#[test]
fn lab_lightness_is_strictly_monotonic_over_all_byte_greys() {
    let mut previous = -1.0;
    for grey in 0..=255 {
        let g = grey as f32 / 255.0;
        let l = values(Mode::Lab, [g, g, g, 1.0])[0];
        assert!(l > previous, "grey {grey}: {previous} -> {l}");
        previous = l;
    }
}
