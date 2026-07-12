//! wgpu renderer: a GPU canvas that draws a varos-core `Scene`. Stencil-then-cover fills,
//! MSAA, non-sRGB surface, Mailbox present (low latency). Knows nothing about winit/tauri.

pub mod perf;
mod tess;
use std::io::Write;
use tess::{build_bg, build_content, build_fg, Draw, GroupDraw, Vertex};
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
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipe_main: wgpu::RenderPipeline,
    pipe_stencil: wgpu::RenderPipeline,
    pipe_cover: wgpu::RenderPipeline,
    pipe_smark: wgpu::RenderPipeline, // stencil-MARK the band bit 0x80 (Replace, colour off)
    pipe_cover_knock: wgpu::RenderPipeline, // fill cover for knockout: inside AND not under the band
    pipe_cover_band: wgpu::RenderPipeline, // band cover: paint the stroke once where marked, clear the bit
    // clip-mask (MASKS_PLAN §3.1) — the dedicated bit 0x02 pipelines:
    pipe_mask_fan: wgpu::RenderPipeline, // fan the mask silhouette into 0x02 (even-odd, colour off)
    pipe_mask_clear: wgpu::RenderPipeline, // zero 0x02 over the mask bbox after members draw (colour off)
    pipe_cover_clip: wgpu::RenderPipeline, // clipped fill cover: paint where parity∧clip, clear parity
    pipe_fill_clear: wgpu::RenderPipeline, // clear leftover parity outside the mask (colour off)
    pipe_main_clip: wgpu::RenderPipeline, // clipped opaque draw: paint only where 0x02 is set
    msaa: wgpu::TextureView,
    ds: wgpu::TextureView,
    samples: u32,
    bg_buf: wgpu::Buffer,
    bg_cap: u64,
    fill_buf: wgpu::Buffer,
    fill_cap: u64,
    fg_buf: wgpu::Buffer,
    fg_cap: u64,
    // offscreen scene target (the canvas is rendered here, then blitted to the surface).
    scene_tex: wgpu::Texture,
    scene_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    // isolated-layer (group opacity): a reusable offscreen MSAA target — each translucent object is
    // rendered here opaquely, resolved, then composited onto the scene at its opacity.
    layer_msaa: wgpu::TextureView,
    layer_view: wgpu::TextureView,
    pipe_composite: wgpu::RenderPipeline,
    comp_bg: wgpu::BindGroup,
    op_buf: wgpu::Buffer,
    op_cap: u64,
    blit_pipe: wgpu::RenderPipeline,
    blit_bgl: wgpu::BindGroupLayout,
    blit_bg: wgpu::BindGroup,
    // native GPU UI (egui paints onto OUR frame, sharing OUR device/queue)
    egui_rend: egui_wgpu::Renderer,
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

// Composite an isolated layer onto the scene at the object's opacity. The layer texture is PREMULTIPLIED
// (rendered opaquely over transparent, then MSAA-resolved — so edge texels are already colour·coverage).
// We scale the whole premultiplied texel by `o` and blend premultiplied-over (src=One, dst=1-srcAlpha):
// out = layer·o + scene·(1 − layer.a·o). In the stroke region that is scene·(1−o) + stroke·o — no fill
// bleed. Opacity rides in the quad's colour.a (no push constants / uniforms needed).
const COMP_SHADER: &str = r#"
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
struct VO { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) @interpolate(flat) o: f32 };
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> VO {
    var out: VO;
    out.p = vec4<f32>(pos, 0.0, 1.0);
    out.uv = vec2<f32>(pos.x * 0.5 + 0.5, 0.5 - pos.y * 0.5);   // NDC → texture uv (y flipped, matches blit)
    out.o = color.a;
    return out;
}
@fragment fn fs(in: VO) -> @location(0) vec4<f32> {
    let c = textureSample(t, s, in.uv);        // premultiplied (colour·coverage, coverage)
    return vec4<f32>(c.rgb * in.o, c.a * in.o);
}
"#;

/// Sampleable color texture (the offscreen scene target). Returns (texture, view).
fn make_scene_tex(d: &wgpu::Device, c: &wgpu::SurfaceConfiguration) -> (wgpu::Texture, wgpu::TextureView) {
    let tex = d.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene"),
        size: wgpu::Extent3d { width: c.width.max(1), height: c.height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: c.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    (tex, view)
}
fn make_blit_bg(
    d: &wgpu::Device,
    l: &wgpu::BindGroupLayout,
    view: &wgpu::TextureView,
    samp: &wgpu::Sampler,
) -> wgpu::BindGroup {
    d.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("blit"),
        layout: l,
        entries: &[
            wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
            wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(samp) },
        ],
    })
}

fn make_attach(
    d: &wgpu::Device,
    c: &wgpu::SurfaceConfiguration,
    samples: u32,
    format: wgpu::TextureFormat,
    label: &str,
) -> wgpu::TextureView {
    d.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: c.width.max(1), height: c.height.max(1), depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
    .create_view(&Default::default())
}
fn make_pipe(
    d: &wgpu::Device,
    l: &wgpu::PipelineLayout,
    sh: &wgpu::ShaderModule,
    fmt: wgpu::TextureFormat,
    samples: u32,
    color: bool,
    stencil: wgpu::StencilState,
) -> wgpu::RenderPipeline {
    d.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(l),
        vertex: wgpu::VertexState {
            module: sh,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &VATTRS,
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: sh,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: fmt,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: if color { wgpu::ColorWrites::ALL } else { wgpu::ColorWrites::empty() },
            })],
        }),
        primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DS_FORMAT,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil,
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
        multiview_mask: None,
        cache: None,
    })
}

impl Renderer {
    /// GPU init is the one edge that genuinely fails in the wild (no/old driver, remote desktop, broken
    /// surface) — so it returns a human-readable Err instead of panicking; the app shows it in a dialog
    /// (ENGINEERING_REVIEW §3.3: "GPU/Win32/external edges never panic; internal invariants may").
    pub async fn new(target: impl Into<wgpu::SurfaceTarget<'static>>, width: u32, height: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(target)
            .map_err(|e| format!("couldn't create a draw surface on the window: {e}"))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("no compatible graphics adapter (driver missing or too old): {e}"))?;
        log!("[varos] adapter: {:?} | backend: {:?}", adapter.get_info().name, adapter.get_info().backend);
        // unlock adapter-specific format caps (needed for 8x MSAA on Bgra8 etc.) when the GPU has them
        let extra = wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES & adapter.features();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: extra,
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("the graphics device couldn't start: {e}"))?;
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        // crisper edges: 8x MSAA only if the DEVICE truly supports it (needs the feature above), else 4x
        let samples = {
            use wgpu::TextureFormatFeatureFlags as Msf;
            let has = extra.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);
            let cf = adapter.get_texture_format_features(format).flags;
            let df = adapter.get_texture_format_features(DS_FORMAT).flags;
            if has && cf.contains(Msf::MULTISAMPLE_X8) && df.contains(Msf::MULTISAMPLE_X8) {
                8
            } else if cf.contains(Msf::MULTISAMPLE_X4) && df.contains(Msf::MULTISAMPLE_X4) {
                4
            } else {
                1
            }
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: width.max(1),
            height: height.max(1),
            present_mode: if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else if caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
                wgpu::PresentMode::Immediate
            } else {
                wgpu::PresentMode::Fifo
            },
            // prefer a transparent-capable composite mode so the startup splash can float over the desktop
            alpha_mode: caps
                .alpha_modes
                .iter()
                .copied()
                .find(|m| {
                    matches!(m, wgpu::CompositeAlphaMode::PreMultiplied | wgpu::CompositeAlphaMode::PostMultiplied)
                })
                .unwrap_or(caps.alpha_modes[0]),
            view_formats: vec![],
            desired_maximum_frame_latency: 1,
        };
        surface.configure(&device, &config);
        log!("[varos] present: {:?} | format: {:?} | msaa: {}", config.present_mode, config.format, samples);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        // TWO stencil bits share the buffer: bit 0x01 = fill even-odd parity, bit 0x80 = translucent-stroke
        // band mask. Fans/covers confine themselves to 0x01, band mark/cover to 0x80, so a knockout object
        // can hold both at once (fill paints where parity=1 AND band=0 → the stroke cuts the fill beneath it).
        let inv = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Invert,
        };
        let cov = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::NotEqual,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Zero,
        };
        let st_fan = wgpu::StencilState { front: inv, back: inv, read_mask: 0x01, write_mask: 0x01 };
        let st_cov = wgpu::StencilState { front: cov, back: cov, read_mask: 0x01, write_mask: 0x01 };
        // band MARK: unconditionally write the band bit wherever any stroke triangle covers —
        // overlap-count-proof (unlike Invert's even-odd), so the band later paints exactly once.
        let mark = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Replace,
        };
        let st_mark = wgpu::StencilState { front: mark, back: mark, read_mask: 0x80, write_mask: 0x80 };
        // knockout fill cover: paint where (stencil & 0x81) == 0x01 — inside the shape AND not under the band
        let knock = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Zero,
        };
        let st_knock = wgpu::StencilState { front: knock, back: knock, read_mask: 0x81, write_mask: 0x01 };
        // band cover: paint where the band bit is set, then clear BOTH bits (write 0x81) — the knockout
        // fill-cover can't zero parity under the band (its Equal test fails there), so the band cover
        // sweeps that leftover parity too; otherwise it would leak into the next fill's even-odd.
        let st_band = wgpu::StencilState { front: cov, back: cov, read_mask: 0x80, write_mask: 0x81 };
        // ── CLIP MASK (MASKS_PLAN §3.1): a THIRD, dedicated stencil bit 0x02, orthogonal to 0x01 (fill
        // parity) and 0x80 (knockout band). The mask silhouette is fanned even-odd into 0x02 (persists
        // across all member draws — none of 0x01/0x80/0x81 write it, so it SURVIVES for free), members
        // paint only where 0x02 is set, then 0x02 is zeroed for the next group. ──
        // mask fan: even-odd toggle 0x02 wherever the silhouette covers → 0x02 = 1 inside the mask.
        let st_mask_fan = wgpu::StencilState { front: inv, back: inv, read_mask: 0x02, write_mask: 0x02 };
        // mask clear: unconditionally zero 0x02 over the mask bbox (Replace-to-0 via `cov`? no — `cov`
        // only zeros where NotEqual; use a dedicated Always→Zero so the whole bbox is wiped clean).
        let clear2 = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Zero,
        };
        let st_mask_clear = wgpu::StencilState { front: clear2, back: clear2, read_mask: 0x02, write_mask: 0x02 };
        // clipped fill cover: paint where (parity 0x01 AND clip 0x02) == both set, i.e. (stencil & 0x03)
        // == 0x03; pass_op Zero with write_mask 0x01 clears ONLY the parity bit there (clip bit kept).
        let clip_cov = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Zero,
        };
        let st_cover_clip = wgpu::StencilState { front: clip_cov, back: clip_cov, read_mask: 0x03, write_mask: 0x01 };
        // clipped opaque draw (strokes/dashes): paint where clip 0x02 set; touch NO stencil bit (the fill
        // even-odd of a later member must stay intact). compare Equal ref 0x02 read 0x02, all-Keep.
        let clip_test = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        };
        let st_main_clip = wgpu::StencilState { front: clip_test, back: clip_test, read_mask: 0x02, write_mask: 0x00 };
        let pipe_main =
            make_pipe(&device, &layout, &shader, config.format, samples, true, wgpu::StencilState::default());
        let pipe_stencil = make_pipe(&device, &layout, &shader, config.format, samples, false, st_fan);
        let pipe_cover = make_pipe(&device, &layout, &shader, config.format, samples, true, st_cov);
        let pipe_smark = make_pipe(&device, &layout, &shader, config.format, samples, false, st_mark);
        let pipe_cover_knock = make_pipe(&device, &layout, &shader, config.format, samples, true, st_knock);
        let pipe_cover_band = make_pipe(&device, &layout, &shader, config.format, samples, true, st_band);
        // clip pipelines: fan/clear write bit 0x02 with colour OFF; the clipped cover paints colour where
        // parity∧clip and clears parity; `pipe_fill_clear` (colour OFF) sweeps the parity the clipped
        // cover left set OUTSIDE the mask, so a later member's even-odd starts clean; `pipe_main_clip`
        // paints opaque strokes only inside the mask.
        let pipe_mask_fan = make_pipe(&device, &layout, &shader, config.format, samples, false, st_mask_fan);
        let pipe_mask_clear = make_pipe(&device, &layout, &shader, config.format, samples, false, st_mask_clear);
        let pipe_cover_clip = make_pipe(&device, &layout, &shader, config.format, samples, true, st_cover_clip);
        // same state as `st_cov` (NotEqual 0 on parity → Zero), but colour OFF: a pure parity wipe.
        let st_fill_clear = wgpu::StencilState { front: cov, back: cov, read_mask: 0x01, write_mask: 0x01 };
        let pipe_fill_clear = make_pipe(&device, &layout, &shader, config.format, samples, false, st_fill_clear);
        let pipe_main_clip = make_pipe(&device, &layout, &shader, config.format, samples, true, st_main_clip);
        let msaa = make_attach(&device, &config, samples, config.format, "msaa");
        let ds = make_attach(&device, &config, samples, DS_FORMAT, "ds");
        let mk = |cap: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("v"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let (bg_cap, fill_cap, fg_cap) = (1u64 << 18, 1u64 << 21, 1u64 << 22);
        let (bg_buf, fill_buf, fg_buf) = (mk(bg_cap), mk(fill_cap), mk(fg_cap));
        // offscreen scene target + blit-to-surface pipeline
        let (scene_tex, scene_view) = make_scene_tex(&device, &config);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("samp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let blit_sh = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
        });
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
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
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&blit_bgl)],
            immediate_size: 0,
        });
        let blit_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&blit_layout),
            vertex: wgpu::VertexState {
                module: &blit_sh,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_sh,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let blit_bg = make_blit_bg(&device, &blit_bgl, &scene_view, &sampler);
        // isolated-layer target + composite pipeline (group opacity). Reuses blit_bgl (texture+sampler).
        let layer_msaa = make_attach(&device, &config, samples, config.format, "layer_msaa");
        let (_layer_tex, layer_view) = make_scene_tex(&device, &config);
        let comp_sh = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite"),
            source: wgpu::ShaderSource::Wgsl(COMP_SHADER.into()),
        });
        let comp_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite"),
            bind_group_layouts: &[Some(&blit_bgl)],
            immediate_size: 0,
        });
        // premultiplied-over: src is already colour·coverage·o, so src_factor = One (NOT SrcAlpha — that would double-premultiply → dark halos)
        let premul_over = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        let pipe_composite = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite"),
            layout: Some(&comp_layout),
            vertex: wgpu::VertexState {
                module: &comp_sh,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &VATTRS,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &comp_sh,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState { color: premul_over, alpha: premul_over }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() },
            // the scene pass carries a depth-stencil attachment, so this pipeline must declare one too (it ignores it)
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DS_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: samples, ..Default::default() },
            multiview_mask: None,
            cache: None,
        });
        let comp_bg = make_blit_bg(&device, &blit_bgl, &layer_view, &sampler);
        let op_cap = 1u64 << 16;
        let op_buf = mk(op_cap);
        // egui paints onto the (single-sample) surface in its own pass we own
        let egui_rend = egui_wgpu::Renderer::new(
            &device,
            config.format,
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                ..Default::default()
            },
        );
        Ok(Renderer {
            surface,
            device,
            queue,
            config,
            pipe_main,
            pipe_stencil,
            pipe_cover,
            pipe_smark,
            pipe_cover_knock,
            pipe_cover_band,
            pipe_mask_fan,
            pipe_mask_clear,
            pipe_cover_clip,
            pipe_fill_clear,
            pipe_main_clip,
            msaa,
            ds,
            samples,
            bg_buf,
            bg_cap,
            fill_buf,
            fill_cap,
            fg_buf,
            fg_cap,
            scene_tex,
            scene_view,
            sampler,
            layer_msaa,
            layer_view,
            pipe_composite,
            comp_bg,
            op_buf,
            op_cap,
            blit_pipe,
            blit_bgl,
            blit_bg,
            egui_rend,
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w > 0 && h > 0 {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            self.msaa = make_attach(&self.device, &self.config, self.samples, self.config.format, "msaa");
            self.ds = make_attach(&self.device, &self.config, self.samples, DS_FORMAT, "ds");
            let (st, sv) = make_scene_tex(&self.device, &self.config);
            self.scene_tex = st;
            self.scene_view = sv;
            self.layer_msaa = make_attach(&self.device, &self.config, self.samples, self.config.format, "layer_msaa");
            let (_lt, lv) = make_scene_tex(&self.device, &self.config);
            self.layer_view = lv;
            self.comp_bg = make_blit_bg(&self.device, &self.blit_bgl, &self.layer_view, &self.sampler);
            self.blit_bg = make_blit_bg(&self.device, &self.blit_bgl, &self.scene_view, &self.sampler);
        }
    }
    /// Shared GPU handles so the UI layer (egui) can render into OUR frame (mandatory: same device/queue).
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }

    fn upload(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buf: &mut wgpu::Buffer,
        cap: &mut u64,
        verts: &[Vertex],
    ) -> u32 {
        let bytes: &[u8] = bytemuck::cast_slice(verts);
        if bytes.len() as u64 > *cap {
            *cap = (bytes.len() as u64).next_power_of_two().max(1024);
            *buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("v"),
                size: *cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        if !bytes.is_empty() {
            queue.write_buffer(buf, 0, bytes);
        }
        verts.len() as u32
    }

    /// The shared depth-stencil attachment for a scene/layer pass: stencil cleared to 0 (the even-odd fill
    /// algorithm needs a known-zero start — never assume a loaded stencil is zero), depth cleared, both discarded.
    fn ds_clear(&self) -> wgpu::RenderPassDepthStencilAttachment<'_> {
        wgpu::RenderPassDepthStencilAttachment {
            view: &self.ds,
            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
            stencil_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(0), store: wgpu::StoreOp::Discard }),
        }
    }

    /// Play a group's draw steps in paint order: each object's fill (stencil fan → cover) immediately
    /// followed by its own stroke — so an object above covers the stroke of the one below (Illustrator
    /// stacking). Works inside any pass (scene or isolated layer); stroke drawing never touches stencil.
    fn draw_steps<'a>(&'a self, rp: &mut wgpu::RenderPass<'a>, draws: &[Draw], clip: bool) {
        for d in draws {
            match d {
                Draw::Fill { fan, cover } => {
                    rp.set_vertex_buffer(0, self.fill_buf.slice(..));
                    rp.set_pipeline(&self.pipe_stencil);
                    rp.draw(fan.0..fan.0 + fan.1, 0..1);
                    if clip {
                        // clipped fill (MASKS_PLAN §3.1): the fan set parity 0x01 across the whole shape,
                        // even OUTSIDE the mask. (1) cover paints where parity∧clip (ref 0x03) and clears
                        // parity THERE; (2) `pipe_fill_clear` (colour off) sweeps the parity still set
                        // outside the mask, so the next member's even-odd starts clean. The clip bit 0x02
                        // is never written by either step, so it survives for the rest of the members.
                        rp.set_stencil_reference(0x03);
                        rp.set_pipeline(&self.pipe_cover_clip);
                        rp.draw(cover.0..cover.0 + cover.1, 0..1);
                        rp.set_stencil_reference(0);
                        rp.set_pipeline(&self.pipe_fill_clear);
                        rp.draw(cover.0..cover.0 + cover.1, 0..1);
                    } else {
                        rp.set_pipeline(&self.pipe_cover);
                        rp.draw(cover.0..cover.0 + cover.1, 0..1);
                    }
                }
                Draw::Fg { range, scissor } => {
                    rp.set_vertex_buffer(0, self.fg_buf.slice(..));
                    // inside a clip, opaque strokes paint only where the mask bit 0x02 is set (ref 0x02);
                    // the pipeline writes no stencil, so parity/clip bits are untouched.
                    if clip {
                        rp.set_stencil_reference(0x02);
                        rp.set_pipeline(&self.pipe_main_clip);
                    } else {
                        rp.set_pipeline(&self.pipe_main);
                    }
                    // A2: an artboard-clipped stroke draws under a GPU scissor set to its page rect, so the
                    // band is trimmed to the page edge. Reset to the FULL framebuffer immediately after, so
                    // no later draw in this pass can inherit the scissor (a leaked scissor could hide other
                    // content). `None` ⇒ draw across the whole framebuffer as usual (fail-open: a missed or
                    // degenerate clip overflows the page, it never vanishes).
                    match scissor {
                        Some(s) => {
                            rp.set_scissor_rect(s[0], s[1], s[2], s[3]);
                            rp.draw(range.0..range.0 + range.1, 0..1);
                            rp.set_scissor_rect(0, 0, self.config.width, self.config.height);
                        }
                        None => rp.draw(range.0..range.0 + range.1, 0..1),
                    }
                    if clip {
                        rp.set_stencil_reference(0);
                    }
                }
                // translucent stroke (no fill): mark the band bit on every covered pixel, then cover ONCE
                // at the stroke colour (paints where marked, clears the bit after)
                Draw::StrokeCov { tris, cover } => {
                    rp.set_vertex_buffer(0, self.fg_buf.slice(..));
                    rp.set_stencil_reference(0x80);
                    rp.set_pipeline(&self.pipe_smark);
                    rp.draw(tris.0..tris.0 + tris.1, 0..1);
                    rp.set_stencil_reference(0);
                    rp.set_pipeline(&self.pipe_cover_band);
                    rp.draw(cover.0..cover.0 + cover.1, 0..1);
                }
                // knockout object (fill + translucent stroke): the band CUTS the fill beneath it, so the
                // stroke blends against what's behind the OBJECT — never against its own fill.
                Draw::Knockout { band, fan, fcover, bcover } => {
                    // 1) mark the band bit (0x80) wherever the stroke covers, overlap-proof
                    rp.set_vertex_buffer(0, self.fg_buf.slice(..));
                    rp.set_stencil_reference(0x80);
                    rp.set_pipeline(&self.pipe_smark);
                    if band.1 > 0 {
                        rp.draw(band.0..band.0 + band.1, 0..1);
                    }
                    // 2) even-odd fan the fill into parity bit 0x01
                    rp.set_vertex_buffer(0, self.fill_buf.slice(..));
                    rp.set_pipeline(&self.pipe_stencil);
                    if fan.1 > 0 {
                        rp.draw(fan.0..fan.0 + fan.1, 0..1);
                    }
                    // 3) fill cover: paint where inside AND NOT under the band ((stencil&0x81)==0x01)
                    rp.set_stencil_reference(0x01);
                    rp.set_pipeline(&self.pipe_cover_knock);
                    if fcover.1 > 0 {
                        rp.draw(fcover.0..fcover.0 + fcover.1, 0..1);
                    }
                    // 4) band cover: paint the stroke once where marked; clears the band bit
                    rp.set_vertex_buffer(0, self.fg_buf.slice(..));
                    rp.set_stencil_reference(0);
                    rp.set_pipeline(&self.pipe_cover_band);
                    if bcover.1 > 0 {
                        rp.draw(bcover.0..bcover.0 + bcover.1, 0..1);
                    }
                }
            }
        }
    }

    /// Record the canvas into `self.msaa`, resolving into `self.scene_view`. Opaque groups paint straight
    /// onto the (accumulating, MSAA) scene target; each isolated layer is rendered opaquely into `layer_msaa`,
    /// resolved to `layer_view`, then composited back at its opacity — in z-order, so a translucent object
    /// sent behind an opaque one stays behind it. Overlay (editing chrome) draws last, on top of everything.
    /// The scene target is cleared exactly once (bg pass); every later scene pass LOADs; resolve happens on
    /// the final pass only.
    fn record_scene(&self, enc: &mut wgpu::CommandEncoder, nbg: u32, metas: &[GroupDraw], overlay: (u32, u32)) {
        let clearc = wgpu::Color { r: BG[0] as f64, g: BG[1] as f64, b: BG[2] as f64, a: 1.0 };
        // bg pass — clear the scene target and lay the dot grid
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("scene-bg"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &self.msaa,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clearc), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(self.ds_clear()),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_stencil_reference(0);
            if nbg > 0 {
                rp.set_pipeline(&self.pipe_main);
                rp.set_vertex_buffer(0, self.bg_buf.slice(..));
                rp.draw(0..nbg, 0..1);
            }
        }
        for m in metas {
            match m {
                GroupDraw::Opaque { draws } => {
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        multiview_mask: None,
                        label: Some("scene-opaque"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            depth_slice: None,
                            view: &self.msaa,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: Some(self.ds_clear()),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    rp.set_stencil_reference(0);
                    self.draw_steps(&mut rp, draws, false);
                }
                // CLIPPING MASK (MASKS_PLAN §3.1) — ONE self-contained pass so the clip bit 0x02 persists
                // across every member draw (each pass clears the stencil at entry via `ds_clear`). Fan the
                // mask silhouette into 0x02, replay the members clip-tested, then zero 0x02. The pass LOADs
                // the accumulating scene colour, so the clipped art composites over what is already drawn.
                GroupDraw::Clip { mask_fan, mask_clear, members } => {
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        multiview_mask: None,
                        label: Some("scene-clip"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            depth_slice: None,
                            view: &self.msaa,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: Some(self.ds_clear()),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    // 1) stamp the mask silhouette into clip bit 0x02 (even-odd, colour off)
                    rp.set_stencil_reference(0);
                    rp.set_vertex_buffer(0, self.fill_buf.slice(..));
                    rp.set_pipeline(&self.pipe_mask_fan);
                    if mask_fan.1 > 0 {
                        rp.draw(mask_fan.0..mask_fan.0 + mask_fan.1, 0..1);
                    }
                    // 2) the members, each tested against the mask bit
                    self.draw_steps(&mut rp, members, true);
                    // 3) wipe the clip bit so the next group starts clean
                    rp.set_stencil_reference(0);
                    rp.set_vertex_buffer(0, self.fill_buf.slice(..));
                    rp.set_pipeline(&self.pipe_mask_clear);
                    if mask_clear.1 > 0 {
                        rp.draw(mask_clear.0..mask_clear.0 + mask_clear.1, 0..1);
                    }
                }
                GroupDraw::Layer { draws, quad } => {
                    // render the object OPAQUELY into the isolated layer (cleared transparent, MSAA-resolved)
                    {
                        let mut lp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            multiview_mask: None,
                            label: Some("layer"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                depth_slice: None,
                                view: &self.layer_msaa,
                                resolve_target: Some(&self.layer_view),
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: Some(self.ds_clear()),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                        lp.set_stencil_reference(0);
                        self.draw_steps(&mut lp, draws, false);
                    }
                    // composite the resolved layer onto the scene at the object's opacity
                    {
                        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            multiview_mask: None,
                            label: Some("composite"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                depth_slice: None,
                                view: &self.msaa,
                                resolve_target: None,
                                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                            })],
                            depth_stencil_attachment: Some(self.ds_clear()),
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                        rp.set_pipeline(&self.pipe_composite);
                        rp.set_bind_group(0, &self.comp_bg, &[]);
                        rp.set_vertex_buffer(0, self.op_buf.slice(..));
                        rp.draw(quad.0..quad.0 + quad.1, 0..1);
                    }
                }
            }
        }
        // overlay (editing chrome) on top of everything, and RESOLVE the finished scene → scene_view
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("scene-overlay"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &self.msaa,
                    resolve_target: Some(&self.scene_view),
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(self.ds_clear()),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_stencil_reference(0);
            if overlay.1 > 0 {
                rp.set_pipeline(&self.pipe_main);
                rp.set_vertex_buffer(0, self.fg_buf.slice(..));
                rp.draw(overlay.0..overlay.0 + overlay.1, 0..1);
            }
        }
    }

    pub fn render(&mut self, world: &Scene, ui: &[Prim], view: View) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let tview = frame.texture.create_view(&Default::default());
        let (fw, fh) = (self.config.width as f32, self.config.height as f32);
        let bg = build_bg(view, fw, fh);
        let (fillv, mut fgv, opv, metas) = build_content(&world.content, view, view.zoom, fw, fh);
        let ov_start = fgv.len() as u32;
        fgv.extend(build_fg(&world.overlay, view, 1.0, fw, fh)); // editing chrome: constant screen size
        fgv.extend(build_fg(ui, View::identity(), 1.0, fw, fh)); // toolbar: screen-fixed
        let overlay = (ov_start, fgv.len() as u32 - ov_start);
        let nbg = Self::upload(&self.device, &self.queue, &mut self.bg_buf, &mut self.bg_cap, &bg);
        let _ = Self::upload(&self.device, &self.queue, &mut self.fill_buf, &mut self.fill_cap, &fillv);
        let _ = Self::upload(&self.device, &self.queue, &mut self.fg_buf, &mut self.fg_cap, &fgv);
        let _ = Self::upload(&self.device, &self.queue, &mut self.op_buf, &mut self.op_cap, &opv);
        let mut enc = self.device.create_command_encoder(&Default::default());
        // Pass 1: the varos-core Scene → offscreen (opaque + isolated layers, MSAA resolve to scene_view).
        self.record_scene(&mut enc, nbg, &metas, overlay);
        // Pass 2: blit the offscreen scene onto the surface.
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &tview,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.blit_pipe);
            rp.set_bind_group(0, &self.blit_bg, &[]);
            rp.draw(0..3, 0..1);
        }
        self.queue.submit(Some(enc.finish()));
        frame.present();
    }

    /// Render the canvas Scene AND the native egui UI into one frame (the spike path):
    /// scene → offscreen → blit to surface → egui onto a pass WE own → present. egui shares our
    /// Device/Queue. `paint_jobs`/`tdelta` come from the app's egui Context; no second surface/window.
    /// Startup splash: clear the surface fully transparent and render ONLY egui (the floating card),
    /// so it composites over the desktop — no board, no panels, no dark scrim.
    pub fn render_splash(
        &mut self,
        paint_jobs: &[egui::ClippedPrimitive],
        tdelta: &egui::TexturesDelta,
        screen: &egui_wgpu::ScreenDescriptor,
    ) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let tview = frame.texture.create_view(&Default::default());
        for (id, delta) in &tdelta.set {
            self.egui_rend.update_texture(&self.device, &self.queue, *id, delta);
        }
        let mut enc = self.device.create_command_encoder(&Default::default());
        let user_cmds = self.egui_rend.update_buffers(&self.device, &self.queue, &mut enc, paint_jobs, screen);
        {
            // egui-wgpu 0.35 wants RenderPass<'static> → detach from the encoder borrow (behaviour identical)
            let mut rp = enc
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
                    label: Some("splash"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: &tview,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.egui_rend.render(&mut rp, paint_jobs, screen);
        }
        self.queue.submit(user_cmds.into_iter().chain(std::iter::once(enc.finish())));
        frame.present();
        for id in &tdelta.free {
            self.egui_rend.free_texture(id);
        }
    }

    pub fn render_ui(
        &mut self,
        world: &Scene,
        view: View,
        paint_jobs: &[egui::ClippedPrimitive],
        tdelta: &egui::TexturesDelta,
        screen: &egui_wgpu::ScreenDescriptor,
    ) -> bool {
        self.render_ui_impl(Some((world, view)), paint_jobs, tdelta, screen)
    }

    /// Present the previous offscreen canvas under a fresh egui frame. The app calls this only when
    /// its conservative scene signature matches, so no scene CPU work, upload, or GPU passes repeat.
    pub fn render_ui_cached(
        &mut self,
        paint_jobs: &[egui::ClippedPrimitive],
        tdelta: &egui::TexturesDelta,
        screen: &egui_wgpu::ScreenDescriptor,
    ) -> bool {
        self.render_ui_impl(None, paint_jobs, tdelta, screen)
    }

    fn render_ui_impl(
        &mut self,
        scene: Option<(&Scene, View)>,
        paint_jobs: &[egui::ClippedPrimitive],
        tdelta: &egui::TexturesDelta,
        screen: &egui_wgpu::ScreenDescriptor,
    ) -> bool {
        let perf_start = std::time::Instant::now();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return false;
            }
            _ => return false,
        };
        let tview = frame.texture.create_view(&Default::default());
        let prepared = scene.map(|(world, view)| {
            let (fw, fh) = (self.config.width as f32, self.config.height as f32);
            let bg = build_bg(view, fw, fh);
            let content_start = std::time::Instant::now();
            let (fillv, mut fgv, opv, metas) = build_content(&world.content, view, view.zoom, fw, fh);
            let content_elapsed = content_start.elapsed();
            let ov_start = fgv.len() as u32;
            fgv.extend(build_fg(&world.overlay, view, 1.0, fw, fh));
            let overlay = (ov_start, fgv.len() as u32 - ov_start);
            let counts = (fillv.len(), fgv.len(), opv.len());
            let nbg = Self::upload(&self.device, &self.queue, &mut self.bg_buf, &mut self.bg_cap, &bg);
            let _ = Self::upload(&self.device, &self.queue, &mut self.fill_buf, &mut self.fill_cap, &fillv);
            let _ = Self::upload(&self.device, &self.queue, &mut self.fg_buf, &mut self.fg_cap, &fgv);
            let _ = Self::upload(&self.device, &self.queue, &mut self.op_buf, &mut self.op_cap, &opv);
            (nbg, metas, overlay, content_elapsed, counts)
        });
        for (id, delta) in &tdelta.set {
            self.egui_rend.update_texture(&self.device, &self.queue, *id, delta);
        }
        let mut enc = self.device.create_command_encoder(&Default::default());
        let user_cmds = self.egui_rend.update_buffers(&self.device, &self.queue, &mut enc, paint_jobs, screen);
        // A signature miss rebuilds the offscreen scene. A hit keeps its last resolved texture and only
        // blits it below the fresh egui pass.
        if let Some((nbg, metas, overlay, _, _)) = &prepared {
            self.record_scene(&mut enc, *nbg, metas, *overlay);
        }
        // blit the offscreen scene → surface (egui chrome is drawn over it in the next pass)
        {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                multiview_mask: None,
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &tview,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            rp.set_pipeline(&self.blit_pipe);
            rp.set_bind_group(0, &self.blit_bg, &[]);
            rp.draw(0..3, 0..1);
        }
        // egui chrome → surface (load — drawn over the canvas)
        {
            // egui-wgpu 0.35 wants RenderPass<'static> → detach from the encoder borrow (behaviour identical)
            let mut rp = enc
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
                    label: Some("egui"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: &tview,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.egui_rend.render(&mut rp, paint_jobs, screen);
        }
        self.queue.submit(user_cmds.into_iter().chain(std::iter::once(enc.finish())));
        frame.present();
        for id in &tdelta.free {
            self.egui_rend.free_texture(id);
        }
        if std::env::var_os("VAROS_PERF").is_some() {
            match prepared {
                Some((_, _, _, content_elapsed, counts)) => {
                    log!(
                        "[varos-perf] scene_cache=miss build_content={:.3}ms render_ui={:.3}ms fill_v={} fg_v={} op_v={}",
                        content_elapsed.as_secs_f64() * 1_000.0,
                        perf_start.elapsed().as_secs_f64() * 1_000.0,
                        counts.0,
                        counts.1,
                        counts.2
                    );
                }
                None => {
                    log!(
                        "[varos-perf] scene_cache=hit build_content=0.000ms render_ui={:.3}ms",
                        perf_start.elapsed().as_secs_f64() * 1_000.0
                    );
                }
            }
        }
        true
    }
}
