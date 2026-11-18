#version 300 es
precision highp float;
in vec4 v_color;
in vec2 v_uv;
uniform sampler2D u_atlas;
out vec4 color;
void main() { color = v_color*texture(u_atlas,v_uv); }
