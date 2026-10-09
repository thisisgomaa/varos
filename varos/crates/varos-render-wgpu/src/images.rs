//! Keyed image textures. Uploads occur only on cache misses, never on idle frames.
use std::collections::{HashMap, HashSet};
use varos_core::{
    images::{BlobKey, Pixels},
    scene::{Group, Prim},
};
const BUDGET: u64 = 256 * 1024 * 1024;
const SHADER: &str = r#"
@group(0) @binding(0) var tex:texture_2d<f32>;
@group(0) @binding(1) var samp:sampler;
struct Out { @builtin(position) p:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) alpha:f32 };
@vertex fn vs(@location(0) p:vec2<f32>,@location(1) v:vec4<f32>)->Out {var o:Out;o.p=vec4<f32>(p,0.,1.);o.uv=v.xy;o.alpha=v.z;return o;}
@fragment fn fs(o:Out)->@location(0) vec4<f32> {let c=textureSample(tex,samp,o.uv);return vec4<f32>(c.rgb,c.a*o.alpha);}
"#;
struct Resident {
    dimensions: [u32; 2],
    _texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    charge: u64,
    last: u64,
}
pub struct ImageCache {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    entries: HashMap<BlobKey, Resident>,
    clock: u64,
    pub normal: wgpu::RenderPipeline,
    pub clipped: wgpu::RenderPipeline,
    pub uploads: u64,
}
pub fn mip_dimensions(mut w: u32, mut h: u32) -> Vec<[u32; 2]> {
    let mut out = vec![[w, h]];
    while w > 1 || h > 1 {
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        out.push([w, h]);
    }
    out
}
pub fn residency_charge(w: u32, h: u32) -> u64 {
    mip_dimensions(w, h).iter().map(|[w, h]| u64::from(*w) * u64::from(*h) * 4).sum::<u64>()
        + u64::from(w) * u64::from(h) * 4
}
fn resources(groups: &[Group], out: &mut HashMap<BlobKey, std::sync::Arc<Pixels>>) {
    for g in groups {
        match g {
            Group::Clip { members, .. } => resources(members, out),
            _ => {
                for p in g.prims() {
                    if let Prim::Image { key, pixels, .. } = p {
                        out.insert(key.clone(), pixels.clone());
                    }
                }
            }
        }
    }
}
impl ImageCache {
    pub fn new(d: &wgpu::Device, format: wgpu::TextureFormat, samples: u32) -> Self {
        let layout = d.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("image"),
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
            ],
        });
        let pl = d.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("image"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("image"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let normal = super::make_pipe(d, &pl, &shader, format, samples, true, Default::default());
        let face = wgpu::StencilFaceState { compare: wgpu::CompareFunction::Equal, ..Default::default() };
        let clipped = super::make_pipe(
            d,
            &pl,
            &shader,
            format,
            samples,
            true,
            wgpu::StencilState { front: face, back: face, read_mask: 2, write_mask: 0 },
        );
        let sampler = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("image linear mips"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self { layout, sampler, entries: HashMap::new(), clock: 0, normal, clipped, uploads: 0 }
    }
    pub fn bind(&self, key: &BlobKey) -> Option<&wgpu::BindGroup> {
        self.entries.get(key).map(|r| &r.bind)
    }
    pub fn prepare(&mut self, d: &wgpu::Device, q: &wgpu::Queue, groups: &[Group]) -> Result<(), String> {
        let mut needed = HashMap::new();
        resources(groups, &mut needed);
        let live: HashSet<_> = needed.keys().cloned().collect();
        let demand = needed.values().map(|p| residency_charge(p.width, p.height)).sum::<u64>();
        if demand > BUDGET {
            return Err("Visible images exceed the 256 MiB GPU image budget".into());
        }
        self.clock += 1;
        for (key, p) in needed {
            if self.entries.get(&key).is_some_and(|r| r.dimensions != [p.width, p.height]) {
                self.entries.remove(&key);
            }
            if let Some(r) = self.entries.get_mut(&key) {
                r.last = self.clock;
                continue;
            }
            if p.width > d.limits().max_texture_dimension_2d || p.height > d.limits().max_texture_dimension_2d {
                return Err("Image exceeds the GPU adapter texture limit".into());
            }
            let charge = residency_charge(p.width, p.height);
            while self.entries.values().map(|r| r.charge).sum::<u64>() + charge > BUDGET {
                let Some(old) = self
                    .entries
                    .iter()
                    .filter(|(k, _)| !live.contains(*k))
                    .min_by_key(|(_, r)| r.last)
                    .map(|(k, _)| k.clone())
                else {
                    return Err("GPU image budget admission failed".into());
                };
                self.entries.remove(&old);
            }
            let dims = mip_dimensions(p.width, p.height);
            let texture = d.create_texture(&wgpu::TextureDescriptor {
                label: Some("immutable image"),
                size: wgpu::Extent3d { width: p.width, height: p.height, depth_or_array_layers: 1 },
                mip_level_count: dims.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let mut pixels = p.rgba.to_vec();
            for (level, &[w, h]) in dims.iter().enumerate() {
                q.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: level as u32,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    &pixels,
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
                    wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                );
                if let Some(&[nw, nh]) = dims.get(level + 1) {
                    pixels = downsample(&pixels, [w, h], [nw, nh]);
                }
            }
            let view = texture.create_view(&Default::default());
            // Own two-entry layout (texture + sampler). The blit helper gained Lane E's pixel-preview
            // uniform, which image draws never use (integration w2).
            let bind = d.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("image"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            self.entries.insert(
                key,
                Resident { dimensions: [p.width, p.height], _texture: texture, bind, charge, last: self.clock },
            );
            self.uploads += 1;
        }
        Ok(())
    }
}
fn downsample(src: &[u8], [w, h]: [u32; 2], [nw, nh]: [u32; 2]) -> Vec<u8> {
    let mut dst = vec![0; (nw * nh * 4) as usize];
    for y in 0..nh {
        for x in 0..nw {
            let mut a = 0u32;
            let mut rgb = [0u32; 3];
            let mut count = 0;
            for sy in y * h / nh..((y + 1) * h / nh) {
                for sx in x * w / nw..((x + 1) * w / nw) {
                    let p = &src[((sy * w + sx) * 4) as usize..][..4];
                    a += p[3] as u32;
                    for c in 0..3 {
                        rgb[c] += p[c] as u32 * p[3] as u32;
                    }
                    count += 1;
                }
            }
            let to = ((y * nw + x) * 4) as usize;
            for c in 0..3 {
                dst[to + c] = rgb[c].checked_div(a).unwrap_or(0) as u8;
            }
            dst[to + 3] = (a / count) as u8;
        }
    }
    dst
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mips_and_budget_are_bounded() {
        assert_eq!(mip_dimensions(5, 3), vec![[5, 3], [2, 1], [1, 1]]);
        assert_eq!(residency_charge(4, 4), 64 + 16 + 4 + 64);
        assert!(residency_charge(8000, 8000) > BUDGET);
    }
    #[test]
    fn mip_preserves_transparent_edges() {
        assert_eq!(downsample(&[255, 0, 0, 255, 0, 0, 255, 0], [2, 1], [1, 1]), vec![255, 0, 0, 127]);
    }
}
