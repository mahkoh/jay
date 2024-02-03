#version 450

layout(set = 0, binding = 0) uniform sampler2D tex;
layout(location = 0) in vec2 tex_pos;
layout(location = 0) out vec4 out_color;

void main() {
	out_color = textureLod(tex, tex_pos, 0);
}
