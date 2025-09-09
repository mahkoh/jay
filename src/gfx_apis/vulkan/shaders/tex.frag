#version 450

#extension GL_EXT_scalar_block_layout : require

#define TEX_SET 1

#include "frag_spec_const.glsl"
#include "tex.common.glsl"
#include "tex_set.glsl"
#include "eotfs.glsl"

layout(set = 0, binding = 0) uniform sampler sam;
layout(location = 0) in vec2 tex_pos;
layout(location = 0) out vec4 out_color;

void main() {
	vec4 c = textureLod(sampler2D(tex, sam), tex_pos, 0);
	if (eotf != inv_eotf || has_matrix) {
		vec3 rgb = c.rgb;
		if (src_has_alpha) {
			rgb /= mix(c.a, 1.0, c.a == 0.0);
		}
		rgb = apply_eotf(rgb);
		if (has_matrix) {
			rgb = (cm_data.matrix * vec4(rgb, 1.0)).rgb;
		}
		rgb = apply_inv_eotf(rgb);
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
