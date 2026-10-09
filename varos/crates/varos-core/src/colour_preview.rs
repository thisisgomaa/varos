//! Lane C: view-only profile proof and approximate all-vector-inks multiply preview.
use crate::{
    colour_management::IccProfile,
    scene::{Group, Prim, Scene},
    Editor,
};
use std::sync::Arc;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct State {
    pub proof: bool,
    pub overprint: bool,
}
#[derive(Clone)]
pub(crate) struct Proof {
    to: Arc<moxcms::TransformF32Executor>,
    back: Arc<moxcms::TransformF32Executor>,
    channels: usize,
}
impl Proof {
    pub(crate) fn new(profile: &IccProfile) -> Result<Self, String> {
        let output = profile.parse()?;
        let (layout, channels) = match output.color_space {
            moxcms::DataColorSpace::Rgb => (moxcms::Layout::Rgb, 3),
            moxcms::DataColorSpace::Cmyk => (moxcms::Layout::Rgba, 4),
            moxcms::DataColorSpace::Gray => (moxcms::Layout::Gray, 1),
            _ => return Err("unsupported proof profile".into()),
        };
        let srgb = moxcms::ColorProfile::new_srgb();
        let to = srgb
            .create_transform_f32(moxcms::Layout::Rgb, &output, layout, Default::default())
            .map_err(|e| e.to_string())?;
        let back = output
            .create_transform_f32(layout, &srgb, moxcms::Layout::Rgb, Default::default())
            .map_err(|e| e.to_string())?;
        Ok(Self { to, back, channels })
    }
    fn rgb(&self, rgb: crate::Rgba) -> Result<crate::Rgba, String> {
        let mut process = [0.; 4];
        self.to.transform(&rgb[..3], &mut process[..self.channels]).map_err(|e| e.to_string())?;
        for c in &mut process {
            *c = c.clamp(0., 1.);
        }
        let mut display = [0.; 3];
        self.back.transform(&process[..self.channels], &mut display).map_err(|e| e.to_string())?;
        Ok([display[0].clamp(0., 1.), display[1].clamp(0., 1.), display[2].clamp(0., 1.), rgb[3]])
    }
}
pub fn proof_rgb(rgb: crate::Rgba, profile: &IccProfile) -> Result<crate::Rgba, String> {
    Proof::new(profile)?.rgb(rgb)
}
fn proof_prim(p: &mut Prim, proof: &Proof) -> Result<(), String> {
    if let Prim::GradientFill { gradient, .. } = p {
        for stop in &mut gradient.stops {
            stop.colour = proof.rgb(stop.colour)?;
        }
        return Ok(());
    }
    let colour = match p {
        Prim::Fill { color, .. }
        | Prim::Stroke { color, .. }
        | Prim::StrokeCoverage { color, .. }
        | Prim::Dashed { color, .. }
        | Prim::Square { color, .. }
        | Prim::Disc { color, .. }
        | Prim::Tri { color, .. } => Some(color),
        _ => None,
    };
    if let Some(c) = colour {
        *c = proof.rgb(*c)?;
    }
    Ok(())
}
fn groups(input: Vec<Group>, state: State, proof: Option<&Proof>, errors: &mut Vec<String>) -> Vec<Group> {
    let mut result = Vec::new();
    for group in input {
        let (mut prims, opacity, kind) = match group {
            Group::Clip { mask_rings, members } => {
                result.push(Group::Clip { mask_rings, members: groups(members, state, proof, errors) });
                continue;
            }
            Group::Opaque(prims) => (prims, 1., 0),
            Group::Knockout(prims) => (prims, 1., 1),
            Group::Isolated { opacity, prims } | Group::Overprint { opacity, prims } => (prims, opacity, 2),
        };
        if let Some(proof) = proof {
            for prim in &mut prims {
                if let Err(e) = proof_prim(prim, proof) {
                    errors.push(format!("Proof Colours: {e}"));
                }
            }
        }
        if state.overprint && !prims.iter().all(|p| matches!(p, Prim::Image { .. })) {
            if kind == 0 {
                result.extend(prims.into_iter().map(|p| {
                    if matches!(p, Prim::Image { .. }) {
                        Group::Opaque(vec![p])
                    } else {
                        Group::Overprint { opacity: 1., prims: vec![p] }
                    }
                }));
            } else {
                result.push(Group::Overprint { opacity, prims });
            }
        } else {
            result.push(match kind {
                0 => Group::Opaque(prims),
                1 => Group::Knockout(prims),
                _ => Group::Isolated { opacity, prims },
            });
        }
    }
    result
}
pub fn present(ed: &Editor, mut scene: Scene) -> Scene {
    if ed.colour_preview == State::default() {
        return scene;
    }
    let proof = if ed.colour_preview.proof {
        match ed
            .doc
            .output_profile
            .as_ref()
            .ok_or("Choose an ICC output profile before Proof Colours".into())
            .and_then(|p| ed.colour_transforms.proof(p))
        {
            Ok(p) => Some(p),
            Err(e) => {
                scene.errors.push(e);
                None
            }
        }
    } else {
        None
    };
    scene.content = groups(scene.content, ed.colour_preview, proof.as_ref(), &mut scene.errors);
    scene.report.notes.push(crate::ExportNote {kind:"colour_preview".into(),object_id:None,message:"Vector proof only; overprint approximates all vector inks with multiply, without separation flags or ink calibration. Images are unchanged.".into()});
    scene
}
/// Premultiplied multiply equation used by the preview composite pipeline.
pub fn multiply(back: [f32; 3], front: [f32; 3], alpha: f32) -> [f32; 3] {
    std::array::from_fn(|i| back[i] * (1. - alpha + front[i] * alpha))
}
