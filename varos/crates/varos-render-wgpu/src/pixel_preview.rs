//! Lane E document-pixel preview. GPU pass is canvas-only; normal sampling is unchanged.
pub const SHADER: &str = r#"
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
struct Preview { camera: vec4<f32>, color: vec4<f32> };
@group(0) @binding(2) var<uniform> preview: Preview;
struct VO { @builtin(position) p: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VO {
 var o: VO; let uv=vec2<f32>(f32((i<<1u)&2u),f32(i&2u));
 o.uv=uv; o.p=vec4<f32>(uv*2.0-1.0,0.0,1.0); o.p.y=-o.p.y; return o;
}
@fragment fn fs(in: VO) -> @location(0) vec4<f32> {
 let step=preview.camera.z;
 if step<1.0 {return textureSample(t,s,in.uv);}
 let size=vec2<f32>(textureDimensions(t));
 let local=in.p.xy-preview.camera.xy;
 let pixel=floor(local/step);
 let sample_px=(pixel+vec2<f32>(0.5))*step+preview.camera.xy;
 // The artwork source contains only the clipped Trim grid, never editing overlays.
 let edge=local-pixel*step;
 if preview.camera.w>0.0 && step>=2.0 && min(edge.x,edge.y)<1.0 {
   return textureSample(t,s,in.uv);
 }
 return textureSample(t,s,clamp(sample_px/size,vec2<f32>(0.0),vec2<f32>(1.0)));
}
"#;
pub fn enabled(step: Option<f32>, view: varos_core::geom::View) -> bool {
    step.is_some_and(|step| step * view.zoom >= 1.0)
}
pub fn parameters(step: Option<f32>, view: varos_core::geom::View, color: [f32; 4]) -> [f32; 8] {
    let s = step.unwrap_or(0.0) * view.zoom;
    [
        view.pan[0],
        view.pan[1],
        s,
        if step.is_some() && view.zoom >= 6.0 { 1.0 } else { 0.0 },
        color[0],
        color[1],
        color[2],
        color[3],
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn artwork_sampling_precedes_overlay_resolve_and_final_blit_is_unfiltered() {
        // Structural headless regression: actual recorder order and its independent bind groups.
        let source = include_str!("lib.rs");
        let record = source.split("fn record_scene(").nth(1).expect("scene recorder");
        let record = record.split("pub fn render(").next().expect("record body");
        assert!(record.find("artwork-resolve").unwrap() < record.find("pixel-preview-artwork").unwrap());
        assert!(record.find("pixel-preview-artwork").unwrap() < record.find("scene-overlay").unwrap());
        assert!(record.contains("rp.set_bind_group(0, &self.preview_bg"));
        assert!(source.contains("&scene_view, &sampler, &normal_blit_buf"));
        assert!(source.contains("&layer_view, &sampler, &normal_blit_buf"));
        assert!(enabled(Some(1.0), varos_core::geom::View { zoom: 12.0, pan: [0.0; 2] }));
        assert!(!enabled(None, varos_core::geom::View::identity()));
        assert!(!enabled(Some(0.5), varos_core::geom::View::identity()));
    }
    #[test]
    fn preview_ppi_and_zoom() {
        let v = varos_core::geom::View { pan: [-5.0, 8.0], zoom: 6.0 };
        let p = parameters(Some(72.0 / 144.0), v, [0.5; 4]);
        assert_eq!(&p[..4], &[-5.0, 8.0, 3.0, 1.0]);
        assert_eq!(parameters(None, v, [0.5; 4])[2], 0.0);
    }
}

#[cfg(test)]
mod shader_tests {
    #[test]
    fn shader_validates_without_gpu() {
        let module = naga::front::wgsl::parse_str(super::SHADER).expect("preview WGSL parse");
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .expect("preview WGSL validation");
    }
}
