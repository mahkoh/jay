#version 450

#include "frag_spec_const.glsl"

layout(push_constant, std430) uniform Data {
	layout(offset = 32) vec4 color;
} data;

layout(location = 0) out vec4 out_color;

void main() {
	out_color = data.color;
}
