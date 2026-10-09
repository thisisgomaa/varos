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
 var color=textureSample(t,s,clamp(sample_px/size,vec2<f32>(0.0),vec2<f32>(1.0)));
 if preview.camera.w>0.0 && step>=2.0 {
   let edge=local-pixel*step;
   if min(edge.x,edge.y)<1.0 { color=mix(color,preview.color,0.45); }
 }
 return color;
}
"#;
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
