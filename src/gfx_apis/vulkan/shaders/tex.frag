#version 450

#include "frag_spec_const.glsl"

#ifdef ALPHA_MULTIPLIER
layout(push_constant, std430) uniform Data {
	layout(offset = 64) float mul;
} data;
#endif
layout(set = 0, binding = 0) uniform sampler2D tex;
layout(location = 0) in vec2 tex_pos;
layout(location = 0) out vec4 out_color;

void main() {
#ifdef ALPHA_MULTIPLIER
	if (src_has_alpha) {
		out_color = textureLod(tex, tex_pos, 0) * data.mul;
	} else {
		out_color = vec4(textureLod(tex, tex_pos, 0).rgb * data.mul, data.mul);
	}
#else // !ALPHA_MULTIPLIER
	out_color = textureLod(tex, tex_pos, 0);
#endif
}
