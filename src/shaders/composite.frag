#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_scene;
uniform sampler2D u_bloom0;
uniform sampler2D u_bloom1;
uniform sampler2D u_bloom2;
uniform sampler2D u_bloom3;
uniform sampler2D u_bloom4;
uniform float u_strength;
uniform float u_radius;
uniform float u_exposure;
out vec4 color;

// ACES fitted RRT/ODT, with the same exposure scale as Three.js.
vec3 aces(vec3 c) {
    const mat3 inputMatrix = mat3(
        0.59719,0.07600,0.02840,
        0.35458,0.90834,0.13383,
        0.04823,0.01566,0.83777);
    const mat3 outputMatrix = mat3(
        1.60475,-0.10208,-0.00327,
        -0.53108,1.10813,-0.07276,
        -0.07367,-0.00605,1.07602);
    c = inputMatrix * (c * u_exposure / 0.6);
    vec3 a = c*(c+0.0245786)-0.000090537;
    vec3 b = c*(0.983729*c+0.4329510)+0.238081;
    return clamp(outputMatrix*(a/b),0.0,1.0);
}
vec3 srgb(vec3 c) {
    return mix(12.92*c, 1.055*pow(c,vec3(1.0/2.4))-0.055, step(vec3(0.0031308),c));
}
float weight(float w) { return mix(w, 1.2-w, u_radius); }
void main() {
    vec3 bloom = weight(1.0)*texture(u_bloom0,v_uv).rgb
        + weight(0.8)*texture(u_bloom1,v_uv).rgb
        + weight(0.6)*texture(u_bloom2,v_uv).rgb
        + weight(0.4)*texture(u_bloom3,v_uv).rgb
        + weight(0.2)*texture(u_bloom4,v_uv).rgb;
    color = vec4(srgb(aces(texture(u_scene,v_uv).rgb + u_strength*bloom)),1.0);
}
