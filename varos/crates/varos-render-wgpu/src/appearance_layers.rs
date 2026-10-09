//! Lane A: bounded recursive GPU targets. No device/window is needed by the planner or draw-list tests.
use crate::{
    tess::{Draw, GroupDraw, Vertex},
    Renderer,
};
use varos_core::{Group, View};
pub const DEPTH_CAP: usize = 6;
pub const TEXTURE_BUDGET: u64 = 512 * 1024 * 1024;
/// Pool-only colour MSAA + resolved RGBA budget, two slots per mask level.
/// Depth/stencil reuses the renderer's existing attachment.
pub fn allowed_depth(size: [u32; 2], samples: u32) -> usize {
    let per = u64::from(size[0])
        .saturating_mul(u64::from(size[1]))
        .saturating_mul(u64::from(samples) * 4 + 4)
        .saturating_mul(2);
    if per == 0 {
        return 0;
    }
    (TEXTURE_BUDGET / per).min(DEPTH_CAP as u64) as usize
}
pub struct Target {
    pub msaa: wgpu::TextureView,
    pub view: wgpu::TextureView,
}
pub struct Pool {
    size: [u32; 2],
    pub targets: Vec<Target>,
    pub layout: wgpu::BindGroupLayout,
    pub pipeline: wgpu::RenderPipeline,
}
const SHADER: &str = r#"
struct Out { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) opts: vec2<f32> }
@vertex fn vs(@location(0) p: vec2<f32>, @location(1) color: vec4<f32>) -> Out {
 var o: Out; o.p = vec4(p,0.,1.); o.uv = vec2((p.x+1.)*.5,(1.-p.y)*.5); o.opts = vec2(color.a,color.r); return o;
}
@group(0) @binding(0) var content: texture_2d<f32>;
@group(0) @binding(1) var alpha: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;
@fragment fn fs(o: Out) -> @location(0) vec4<f32> {
 let c = textureSample(content,samp,o.uv);
 let a = textureSample(alpha,samp,o.uv).a;
 return c * o.opts.x * select(1., a, o.opts.y > .5);
}
"#;
impl Pool {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, samples: u32) -> Self {
        let entries = [0, 1].map(|binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("appearance-mask"),
            entries: &[
                entries[0],
                entries[1],
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("appearance-composite"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("appearance-composite"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let blend = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("appearance-composite"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &crate::VATTRS,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState { color: blend, alpha: blend }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: None,
        });
        Self { size: [0, 0], targets: vec![], layout, pipeline }
    }
    pub fn prepare(&mut self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, samples: u32, count: usize) {
        let size = [config.width, config.height];
        if self.size != size {
            self.targets.clear();
            self.size = size;
        }
        while self.targets.len() < count {
            let msaa = crate::make_attach(device, config, samples, config.format, "appearance-msaa");
            let (_, view) = crate::make_scene_tex(device, config);
            self.targets.push(Target { msaa, view });
        }
        self.targets.truncate(count);
    }
}
fn shift_range(r: &mut (u32, u32), offset: u32) {
    r.0 += offset;
}
fn shift_draw(draw: &mut Draw, fill: u32, fg: u32) {
    match draw {
        Draw::Image { range, .. } | Draw::Fg { range, .. } => shift_range(range, fg),
        Draw::Fill { fan, cover } => {
            shift_range(fan, fill);
            shift_range(cover, fill);
        }
        Draw::MaskedFill { fan, cover, band, clear } => {
            shift_range(fan, fill);
            shift_range(cover, fill);
            shift_range(band, fg);
            shift_range(clear, fg);
        }
        Draw::Gradient { fan, cover, mask, .. } => {
            shift_range(fan, fill);
            shift_range(cover, fill);
            if let Some((a, b)) = mask {
                shift_range(a, fg);
                shift_range(b, fg);
            }
        }
        Draw::StrokeCov { tris, cover } => {
            shift_range(tris, fg);
            shift_range(cover, fg);
        }
        Draw::Knockout { band, fan, fcover, bcover } => {
            shift_range(band, fg);
            shift_range(fan, fill);
            shift_range(fcover, fill);
            shift_range(bcover, fg);
        }
    }
}
fn shift(meta: &mut GroupDraw, fill: u32, fg: u32, op: u32) {
    match meta {
        GroupDraw::Opaque { draws } => draws.iter_mut().for_each(|d| shift_draw(d, fill, fg)),
        GroupDraw::Layer { draws, quad } | GroupDraw::ClippedLayer { draws, quad, .. } => {
            draws.iter_mut().for_each(|d| shift_draw(d, fill, fg));
            shift_range(quad, op);
            if let GroupDraw::ClippedLayer { mask_fan, .. } = meta {
                shift_range(mask_fan, fill);
            }
        }
        GroupDraw::Clip { mask_fan, mask_clear, members } => {
            shift_range(mask_fan, fill);
            shift_range(mask_clear, fill);
            members.iter_mut().for_each(|d| shift_draw(d, fill, fg));
        }
        GroupDraw::Nested { members, mask, quad } => {
            members.iter_mut().for_each(|m| shift(m, fill, fg, op));
            if let Some(ms) = mask {
                ms.iter_mut().for_each(|m| shift(m, fill, fg, op));
            }
            shift_range(quad, op);
        }
    }
}
pub fn append(
    group: &Group,
    view: View,
    zoom: f32,
    w: f32,
    h: f32,
    buffers: (&mut Vec<Vertex>, &mut Vec<Vertex>, &mut Vec<Vertex>),
) -> GroupDraw {
    let (fill, fg, op) = buffers;
    let (opacity, members, mask) = match group {
        Group::Composite { opacity, members, mask } => (*opacity, members.clone(), mask.clone()),
        Group::Clip { mask_rings, members } => (
            1.,
            members.clone(),
            Some(vec![Group::Opaque(vec![varos_core::Prim::Fill { rings: mask_rings.clone(), color: [1.; 4] }])]),
        ),
        Group::Isolated { opacity, prims } => (
            *opacity,
            vec![if super::tess::needs_knockout(prims) {
                Group::Knockout(prims.clone())
            } else {
                Group::Opaque(prims.clone())
            }],
            None,
        ),
        _ => {
            let (fv, gv, ov, mut metas) = crate::tess::build_content(std::slice::from_ref(group), view, zoom, w, h);
            let mut meta = metas.pop().unwrap_or(GroupDraw::Opaque { draws: vec![] });
            shift(&mut meta, fill.len() as u32, fg.len() as u32, op.len() as u32);
            fill.extend(fv);
            fg.extend(gv);
            op.extend(ov);
            return meta;
        }
    };
    let nested = members.iter().map(|g| append(g, view, zoom, w, h, (fill, fg, op))).collect();
    let mask_draws = mask.as_ref().map(|ms| ms.iter().map(|g| append(g, view, zoom, w, h, (fill, fg, op))).collect());
    let start = op.len() as u32;
    crate::tess::fullscreen_quad(op, opacity);
    for v in &mut op[start as usize..] {
        v.color[0] = if mask.is_some() { 1. } else { 0. };
    }
    GroupDraw::Nested { members: nested, mask: mask_draws, quad: (start, 6) }
}
pub fn depth(metas: &[GroupDraw]) -> usize {
    metas
        .iter()
        .map(|m| match m {
            GroupDraw::Nested { members, mask, .. } => 1 + depth(members).max(mask.as_ref().map_or(0, |m| depth(m))),
            _ => 0,
        })
        .max()
        .unwrap_or(0)
}
impl Renderer {
    pub(crate) fn draw_nested(
        &self,
        enc: &mut wgpu::CommandEncoder,
        metas: &[GroupDraw],
        dest: &wgpu::TextureView,
        resolve: Option<&wgpu::TextureView>,
        slot: usize,
    ) {
        for meta in metas {
            if let GroupDraw::Nested { members, mask, quad } = meta {
                let Some(target) = self.appearance_pool.targets.get(slot) else {
                    self.draw_nested(enc, members, dest, resolve, slot);
                    continue;
                };
                self.clear_appearance(enc, target);
                self.draw_nested(enc, members, &target.msaa, Some(&target.view), slot + 2);
                let mask_view = if let Some(metas) = mask {
                    let Some(mask_target) = self.appearance_pool.targets.get(slot + 1) else { continue };
                    self.clear_appearance(enc, mask_target);
                    self.draw_nested(enc, metas, &mask_target.msaa, Some(&mask_target.view), slot + 2);
                    &mask_target.view
                } else {
                    &target.view
                };
                let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("appearance-composite"),
                    layout: &self.appearance_pool.layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&target.view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(mask_view) },
                        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                    ],
                });
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("appearance-composite"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: dest,
                        resolve_target: resolve,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    ..Default::default()
                });
                pass.set_pipeline(&self.appearance_pool.pipeline);
                pass.set_bind_group(0, &bg, &[]);
                pass.set_vertex_buffer(0, self.op_buf.slice(..));
                pass.draw(quad.0..quad.0 + quad.1, 0..1);
            } else if let GroupDraw::Opaque { draws } = meta {
                let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("appearance-leaf"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: dest,
                        resolve_target: resolve,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: Some(self.ds_clear()),
                    ..Default::default()
                });
                self.draw_steps(&mut pass, draws, false);
            }
        }
    }
    fn clear_appearance(&self, enc: &mut wgpu::CommandEncoder, target: &Target) {
        let _pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("appearance-clear"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: &target.msaa,
                resolve_target: Some(&target.view),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    }
}

pub fn recursive(groups: &[Group]) -> bool {
    groups.iter().any(|g| match g {
        Group::Composite { .. } => true,
        Group::Clip { members, .. } => {
            members.iter().any(|m| matches!(m, Group::Clip { .. } | Group::Composite { .. })) || recursive(members)
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budget_is_bounded_at_retina_and_overflow_safe() {
        assert!(allowed_depth([2880, 1800], 8) < DEPTH_CAP);
        assert_eq!(allowed_depth([u32::MAX, u32::MAX], 8), 0);
        assert_eq!(allowed_depth([32, 32], 4), DEPTH_CAP);
    }
    #[test]
    fn shader_validates_without_gpu() {
        let m = naga::front::wgsl::parse_str(SHADER).unwrap();
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&m)
            .unwrap();
    }
    #[test]
    fn nested_draw_list_keeps_mask_and_opacity() {
        let paint =
            varos_core::Prim::Fill { rings: vec![vec![[0., 0.], [20., 0.], [20., 20.], [0., 20.]]], color: [1.; 4] };
        let scene = vec![
            Group::Opaque(vec![paint.clone()]),
            Group::Composite {
                opacity: 0.5,
                members: vec![Group::Isolated { opacity: 0.3, prims: vec![paint.clone()] }],
                mask: Some(vec![Group::Opaque(vec![paint])]),
            },
        ];
        let (fill, _, opacity, metas) = crate::tess::build_content(&scene, View::identity(), 1., 32., 32.);
        assert_eq!(depth(&metas), 2);
        let GroupDraw::Nested { members, mask: Some(mask), quad } = &metas[1] else { panic!("missing alpha layer") };
        assert_eq!(mask.len(), 1);
        assert_eq!(quad.1, 6);
        assert_eq!(opacity[quad.0 as usize].color, [1., 0., 0., 0.5]);
        let GroupDraw::Nested { members: inner, mask: None, quad } = &members[0] else {
            panic!("missing isolated child")
        };
        assert_eq!(opacity[quad.0 as usize].color, [0., 0., 0., 0.3]);
        for leaf in [&metas[0], &inner[0], &mask[0]] {
            let GroupDraw::Opaque { draws } = leaf else { panic!("missing vector paint") };
            assert!(!draws.is_empty());
            for draw in draws {
                if let Draw::Fill { fan, cover } = draw {
                    for range in [fan, cover] {
                        assert!(range.1 > 0);
                        assert!((range.0 + range.1) as usize <= fill.len());
                    }
                }
            }
        }
    }
}
