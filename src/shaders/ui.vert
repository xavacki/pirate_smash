#version 300 es
precision highp float;
layout(location=0) in vec2 a_position;
layout(location=1) in vec4 a_color;
layout(location=2) in vec2 a_uv;
out vec2 v_uv;
uniform vec2 u_size;
out vec4 v_color;
void main() {
    gl_Position = vec4(a_position/u_size*vec2(2.0,-2.0)+vec2(-1.0,1.0),0.0,1.0);
    v_color = a_color;
    v_uv=a_uv;
}
