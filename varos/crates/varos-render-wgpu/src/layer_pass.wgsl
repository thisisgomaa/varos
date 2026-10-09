// Rebuilt from PhotoCraft gpu/src/compose.wgsl map-pass ideas @ 4cb7cf3.
// Copyright (c) 2026 ArtCraft Team and contributors. MIT OR Apache-2.0.
struct Params { mode:u32, operation:u32, radius:i32, vertical:i32,
 opacity:f32, dx:i32, dy:i32, outer:u32, colour:vec4<f32> }
@group(0) @binding(0) var a:texture_2d<f32>;
@group(0) @binding(1) var b:texture_2d<f32>;
@group(0) @binding(2) var mask:texture_2d<f32>;
@group(0) @binding(3) var<uniform> op:Params;
@group(0) @binding(4) var<storage,read> weights:array<f32>;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
 let uv=vec2<f32>(f32((i<<1u)&2u),f32(i&2u)); return vec4<f32>(uv*2.0-1.0,0.0,1.0);
}
fn lum(c:vec3<f32>)->f32 {return dot(c,vec3(0.3,0.59,0.11));}
fn sat(c:vec3<f32>)->f32 {return max(c.r,max(c.g,c.b))-min(c.r,min(c.g,c.b));}
fn sl(c0:vec3<f32>,l0:f32)->vec3<f32> {
 var c=c0+vec3(l0-lum(c0)); let l=lum(c); let n=min(c.r,min(c.g,c.b)); let x=max(c.r,max(c.g,c.b));
 if n<0.0 {c=vec3(l)+(c-vec3(l))*l/(l-n);}
 if x>1.0 {c=vec3(l)+(c-vec3(l))*(1.0-l)/(x-l);} return c;
}
fn ss(c:vec3<f32>,s:f32)->vec3<f32> {
 let n=min(c.r,min(c.g,c.b)); let d=sat(c); if d>0.0 {return (c-vec3(n))*s/d;} return vec3(0.0);
}
fn hard(b:f32,s:f32)->f32 {if s<=0.5 {return 2.0*b*s;} return 1.0-2.0*(1.0-b)*(1.0-s);}
fn channel(b:f32,s:f32,m:u32)->f32 {
 switch m {
 case 1u:{return b*s;} case 2u:{return b+s-b*s;} case 3u:{return hard(s,b);}
 case 4u:{return min(b,s);} case 5u:{return max(b,s);}
 case 6u:{if b<=0.0001 {return 0.0;} if s>=1.0 {return 1.0;} return min(b/(1.0-s),1.0);}
 case 7u:{if b>=0.9999 {return 1.0;} if s<=0.0 {return 0.0;} return 1.0-min((1.0-b)/s,1.0);}
 case 8u:{return hard(b,s);}
 case 9u:{if s<=0.5 {return b-(1.0-2.0*s)*b*(1.0-b);}
 var d=sqrt(b); if b<=0.25 {d=((16.0*b-12.0)*b+4.0)*b;} return b+(2.0*s-1.0)*(d-b);}
 case 10u:{return abs(b-s);} case 11u:{return b+s-2.0*b*s;} default:{return s;}
 }
}
fn blend(b:vec3<f32>,s:vec3<f32>,m:u32)->vec3<f32> {
 switch m {
 case 12u:{return sl(ss(s,sat(b)),lum(b));} case 13u:{return sl(ss(b,sat(s)),lum(b));}
 case 14u:{return sl(s,lum(b));} case 15u:{return sl(b,lum(s));}
 default:{return vec3(channel(b.r,s.r,m),channel(b.g,s.g,m),channel(b.b,s.b,m));}
 }
}
fn over(b:vec4<f32>,s:vec4<f32>,m:u32)->vec4<f32> {
 var cb=vec3(0.0); var cs=vec3(0.0); if b.a>0.0 {cb=b.rgb/b.a;} if s.a>0.0 {cs=s.rgb/s.a;}
 return vec4((1.0-s.a)*b.rgb+(1.0-b.a)*s.rgb+s.a*b.a*blend(cb,cs,m),s.a+b.a*(1.0-s.a));
}
fn load_zero(p:vec2<i32>,tex:texture_2d<f32>)->vec4<f32> {
 let size=vec2<i32>(textureDimensions(tex));
 if any(p<vec2(0))||any(p>=size) {return vec4(0.0);} return textureLoad(tex,p,0);
}
@fragment fn fs(@builtin(position) pos:vec4<f32>)->@location(0) vec4<f32> {
 let p=vec2<i32>(pos.xy);
 if op.operation==1u {
 var step=vec2(1,0); if op.vertical!=0 {step=vec2(0,1);}
 var acc=vec4(0.0);
 for(var k=-op.radius;k<=op.radius;k++) {acc+=load_zero(p+step*k,a)*weights[u32(k+op.radius)];} return acc;
 }
 if op.operation==2u {
 let content=textureLoad(b,p,0); var alpha=load_zero(p-vec2(op.dx,op.dy),a).a;
 if op.outer!=0u {alpha*=1.0-content.a;} alpha*=op.colour.a;
 return over(vec4(op.colour.rgb*alpha,alpha),content,0u);
 }
 return over(textureLoad(b,p,0),textureLoad(a,p,0)*op.opacity*textureLoad(mask,p,0).r,op.mode);
}
