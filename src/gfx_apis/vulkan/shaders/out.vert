#version 450
//#extension GL_EXT_debug_printf : enable

#include "out.common.glsl"

void main() {
	vec2 pos;
	switch (gl_VertexIndex) {
		case 0: pos = data.pos[0]; break;
		case 1: pos = data.pos[1]; break;
		case 2: pos = data.pos[2]; break;
		case 3: pos = data.pos[3]; break;
	}
	gl_Position = vec4(pos, 0.0, 1.0);
//	debugPrintfEXT("X gl_Position = %v4f, pos = %v2f", gl_Position, pos);
}
