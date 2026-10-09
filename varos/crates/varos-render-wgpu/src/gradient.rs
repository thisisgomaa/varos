//! Lane B: per-gradient 1024-texel LUT and world-space placement under existing stencil covers.
//! Radial ray/circle equation idea: PhotoCraft gpu/src/compose.wgsl:632-680@a469568
//! (MIT OR Apache-2.0). GPU resources live only in the renderer.
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};
use varos_core::{geom::View, gradient::Gradient};
use wgpu::util::DeviceExt;
/// A draw binding is stable across viewport changes. Its uniform is refreshed in place.
pub fn key(g: &Gradient, _view: View, _frame: [f32; 2], opacity: f32) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.hash(&mut h);
    opacity.to_bits().hash(&mut h);
    h.finish()
}
/// LUT pixels depend only on stops and interpolation, never placement, spread or viewport.
fn lut_key(g: &Gradient) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for stop in &g.stops {
        for v in [stop.offset, stop.opacity, stop.midpoint].into_iter().chain(stop.colour) {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish()
}
struct Binding {
    group: wgpu::BindGroup,
    uniform: wgpu::Buffer,
}
const SHADER: &str = include_str!("gradient.wgsl");
pub struct Gradients {
    layout: wgpu::BindGroupLayout,
    pub normal: wgpu::RenderPipeline,
    pub clipped: wgpu::RenderPipeline,
    pub knockout: wgpu::RenderPipeline,
    pub knockout_clip: wgpu::RenderPipeline,
    groups: HashMap<u64, Binding>,
    luts: HashMap<u64, wgpu::TextureView>,
    sampler: wgpu::Sampler,
}
impl Gradients {
    pub fn new(
        d: &wgpu::Device,
        fmt: wgpu::TextureFormat,
        samples: u32,
        normal: wgpu::StencilState,
        clipped: wgpu::StencilState,
        knockout: wgpu::StencilState,
        knockout_clip: wgpu::StencilState,
    ) -> Self {
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gradient"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gradient"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let sh = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gradient"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        Self {
            knockout: super::make_pipe(d, &pl, &sh, fmt, samples, true, knockout),
            knockout_clip: super::make_pipe(d, &pl, &sh, fmt, samples, true, knockout_clip),
            normal: super::make_pipe(d, &pl, &sh, fmt, samples, true, normal),
            clipped: super::make_pipe(d, &pl, &sh, fmt, samples, true, clipped),
            layout,
            groups: HashMap::new(),
            luts: HashMap::new(),
            sampler: d.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }),
        }
    }
    pub fn prepare(&mut self, d: &wgpu::Device, q: &wgpu::Queue, metas: &[super::GroupDraw]) {
        let mut used = std::collections::HashSet::new();
        let mut used_luts = std::collections::HashSet::new();
        // ---- Lane A ----
        fn leaves<'a>(metas: &'a [super::GroupDraw], out: &mut Vec<&'a super::GroupDraw>) {
            for m in metas {
                if let super::GroupDraw::Nested { members, mask, .. } = m {
                    leaves(members, out);
                    if let Some(ms) = mask {
                        leaves(ms, out);
                    }
                } else {
                    out.push(m);
                }
            }
        }
        let mut leaf_metas = vec![];
        leaves(metas, &mut leaf_metas);
        for m in leaf_metas {
            let draws = match m {
                super::GroupDraw::Opaque { draws }
                | super::GroupDraw::Layer { draws, .. }
                | super::GroupDraw::ClippedLayer { draws, .. } => draws,
                super::GroupDraw::Clip { members, .. } => members,
                super::GroupDraw::Nested { .. } => continue,
            };
            for draw in draws {
                if let super::Draw::Gradient { key, gradient, pan, zoom, opacity, .. } = draw {
                    used.insert(*key);
                    let lut = lut_key(gradient);
                    used_luts.insert(lut);
                    let [a, b, c, e, x, y] = gradient.placement;
                    let det = a * e - b * c;
                    let params = [
                        [e / det, -b / det, -c / det, a / det],
                        [x, y, pan[0], pan[1]],
                        [
                            *zoom,
                            if gradient.kind == varos_core::gradient::GradientKind::Radial { 1. } else { 0. },
                            match gradient.spread {
                                varos_core::gradient::Spread::Pad => 0.,
                                varos_core::gradient::Spread::Reflect => 1.,
                                varos_core::gradient::Spread::Repeat => 2.,
                            },
                            *opacity,
                        ],
                        [gradient.focal[0], gradient.focal[1], 0., 0.],
                    ];
                    if let Some(binding) = self.groups.get(key) {
                        q.write_buffer(&binding.uniform, 0, bytemuck::cast_slice(&params));
                        continue;
                    }
                    let buffer = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("gradient placement"),
                        contents: bytemuck::cast_slice(&params),
                        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    });
                    let view = self.luts.entry(lut).or_insert_with(|| {
                        let tex = d.create_texture(&wgpu::TextureDescriptor {
                            label: Some("gradient LUT"),
                            size: wgpu::Extent3d { width: 1024, height: 1, depth_or_array_layers: 1 },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Rgba8Unorm,
                            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                            view_formats: &[],
                        });
                        let bytes: Vec<u8> = gradient
                            .lut()
                            .into_iter()
                            .flatten()
                            .map(|v| (v * 255.).round().clamp(0., 255.) as u8)
                            .collect();
                        q.write_texture(
                            wgpu::TexelCopyTextureInfo {
                                texture: &tex,
                                mip_level: 0,
                                origin: wgpu::Origin3d::ZERO,
                                aspect: wgpu::TextureAspect::All,
                            },
                            &bytes,
                            wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(4096),
                                rows_per_image: Some(1),
                            },
                            wgpu::Extent3d { width: 1024, height: 1, depth_or_array_layers: 1 },
                        );
                        tex.create_view(&Default::default())
                    });
                    let bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("gradient"),
                        layout: &self.layout,
                        entries: &[
                            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                            wgpu::BindGroupEntry { binding: 2, resource: buffer.as_entire_binding() },
                        ],
                    });
                    self.groups.insert(*key, Binding { group: bg, uniform: buffer });
                }
            }
        }
        self.groups.retain(|key, _| used.contains(key));
        self.luts.retain(|key, _| used_luts.contains(key));
    }
    pub fn group(&self, key: u64) -> Option<&wgpu::BindGroup> {
        self.groups.get(&key).map(|b| &b.group)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn wgsl_parses_and_validates_without_gpu() {
        let module = wgpu::naga::front::wgsl::parse_str(super::SHADER).expect("WGSL syntax");
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("WGSL validation");
    }
}

#[cfg(test)]
mod routing_tests {
    use super::*;
    use varos_core::{Group, Prim};
    #[test]
    fn gradient_cache_keys_and_clipped_knockout_steps_are_complete() {
        let g = Gradient::default();
        let k = key(&g, View::identity(), [100., 100.], 1.);
        let mut changed = g.clone();
        changed.stops[0].midpoint = 0.2;
        assert_ne!(k, key(&changed, View::identity(), [100., 100.], 1.));
        assert_eq!(k, key(&g, View { pan: [10., 0.], zoom: 2. }, [200., 200.], 1.));
        assert_ne!(lut_key(&g), lut_key(&changed));
        let mut placed = g.clone();
        placed.placement[4] = 80.;
        placed.kind = varos_core::gradient::GradientKind::Radial;
        placed.focal = [0.1, 0.1];
        placed.spread = varos_core::gradient::Spread::Repeat;
        assert_eq!(lut_key(&g), lut_key(&placed));
        assert_ne!(k, key(&placed, View::identity(), [100., 100.], 1.));
        let rings = vec![vec![[0., 0.], [80., 0.], [80., 80.], [0., 80.]]];
        let fill = Prim::GradientFill { rings: rings.clone(), gradient: g.clone(), opacity: 1., stroke: false };
        let mut stroke = g;
        stroke.stops[0].opacity = 0.5;
        let band = Prim::GradientFill { rings: rings.clone(), gradient: stroke, opacity: 1., stroke: true };
        let group = Group::Clip { mask_rings: rings, members: vec![Group::Knockout(vec![fill, band])] };
        let (fv, fgv, _, metas) = super::super::tess::build_content(&[group], View::identity(), 1., 100., 100.);
        let super::super::GroupDraw::Clip { members, .. } = &metas[0] else { panic!() };
        assert!(matches!(&members[0], super::super::Draw::Gradient { mask: Some(_), .. }));
        assert!(matches!(&members[1], super::super::Draw::Gradient { mask: None, .. }));
        assert!(!fv.is_empty());
        assert!(!fgv.is_empty());
    }
}
