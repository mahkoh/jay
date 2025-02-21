#version 450

#include "frag_spec_const.glsl"
#include "transfer_functions.glsl"
#include "out.common.glsl"

layout(input_attachment_index = 0, set = 0, binding = 0) uniform subpassInput in_color;
layout(location = 1) out vec4 out_color;

void main() {
	vec4 c = subpassLoad(in_color);
	c.rgb /= mix(c.a, 1.0, c.a == 0.0);
	c.rgb = oetf_srgb(c.rgb);
	c.rgb *= c.a;
	out_color = c;
}
