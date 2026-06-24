//! wgpu renderer: a GPU canvas that draws a varos-core `Scene`. Stencil-then-cover fills,
//! MSAA, non-sRGB surface, Mailbox present (low latency). Knows nothing about winit/tauri.

mod tess;
use std::io::Write;
use tess::{build_bg, build_fg, build_fills, Vertex};
use varos_core::geom::View;
use varos_core::scene::{Prim, Scene};

/// log to stderr but NEVER panic if there's no console (e.g. windows-subsystem build with no redirect)
macro_rules! log { ($($a:tt)*) => { let _ = writeln!(std::io::stderr(), $($a)*); } }

const BG: [f32; 4] = [0.117, 0.117, 0.117, 1.0];
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
}

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
        Renderer { surface, device, queue, config, pipe_main, pipe_stencil, pipe_cover, msaa, ds, samples, bg_buf, bg_cap, fill_buf, fill_cap, fg_buf, fg_cap }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w; self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            self.msaa = make_attach(&self.device, &self.config, self.samples, self.config.format, "msaa");
            self.ds = make_attach(&self.device, &self.config, self.samples, DS_FORMAT, "ds");
        }
    }

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
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor { label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &self.msaa, resolve_target: Some(&tview),
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
        self.queue.submit(Some(enc.finish()));
        frame.present();
    }
}
