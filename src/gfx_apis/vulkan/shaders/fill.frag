#version 450

#include "fill.common.glsl"

layout(location = 0) out vec4 out_color;

void main() {
	out_color = data.color;
}
