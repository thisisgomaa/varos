// Varos — Phase-5 shell spike.
// One winit window: a `wry` WebView panel in a sibling child HWND on the LEFT, our wgpu canvas on
// the RIGHT. The winit event loop + wgpu present path are the SAME we use today, so canvas input
// latency is untouched. A blue box follows the cursor on the canvas (drag-feel test). A button in
// the HTML panel does an IPC round-trip (panel -> Rust -> recolor box + reply text).
//
// Spike questions: (1) does it build with winit 0.29 + wry 0.52? (2) does the box track the cursor
// with native feel? (3) does the IPC round-trip work? Ahmed judges feel by hand.

#![cfg_attr(windows, windows_subsystem = "windows")]
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use wry::{
    dpi::{LogicalPosition as WPos, LogicalSize as WSize},
    Rect, WebViewBuilder,
};

#[derive(Debug, Clone)]
enum UserEvent {
    Ipc(String),
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    pos: [f32; 2],
    color: [f32; 4],
}

const SHADER: &str = r#"
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) color: vec4<f32> };
@vertex fn vs(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var o: VsOut; o.clip = vec4<f32>(pos, 0.0, 1.0); o.color = color; return o;
}
@fragment fn fs(in: VsOut) -> @location(0) vec4<f32> { return in.color; }
"#;

const SIDEBAR_W: f64 = 280.0; // logical px

const PANEL_HTML: &str = r#"<!doctype html><html lang="ar" dir="rtl"><head><meta charset="utf-8">
<style>
  html,body{margin:0;height:100%;background:#1e1e22;color:#e8e8e8;font:14px "Segoe UI",system-ui,sans-serif}
  .wrap{padding:16px;display:flex;flex-direction:column;gap:14px}
  h2{margin:0;color:#0c8ce9;font-size:15px}
  p{margin:0;color:#9a9a9a;line-height:1.5}
  button{background:#0c8ce9;color:#fff;border:0;border-radius:8px;padding:10px 14px;font-size:14px;cursor:pointer}
  button:hover{background:#0a78c8}
  #reply{color:#34c759;min-height:20px}
</style></head><body><div class="wrap">
  <h2>لوحة Varos (HTML)</h2>
  <p>دي لوحة web جوّه نافذة Varos، جنب الكانفس مباشرة. الكانفس على اليمين بيشتغل native من غير ما يمرّ هنا.</p>
  <button id="b">كلّم القلب (Rust)</button>
  <div id="reply">—</div>
</div>
<script>
  document.getElementById('b').addEventListener('click', () => {
    window.ipc.postMessage('ping من اللوحة');
  });
</script></body></html>"#;

fn main() {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build().unwrap();
    let proxy = event_loop.create_proxy();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Varos — wry + wgpu shell spike")
            .with_inner_size(LogicalSize::new(1100.0, 720.0))
            .build(&event_loop)
            .unwrap(),
    );
    let scale = window.scale_factor();

    // ---------- wgpu (same setup as the real canvas) ----------
    let instance = wgpu::Instance::default();
    let surface = instance.create_surface(window.clone()).unwrap();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .unwrap();
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)).unwrap();

    let size = window.inner_size();
    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats[0];
    let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else {
        wgpu::PresentMode::AutoVsync
    };
    let mut config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: size.width.max(1),
        height: size.height.max(1),
        present_mode,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };
    surface.configure(&device, &config);

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: None,
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs",
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs",
            targets: &[Some(format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    // ---------- wry panel (sibling child HWND on the left) ----------
    let webview = WebViewBuilder::new()
        .with_bounds(Rect {
            position: WPos::new(0.0, 0.0).into(),
            size: WSize::new(SIDEBAR_W, size.height as f64 / scale).into(),
        })
        .with_background_color((30, 30, 34, 255)) // kill the white first-paint flash
        .with_html(PANEL_HTML)
        .with_ipc_handler(move |req| {
            let _ = proxy.send_event(UserEvent::Ipc(req.body().clone()));
        })
        .build_as_child(&*window)
        .unwrap();

    // ---------- state ----------
    let mut cursor = PhysicalPosition::new(0.0f64, 0.0f64);
    let mut box_color = [0.047f32, 0.549, 0.914, 1.0]; // varos blue; turns green on IPC

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);
            match event {
                Event::UserEvent(UserEvent::Ipc(msg)) => {
                    box_color = [0.204, 0.78, 0.349, 1.0]; // proves the message reached Rust
                    let reply = format!("Rust ردّ: pong ✓ ({msg})");
                    let _ = webview.evaluate_script(&format!(
                        "document.getElementById('reply').innerText = {reply:?};"
                    ));
                    window.request_redraw();
                }
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(s) => {
                        config.width = s.width.max(1);
                        config.height = s.height.max(1);
                        surface.configure(&device, &config);
                        let _ = webview.set_bounds(Rect {
                            position: WPos::new(0.0, 0.0).into(),
                            size: WSize::new(SIDEBAR_W, s.height as f64 / scale).into(),
                        });
                        window.request_redraw();
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        cursor = position;
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        let (w, h) = (config.width as f32, config.height as f32);
                        let half = 30.0f32;
                        let c = [cursor.x as f32, cursor.y as f32];
                        let ndc = |p: [f32; 2]| [p[0] / w * 2.0 - 1.0, 1.0 - p[1] / h * 2.0];
                        let cor = [
                            [c[0] - half, c[1] - half],
                            [c[0] + half, c[1] - half],
                            [c[0] + half, c[1] + half],
                            [c[0] - half, c[1] + half],
                        ];
                        let v = |i: usize| Vertex { pos: ndc(cor[i]), color: box_color };
                        let verts = [v(0), v(1), v(2), v(0), v(2), v(3)];
                        let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: None,
                            contents: bytemuck::cast_slice(&verts),
                            usage: wgpu::BufferUsages::VERTEX,
                        });

                        let frame = match surface.get_current_texture() {
                            Ok(f) => f,
                            Err(_) => {
                                surface.configure(&device, &config);
                                return;
                            }
                        };
                        let view = frame.texture.create_view(&Default::default());
                        let mut enc = device.create_command_encoder(&Default::default());
                        {
                            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: None,
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color {
                                            r: 0.117,
                                            g: 0.117,
                                            b: 0.117,
                                            a: 1.0,
                                        }),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });
                            rp.set_pipeline(&pipeline);
                            rp.set_vertex_buffer(0, vbuf.slice(..));
                            rp.draw(0..verts.len() as u32, 0..1);
                        }
                        queue.submit(Some(enc.finish()));
                        frame.present();
                    }
                    _ => {}
                },
                _ => {}
            }
        })
        .unwrap();
}
