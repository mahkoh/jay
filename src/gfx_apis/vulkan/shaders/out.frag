#version 450

#extension GL_EXT_samplerless_texture_functions : require
#extension GL_EXT_scalar_block_layout : require

#include "frag_spec_const.glsl"
#include "eotfs.glsl"
#include "out.common.glsl"

layout(set = 0, binding = 0) uniform texture2D in_color;
layout(set = 0, binding = 1, row_major, std430) uniform ColorManagementData {
	mat4x4 matrix;
} cm_data;
layout(location = 0) out vec4 out_color;

void main() {
	vec4 c = texelFetch(in_color, ivec2(gl_FragCoord.xy), 0);
	if (eotf != inv_eotf || has_matrix) {
		vec3 rgb = c.rgb;
		rgb /= mix(c.a, 1.0, c.a == 0.0);
		rgb = apply_eotf(rgb);
		if (has_matrix) {
			rgb = (cm_data.matrix * vec4(rgb, 1.0)).rgb;
		}
		rgb = apply_inv_eotf(rgb);
		rgb *= c.a;
		c.rgb = rgb;
	}
	out_color = c;
}
