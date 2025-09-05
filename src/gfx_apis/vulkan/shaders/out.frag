#version 450
#extension GL_EXT_samplerless_texture_functions : require

#include "frag_spec_const.glsl"
#include "eotfs.glsl"
#include "out.common.glsl"

layout(set = 0, binding = 0) uniform texture2D in_color;
layout(location = 0) out vec4 out_color;

void main() {
	vec4 c = texelFetch(in_color, ivec2(gl_FragCoord.xy), 0);
	if (eotf != inv_eotf) {
		c.rgb /= mix(c.a, 1.0, c.a == 0.0);
		c.rgb = apply_eotf(c.rgb);
		c.rgb = apply_inv_eotf(c.rgb);
		c.rgb *= c.a;
	}
	out_color = c;
}
