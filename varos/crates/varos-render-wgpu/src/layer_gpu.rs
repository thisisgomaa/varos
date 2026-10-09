//! Encoder-only layer passes; no window, submission, or work on an empty list.
//! Adapted map-pass structure from PhotoCraft gpu/src/compose.wgsl @ 4cb7cf3.
//! Copyright (c) 2026 ArtCraft Team and contributors. MIT OR Apache-2.0. See NOTICE.
use crate::layers::{self, Blend, CacheKey, Limits, Prim, Report};
use std::collections::HashMap;
use wgpu::util::DeviceExt;
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    mode: u32,
    operation: u32,
    radius: i32,
    vertical: i32,
    opacity: f32,
    dx: i32,
    dy: i32,
    outer: u32,
    colour: [f32; 4],
}
impl Default for Params {
    fn default() -> Self {
        Self { mode: 0, operation: 0, radius: 0, vertical: 0, opacity: 1.0, dx: 0, dy: 0, outer: 0, colour: [0.0; 4] }
    }
}
struct Surface {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
}
fn surface(d: &wgpu::Device, size: [u32; 2]) -> Surface {
    let texture = d.create_texture(&wgpu::TextureDescriptor {
        label: Some("Lane D pooled layer"),
        size: extent(size),
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&Default::default());
    Surface { texture, view }
}
fn extent(s: [u32; 2]) -> wgpu::Extent3d {
    wgpu::Extent3d { width: s[0], height: s[1], depth_or_array_layers: 1 }
}
fn copy(e: &mut wgpu::CommandEncoder, a: &wgpu::Texture, b: &wgpu::Texture, s: [u32; 2]) {
    e.copy_texture_to_texture(a.as_image_copy(), b.as_image_copy(), extent(s));
}
pub struct GpuLayers {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    pool: Vec<Surface>,
    size: [u32; 2],
    cache: HashMap<CacheKey, Surface>,
}
impl GpuLayers {
    pub fn invalidate(&mut self, object: u64) {
        self.cache.retain(|k, _| k.object != object);
    }
    pub fn new(d: &wgpu::Device) -> Self {
        let tex = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let buf = |binding, ty| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lane D layout"),
            entries: &[
                tex(0),
                tex(1),
                tex(2),
                buf(3, wgpu::BufferBindingType::Uniform),
                buf(4, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lane D shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("layer_pass.wgsl").into()),
        });
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Lane D pipeline"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lane D effects"),
            layout: Some(&pl),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self { pipeline, layout, pool: Vec::new(), size: [0, 0], cache: HashMap::new() }
    }
    fn take(&mut self, d: &wgpu::Device, s: [u32; 2]) -> Surface {
        self.pool.pop().unwrap_or_else(|| surface(d, s))
    }
    fn pass(
        &self,
        d: &wgpu::Device,
        e: &mut wgpu::CommandEncoder,
        inputs: [&wgpu::TextureView; 3],
        dst: &wgpu::TextureView,
        p: Params,
        w: &[f32],
    ) {
        let uniform = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lane D params"),
            contents: bytemuck::bytes_of(&p),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let weights = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lane D kernel"),
            contents: bytemuck::cast_slice(w),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let bg = d.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lane D inputs"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(inputs[0]) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(inputs[1]) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(inputs[2]) },
                wgpu::BindGroupEntry { binding: 3, resource: uniform.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: weights.as_entire_binding() },
            ],
        });
        let mut rp = e.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Lane D effect"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: dst,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        rp.set_pipeline(&self.pipeline);
        rp.set_bind_group(0, &bg, &[]);
        rp.draw(0..3, 0..1);
    }
    /// Target: single-sample FORMAT with COPY_SRC/DST, TEXTURE_BINDING and RENDER_ATTACHMENT.
    /// draw records geometry with Load, premultiplied colours, and the supplied target format.
    /// Use layers::bucket_zoom for geometry too. Cache IDs identify the entire current layer
    /// (including previous effects); bump revision when that input changes. Target pan/size changes
    /// must also change revision. Budget charges actual RGBA16F storage.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        d: &wgpu::Device,
        e: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        prims: &[Prim],
        zoom: f32,
        limits: Limits,
        mut draw: impl FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView, &varos_core::Prim),
    ) -> Result<Report, String> {
        let size = [target.width(), target.height()];
        let steps = layers::plan_storage(prims, size, zoom, limits, 8)?;
        let required = wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT;
        if target.format() != FORMAT
            || target.sample_count() != 1
            || !target.usage().contains(required)
            || target.dimension() != wgpu::TextureDimension::D2
            || target.size().depth_or_array_layers != 1
        {
            return Err("invalid layer target format/samples".into());
        }
        let mut report = Report::default();
        if prims.is_empty() {
            return Ok(report);
        }
        if size != self.size {
            self.pool.clear();
            self.cache.clear();
            self.size = size;
        }
        let bytes = size[0] as usize * size[1] as usize * 8;
        let cache_budget = limits.bytes / 4;
        if self.cache.len() * bytes > cache_budget {
            self.cache.clear();
        }
        while self.pool.len() * bytes > limits.bytes.saturating_sub(cache_budget) {
            self.pool.pop();
        }
        struct Frame {
            surfaces: Option<[Surface; 4]>,
            opacity: f32,
            blend: Blend,
        }
        let mut stack: Vec<Frame> = Vec::new();
        let mut active = 0usize;
        let target_view = target.create_view(&Default::default());
        for (p, step) in prims.iter().zip(steps) {
            if let Prim::LayerBegin { opacity, blend, mask } = p {
                let allowed = matches!(step, layers::Step::Begin { isolated: true });
                let surfaces = if allowed {
                    let s: [Surface; 4] = std::array::from_fn(|_| self.take(d, size));
                    // Encoder-ordered mask upload: queue.write_texture would overwrite a reused
                    // pooled mask before earlier passes in this encoder execute.
                    let row_bytes = (size[0] * 8).div_ceil(256) * 256;
                    let mut values = vec![0u16; row_bytes as usize / 2 * size[1] as usize];
                    for y in 0..size[1] as usize {
                        for x in 0..size[0] as usize {
                            values[y * row_bytes as usize / 2 + x * 4] =
                                half(mask.as_ref().map_or(1.0, |m| m[y * size[0] as usize + x]));
                        }
                    }
                    let staging = d.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Lane D mask upload"),
                        contents: bytemuck::cast_slice(&values),
                        usage: wgpu::BufferUsages::COPY_SRC,
                    });
                    e.copy_buffer_to_texture(
                        wgpu::TexelCopyBufferInfo {
                            buffer: &staging,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(row_bytes),
                                rows_per_image: Some(size[1]),
                            },
                        },
                        s[3].texture.as_image_copy(),
                        extent(size),
                    );
                    {
                        let _clear = e.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("Lane D clear"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &s[0].view,
                                resolve_target: None,
                                depth_slice: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            ..Default::default()
                        });
                    }
                    active += 1;
                    report.peak_bytes =
                        report.peak_bytes.max(active * 4 * bytes + (self.cache.len() + self.pool.len()) * bytes);
                    Some(s)
                } else {
                    report.flattened = true;
                    None
                };
                stack.push(Frame { surfaces, opacity: *opacity, blend: *blend });
                continue;
            }
            if matches!(p, Prim::LayerEnd) {
                let Some(frame) = stack.pop() else {
                    return Err("unmatched LayerEnd".into());
                };
                if let Some(s) = frame.surfaces {
                    let parent = stack.iter().rev().find_map(|f| f.surfaces.as_ref()).map_or(target, |s| &s[0].texture);
                    let pv = parent.create_view(&Default::default());
                    copy(e, parent, &s[2].texture, size);
                    self.pass(
                        d,
                        e,
                        [&s[0].view, &s[2].view, &s[3].view],
                        &pv,
                        Params { mode: frame.blend as u32, opacity: frame.opacity, ..Default::default() },
                        &[1.0],
                    );
                    self.pool.extend(s);
                    active -= 1;
                    report.passes += 1;
                }
                continue;
            }
            let surfaces = stack.iter().rev().find_map(|f| f.surfaces.as_ref());
            if let Prim::Draw(p) = p {
                draw(e, surfaces.map_or(&target_view, |s| &s[0].view), p);
                continue;
            }
            let Some(s) = surfaces.filter(|_| stack.last().is_some_and(|f| f.surfaces.is_some())) else {
                report.flattened = true;
                continue;
            };
            let (object, revision, radius, shadow) = match p {
                Prim::Blur { object, revision, radius } => (*object, *revision, *radius, None),
                Prim::Shadow { object, revision, blur, offset, colour, outer } => {
                    (*object, *revision, *blur, Some((*offset, *colour, *outer)))
                }
                _ => continue,
            };
            if radius == 0.0 && shadow.is_none() {
                continue;
            }
            let mut key = CacheKey::new(object, revision, radius, zoom, size);
            key.shadow = shadow.is_some();
            let out = if shadow.is_some() { &s[2] } else { &s[0] };
            if let Some(hit) = self.cache.get(&key) {
                copy(e, &hit.texture, &out.texture, size);
                report.cache_hits += 1;
            } else {
                let weights = layers::kernel(radius, zoom);
                let r = weights.len() as i32 / 2;
                self.pass(
                    d,
                    e,
                    [&s[0].view, &s[2].view, &s[3].view],
                    &s[1].view,
                    Params { operation: 1, radius: r, ..Default::default() },
                    &weights,
                );
                self.pass(
                    d,
                    e,
                    [&s[1].view, &s[3].view, &s[3].view],
                    &out.view,
                    Params { operation: 1, radius: r, vertical: 1, ..Default::default() },
                    &weights,
                );
                report.passes += 2;
                if (self.cache.len() + 1) * bytes <= cache_budget {
                    let cached = surface(d, size);
                    copy(e, &out.texture, &cached.texture, size);
                    self.cache.insert(key, cached);
                    report.peak_bytes =
                        report.peak_bytes.max(active * 4 * bytes + (self.cache.len() + self.pool.len()) * bytes);
                }
            }
            if let Some((offset, colour, outer)) = shadow {
                let [dx, dy] = offset.map(|v| (v * layers::bucket_zoom(zoom)).round() as i32);
                self.pass(
                    d,
                    e,
                    [&s[2].view, &s[0].view, &s[3].view],
                    &s[1].view,
                    Params { operation: 2, dx, dy, colour, outer: u32::from(outer), ..Default::default() },
                    &[1.0],
                );
                copy(e, &s[1].texture, &s[0].texture, size);
                report.passes += 1;
            }
        }
        Ok(report)
    }
}
fn half(v: f32) -> u16 {
    let bits = v.to_bits();
    let exp = ((bits >> 23) & 255) as i32 - 127 + 15;
    let mant = bits & 0x7fffff;
    if exp <= 0 {
        if exp < -10 {
            0
        } else {
            (((mant | 0x800000) + (1 << (13 - exp))) >> (14 - exp)) as u16
        }
    } else {
        (((exp as u32) << 10) + ((mant + 0x1000) >> 13)) as u16
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn shader_validates_without_gpu() {
        let m = naga::front::wgsl::parse_str(include_str!("layer_pass.wgsl")).expect("WGSL parse");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&m)
            .expect("WGSL validation");
    }
    #[test]
    fn half_coverage() {
        assert_eq!(super::half(0.0), 0);
        assert_eq!(super::half(1.0), 0x3c00);
        assert_eq!(super::half(0.5), 0x3800);
    }
}
#[cfg(test)]
mod plan_tests {
    use crate::layers::{self, Blend, Limits, Prim, Step};
    #[test]
    fn gpu_storage_budget_uses_eight_bytes_per_pixel_without_gpu() {
        let list = [Prim::LayerBegin { opacity: 1.0, blend: Blend::Normal, mask: None }, Prim::LayerEnd];
        assert_eq!(
            layers::plan_storage(&list, [1, 1], 1.0, Limits { depth: 16, bytes: 43 }, 8).unwrap(),
            [Step::Begin { isolated: true }, Step::End { isolated: true }]
        );
        assert_eq!(
            layers::plan_storage(&list, [1, 1], 1.0, Limits { depth: 16, bytes: 41 }, 8).unwrap(),
            [Step::Begin { isolated: false }, Step::End { isolated: false }]
        );
    }
}
