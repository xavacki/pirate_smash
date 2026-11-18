#version 300 es
precision highp float;
in vec2 v_uv;
uniform sampler2D u_source;
uniform vec2 u_step;
uniform int u_radius;
out vec4 color;
void main() {
    float sigma = float(u_radius) * 0.5;
    vec3 sum = texture(u_source, v_uv).rgb;
    float weights = 1.0;
    for (int i=1; i<=11; ++i) {
        if (i >= u_radius) break;
        float w = exp(-0.5 * float(i*i)/(sigma*sigma));
        vec2 offset = u_step * float(i);
        sum += (texture(u_source, v_uv+offset).rgb + texture(u_source, v_uv-offset).rgb)*w;
        weights += 2.0*w;
    }
    color = vec4(sum/weights, 1.0);
}
