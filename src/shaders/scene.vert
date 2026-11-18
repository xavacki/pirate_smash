#version 300 es
precision highp float;
layout(location=0) in vec3 a_position;
layout(location=1) in vec3 a_normal;
layout(location=2) in vec4 a_color;
layout(location=3) in vec2 a_uv;
out vec2 v_uv;
uniform mat4 u_model;
uniform mat4 u_view_projection;
uniform mat3 u_normal;
out vec3 v_position;
out vec3 v_normal;
out vec4 v_color;
void main() {
    vec4 world = u_model * vec4(a_position, 1.0);
    v_position = world.xyz;
    v_normal = u_normal * a_normal;
    v_color = a_color;
    v_uv=a_uv;
    gl_Position = u_view_projection * world;
}
