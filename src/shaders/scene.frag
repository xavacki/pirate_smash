#version 300 es
precision highp float;
in vec3 v_position;
in vec3 v_normal;
in vec4 v_color;
in vec2 v_uv;
uniform sampler2D u_base_map;
uniform int u_has_texture;
uniform vec3 u_camera;
uniform vec4 u_color;
uniform vec3 u_emissive;
uniform float u_metallic;
uniform float u_roughness;
uniform float u_clear_coat;
uniform int u_glass;
uniform int u_unlit;
out vec4 color;
const float PI = 3.14159265359;
void main() {
    if(u_unlit!=0){
        vec4 texel = u_has_texture!=0 ? texture(u_base_map,v_uv) : vec4(1.0);
        vec3 linearTexel=mix(texel.rgb/12.92,pow((texel.rgb+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),texel.rgb));
        color=vec4(u_emissive*v_color.rgb*linearTexel,u_color.a*v_color.a*texel.a);return;
    }
    vec4 base = u_color * v_color;
    base.rgb=mix(base.rgb/12.92,pow((base.rgb+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),base.rgb));
    vec4 sampled=u_has_texture!=0?texture(u_base_map,v_uv):vec4(1.0);
    vec3 texel=sampled.rgb;
    base.a*=sampled.a;
    // Recovered base-color maps are sRGB; lighting and bloom operate in linear space.
    base.rgb*=mix(texel/12.92,pow((texel+0.055)/1.055,vec3(2.4)),step(vec3(0.04045),texel));
    vec3 n = normalize(v_normal) * (gl_FrontFacing ? 1.0 : -1.0);
    vec3 delta = u_camera - v_position;
    float distance2 = max(dot(delta, delta), 0.01);
    vec3 v = normalize(delta);
    vec3 l = normalize(vec3(-0.5,1.0,0.8));
    vec3 h = normalize(l + v);
    float nl = clamp(dot(n,l), 0.0, 1.0);
    float nv = clamp(dot(n,v), 0.001, 1.0);
    float nh = clamp(dot(n,h), 0.0, 1.0);
    // Roundoff can put normalized-vector dot products slightly above one.
    // A negative base to GLSL pow is undefined, even for an integer exponent.
    float vh = clamp(dot(v,h), 0.0, 1.0);
    float alpha = max(u_roughness * u_roughness, 0.002);
    float a2 = alpha * alpha;
    float d = a2 / (PI * pow(nh * nh * (a2 - 1.0) + 1.0, 2.0));
    float gv = nl * sqrt(nv*nv*(1.0-a2)+a2);
    float gl = nv * sqrt(nl*nl*(1.0-a2)+a2);
    float visibility = 0.5 / max(gv+gl, 0.00001);
    vec3 f0 = mix(vec3(0.04), base.rgb, u_metallic);
    vec3 fresnel = f0 + (1.0-f0) * pow(1.0-vh,5.0);
    vec3 diffuse = (1.0-u_metallic) * base.rgb / PI;
    vec3 radiance = vec3(3.7,3.5,3.2);
    // 0xcccccc ambient light converted from sRGB to linear.
    vec3 ambient = mix(vec3(0.15,0.19,0.26),vec3(0.7,0.8,0.95),n.y*0.5+0.5)*base.rgb*0.65;
    vec3 lit=ambient + (diffuse + fresnel*d*visibility)*radiance*nl + u_emissive;
    lit += radiance * (u_clear_coat * 0.14 * pow(nh, 96.0) * nl);
    if(u_glass!=0){
        float rim=pow(1.0-nv,3.0);
        vec3 reflectedSky=mix(vec3(0.12,0.2,0.35),vec3(0.55,0.8,1.0),n.y*0.5+0.5);
        // A lightly tinted transparent shell with Fresnel reflections; no scene refraction.
        vec3 glassLight=base.rgb*0.15+reflectedSky*(0.08+0.5*rim)+fresnel*d*visibility*radiance*nl*0.25;
        color=vec4(glassLight,clamp(base.a+0.28*rim,0.0,0.65));
        return;
    }
    float fog=1.0-exp(-0.00002*distance2);
    color = vec4(mix(lit,vec3(0.018,0.027,0.046),fog),base.a);
}
