// Lane B: ray/circle radial maths idea from PhotoCraft compose.wgsl (MIT OR Apache-2.0).
struct Params { inverse:vec4<f32>, origin_pan:vec4<f32>, options:vec4<f32>, focal:vec4<f32> };
@group(0) @binding(0) var lut:texture_2d<f32>;
@group(0) @binding(1) var smp:sampler;
@group(0) @binding(2) var<uniform> p:Params;
struct V { @builtin(position) pos:vec4<f32>, @location(0) colour:vec4<f32> };
@vertex fn vs(@location(0) pos:vec2<f32>,@location(1) colour:vec4<f32>)->V { var o:V;o.pos=vec4<f32>(pos,0.,1.);o.colour=colour;return o; }
@fragment fn fs(v:V)->@location(0) vec4<f32> {
 let world=(v.pos.xy-p.origin_pan.zw)/p.options.x-p.origin_pan.xy;
 let pt=vec2<f32>(p.inverse.x*world.x+p.inverse.z*world.y,p.inverse.y*world.x+p.inverse.w*world.y);
 var t=pt.x;
 if p.options.y>0.5 {let ray=pt-p.focal.xy;let a=dot(ray,ray);let b=2.*dot(ray,p.focal.xy);let c=dot(p.focal.xy,p.focal.xy)-1.;t=0.;if a>1e-12 {t=2.*a/(-b+sqrt(max(0.,b*b-4.*a*c)));}}
 if p.options.z>1.5 {t=fract(t);} else if p.options.z>0.5 {let u=t-2.*floor(t/2.);t=select(u,2.-u,u>1.);} else {t=clamp(t,0.,1.);}
 let colour=textureSampleLevel(lut,smp,vec2<f32>((t*1023.+0.5)/1024.,0.5),0.);
 let noise=(fract(sin(dot(floor(v.pos.xy),vec2<f32>(12.9898,78.233)))*43758.5453)-0.5)/255.;
 return vec4<f32>(clamp(colour.rgb+vec3<f32>(noise),vec3<f32>(0.),vec3<f32>(1.)),colour.a*p.options.w);
}
