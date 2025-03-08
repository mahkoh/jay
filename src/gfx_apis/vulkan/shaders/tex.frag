#version 450

#extension GL_EXT_scalar_block_layout : require

#include "frag_spec_const.glsl"
#include "transfer_functions.glsl"
#include "tex.common.glsl"

layout(set = 0, binding = 0) uniform sampler sam;
layout(set = 1, binding = 0) uniform texture2D tex;
layout(set = 1, binding = 1, row_major, std430) uniform ColorManagementData {
	mat4x4 matrix;
} cm_data;
layout(location = 0) in vec2 tex_pos;
layout(location = 0) out vec4 out_color;

void main() {
	vec4 c = textureLod(sampler2D(tex, sam), tex_pos, 0);
	if (eotf != oetf || has_matrix) {
		vec3 rgb = c.rgb;
		if (src_has_alpha) {
			rgb /= mix(c.a, 1.0, c.a == 0.0);
		}
		rgb = apply_eotf(rgb);
		if (has_matrix) {
			rgb = (cm_data.matrix * vec4(rgb, 1.0)).rgb;
		}
		rgb = apply_oetf(rgb);
		if (src_has_alpha) {
			rgb *= c.a;
		}
		c.rgb = rgb;
	}
	if (has_alpha_multiplier) {
		if (src_has_alpha) {
			c *= data.mul;
		} else {
			c = vec4(c.rgb * data.mul, data.mul);
		}
	}
	out_color = c;
}
