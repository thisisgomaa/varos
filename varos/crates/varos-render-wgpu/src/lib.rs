//! wgpu renderer: a GPU canvas that draws a varos-core `Scene`. Stencil-then-cover fills,
//! MSAA, non-sRGB surface, Mailbox present (low latency). Knows nothing about winit/tauri.

mod tess;
use std::io::Write;
use tess::{build_bg, build_fg, build_fills, Vertex};
use varos_core::geom::View;
use varos_core::scene::{Prim, Scene};

/// log to stderr but NEVER panic if there's no console (e.g. windows-subsystem build with no redirect)
macro_rules! log { ($($a:tt)*) => { let _ = writeln!(std::io::stderr(), $($a)*); } }

const BG: [f32; 4] = [0.078, 0.075, 0.075, 1.0]; // #141313 — matches UI_FIGMA --bg-app (floating panels blend in)
const DS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;
const VATTRS: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
    wgpu::VertexAttribute { offset: 8, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
];
const SHADER: &str = r#"
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var o: VsOut; o.clip = vec4<f32>(pos, 0.0, 1.0); o.color = color; return o;
}
@fragment fn fs(in: VsOut) -> @location(0) vec4<f32> { return in.color; }
"#;

pub struct Renderer {
    surface: wgpu::Surface<'static>, device: wgpu::Device, queue: wgpu::Queue, config: wgpu::SurfaceConfiguration,
    pipe_main: wgpu::RenderPipeline, pipe_stencil: wgpu::RenderPipeline, pipe_cover: wgpu::RenderPipeline,
    msaa: wgpu::TextureView, ds: wgpu::TextureView, samples: u32,
    bg_buf: wgpu::Buffer, bg_cap: u64, fill_buf: wgpu::Buffer, fill_cap: u64, fg_buf: wgpu::Buffer, fg_cap: u64,
    // offscreen scene target (the canvas is rendered here, then blitted to the surface). It is the
    // SAMPLE SOURCE for the frosted-glass panels (we blur this behind each panel).
    scene_tex: wgpu::Texture, scene_view: wgpu::TextureView, sampler: wgpu::Sampler,
    blit_pipe: wgpu::RenderPipeline, blit_bgl: wgpu::BindGroupLayout, blit_bg: wgpu::BindGroup,
    frost_pipe: wgpu::RenderPipeline, frost_bgl: wgpu::BindGroupLayout, frost_bg: wgpu::BindGroup, frost_uni: wgpu::Buffer,
    // native GPU UI (egui paints onto OUR frame, sharing OUR device/queue)
    egui_rend: egui_wgpu::Renderer,
}

fn make_frost_bg(d: &wgpu::Device, l: &wgpu::BindGroupLayout, view: &wgpu::TextureView, samp: &wgpu::Sampler, uni: &wgpu::Buffer) -> wgpu::BindGroup {
    d.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("frost"), layout: l, entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(samp) },
        wgpu::BindGroupEntry { binding: 2, resource: uni.as_entire_binding() },
    ] })
}

const BLIT_SHADER: &str = r#"
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
struct VO { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VO {
    // fullscreen triangle
    var o: VO;
    let uv = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    o.uv = uv; o.p = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0); o.p.y = -o.p.y; return o;
}
@fragment fn fs(in: VO) -> @location(0) vec4<f32> { return textureSample(t, s, in.uv); }
"#;

/// Sampleable color texture (the offscreen scene target). Returns (texture, view).
fn make_scene_tex(d: &wgpu::Device, c: &wgpu::SurfaceConfiguration) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = d.create_texture(&wgpu::TextureDescriptor { label: Some("scene"),
        size: wgpu::Extent3d { width: c.width.max(1), height: c.height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: 1, dimension: wgpu::TextureDimension::D2,
        format: c.format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING, view_formats: &[] });
    let view = tex.create_view(&Default::default());
    (tex, view)
}
fn make_blit_bg(d: &wgpu::Device, l: &wgpu::BindGroupLayout, view: &wgpu::TextureView, samp: &wgpu::Sampler) -> wgpu::BindGroup {
    d.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("blit"), layout: l, entries: &[
        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(samp) },
    ] })
}

// ---- frosted-glass panels (the hand-painted GPU UI primitive) ----
// For each panel rect we draw 2 instances onto the surface: a soft drop-shadow, then the frosted
// body (samples the offscreen scene behind the panel, blurs it, tints it dark, rounded-rect mask).
pub const MAX_PANELS: usize = 8;
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrostP { rect: [f32; 4], col: [f32; 4], prm: [f32; 4] } // rect=x,y,w,h px · col=tint+mix · prm=radius,blur,shadow,solid
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FrostU { fb: [f32; 2], _pad: [f32; 2], panels: [FrostP; MAX_PANELS] }

const FROST_SHADER: &str = r#"
struct P { rect: vec4<f32>, col: vec4<f32>, prm: vec4<f32> };
struct U { fb: vec2<f32>, pad: vec2<f32>, panels: array<P, 8> };
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: U;
struct VO { @builtin(position) pos: vec4<f32>, @location(0) px: vec2<f32>,
            @location(1) @interpolate(flat) pi: u32, @location(2) @interpolate(flat) mode: u32 };
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VO {
    var o: VO;
    let pi = ii / 2u; let mode = ii % 2u;
    let p = u.panels[pi];
    var rect = p.rect;
    if (mode == 0u) { let m = p.prm.z; rect = vec4<f32>(rect.x - m, rect.y - m + 8.0, rect.z + 2.0*m, rect.w + 2.0*m); }
    var cx = array<f32,6>(0.0,1.0,0.0, 0.0,1.0,1.0);
    var cy = array<f32,6>(0.0,0.0,1.0, 1.0,0.0,1.0);
    let c = vec2<f32>(cx[vi], cy[vi]);
    let px = rect.xy + c * rect.zw;
    o.px = px; o.pi = pi; o.mode = mode;
    o.pos = vec4<f32>(px.x / u.fb.x * 2.0 - 1.0, 1.0 - px.y / u.fb.y * 2.0, 0.0, 1.0);
    return o;
}
fn sdRound(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - b + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}
@fragment fn fs(in: VO) -> @location(0) vec4<f32> {
    let p = u.panels[in.pi];
    let center = p.rect.xy + p.rect.zw * 0.5;
    let half = p.rect.zw * 0.5;
    let r = p.prm.x;
    if (in.mode == 0u) {
        let d = sdRound(in.px - center, half, r);
        let a = (1.0 - smoothstep(0.0, p.prm.z, max(d, 0.0))) * 0.42;
        return vec4<f32>(0.0, 0.0, 0.0, clamp(a, 0.0, 1.0));
    }
    let d = sdRound(in.px - center, half, r);
    let aa = 1.0 - smoothstep(-1.0, 1.0, d);
    if (aa <= 0.001) { discard; }
    var rgb: vec3<f32>;
    if (p.prm.w > 0.5) {
        rgb = p.col.rgb;
    } else {
        let uv = in.px / u.fb;
        let s = vec2<f32>(p.prm.y) / u.fb;
        var ox = array<f32,13>(0.0, 1.0,-1.0, 0.0, 0.0, 1.0,-1.0, 1.0,-1.0, 2.0,-2.0, 0.0, 0.0);
        var oy = array<f32,13>(0.0, 0.0, 0.0, 1.0,-1.0, 1.0, 1.0,-1.0,-1.0, 0.0, 0.0, 2.0,-2.0);
        var wt = array<f32,13>(4.0, 2.0,2.0,2.0,2.0, 1.0,1.0,1.0,1.0, 0.8,0.8,0.8,0.8);
        var acc = vec3<f32>(0.0); var wsum = 0.0;
        for (var i = 0; i < 13; i = i + 1) {
            acc = acc + textureSampleLevel(tex, samp, uv + vec2<f32>(ox[i], oy[i]) * s, 0.0).rgb * wt[i];
            wsum = wsum + wt[i];
        }
        rgb = mix(acc / wsum, p.col.rgb, p.col.a);
    }
    return vec4<f32>(rgb, aa);
}
"#;

fn make_attach(d: &wgpu::Device, c: &wgpu::SurfaceConfiguration, samples: u32, format: wgpu::TextureFormat, label: &str) -> wgpu::TextureView {
    d.create_texture(&wgpu::TextureDescriptor { label: Some(label),
        size: wgpu::Extent3d { width: c.width.max(1), height: c.height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1, sample_count: samples, dimension: wgpu::TextureDimension::D2,
        format, usage: wgpu::TextureUsages::RENDER_ATTACHMENT, view_formats: &[] }).create_view(&Default::default())
}
fn make_pipe(d: &wgpu::Device, l: &wgpu::PipelineLayout, sh: &wgpu::ShaderModule, fmt: wgpu::TextureFormat, samples: u32, color: bool, stencil: wgpu::StencilState) -> wgpu::RenderPipeline {
    d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None, layout: Some(l),
        vertex: wgpu::VertexState { module: sh, entry_point: "vs", buffers: &[wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &VATTRS }] },
        fragment: Some(wgpu::FragmentState { module: sh, entry_point: "fs", targets: &[Some(wgpu::ColorTargetState {
            format: fmt, blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: if color { wgpu::ColorWrites::ALL } else { wgpu::ColorWrites::empty() } })] }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState { format: DS_FORMAT, depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always, stencil, bias: wgpu::DepthBiasState::default() }),
        multisample: wgpu::MultisampleState { count: samples, ..Default::default() }, multiview: None,
    })
}

impl Renderer {
    pub async fn new(target: impl Into<wgpu::SurfaceTarget<'static>>, width: u32, height: u32) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::PRIMARY, ..Default::default() });
        let surface = instance.create_surface(target).unwrap();
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: Some(&surface), force_fallback_adapter: false,
        }).await.expect("no GPU adapter");
        log!("[varos] adapter: {:?} | backend: {:?}", adapter.get_info().name, adapter.get_info().backend);
        // unlock adapter-specific format caps (needed for 8x MSAA on Bgra8 etc.) when the GPU has them
        let extra = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES & adapter.features();
        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: None, required_features: extra, required_limits: wgpu::Limits::downlevel_defaults(),
        }, None).await.expect("no device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        // crisper edges: 8x MSAA only if the DEVICE truly supports it (needs the feature above), else 4x
        let samples = {
            use wgpu::TextureFormatFeatureFlags as Msf;
            let has = extra.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
            let cf = adapter.get_texture_format_features(format).flags;
            let df = adapter.get_texture_format_features(DS_FORMAT).flags;
            if has && cf.contains(Msf::MULTISAMPLE_X8) && df.contains(Msf::MULTISAMPLE_X8) { 8 }
            else if cf.contains(Msf::MULTISAMPLE_X4) && df.contains(Msf::MULTISAMPLE_X4) { 4 } else { 1 }
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, format, width: width.max(1), height: height.max(1),
            present_mode: if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) { wgpu::PresentMode::Mailbox }
                          else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) { wgpu::PresentMode::Immediate }
                          else { wgpu::PresentMode::Fifo },
            alpha_mode: caps.alpha_modes[0], view_formats: vec![], desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);
        log!("[varos] present: {:?} | format: {:?} | msaa: {}", config.present_mode, config.format, samples);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[], push_constant_ranges: &[] });
        let inv = wgpu::StencilFaceState { compare: wgpu::CompareFunction::Always, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::Invert };
        let cov = wgpu::StencilFaceState { compare: wgpu::CompareFunction::NotEqual, fail_op: wgpu::StencilOperation::Keep, depth_fail_op: wgpu::StencilOperation::Keep, pass_op: wgpu::StencilOperation::Zero };
        let st_fan = wgpu::StencilState { front: inv, back: inv, read_mask: 0xff, write_mask: 0xff };
        let st_cov = wgpu::StencilState { front: cov, back: cov, read_mask: 0xff, write_mask: 0xff };
        let pipe_main = make_pipe(&device, &layout, &shader, config.format, samples, true, wgpu::StencilState::default());
        let pipe_stencil = make_pipe(&device, &layout, &shader, config.format, samples, false, st_fan);
        let pipe_cover = make_pipe(&device, &layout, &shader, config.format, samples, true, st_cov);
        let msaa = make_attach(&device, &config, samples, config.format, "msaa");
        let ds = make_attach(&device, &config, samples, DS_FORMAT, "ds");
        let mk = |cap: u64| device.create_buffer(&wgpu::BufferDescriptor { label: Some("v"), size: cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let (bg_cap, fill_cap, fg_cap) = (1u64 << 18, 1u64 << 21, 1u64 << 22);
        let (bg_buf, fill_buf, fg_buf) = (mk(bg_cap), mk(fill_cap), mk(fg_cap));
        // offscreen scene target + blit-to-surface pipeline
        let (scene_tex, scene_view) = make_scene_tex(&device, &config);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor { label: Some("samp"),
            mag_filter: wgpu::FilterMode::Linear, min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge, address_mode_v: wgpu::AddressMode::ClampToEdge, ..Default::default() });
        let blit_sh = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("blit"), source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()) });
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("blit"), entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
        ] });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("blit"), bind_group_layouts: &[&blit_bgl], push_constant_ranges: &[] });
        let blit_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor { label: Some("blit"), layout: Some(&blit_layout),
            vertex: wgpu::VertexState { module: &blit_sh, entry_point: "vs", buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &blit_sh, entry_point: "fs", targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: None, write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None, multisample: wgpu::MultisampleState::default(), multiview: None });
        let blit_bg = make_blit_bg(&device, &blit_bgl, &scene_view, &sampler);
        // frosted-glass pipeline (samples scene_view; blends onto the surface)
        let frost_sh = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("frost"), source: wgpu::ShaderSource::Wgsl(FROST_SHADER.into()) });
        let frost_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("frost"), entries: &[
            wgpu::BindGroupLayoutEntry { binding: 0, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Texture { sample_type: wgpu::TextureSampleType::Float { filterable: true }, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }, count: None },
            wgpu::BindGroupLayoutEntry { binding: 1, visibility: wgpu::ShaderStages::FRAGMENT, ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering), count: None },
            wgpu::BindGroupLayoutEntry { binding: 2, visibility: wgpu::ShaderStages::VERTEX_FRAGMENT, ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None }, count: None },
        ] });
        let frost_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("frost"), bind_group_layouts: &[&frost_bgl], push_constant_ranges: &[] });
        let frost_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor { label: Some("frost"), layout: Some(&frost_layout),
            vertex: wgpu::VertexState { module: &frost_sh, entry_point: "vs", buffers: &[] },
            fragment: Some(wgpu::FragmentState { module: &frost_sh, entry_point: "fs", targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })] }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None, multisample: wgpu::MultisampleState::default(), multiview: None });
        let frost_uni = device.create_buffer(&wgpu::BufferDescriptor { label: Some("frostU"), size: std::mem::size_of::<FrostU>() as u64, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let frost_bg = make_frost_bg(&device, &frost_bgl, &scene_view, &sampler, &frost_uni);
        // egui paints onto the (single-sample) surface in its own pass we own
        let egui_rend = egui_wgpu::Renderer::new(&device, config.format, None, 1);
        Renderer { surface, device, queue, config, pipe_main, pipe_stencil, pipe_cover, msaa, ds, samples, bg_buf, bg_cap, fill_buf, fill_cap, fg_buf, fg_cap,
                   scene_tex, scene_view, sampler, blit_pipe, blit_bgl, blit_bg, frost_pipe, frost_bgl, frost_bg, frost_uni, egui_rend }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w; self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            self.msaa = make_attach(&self.device, &self.config, self.samples, self.config.format, "msaa");
            self.ds = make_attach(&self.device, &self.config, self.samples, DS_FORMAT, "ds");
            let (st, sv) = make_scene_tex(&self.device, &self.config);
            self.scene_tex = st; self.scene_view = sv;
            self.blit_bg = make_blit_bg(&self.device, &self.blit_bgl, &self.scene_view, &self.sampler);
            self.frost_bg = make_frost_bg(&self.device, &self.frost_bgl, &self.scene_view, &self.sampler, &self.frost_uni);
        }
    }
    /// Shared GPU handles so the UI layer (egui) can render into OUR frame (mandatory: same device/queue).
    pub fn device(&self) -> &wgpu::Device { &self.device }
    pub fn queue(&self) -> &wgpu::Queue { &self.queue }
    pub fn surface_format(&self) -> wgpu::TextureFormat { self.config.format }

    fn upload(device: &wgpu::Device, queue: &wgpu::Queue, buf: &mut wgpu::Buffer, cap: &mut u64, verts: &[Vertex]) -> u32 {
        let bytes: &[u8] = bytemuck::cast_slice(verts);
        if bytes.len() as u64 > *cap {
            *cap = (bytes.len() as u64).next_power_of_two().max(1024);
            *buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("v"), size: *cap, usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        }
        if !bytes.is_empty() { queue.write_buffer(buf, 0, bytes); }
        verts.len() as u32
    }

    pub fn render(&mut self, world: &Scene, ui: &[Prim], view: View) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => { self.surface.configure(&self.device, &self.config); return; }
            Err(_) => return,
        };
        let tview = frame.texture.create_view(&Default::default());
        let (fw, fh) = (self.config.width as f32, self.config.height as f32);
        let bg = build_bg(fw, fh);
        let (fillv, franges) = build_fills(&world.content, view, fw, fh);
        let mut fg = build_fg(&world.content, view, view.zoom, fw, fh);   // artwork: strokes scale with zoom
        fg.extend(build_fg(&world.overlay, view, 1.0, fw, fh));           // editing chrome: constant screen size
        fg.extend(build_fg(ui, View::identity(), 1.0, fw, fh));           // toolbar: screen-fixed
        let nbg = Self::upload(&self.device, &self.queue, &mut self.bg_buf, &mut self.bg_cap, &bg);
        let _ = Self::upload(&self.device, &self.queue, &mut self.fill_buf, &mut self.fill_cap, &fillv);
        let nfg = Self::upload(&self.device, &self.queue, &mut self.fg_buf, &mut self.fg_cap, &fg);
        let mut enc = self.device.create_command_encoder(&Default::default());
        // Pass 1: the varos-core Scene → offscreen (MSAA resolve to scene_view).
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.msaa, resolve_target: Some(&self.scene_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: BG[0] as f64, g: BG[1] as f64, b: BG[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.ds,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0), store: wgpu::StoreOp::Discard }) }),
                timestamp_writes: None, occlusion_query_set: None });
            rp.set_stencil_reference(0);
            if nbg > 0 { rp.set_pipeline(&self.pipe_main); rp.set_vertex_buffer(0, self.bg_buf.slice(..)); rp.draw(0..nbg, 0..1); }
            if !franges.is_empty() {
                rp.set_vertex_buffer(0, self.fill_buf.slice(..));
                for ((fs, fl), (cs, cl)) in &franges {
                    rp.set_pipeline(&self.pipe_stencil); rp.draw(*fs..*fs + *fl, 0..1);
                    rp.set_pipeline(&self.pipe_cover);   rp.draw(*cs..*cs + *cl, 0..1);
                }
            }
            if nfg > 0 { rp.set_pipeline(&self.pipe_main); rp.set_vertex_buffer(0, self.fg_buf.slice(..)); rp.draw(0..nfg, 0..1); }
        }
        // Pass 2: blit the offscreen scene onto the surface.
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &tview, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
            rp.set_pipeline(&self.blit_pipe); rp.set_bind_group(0, &self.blit_bg, &[]); rp.draw(0..3, 0..1);
        }
        self.queue.submit(Some(enc.finish()));
        frame.present();
    }

    /// Render the canvas Scene AND the native egui UI into one frame (the spike path):
    /// scene → offscreen → blit to surface → egui onto a pass WE own → present. egui shares our
    /// Device/Queue. `paint_jobs`/`tdelta` come from the app's egui Context; no second surface/window.
    pub fn render_ui(&mut self, world: &Scene, view: View,
        paint_jobs: &[egui::ClippedPrimitive], tdelta: &egui::TexturesDelta, screen: &egui_wgpu::ScreenDescriptor,
        panels: &[[f32; 4]], frosted: bool) {
        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(wgpu::SurfaceError::Lost) | Err(wgpu::SurfaceError::Outdated) => { self.surface.configure(&self.device, &self.config); return; }
            Err(_) => return,
        };
        let tview = frame.texture.create_view(&Default::default());
        let (fw, fh) = (self.config.width as f32, self.config.height as f32);
        let bg = build_bg(fw, fh);
        let (fillv, franges) = build_fills(&world.content, view, fw, fh);
        let mut fg = build_fg(&world.content, view, view.zoom, fw, fh);
        fg.extend(build_fg(&world.overlay, view, 1.0, fw, fh));
        let nbg = Self::upload(&self.device, &self.queue, &mut self.bg_buf, &mut self.bg_cap, &bg);
        let _ = Self::upload(&self.device, &self.queue, &mut self.fill_buf, &mut self.fill_cap, &fillv);
        let nfg = Self::upload(&self.device, &self.queue, &mut self.fg_buf, &mut self.fg_cap, &fg);
        for (id, delta) in &tdelta.set { self.egui_rend.update_texture(&self.device, &self.queue, *id, delta); }
        // frosted-glass uniform: one entry per panel (tint #141313, mixed 0.62 over the blurred scene)
        let np = panels.len().min(MAX_PANELS);
        let mut fu = FrostU { fb: [fw, fh], _pad: [0.0, 0.0], panels: [FrostP { rect: [0.0;4], col: [0.0;4], prm: [0.0;4] }; MAX_PANELS] };
        for i in 0..np {
            fu.panels[i] = FrostP { rect: panels[i], col: [BG[0], BG[1], BG[2], 0.62],
                prm: [12.0, 14.0, 26.0, if frosted { 0.0 } else { 1.0 }] };
        }
        self.queue.write_buffer(&self.frost_uni, 0, bytemuck::bytes_of(&fu));
        let mut enc = self.device.create_command_encoder(&Default::default());
        let user_cmds = self.egui_rend.update_buffers(&self.device, &self.queue, &mut enc, paint_jobs, screen);
        // scene → offscreen
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.msaa, resolve_target: Some(&self.scene_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: BG[0] as f64, g: BG[1] as f64, b: BG[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment { view: &self.ds,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                    stencil_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0), store: wgpu::StoreOp::Discard }) }),
                timestamp_writes: None, occlusion_query_set: None });
            rp.set_stencil_reference(0);
            if nbg > 0 { rp.set_pipeline(&self.pipe_main); rp.set_vertex_buffer(0, self.bg_buf.slice(..)); rp.draw(0..nbg, 0..1); }
            if !franges.is_empty() {
                rp.set_vertex_buffer(0, self.fill_buf.slice(..));
                for ((fs, fl), (cs, cl)) in &franges {
                    rp.set_pipeline(&self.pipe_stencil); rp.draw(*fs..*fs + *fl, 0..1);
                    rp.set_pipeline(&self.pipe_cover);   rp.draw(*cs..*cs + *cl, 0..1);
                }
            }
            if nfg > 0 { rp.set_pipeline(&self.pipe_main); rp.set_vertex_buffer(0, self.fg_buf.slice(..)); rp.draw(0..nfg, 0..1); }
        }
        // blit offscreen → surface, then the frosted-glass panel backings on top of the canvas
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("blit+frost"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &tview, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
            rp.set_pipeline(&self.blit_pipe); rp.set_bind_group(0, &self.blit_bg, &[]); rp.draw(0..3, 0..1);
            if np > 0 { rp.set_pipeline(&self.frost_pipe); rp.set_bind_group(0, &self.frost_bg, &[]); rp.draw(0..6, 0..(np as u32 * 2)); }
        }
        // egui chrome → surface (load — drawn over the canvas)
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &tview, resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store } })],
                depth_stencil_attachment: None, timestamp_writes: None, occlusion_query_set: None });
            self.egui_rend.render(&mut rp, paint_jobs, screen);
        }
        self.queue.submit(user_cmds.into_iter().chain(std::iter::once(enc.finish())));
        frame.present();
        for id in &tdelta.free { self.egui_rend.free_texture(id); }
    }
}
