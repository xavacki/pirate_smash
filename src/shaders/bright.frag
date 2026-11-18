#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_source;
uniform float u_threshold;
out vec4 color;
void main() {
    vec3 c = texture(u_source, v_uv).rgb;
    float brightness = dot(c, vec3(0.299, 0.587, 0.114));
    color = vec4(c * smoothstep(u_threshold, u_threshold+0.01, brightness), 1.0);
}
